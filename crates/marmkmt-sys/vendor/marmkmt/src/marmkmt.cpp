#include "marmkmt.h"
#include "MargretePlugin.h"

#include <Windows.h>
#include <atomic>
#include <cmath>
#include <cstddef>
#include <cstdint>
#include <cstring>
#include <limits>
#include <mutex>
#include <new>
#include <string>
#include <thread>
#include <unordered_map>
#include <utility>
#include <vector>

namespace
{

    enum class Kind
    {
        context,
        document,
        chart,
        note,
        event,
        undo
    };

    struct Invocation;
    struct Handle
    {
        Kind kind{};
        void *sdk{};
        Invocation *invocation{};
        std::thread::id thread{};
        int refs{};
        MgEventKind event_kind{};
        bool owned{};
        bool deleted{};
    };

    struct Invocation
    {
        std::thread::id thread{std::this_thread::get_id()};
        std::vector<uintptr_t> handles;
        HWND owner{};
        bool active{true};
    };

    std::mutex g_handles_mutex;
    std::unordered_map<uintptr_t, Handle> g_handles;
    std::atomic<uintptr_t> g_next_handle{0x10000};

    template <class F> MgResult guarded(F &&f) noexcept
    {
        try
        {
            return f();
        }
        catch (...)
        {
            return MG_ERR_INTERNAL;
        }
    }

    uintptr_t token_value(const void *p)
    {
        return reinterpret_cast<uintptr_t>(p);
    }

    void *make_handle(Invocation &inv, Kind kind, void *sdk, bool owned, MgEventKind event_kind = 0) noexcept
    {
        if (!sdk)
            return nullptr;
        const uintptr_t token = g_next_handle.fetch_add(0x10, std::memory_order_relaxed);
        Handle h{kind, sdk, &inv, inv.thread, 1, event_kind, owned, false};
        bool inserted = false;
        try
        {
            {
                std::lock_guard<std::mutex> lock(g_handles_mutex);
                inserted = g_handles.emplace(token, h).second;
            }
            if (!inserted)
                throw std::bad_alloc();
            inv.handles.push_back(token);
            return reinterpret_cast<void *>(token);
        }
        catch (...)
        {
            if (inserted)
            {
                try
                {
                    std::lock_guard<std::mutex> lock(g_handles_mutex);
                    g_handles.erase(token);
                }
                catch (...)
                {
                }
            }
            if (owned)
            {
                try
                {
                    reinterpret_cast<IMargretePluginBase *>(sdk)->release();
                }
                catch (...)
                {
                }
            }
            return nullptr;
        }
    }

    MgResult get_handle(const void *token, Kind expected, Handle &out, bool allow_deleted = false)
    {
        if (!token)
            return MG_ERR_NULL_POINTER;
        std::lock_guard<std::mutex> lock(g_handles_mutex);
        const auto it = g_handles.find(token_value(token));
        if (it == g_handles.end() || it->second.kind != expected || !it->second.invocation ||
            !it->second.invocation->active || (it->second.deleted && !allow_deleted) || it->second.refs == 0)
        {
            return MG_ERR_INVALID_HANDLE;
        }
        if (it->second.thread != std::this_thread::get_id())
            return MG_ERR_WRONG_THREAD;
        out = it->second;
        return MG_OK;
    }

    MgResult get_any_handle(const void *token, Handle &out, bool allow_deleted = false)
    {
        if (!token)
            return MG_ERR_NULL_POINTER;
        std::lock_guard<std::mutex> lock(g_handles_mutex);
        const auto it = g_handles.find(token_value(token));
        if (it == g_handles.end() || !it->second.invocation || !it->second.invocation->active ||
            (it->second.deleted && !allow_deleted) || it->second.refs == 0)
            return MG_ERR_INVALID_HANDLE;
        if (it->second.thread != std::this_thread::get_id())
            return MG_ERR_WRONG_THREAD;
        out = it->second;
        return MG_OK;
    }

    bool same_invocation(const Handle &a, const Handle &b)
    {
        return a.invocation == b.invocation;
    }

    IMargretePluginBase *base(void *p)
    {
        return reinterpret_cast<IMargretePluginBase *>(p);
    }

    void close_invocation(Invocation &inv) noexcept
    {
        std::vector<Handle> remaining;
        {
            std::lock_guard<std::mutex> lock(g_handles_mutex);
            inv.active = false;
            for (const uintptr_t token : inv.handles)
            {
                auto it = g_handles.find(token);
                if (it == g_handles.end())
                    continue;
                if (it->second.owned && it->second.refs > 0)
                    remaining.push_back(it->second);
                g_handles.erase(it);
            }
        }
        for (const Handle &h : remaining)
        {
            for (int i = 0; i < h.refs; ++i)
            {
                try
                {
                    base(h.sdk)->release();
                }
                catch (...)
                {
                }
            }
        }
    }

    bool guid_equal(const MpGuid &a, const MpGuid &b) noexcept
    {
        return a.data1 == b.data1 && a.data2 == b.data2 && a.data3 == b.data3 &&
               std::memcmp(a.data4, b.data4, sizeof(a.data4)) == 0;
    }

    bool valid_event_kind(MgEventKind kind)
    {
        return kind >= MG_EVENT_KIND_TIMELINE_SPEED && kind <= MG_EVENT_KIND_BEAT_CHANGE;
    }

    const MpGuid *event_iid(MgEventKind kind)
    {
        switch (kind)
        {
        case MG_EVENT_KIND_TIMELINE_SPEED:
            return &IID_IMargretePluginEventTimelineSpeed;
        case MG_EVENT_KIND_NOTE_SPEED_MODIFIER:
            return &IID_IMargretePluginEventNoteSpeedModifier;
        case MG_EVENT_KIND_BPM:
            return &IID_IMargretePluginEventBpm;
        case MG_EVENT_KIND_BEAT_CHANGE:
            return &IID_IMargretePluginEventBeatChange;
        default:
            return nullptr;
        }
    }

    bool valid_note_info(const MgNoteInfo &i)
    {
        return i.type >= MG_NOTE_TYPE_UNKNOWN && i.type <= MG_NOTE_TYPE_CLICK &&
               i.long_attr >= MG_NOTE_LONG_ATTR_NONE && i.long_attr <= MG_NOTE_LONG_ATTR_END_NOACT &&
               i.direction >= MG_NOTE_DIRECTION_NONE && i.direction <= MG_NOTE_DIRECTION_OUT_IN &&
               i.ex_attr >= MG_NOTE_EX_ATTR_NONE && i.ex_attr <= MG_NOTE_EX_ATTR_EXJDG;
    }

    MP_NOTEINFO to_sdk(const MgNoteInfo &i)
    {
        return {i.type,  i.long_attr, i.direction, i.ex_attr,     i.variation_id, i.x,
                i.width, i.height,    i.tick,      i.timeline_id, i.option_value};
    }

    MgNoteInfo from_sdk(const MP_NOTEINFO &i)
    {
        return {i.type,  i.longAttr, i.direction, i.exAttr,     i.variationId, i.x,
                i.width, i.height,   i.tick,      i.timelineId, i.optionValue};
    }

    void show_dialog(HWND owner, const wchar_t *message) noexcept
    {
        MessageBoxW(owner, message, L"marmkmt", MB_OK | MB_ICONERROR);
    }

    void show_result(HWND owner, const wchar_t *operation, MgResult result) noexcept
    {
        wchar_t text[256]{};
        swprintf_s(text, L"%s failed (MgResult %d).", operation, result);
        show_dialog(owner, text);
    }

    bool utf8_to_wide(const char *bytes, size_t length, std::wstring &out)
    {
        out.clear();
        if (!bytes && length != 0)
            return false;
        if (length > static_cast<size_t>((std::numeric_limits<int>::max)()))
            return false;
        if (length == 0)
            return true;
        const int count =
            MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, bytes, static_cast<int>(length), nullptr, 0);
        if (count <= 0)
            return false;
        out.resize(static_cast<size_t>(count));
        return MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, bytes, static_cast<int>(length), out.data(), count) ==
               count;
    }

    MgResult validate_utf8_message(const char *message, uint32_t length, std::wstring &out)
    {
        if (!message)
            return MG_ERR_NULL_POINTER;
        if (std::memchr(message, '\0', length))
            return MG_ERR_INVALID_ARGUMENT;
        return utf8_to_wide(message, length, out) ? MG_OK : MG_ERR_INVALID_ARGUMENT;
    }

    void copy_wide(wchar_t *buffer, MpInteger capacity, const std::wstring &value)
    {
        if (!buffer || capacity <= 0)
            return;
        size_t n = (std::min)(value.size(), static_cast<size_t>(capacity - 1));
        if (n && n < value.size() && n < value.size() && value[n - 1] >= 0xD800 && value[n - 1] <= 0xDBFF)
            --n;
        if (n)
            std::memcpy(buffer, value.data(), n * sizeof(wchar_t));
        buffer[n] = L'\0';
    }

    MgResult MG_CALL object_add_ref(void *object)
    {
        return guarded([&] {
            Handle h{};
            MgResult r = get_any_handle(object, h);
            if (r != MG_OK)
                return r;
            if (!h.owned)
                return static_cast<MgResult>(MG_ERR_INVALID_ARGUMENT);
            base(h.sdk)->addRef();
            std::lock_guard<std::mutex> lock(g_handles_mutex);
            auto it = g_handles.find(token_value(object));
            if (it == g_handles.end())
            {
                base(h.sdk)->release();
                return static_cast<MgResult>(MG_ERR_INVALID_HANDLE);
            }
            ++it->second.refs;
            return static_cast<MgResult>(MG_OK);
        });
    }

    MgResult MG_CALL object_release(void *object)
    {
        return guarded([&] {
            Handle h{};
            MgResult r = get_any_handle(object, h, true);
            if (r != MG_OK)
                return r;
            if (!h.owned)
                return static_cast<MgResult>(MG_ERR_INVALID_ARGUMENT);
            if (h.kind == Kind::undo && h.refs == 1 && static_cast<IMargretePluginUndoBuffer *>(h.sdk)->isRecording())
            {
                return static_cast<MgResult>(MG_ERR_INVALID_ARGUMENT);
            }
            {
                std::lock_guard<std::mutex> lock(g_handles_mutex);
                auto it = g_handles.find(token_value(object));
                if (it == g_handles.end() || it->second.refs <= 0)
                    return static_cast<MgResult>(MG_ERR_INVALID_HANDLE);
                --it->second.refs;
            }
            base(h.sdk)->release();
            return static_cast<MgResult>(MG_OK);
        });
    }

    MgResult MG_CALL report_error(MgContext *context, const char *message, uint32_t length)
    {
        return guarded([&] {
            Handle h{};
            MgResult r = get_handle(context, Kind::context, h);
            if (r != MG_OK)
                return r;
            std::wstring wide;
            r = validate_utf8_message(message, length, wide);
            if (r != MG_OK)
                return r;
            show_dialog(h.invocation->owner, wide.c_str());
            return static_cast<MgResult>(MG_OK);
        });
    }

    template <class T>
    MgResult sdk_out_handle(void *parent_token, Kind parent_kind, Kind out_kind, T **out,
                            MpBoolean(T::*dummy) = nullptr) = delete;

    MgResult MG_CALL context_get_document(MgContext *context, MgDocument **out)
    {
        if (!out)
            return MG_ERR_NULL_POINTER;
        *out = nullptr;
        return guarded([&] {
            Handle h{};
            MgResult r = get_handle(context, Kind::context, h);
            if (r != MG_OK)
                return r;
            IMargretePluginDocument *p{};
            if (!static_cast<IMargretePluginContext *>(h.sdk)->getDocument(&p) || !p)
                return static_cast<MgResult>(MG_ERR_SDK_FAILURE);
            *out = static_cast<MgDocument *>(make_handle(*h.invocation, Kind::document, p, true));
            return *out ? static_cast<MgResult>(MG_OK) : static_cast<MgResult>(MG_ERR_INTERNAL);
        });
    }

    MgResult MG_CALL context_get_main_window_handle(MgContext *context, MgNativeWindowHandle *out)
    {
        if (!out)
            return MG_ERR_NULL_POINTER;
        *out = nullptr;
        return guarded([&] {
            Handle h{};
            MgResult r = get_handle(context, Kind::context, h);
            if (r != MG_OK)
                return r;
            *out = static_cast<IMargretePluginContext *>(h.sdk)->getMainWindowHandle();
            h.invocation->owner = static_cast<HWND>(*out);
            return static_cast<MgResult>(MG_OK);
        });
    }
    MgResult MG_CALL context_get_current_tick(MgContext *context, MgInt *out)
    {
        if (!out)
            return MG_ERR_NULL_POINTER;
        *out = 0;
        return guarded([&] {
            Handle h{};
            MgResult r = get_handle(context, Kind::context, h);
            if (r != MG_OK)
                return r;
            *out = static_cast<IMargretePluginContext *>(h.sdk)->getCurrentTick();
            return static_cast<MgResult>(MG_OK);
        });
    }
    MgResult MG_CALL context_update(MgContext *context)
    {
        return guarded([&] {
            Handle h{};
            MgResult r = get_handle(context, Kind::context, h);
            if (r != MG_OK)
                return r;
            static_cast<IMargretePluginContext *>(h.sdk)->update();
            return static_cast<MgResult>(MG_OK);
        });
    }

    MgResult MG_CALL document_get_chart(MgDocument *doc, MgChart **out)
    {
        if (!out)
            return MG_ERR_NULL_POINTER;
        *out = nullptr;
        return guarded([&] {
            Handle h{};
            MgResult r = get_handle(doc, Kind::document, h);
            if (r != MG_OK)
                return r;
            IMargretePluginChart *p{};
            if (!static_cast<IMargretePluginDocument *>(h.sdk)->getChart(&p) || !p)
                return static_cast<MgResult>(MG_ERR_SDK_FAILURE);
            *out = static_cast<MgChart *>(make_handle(*h.invocation, Kind::chart, p, true));
            return *out ? static_cast<MgResult>(MG_OK) : static_cast<MgResult>(MG_ERR_INTERNAL);
        });
    }
    MgResult MG_CALL document_get_undo_buffer(MgDocument *doc, MgUndoBuffer **out)
    {
        if (!out)
            return MG_ERR_NULL_POINTER;
        *out = nullptr;
        return guarded([&] {
            Handle h{};
            MgResult r = get_handle(doc, Kind::document, h);
            if (r != MG_OK)
                return r;
            IMargretePluginUndoBuffer *p{};
            if (!static_cast<IMargretePluginDocument *>(h.sdk)->getUndoBuffer(&p) || !p)
                return static_cast<MgResult>(MG_ERR_SDK_FAILURE);
            *out = static_cast<MgUndoBuffer *>(make_handle(*h.invocation, Kind::undo, p, true));
            return *out ? static_cast<MgResult>(MG_OK) : static_cast<MgResult>(MG_ERR_INTERNAL);
        });
    }

    template <MpBoolean (IMargretePluginUndoBuffer::*Method)()> MgResult undo_action(MgUndoBuffer *undo)
    {
        return guarded([&] {
            Handle h{};
            MgResult r = get_handle(undo, Kind::undo, h);
            if (r != MG_OK)
                return r;
            return (static_cast<IMargretePluginUndoBuffer *>(h.sdk)->*Method)()
                       ? static_cast<MgResult>(MG_OK)
                       : static_cast<MgResult>(MG_ERR_SDK_FAILURE);
        });
    }
    MgResult MG_CALL undo_begin_recording(MgUndoBuffer *p)
    {
        return undo_action<&IMargretePluginUndoBuffer::beginRecording>(p);
    }
    MgResult MG_CALL undo_commit_recording(MgUndoBuffer *p)
    {
        return undo_action<&IMargretePluginUndoBuffer::commitRecording>(p);
    }
    MgResult MG_CALL undo_discard_recording(MgUndoBuffer *p)
    {
        return undo_action<&IMargretePluginUndoBuffer::discardRecording>(p);
    }
    MgResult MG_CALL undo_undo(MgUndoBuffer *p)
    {
        return undo_action<&IMargretePluginUndoBuffer::undo>(p);
    }
    MgResult MG_CALL undo_redo(MgUndoBuffer *p)
    {
        return undo_action<&IMargretePluginUndoBuffer::redo>(p);
    }

    template <MpBoolean (IMargretePluginUndoBuffer::*Method)() const>
    MgResult undo_query(MgUndoBuffer *undo, MgBool *out)
    {
        if (!out)
            return MG_ERR_NULL_POINTER;
        *out = MG_FALSE;
        return guarded([&] {
            Handle h{};
            MgResult r = get_handle(undo, Kind::undo, h);
            if (r != MG_OK)
                return r;
            *out = (static_cast<IMargretePluginUndoBuffer *>(h.sdk)->*Method)() ? MG_TRUE : MG_FALSE;
            return static_cast<MgResult>(MG_OK);
        });
    }
    MgResult MG_CALL undo_can_undo(MgUndoBuffer *p, MgBool *out)
    {
        return undo_query<&IMargretePluginUndoBuffer::canUndo>(p, out);
    }
    MgResult MG_CALL undo_can_redo(MgUndoBuffer *p, MgBool *out)
    {
        return undo_query<&IMargretePluginUndoBuffer::canRedo>(p, out);
    }
    MgResult MG_CALL undo_is_recording(MgUndoBuffer *p, MgBool *out)
    {
        return undo_query<&IMargretePluginUndoBuffer::isRecording>(p, out);
    }

    MgResult MG_CALL chart_create_note(MgChart *chart, MgNote **out)
    {
        if (!out)
            return MG_ERR_NULL_POINTER;
        *out = nullptr;
        return guarded([&] {
            Handle h{};
            MgResult r = get_handle(chart, Kind::chart, h);
            if (r != MG_OK)
                return r;
            IMargretePluginNote *p{};
            if (!static_cast<IMargretePluginChart *>(h.sdk)->createNote(&p) || !p)
                return static_cast<MgResult>(MG_ERR_SDK_FAILURE);
            *out = static_cast<MgNote *>(make_handle(*h.invocation, Kind::note, p, true));
            return *out ? static_cast<MgResult>(MG_OK) : static_cast<MgResult>(MG_ERR_INTERNAL);
        });
    }
    MgResult MG_CALL chart_get_notes_count(MgChart *chart, MgInt *out)
    {
        if (!out)
            return MG_ERR_NULL_POINTER;
        *out = 0;
        return guarded([&] {
            Handle h{};
            MgResult r = get_handle(chart, Kind::chart, h);
            if (r != MG_OK)
                return r;
            *out = static_cast<IMargretePluginChart *>(h.sdk)->getNotesCount();
            return *out < 0 ? static_cast<MgResult>(MG_ERR_SDK_FAILURE) : static_cast<MgResult>(MG_OK);
        });
    }
    MgResult MG_CALL chart_get_note(MgChart *chart, MgInt index, MgNote **out)
    {
        if (!out)
            return MG_ERR_NULL_POINTER;
        *out = nullptr;
        if (index < 0)
            return MG_ERR_INVALID_ARGUMENT;
        return guarded([&] {
            Handle h{};
            MgResult r = get_handle(chart, Kind::chart, h);
            if (r != MG_OK)
                return r;
            IMargretePluginNote *p{};
            if (!static_cast<IMargretePluginChart *>(h.sdk)->getNote(index, &p) || !p)
                return static_cast<MgResult>(MG_ERR_NOT_FOUND);
            *out = static_cast<MgNote *>(make_handle(*h.invocation, Kind::note, p, true));
            return *out ? static_cast<MgResult>(MG_OK) : static_cast<MgResult>(MG_ERR_INTERNAL);
        });
    }

    MgResult pair_handles(void *a, Kind ak, void *b, Kind bk, Handle &ah, Handle &bh)
    {
        MgResult r = get_handle(a, ak, ah);
        if (r != MG_OK)
            return r;
        r = get_handle(b, bk, bh);
        if (r != MG_OK)
            return r;
        return same_invocation(ah, bh) ? MG_OK : MG_ERR_INVALID_HANDLE;
    }
    MgResult MG_CALL chart_append_note(MgChart *c, MgNote *n)
    {
        return guarded([&] {
            Handle ch{}, nh{};
            MgResult r = pair_handles(c, Kind::chart, n, Kind::note, ch, nh);
            if (r != MG_OK)
                return r;
            return static_cast<IMargretePluginChart *>(ch.sdk)->appendNote(static_cast<IMargretePluginNote *>(nh.sdk))
                       ? static_cast<MgResult>(MG_OK)
                       : static_cast<MgResult>(MG_ERR_SDK_FAILURE);
        });
    }
    MgResult mark_deleted(void *token)
    {
        std::lock_guard<std::mutex> lock(g_handles_mutex);
        auto it = g_handles.find(token_value(token));
        if (it == g_handles.end())
            return MG_ERR_INVALID_HANDLE;
        void *sdk = it->second.sdk;
        Invocation *inv = it->second.invocation;
        for (auto &entry : g_handles)
            if (entry.second.sdk == sdk && entry.second.invocation == inv)
                entry.second.deleted = true;
        return MG_OK;
    }
    MgResult MG_CALL chart_delete_note(MgChart *c, MgNote *n)
    {
        return guarded([&] {
            Handle ch{}, nh{};
            MgResult r = pair_handles(c, Kind::chart, n, Kind::note, ch, nh);
            if (r != MG_OK)
                return r;
            if (!static_cast<IMargretePluginChart *>(ch.sdk)->deleteNote(static_cast<IMargretePluginNote *>(nh.sdk)))
                return static_cast<MgResult>(MG_ERR_SDK_FAILURE);
            return mark_deleted(n);
        });
    }
    MgResult MG_CALL chart_offset_notes(MgChart *c, MgInt tick)
    {
        return guarded([&] {
            Handle h{};
            MgResult r = get_handle(c, Kind::chart, h);
            if (r != MG_OK)
                return r;
            static_cast<IMargretePluginChart *>(h.sdk)->offsetNotes(tick);
            return static_cast<MgResult>(MG_OK);
        });
    }

    MgResult MG_CALL note_get_id(MgNote *n, MgInt *out)
    {
        if (!out)
            return MG_ERR_NULL_POINTER;
        *out = 0;
        return guarded([&] {
            Handle h{};
            MgResult r = get_handle(n, Kind::note, h);
            if (r != MG_OK)
                return r;
            *out = static_cast<IMargretePluginNote *>(h.sdk)->getId();
            return static_cast<MgResult>(MG_OK);
        });
    }
    MgResult MG_CALL note_get_info(MgNote *n, MgNoteInfo *out)
    {
        if (!out)
            return MG_ERR_NULL_POINTER;
        std::memset(out, 0, sizeof(*out));
        return guarded([&] {
            Handle h{};
            MgResult r = get_handle(n, Kind::note, h);
            if (r != MG_OK)
                return r;
            MP_NOTEINFO i{};
            static_cast<IMargretePluginNote *>(h.sdk)->getInfo(&i);
            *out = from_sdk(i);
            return static_cast<MgResult>(MG_OK);
        });
    }
    MgResult MG_CALL note_set_info(MgNote *n, const MgNoteInfo *in)
    {
        if (!in)
            return MG_ERR_NULL_POINTER;
        if (!valid_note_info(*in))
            return MG_ERR_INVALID_ARGUMENT;
        return guarded([&] {
            Handle h{};
            MgResult r = get_handle(n, Kind::note, h);
            if (r != MG_OK)
                return r;
            MP_NOTEINFO i = to_sdk(*in);
            static_cast<IMargretePluginNote *>(h.sdk)->setInfo(&i);
            return static_cast<MgResult>(MG_OK);
        });
    }
    MgResult MG_CALL note_get_children_count(MgNote *n, MgInt *out)
    {
        if (!out)
            return MG_ERR_NULL_POINTER;
        *out = 0;
        return guarded([&] {
            Handle h{};
            MgResult r = get_handle(n, Kind::note, h);
            if (r != MG_OK)
                return r;
            *out = static_cast<IMargretePluginNote *>(h.sdk)->getChildrenCount();
            return *out < 0 ? static_cast<MgResult>(MG_ERR_SDK_FAILURE) : static_cast<MgResult>(MG_OK);
        });
    }
    MgResult note_relation(MgNote *n, MgInt index, MgNote **out, int which)
    {
        if (!out)
            return MG_ERR_NULL_POINTER;
        *out = nullptr;
        if (which == 0 && index < 0)
            return MG_ERR_INVALID_ARGUMENT;
        return guarded([&] {
            Handle h{};
            MgResult r = get_handle(n, Kind::note, h);
            if (r != MG_OK)
                return r;
            IMargretePluginNote *p{};
            auto *s = static_cast<IMargretePluginNote *>(h.sdk);
            MpBoolean ok = which == 0 ? s->getChild(index, &p) : (which == 1 ? s->getParent(&p) : s->getBaseNote(&p));
            if (!ok || !p)
                return static_cast<MgResult>(MG_ERR_NOT_FOUND);
            *out = static_cast<MgNote *>(make_handle(*h.invocation, Kind::note, p, true));
            return *out ? static_cast<MgResult>(MG_OK) : static_cast<MgResult>(MG_ERR_INTERNAL);
        });
    }
    MgResult MG_CALL note_get_child(MgNote *n, MgInt i, MgNote **o)
    {
        return note_relation(n, i, o, 0);
    }
    MgResult MG_CALL note_get_parent(MgNote *n, MgNote **o)
    {
        return note_relation(n, 0, o, 1);
    }
    MgResult MG_CALL note_get_base_note(MgNote *n, MgNote **o)
    {
        return note_relation(n, 0, o, 2);
    }
    MgResult MG_CALL note_append_child(MgNote *n, MgNote *c)
    {
        return guarded([&] {
            Handle a{}, b{};
            MgResult r = pair_handles(n, Kind::note, c, Kind::note, a, b);
            if (r != MG_OK)
                return r;
            return static_cast<IMargretePluginNote *>(a.sdk)->appendChild(static_cast<IMargretePluginNote *>(b.sdk))
                       ? static_cast<MgResult>(MG_OK)
                       : static_cast<MgResult>(MG_ERR_SDK_FAILURE);
        });
    }
    MgResult MG_CALL note_delete_child(MgNote *n, MgNote *c)
    {
        return guarded([&] {
            Handle a{}, b{};
            MgResult r = pair_handles(n, Kind::note, c, Kind::note, a, b);
            if (r != MG_OK)
                return r;
            if (!static_cast<IMargretePluginNote *>(a.sdk)->deleteChild(static_cast<IMargretePluginNote *>(b.sdk)))
                return static_cast<MgResult>(MG_ERR_SDK_FAILURE);
            return mark_deleted(c);
        });
    }
    MgResult MG_CALL note_clone(MgNote *n, MgNote **out)
    {
        if (!out)
            return MG_ERR_NULL_POINTER;
        *out = nullptr;
        return guarded([&] {
            Handle h{};
            MgResult r = get_handle(n, Kind::note, h);
            if (r != MG_OK)
                return r;
            IMargretePluginNote *p{};
            if (!static_cast<IMargretePluginNote *>(h.sdk)->clone(&p) || !p)
                return static_cast<MgResult>(MG_ERR_SDK_FAILURE);
            *out = static_cast<MgNote *>(make_handle(*h.invocation, Kind::note, p, true));
            return *out ? static_cast<MgResult>(MG_OK) : static_cast<MgResult>(MG_ERR_INTERNAL);
        });
    }
    MgResult MG_CALL note_replace_with(MgNote *n, const MgNote *s, MgBool sort)
    {
        return guarded([&] {
            Handle a{}, b{};
            MgResult r = pair_handles(n, Kind::note, const_cast<MgNote *>(s), Kind::note, a, b);
            if (r != MG_OK)
                return r;
            static_cast<IMargretePluginNote *>(a.sdk)->replaceWith(static_cast<IMargretePluginNote *>(b.sdk),
                                                                   sort ? MP_TRUE : MP_FALSE);
            return static_cast<MgResult>(MG_OK);
        });
    }
    MgResult MG_CALL note_copy_info_to(MgNote *n, MgNote *d)
    {
        return guarded([&] {
            Handle a{}, b{};
            MgResult r = pair_handles(n, Kind::note, d, Kind::note, a, b);
            if (r != MG_OK)
                return r;
            static_cast<IMargretePluginNote *>(a.sdk)->copyInfoTo(static_cast<IMargretePluginNote *>(b.sdk));
            return static_cast<MgResult>(MG_OK);
        });
    }
    MgResult MG_CALL note_offset_child(MgNote *n, MgInt tick)
    {
        return guarded([&] {
            Handle h{};
            MgResult r = get_handle(n, Kind::note, h);
            if (r != MG_OK)
                return r;
            static_cast<IMargretePluginNote *>(h.sdk)->offsetChild(tick);
            return static_cast<MgResult>(MG_OK);
        });
    }
    MgResult MG_CALL note_flip_h(MgNote *n, MgBool recursive)
    {
        return guarded([&] {
            Handle h{};
            MgResult r = get_handle(n, Kind::note, h);
            if (r != MG_OK)
                return r;
            static_cast<IMargretePluginNote *>(h.sdk)->flipH(recursive ? MP_TRUE : MP_FALSE);
            return static_cast<MgResult>(MG_OK);
        });
    }

    MgResult MG_CALL chart_create_event(MgChart *c, MgEventKind kind, MgEvent **out)
    {
        if (!out)
            return MG_ERR_NULL_POINTER;
        *out = nullptr;
        if (!valid_event_kind(kind))
            return MG_ERR_INVALID_ARGUMENT;
        return guarded([&] {
            Handle h{};
            MgResult r = get_handle(c, Kind::chart, h);
            if (r != MG_OK)
                return r;
            void *p{};
            if (!static_cast<IMargretePluginChart *>(h.sdk)->createEvent(*event_iid(kind), &p) || !p)
                return static_cast<MgResult>(MG_ERR_SDK_FAILURE);
            *out = static_cast<MgEvent *>(make_handle(*h.invocation, Kind::event, p, true, kind));
            return *out ? static_cast<MgResult>(MG_OK) : static_cast<MgResult>(MG_ERR_INTERNAL);
        });
    }
    IMargretePluginEvent *event_base(const Handle &h)
    {
        switch (h.event_kind)
        {
        case MG_EVENT_KIND_TIMELINE_SPEED:
            return static_cast<IMargretePluginEventTimelineSpeed *>(h.sdk);
        case MG_EVENT_KIND_NOTE_SPEED_MODIFIER:
            return static_cast<IMargretePluginEventNoteSpeedModifier *>(h.sdk);
        case MG_EVENT_KIND_BPM:
            return static_cast<IMargretePluginEventBpm *>(h.sdk);
        default:
            return static_cast<IMargretePluginEventBeatChange *>(h.sdk);
        }
    }
    MgResult MG_CALL chart_append_event(MgChart *c, MgEvent *e)
    {
        return guarded([&] {
            Handle a{}, b{};
            MgResult r = pair_handles(c, Kind::chart, e, Kind::event, a, b);
            if (r != MG_OK)
                return r;
            return static_cast<IMargretePluginChart *>(a.sdk)->appendEvent(event_base(b))
                       ? static_cast<MgResult>(MG_OK)
                       : static_cast<MgResult>(MG_ERR_SDK_FAILURE);
        });
    }
    MgResult MG_CALL chart_delete_event(MgChart *c, MgEvent *e)
    {
        return guarded([&] {
            Handle a{}, b{};
            MgResult r = pair_handles(c, Kind::chart, e, Kind::event, a, b);
            if (r != MG_OK)
                return r;
            if (!static_cast<IMargretePluginChart *>(a.sdk)->deleteEvent(event_base(b)))
                return static_cast<MgResult>(MG_ERR_SDK_FAILURE);
            return mark_deleted(e);
        });
    }
    MgResult find_event(MgChart *c, MgEventKind kind, MgInt a, MgInt b, MgEvent **out)
    {
        if (!out)
            return MG_ERR_NULL_POINTER;
        *out = nullptr;
        return guarded([&] {
            Handle h{};
            MgResult r = get_handle(c, Kind::chart, h);
            if (r != MG_OK)
                return r;
            void *p{};
            auto *s = static_cast<IMargretePluginChart *>(h.sdk);
            MpBoolean ok = MP_FALSE;
            switch (kind)
            {
            case MG_EVENT_KIND_TIMELINE_SPEED:
                ok = s->findEventTimelineSpeed(a, b, &p);
                break;
            case MG_EVENT_KIND_NOTE_SPEED_MODIFIER:
                ok = s->findEventNoteSpeedModifier(a, &p);
                break;
            case MG_EVENT_KIND_BPM:
                ok = s->findEventBpm(a, &p);
                break;
            default:
                ok = s->findEventBeatChange(a, &p);
                break;
            }
            if (!ok || !p)
                return static_cast<MgResult>(MG_ERR_NOT_FOUND);
            *out = static_cast<MgEvent *>(make_handle(*h.invocation, Kind::event, p, true, kind));
            return *out ? static_cast<MgResult>(MG_OK) : static_cast<MgResult>(MG_ERR_INTERNAL);
        });
    }
    MgResult MG_CALL chart_find_event_timeline_speed(MgChart *c, MgInt t, MgInt id, MgEvent **o)
    {
        return find_event(c, MG_EVENT_KIND_TIMELINE_SPEED, t, id, o);
    }
    MgResult MG_CALL chart_find_event_note_speed_modifier(MgChart *c, MgInt t, MgEvent **o)
    {
        return find_event(c, MG_EVENT_KIND_NOTE_SPEED_MODIFIER, t, 0, o);
    }
    MgResult MG_CALL chart_find_event_bpm(MgChart *c, MgInt t, MgEvent **o)
    {
        return find_event(c, MG_EVENT_KIND_BPM, t, 0, o);
    }
    MgResult MG_CALL chart_find_event_beat_change(MgChart *c, MgInt b, MgEvent **o)
    {
        return find_event(c, MG_EVENT_KIND_BEAT_CHANGE, b, 0, o);
    }
    MgResult MG_CALL event_get_kind(MgEvent *e, MgEventKind *out)
    {
        if (!out)
            return MG_ERR_NULL_POINTER;
        *out = 0;
        Handle h{};
        MgResult r = get_handle(e, Kind::event, h);
        if (r == MG_OK)
            *out = h.event_kind;
        return r;
    }
    MgResult MG_CALL event_get_id(MgEvent *e, MgInt *out)
    {
        if (!out)
            return MG_ERR_NULL_POINTER;
        *out = 0;
        return guarded([&] {
            Handle h{};
            MgResult r = get_handle(e, Kind::event, h);
            if (r != MG_OK)
                return r;
            *out = event_base(h)->getId();
            return static_cast<MgResult>(MG_OK);
        });
    }

    template <class Public, class Sdk, class Interface, void (Interface::*Get)(Sdk *) const>
    MgResult event_info_get(MgEvent *e, MgEventKind kind, Public *out)
    {
        if (!out)
            return MG_ERR_NULL_POINTER;
        std::memset(out, 0, sizeof(*out));
        return guarded([&] {
            Handle h{};
            MgResult r = get_handle(e, Kind::event, h);
            if (r != MG_OK)
                return r;
            if (h.event_kind != kind)
                return static_cast<MgResult>(MG_ERR_INVALID_ARGUMENT);
            Sdk i{};
            (static_cast<Interface *>(h.sdk)->*Get)(&i);
            std::memcpy(out, &i, sizeof(i));
            return static_cast<MgResult>(MG_OK);
        });
    }
    template <class Public, class Sdk, class Interface, void (Interface::*Set)(const Sdk *)>
    MgResult event_info_set(MgEvent *e, MgEventKind kind, const Public *in, bool finite)
    {
        if (!in)
            return MG_ERR_NULL_POINTER;
        if (finite && !std::isfinite(in->speed))
            return MG_ERR_INVALID_ARGUMENT;
        return guarded([&] {
            Handle h{};
            MgResult r = get_handle(e, Kind::event, h);
            if (r != MG_OK)
                return r;
            if (h.event_kind != kind)
                return static_cast<MgResult>(MG_ERR_INVALID_ARGUMENT);
            Sdk i{};
            std::memcpy(&i, in, sizeof(i));
            (static_cast<Interface *>(h.sdk)->*Set)(&i);
            return static_cast<MgResult>(MG_OK);
        });
    }
    MgResult MG_CALL event_timeline_speed_get_info(MgEvent *e, MgEventTimelineSpeedInfo *o)
    {
        return event_info_get<MgEventTimelineSpeedInfo, MP_EVENT_TLSINFO, IMargretePluginEventTimelineSpeed,
                              &IMargretePluginEventTimelineSpeed::getInfo>(e, MG_EVENT_KIND_TIMELINE_SPEED, o);
    }
    MgResult MG_CALL event_timeline_speed_set_info(MgEvent *e, const MgEventTimelineSpeedInfo *i)
    {
        return event_info_set<MgEventTimelineSpeedInfo, MP_EVENT_TLSINFO, IMargretePluginEventTimelineSpeed,
                              &IMargretePluginEventTimelineSpeed::setInfo>(e, MG_EVENT_KIND_TIMELINE_SPEED, i, true);
    }
    MgResult MG_CALL event_note_speed_modifier_get_info(MgEvent *e, MgEventNoteSpeedModifierInfo *o)
    {
        return event_info_get<MgEventNoteSpeedModifierInfo, MP_EVENT_NSMINFO, IMargretePluginEventNoteSpeedModifier,
                              &IMargretePluginEventNoteSpeedModifier::getInfo>(e, MG_EVENT_KIND_NOTE_SPEED_MODIFIER, o);
    }
    MgResult MG_CALL event_note_speed_modifier_set_info(MgEvent *e, const MgEventNoteSpeedModifierInfo *i)
    {
        return event_info_set<MgEventNoteSpeedModifierInfo, MP_EVENT_NSMINFO, IMargretePluginEventNoteSpeedModifier,
                              &IMargretePluginEventNoteSpeedModifier::setInfo>(e, MG_EVENT_KIND_NOTE_SPEED_MODIFIER, i,
                                                                               true);
    }
    MgResult MG_CALL event_bpm_get_info(MgEvent *e, MgEventBpmInfo *o)
    {
        return event_info_get<MgEventBpmInfo, MP_EVENT_BPMINFO, IMargretePluginEventBpm,
                              &IMargretePluginEventBpm::getInfo>(e, MG_EVENT_KIND_BPM, o);
    }
    MgResult MG_CALL event_bpm_set_info(MgEvent *e, const MgEventBpmInfo *i)
    {
        if (!i)
            return MG_ERR_NULL_POINTER;
        if (!std::isfinite(i->bpm))
            return MG_ERR_INVALID_ARGUMENT;
        return guarded([&] {
            Handle h{};
            MgResult r = get_handle(e, Kind::event, h);
            if (r != MG_OK)
                return r;
            if (h.event_kind != MG_EVENT_KIND_BPM)
                return static_cast<MgResult>(MG_ERR_INVALID_ARGUMENT);
            MP_EVENT_BPMINFO x{i->tick, i->bpm};
            static_cast<IMargretePluginEventBpm *>(h.sdk)->setInfo(&x);
            return static_cast<MgResult>(MG_OK);
        });
    }
    MgResult MG_CALL event_beat_change_get_info(MgEvent *e, MgEventBeatChangeInfo *o)
    {
        return event_info_get<MgEventBeatChangeInfo, MP_EVENT_BCINFO, IMargretePluginEventBeatChange,
                              &IMargretePluginEventBeatChange::getInfo>(e, MG_EVENT_KIND_BEAT_CHANGE, o);
    }
    MgResult MG_CALL event_beat_change_set_info(MgEvent *e, const MgEventBeatChangeInfo *i)
    {
        if (!i)
            return MG_ERR_NULL_POINTER;
        return guarded([&] {
            Handle h{};
            MgResult r = get_handle(e, Kind::event, h);
            if (r != MG_OK)
                return r;
            if (h.event_kind != MG_EVENT_KIND_BEAT_CHANGE)
                return static_cast<MgResult>(MG_ERR_INVALID_ARGUMENT);
            MP_EVENT_BCINFO x{i->bar, i->beats_per_bar, i->beat_unit};
            static_cast<IMargretePluginEventBeatChange *>(h.sdk)->setInfo(&x);
            return static_cast<MgResult>(MG_OK);
        });
    }

    MgResult event_copy_replace(MgEvent *d, const MgEvent *s, bool replace)
    {
        return guarded([&] {
            Handle a{}, b{};
            MgResult r = pair_handles(d, Kind::event, const_cast<MgEvent *>(s), Kind::event, a, b);
            if (r != MG_OK)
                return r;
            if (a.event_kind != b.event_kind)
                return static_cast<MgResult>(MG_ERR_INVALID_ARGUMENT);
            switch (a.event_kind)
            {
            case MG_EVENT_KIND_TIMELINE_SPEED:
                if (replace)
                    static_cast<IMargretePluginEventTimelineSpeed *>(a.sdk)->replaceWith(
                        static_cast<IMargretePluginEventTimelineSpeed *>(b.sdk));
                else
                    static_cast<IMargretePluginEventTimelineSpeed *>(a.sdk)->copyInfoTo(
                        static_cast<IMargretePluginEventTimelineSpeed *>(b.sdk));
                break;
            case MG_EVENT_KIND_NOTE_SPEED_MODIFIER:
                if (replace)
                    static_cast<IMargretePluginEventNoteSpeedModifier *>(a.sdk)->replaceWith(
                        static_cast<IMargretePluginEventNoteSpeedModifier *>(b.sdk));
                else
                    static_cast<IMargretePluginEventNoteSpeedModifier *>(a.sdk)->copyInfoTo(
                        static_cast<IMargretePluginEventNoteSpeedModifier *>(b.sdk));
                break;
            case MG_EVENT_KIND_BPM:
                if (replace)
                    static_cast<IMargretePluginEventBpm *>(a.sdk)->replaceWith(
                        static_cast<IMargretePluginEventBpm *>(b.sdk));
                else
                    static_cast<IMargretePluginEventBpm *>(a.sdk)->copyInfoTo(
                        static_cast<IMargretePluginEventBpm *>(b.sdk));
                break;
            default:
                if (replace)
                    static_cast<IMargretePluginEventBeatChange *>(a.sdk)->replaceWith(
                        static_cast<IMargretePluginEventBeatChange *>(b.sdk));
                else
                    static_cast<IMargretePluginEventBeatChange *>(a.sdk)->copyInfoTo(
                        static_cast<IMargretePluginEventBeatChange *>(b.sdk));
                break;
            }
            return static_cast<MgResult>(MG_OK);
        });
    }
    MgResult MG_CALL event_replace_with(MgEvent *d, const MgEvent *s)
    {
        return event_copy_replace(d, s, true);
    }
    MgResult MG_CALL event_copy_info_to(MgEvent *s, MgEvent *d)
    {
        return event_copy_replace(s, d, false);
    }

    const MgHostApi g_host_api = {sizeof(MgHostApi),
                                  MG_ABI_VERSION,
                                  object_add_ref,
                                  object_release,
                                  report_error,
                                  context_get_document,
                                  context_get_main_window_handle,
                                  context_get_current_tick,
                                  context_update,
                                  document_get_chart,
                                  document_get_undo_buffer,
                                  undo_begin_recording,
                                  undo_commit_recording,
                                  undo_discard_recording,
                                  undo_undo,
                                  undo_redo,
                                  undo_can_undo,
                                  undo_can_redo,
                                  undo_is_recording,
                                  chart_create_note,
                                  chart_get_notes_count,
                                  chart_get_note,
                                  chart_append_note,
                                  chart_delete_note,
                                  chart_offset_notes,
                                  note_get_id,
                                  note_get_info,
                                  note_set_info,
                                  note_get_children_count,
                                  note_get_child,
                                  note_get_parent,
                                  note_append_child,
                                  note_delete_child,
                                  note_clone,
                                  note_replace_with,
                                  note_copy_info_to,
                                  note_get_base_note,
                                  note_offset_child,
                                  note_flip_h,
                                  chart_create_event,
                                  chart_append_event,
                                  chart_delete_event,
                                  chart_find_event_timeline_speed,
                                  chart_find_event_note_speed_modifier,
                                  chart_find_event_bpm,
                                  chart_find_event_beat_change,
                                  event_get_kind,
                                  event_get_id,
                                  event_timeline_speed_get_info,
                                  event_timeline_speed_set_info,
                                  event_note_speed_modifier_get_info,
                                  event_note_speed_modifier_set_info,
                                  event_bpm_get_info,
                                  event_bpm_set_info,
                                  event_beat_change_get_info,
                                  event_beat_change_set_info,
                                  event_replace_with,
                                  event_copy_info_to};

    std::once_flag g_init_once;
    MgPluginApi g_plugin_api{};
    MgResult g_init_result = MG_ERR_PLUGIN_INIT_FAILED;

    void initialize_plugin() noexcept
    {
        std::call_once(g_init_once, [] {
            g_plugin_api = {};
            g_plugin_api.struct_size = sizeof(g_plugin_api);
            g_plugin_api.abi_version = MG_ABI_VERSION;
            try
            {
                g_init_result = mg_plugin_init(MG_ABI_VERSION, &g_host_api, &g_plugin_api);
            }
            catch (...)
            {
                g_init_result = MG_ERR_PANIC;
            }
            constexpr size_t required = offsetof(MgPluginApi, invoke) + sizeof(g_plugin_api.invoke);
            if (g_init_result == MG_OK &&
                (g_plugin_api.struct_size < required || g_plugin_api.abi_version != MG_ABI_VERSION ||
                 !g_plugin_api.get_plugin_info || !g_plugin_api.create_command || !g_plugin_api.destroy_command ||
                 !g_plugin_api.get_command_name || !g_plugin_api.invoke))
            {
                g_init_result = MG_ERR_PLUGIN_INIT_FAILED;
            }
            if (g_init_result != MG_OK)
                show_result(nullptr, L"mg_plugin_init", g_init_result);
        });
    }

    MgResult get_utf8_string(void *command, bool command_name, std::wstring &out)
    {
        uint32_t required = 0;
        MgResult r;
        if (command_name)
        {
            r = g_plugin_api.get_command_name(g_plugin_api.plugin_data, command, nullptr, 0, &required);
        }
        else
            return MG_ERR_INTERNAL;
        if (r != MG_OK || required == 0 || required > static_cast<uint32_t>(INT_MAX))
            return r == MG_OK ? MG_ERR_INVALID_ARGUMENT : r;
        std::vector<char> buffer(required);
        uint32_t second_required = 0;
        r = g_plugin_api.get_command_name(g_plugin_api.plugin_data, command, buffer.data(), required, &second_required);
        if (r != MG_OK)
            return r;
        if (second_required != required || buffer.back() != '\0' || std::memchr(buffer.data(), '\0', required - 1))
            return MG_ERR_INVALID_ARGUMENT;
        return utf8_to_wide(buffer.data(), required - 1, out) ? MG_OK : MG_ERR_INVALID_ARGUMENT;
    }

    MgResult get_plugin_strings(std::wstring &name, std::wstring &desc, std::wstring &developer)
    {
        uint32_t nr = 0, dr = 0, vr = 0;
        MgPluginInfoBuffers q{sizeof(q), nullptr, 0, &nr, nullptr, 0, &dr, nullptr, 0, &vr};
        MgResult r = g_plugin_api.get_plugin_info(g_plugin_api.plugin_data, &q);
        if (r != MG_OK)
            return r;
        if (!nr || !dr || !vr || nr > static_cast<uint32_t>(INT_MAX) || dr > static_cast<uint32_t>(INT_MAX) ||
            vr > static_cast<uint32_t>(INT_MAX))
            return MG_ERR_INVALID_ARGUMENT;
        std::vector<char> n(nr), d(dr), v(vr);
        uint32_t nr2 = 0, dr2 = 0, vr2 = 0;
        MgPluginInfoBuffers b{sizeof(b), n.data(), nr, &nr2, d.data(), dr, &dr2, v.data(), vr, &vr2};
        r = g_plugin_api.get_plugin_info(g_plugin_api.plugin_data, &b);
        if (r != MG_OK)
            return r;
        auto valid = [](const std::vector<char> &x, uint32_t a, uint32_t z) {
            return a == z && x.back() == '\0' && !std::memchr(x.data(), '\0', z - 1);
        };
        if (!valid(n, nr2, nr) || !valid(d, dr2, dr) || !valid(v, vr2, vr))
            return MG_ERR_INVALID_ARGUMENT;
        if (!utf8_to_wide(n.data(), nr - 1, name) || !utf8_to_wide(d.data(), dr - 1, desc) ||
            !utf8_to_wide(v.data(), vr - 1, developer))
            return MG_ERR_INVALID_ARGUMENT;
        return MG_OK;
    }

    class Command final : public IMargretePluginCommand
    {
      public:
        explicit Command(void *command) : command_(command)
        {
        }
        MpInteger addRef() override
        {
            return ++refs_;
        }
        MpInteger release() override
        {
            const MpInteger n = --refs_;
            if (!n)
                delete this;
            return n;
        }
        MpBoolean queryInterface(const MpGuid &iid, void **out) override
        {
            if (!out)
                return MP_FALSE;
            *out = nullptr;
            if (!guid_equal(iid, IID_IMargretePluginBase) && !guid_equal(iid, IID_IMargretePluginCommand))
                return MP_FALSE;
            *out = this;
            addRef();
            return MP_TRUE;
        }
        MpBoolean getCommandName(wchar_t *text, MpInteger length) const override
        {
            if (!text || length <= 0)
                return MP_FALSE;
            text[0] = L'\0';
            if (poisoned_)
            {
                show_dialog(nullptr, L"This command instance is poisoned.");
                return MP_FALSE;
            }
            try
            {
                std::wstring value;
                MgResult r = get_utf8_string(command_, true, value);
                if (r != MG_OK)
                {
                    show_result(nullptr, L"get_command_name", r);
                    return MP_FALSE;
                }
                copy_wide(text, length, value);
                return MP_TRUE;
            }
            catch (...)
            {
                poisoned_ = true;
                show_dialog(nullptr, L"get_command_name raised an exception; the command is now poisoned.");
                return MP_FALSE;
            }
        }
        MpBoolean invoke(IMargretePluginContext *context) override
        {
            if (!context)
            {
                show_dialog(nullptr, L"Margrete passed a null context.");
                return MP_FALSE;
            }
            bool expected = false;
            if (!invoking_.compare_exchange_strong(expected, true))
            {
                show_dialog(nullptr, L"Concurrent or reentrant invoke was rejected.");
                return MP_FALSE;
            }
            struct Reset
            {
                std::atomic<bool> &v;
                ~Reset()
                {
                    v = false;
                }
            } reset{invoking_};
            if (poisoned_)
            {
                show_dialog(nullptr, L"This command instance is poisoned.");
                return MP_FALSE;
            }
            Invocation inv;
            try
            {
                inv.owner = static_cast<HWND>(context->getMainWindowHandle());
            }
            catch (...)
            {
                poisoned_ = true;
                show_dialog(nullptr, L"The SDK raised an exception; the command is now poisoned.");
                return MP_FALSE;
            }
            MgContext *token = static_cast<MgContext *>(make_handle(inv, Kind::context, context, false));
            MgResult result = MG_ERR_INTERNAL;
            try
            {
                result = g_plugin_api.invoke(g_plugin_api.plugin_data, command_, token);
            }
            catch (...)
            {
                poisoned_ = true;
                result = MG_ERR_PANIC;
                show_dialog(inv.owner, L"invoke raised an exception; the command is now poisoned.");
            }
            bool recording = false;
            std::vector<IMargretePluginUndoBuffer *> undos;
            {
                std::lock_guard<std::mutex> lock(g_handles_mutex);
                for (uintptr_t t : inv.handles)
                {
                    auto it = g_handles.find(t);
                    if (it != g_handles.end() && it->second.kind == Kind::undo && it->second.refs > 0)
                        undos.push_back(static_cast<IMargretePluginUndoBuffer *>(it->second.sdk));
                }
            }
            for (auto *u : undos)
            {
                try
                {
                    if (u->isRecording())
                    {
                        recording = true;
                        u->discardRecording();
                    }
                }
                catch (...)
                {
                    recording = true;
                }
            }
            close_invocation(inv);
            if (recording && result == MG_OK)
                result = MG_ERR_SDK_FAILURE;
            if (result != MG_OK && result != MG_ERR_PANIC)
                show_result(inv.owner, L"invoke", result);
            return result == MG_OK ? MP_TRUE : MP_FALSE;
        }

      private:
        ~Command()
        {
            try
            {
                g_plugin_api.destroy_command(g_plugin_api.plugin_data, command_);
            }
            catch (...)
            {
                show_dialog(nullptr, L"destroy_command raised an exception.");
            }
        }
        std::atomic<MpInteger> refs_{1};
        void *command_{};
        mutable bool poisoned_{};
        std::atomic<bool> invoking_{false};
    };

} // namespace

extern "C" __declspec(dllexport) void __stdcall MargretePluginGetInfo(MP_PLUGININFO *info)
{
    if (!info)
        return;
    info->sdkVersion = MP_SDK_VERSION;
    initialize_plugin();
    if (g_init_result != MG_OK)
        return;
    try
    {
        std::wstring n, d, v;
        MgResult r = get_plugin_strings(n, d, v);
        if (r != MG_OK)
        {
            show_result(nullptr, L"get_plugin_info", r);
            return;
        }
        copy_wide(info->nameBuffer, info->nameBufferLength, n);
        copy_wide(info->descBuffer, info->descBufferLength, d);
        copy_wide(info->developerBuffer, info->developerBufferLength, v);
    }
    catch (...)
    {
        show_dialog(nullptr, L"get_plugin_info raised an exception.");
    }
}

extern "C" __declspec(dllexport) MpBoolean __stdcall MargretePluginCommandCreate(IMargretePluginCommand **out)
{
    if (!out)
        return MP_FALSE;
    *out = nullptr;
    initialize_plugin();
    if (g_init_result != MG_OK)
        return MP_FALSE;
    try
    {
        void *command{};
        MgResult r = g_plugin_api.create_command(g_plugin_api.plugin_data, &command);
        if (r != MG_OK || !command)
        {
            show_result(nullptr, L"create_command", r == MG_OK ? MG_ERR_PLUGIN_INIT_FAILED : r);
            return MP_FALSE;
        }
        Command *wrapper = new (std::nothrow) Command(command);
        if (!wrapper)
        {
            try
            {
                g_plugin_api.destroy_command(g_plugin_api.plugin_data, command);
            }
            catch (...)
            {
            }
            return MP_FALSE;
        }
        *out = wrapper;
        return MP_TRUE;
    }
    catch (...)
    {
        show_dialog(nullptr, L"create_command raised an exception.");
        return MP_FALSE;
    }
}

static_assert(MP_SDK_VERSION == 2, "unexpected Margrete SDK version");
static_assert(MP_NOTETYPE_UNKNOWN == MG_NOTE_TYPE_UNKNOWN && MP_NOTETYPE_CLICK == MG_NOTE_TYPE_CLICK &&
              MP_NOTETYPE_LAST == 13);
static_assert(MP_NOTELONGATTR_NONE == MG_NOTE_LONG_ATTR_NONE &&
              MP_NOTELONGATTR_END_NOACT == MG_NOTE_LONG_ATTR_END_NOACT);
static_assert(MP_NOTEDIR_NONE == MG_NOTE_DIRECTION_NONE && MP_NOTEDIR_OUTIN == MG_NOTE_DIRECTION_OUT_IN);
static_assert(MP_NOTEEXATTR_NONE == MG_NOTE_EX_ATTR_NONE && MP_NOTEEXATTR_EXJDG == MG_NOTE_EX_ATTR_EXJDG);
static_assert(MP_OPTIONVALUE_AIRCRUSH_TRACELIKE == MG_OPTION_AIRCRUSH_TRACE_LIKE);
static_assert(MP_OPTIONVALUE_AIRCRUSH_HEADONLY == MG_OPTION_AIRCRUSH_HEAD_ONLY);
static_assert(sizeof(MP_NOTEINFO) == sizeof(MgNoteInfo));
static_assert(sizeof(MP_EVENT_TLSINFO) == sizeof(MgEventTimelineSpeedInfo));
static_assert(sizeof(MP_EVENT_NSMINFO) == sizeof(MgEventNoteSpeedModifierInfo));
static_assert(sizeof(MP_EVENT_BPMINFO) == sizeof(MgEventBpmInfo));
static_assert(sizeof(MP_EVENT_BCINFO) == sizeof(MgEventBeatChangeInfo));
static_assert(sizeof(MgNoteInfo) == 44 && alignof(MgNoteInfo) == 4);
static_assert(sizeof(MgEventTimelineSpeedInfo) == 16 && offsetof(MgEventTimelineSpeedInfo, speed) == 8);
static_assert(sizeof(MgEventNoteSpeedModifierInfo) == 16 && offsetof(MgEventNoteSpeedModifierInfo, speed) == 8);
static_assert(sizeof(MgEventBpmInfo) == 16 && offsetof(MgEventBpmInfo, bpm) == 8);
static_assert(sizeof(MgEventBeatChangeInfo) == 12);

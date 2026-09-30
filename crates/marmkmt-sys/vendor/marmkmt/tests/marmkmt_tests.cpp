#include "MargretePlugin.h"
#include "marmkmt.h"

#include <atomic>
#ifdef NDEBUG
#undef NDEBUG
#endif
#include <cassert>
#include <cmath>
#include <cstring>
#include <string>
#include <thread>
#include <vector>

extern "C" void __stdcall MargretePluginGetInfo(MP_PLUGININFO *);
extern "C" MpBoolean __stdcall MargretePluginCommandCreate(IMargretePluginCommand **);

namespace
{

    template <class T> MpBoolean qi(T *self, const MpGuid &, void **out)
    {
        if (!out)
            return MP_FALSE;
        *out = self;
        self->addRef();
        return MP_TRUE;
    }

    struct FakeUndo final : IMargretePluginUndoBuffer
    {
        MpInteger refs{1};
        bool recording{};
        bool undoable{};
        bool redoable{};
        MpInteger addRef() override
        {
            return ++refs;
        }
        MpInteger release() override
        {
            return --refs;
        }
        MpBoolean queryInterface(const MpGuid &id, void **out) override
        {
            return qi(this, id, out);
        }
        MpBoolean beginRecording() override
        {
            if (recording)
                return MP_FALSE;
            recording = true;
            return MP_TRUE;
        }
        MpBoolean commitRecording() override
        {
            if (!recording)
                return MP_FALSE;
            recording = false;
            undoable = true;
            return MP_TRUE;
        }
        MpBoolean discardRecording() override
        {
            if (!recording)
                return MP_FALSE;
            recording = false;
            return MP_TRUE;
        }
        MpBoolean undo() override
        {
            if (!undoable)
                return MP_FALSE;
            undoable = false;
            redoable = true;
            return MP_TRUE;
        }
        MpBoolean redo() override
        {
            if (!redoable)
                return MP_FALSE;
            redoable = false;
            undoable = true;
            return MP_TRUE;
        }
        MpBoolean canUndo() const override
        {
            return undoable ? MP_TRUE : MP_FALSE;
        }
        MpBoolean canRedo() const override
        {
            return redoable ? MP_TRUE : MP_FALSE;
        }
        MpBoolean isRecording() const override
        {
            return recording ? MP_TRUE : MP_FALSE;
        }
    };

    struct FakeNote final : IMargretePluginNote
    {
        MpInteger refs{1};
        MpInteger id{42};
        MP_NOTEINFO info{};
        FakeNote *parent{};
        std::vector<FakeNote *> children;
        MpInteger addRef() override
        {
            return ++refs;
        }
        MpInteger release() override
        {
            return --refs;
        }
        MpBoolean queryInterface(const MpGuid &i, void **o) override
        {
            return qi(this, i, o);
        }
        MpInteger getId() const override
        {
            return id;
        }
        void getInfo(MP_NOTEINFO *out) const override
        {
            *out = info;
        }
        void setInfo(const MP_NOTEINFO *in) override
        {
            info = *in;
        }
        MpInteger getChildrenCount() const override
        {
            return static_cast<MpInteger>(children.size());
        }
        MpBoolean getChild(MpInteger i, IMargretePluginNote **out) const override
        {
            if (!out || i < 0 || static_cast<size_t>(i) >= children.size())
                return MP_FALSE;
            *out = children[static_cast<size_t>(i)];
            (*out)->addRef();
            return MP_TRUE;
        }
        MpBoolean getParent(IMargretePluginNote **out) const override
        {
            if (!out || !parent)
                return MP_FALSE;
            *out = parent;
            parent->addRef();
            return MP_TRUE;
        }
        MpBoolean appendChild(IMargretePluginNote *n) override
        {
            auto *p = static_cast<FakeNote *>(n);
            children.push_back(p);
            p->parent = this;
            return MP_TRUE;
        }
        MpBoolean deleteChild(IMargretePluginNote *n) override
        {
            for (auto i = children.begin(); i != children.end(); ++i)
                if (*i == n)
                {
                    children.erase(i);
                    return MP_TRUE;
                }
            return MP_FALSE;
        }
        MpBoolean clone(IMargretePluginNote **out) const override
        {
            if (!out)
                return MP_FALSE;
            auto *n = new FakeNote();
            n->info = info;
            *out = n;
            return MP_TRUE;
        }
        void replaceWith(const IMargretePluginNote *s, MpBoolean) override
        {
            info = static_cast<const FakeNote *>(s)->info;
        }
        void copyInfoTo(IMargretePluginNote *d) const override
        {
            static_cast<FakeNote *>(d)->info = info;
        }
        MpBoolean getBaseNote(IMargretePluginNote **out) const override
        {
            if (!out || !parent)
                return MP_FALSE;
            *out = parent;
            parent->addRef();
            return MP_TRUE;
        }
        void offsetChild(MpInteger tick) override
        {
            for (auto *c : children)
                c->info.tick += tick;
        }
        void flipH(MpBoolean recursive) override
        {
            info.x = -info.x;
            if (recursive)
                for (auto *c : children)
                    c->flipH(MP_TRUE);
        }
    };

    template <class Interface, class Info> struct EventCommon : Interface
    {
        MpInteger refs{1};
        MpInteger id{77};
        Info info{};
        MpInteger addRef() override
        {
            return ++refs;
        }
        MpInteger release() override
        {
            return --refs;
        }
        MpBoolean queryInterface(const MpGuid &i, void **o) override
        {
            return qi(this, i, o);
        }
        MpInteger getId() const override
        {
            return id;
        }
        void getInfo(Info *out) const override
        {
            *out = info;
        }
        void setInfo(const Info *in) override
        {
            info = *in;
        }
        void replaceWith(const Interface *s) override
        {
            info = static_cast<const EventCommon *>(s)->info;
        }
        void copyInfoTo(Interface *d) const override
        {
            static_cast<EventCommon *>(d)->info = info;
        }
    };
    using FakeTls = EventCommon<IMargretePluginEventTimelineSpeed, MP_EVENT_TLSINFO>;
    using FakeNsm = EventCommon<IMargretePluginEventNoteSpeedModifier, MP_EVENT_NSMINFO>;
    using FakeBpm = EventCommon<IMargretePluginEventBpm, MP_EVENT_BPMINFO>;
    using FakeBc = EventCommon<IMargretePluginEventBeatChange, MP_EVENT_BCINFO>;

    struct FakeChart final : IMargretePluginChart
    {
        MpInteger refs{1};
        std::vector<FakeNote *> notes;
        std::vector<IMargretePluginEvent *> events;
        MpInteger addRef() override
        {
            return ++refs;
        }
        MpInteger release() override
        {
            return --refs;
        }
        MpBoolean queryInterface(const MpGuid &i, void **o) override
        {
            return qi(this, i, o);
        }
        MpBoolean createNote(IMargretePluginNote **out) const override
        {
            if (!out)
                return MP_FALSE;
            *out = new FakeNote();
            return MP_TRUE;
        }
        MpInteger getNotesCount() const override
        {
            return static_cast<MpInteger>(notes.size());
        }
        MpBoolean getNote(MpInteger i, IMargretePluginNote **out) override
        {
            if (!out || i < 0 || static_cast<size_t>(i) >= notes.size())
                return MP_FALSE;
            *out = notes[static_cast<size_t>(i)];
            (*out)->addRef();
            return MP_TRUE;
        }
        MpBoolean appendNote(IMargretePluginNote *n) override
        {
            notes.push_back(static_cast<FakeNote *>(n));
            return MP_TRUE;
        }
        MpBoolean deleteNote(IMargretePluginNote *n) override
        {
            for (auto i = notes.begin(); i != notes.end(); ++i)
                if (*i == n)
                {
                    notes.erase(i);
                    return MP_TRUE;
                }
            return MP_FALSE;
        }
        void offsetNotes(MpInteger t) override
        {
            for (auto *n : notes)
                n->info.tick += t;
        }
        MpBoolean createEvent(const MpGuid &id, void **out) const override
        {
            if (!out)
                return MP_FALSE;
            if (std::memcmp(&id, &IID_IMargretePluginEventTimelineSpeed, 16) == 0)
                *out = new FakeTls();
            else if (std::memcmp(&id, &IID_IMargretePluginEventNoteSpeedModifier, 16) == 0)
                *out = new FakeNsm();
            else if (std::memcmp(&id, &IID_IMargretePluginEventBpm, 16) == 0)
                *out = new FakeBpm();
            else if (std::memcmp(&id, &IID_IMargretePluginEventBeatChange, 16) == 0)
                *out = new FakeBc();
            else
                return MP_FALSE;
            return MP_TRUE;
        }
        MpBoolean appendEvent(IMargretePluginEvent *e) override
        {
            events.push_back(e);
            return MP_TRUE;
        }
        MpBoolean deleteEvent(IMargretePluginEvent *e) override
        {
            for (auto i = events.begin(); i != events.end(); ++i)
                if (*i == e)
                {
                    events.erase(i);
                    return MP_TRUE;
                }
            return MP_FALSE;
        }
        MpBoolean findEventTimelineSpeed(MpInteger t, MpInteger l, void **out) override
        {
            for (auto *e : events)
            {
                auto *p = dynamic_cast<FakeTls *>(e);
                if (p && p->info.tick == t && p->info.timelineId == l)
                {
                    p->addRef();
                    *out = p;
                    return MP_TRUE;
                }
            }
            return MP_FALSE;
        }
        MpBoolean findEventNoteSpeedModifier(MpInteger t, void **out) override
        {
            for (auto *e : events)
            {
                auto *p = dynamic_cast<FakeNsm *>(e);
                if (p && p->info.tick == t)
                {
                    p->addRef();
                    *out = p;
                    return MP_TRUE;
                }
            }
            return MP_FALSE;
        }
        MpBoolean findEventBpm(MpInteger t, void **out) override
        {
            for (auto *e : events)
            {
                auto *p = dynamic_cast<FakeBpm *>(e);
                if (p && p->info.tick == t)
                {
                    p->addRef();
                    *out = p;
                    return MP_TRUE;
                }
            }
            return MP_FALSE;
        }
        MpBoolean findEventBeatChange(MpInteger b, void **out) override
        {
            for (auto *e : events)
            {
                auto *p = dynamic_cast<FakeBc *>(e);
                if (p && p->info.bar == b)
                {
                    p->addRef();
                    *out = p;
                    return MP_TRUE;
                }
            }
            return MP_FALSE;
        }
    };

    struct FakeDocument final : IMargretePluginDocument
    {
        MpInteger refs{1};
        FakeChart chart;
        FakeUndo undo;
        MpInteger addRef() override
        {
            return ++refs;
        }
        MpInteger release() override
        {
            return --refs;
        }
        MpBoolean queryInterface(const MpGuid &i, void **o) override
        {
            return qi(this, i, o);
        }
        MpBoolean getChart(IMargretePluginChart **out) override
        {
            if (!out)
                return MP_FALSE;
            chart.addRef();
            *out = &chart;
            return MP_TRUE;
        }
        MpBoolean getUndoBuffer(IMargretePluginUndoBuffer **out) override
        {
            if (!out)
                return MP_FALSE;
            undo.addRef();
            *out = &undo;
            return MP_TRUE;
        }
    };
    struct FakeContext final : IMargretePluginContext
    {
        MpInteger refs{1};
        FakeDocument document;
        int updates{};
        MpInteger addRef() override
        {
            return ++refs;
        }
        MpInteger release() override
        {
            return --refs;
        }
        MpBoolean queryInterface(const MpGuid &i, void **o) override
        {
            return qi(this, i, o);
        }
        MpBoolean getDocument(IMargretePluginDocument **out) override
        {
            if (!out)
                return MP_FALSE;
            document.addRef();
            *out = &document;
            return MP_TRUE;
        }
        void *getMainWindowHandle() override
        {
            return nullptr;
        }
        MpInteger getCurrentTick() const override
        {
            return 960;
        }
        void update() const override
        {
            const_cast<FakeContext *>(this)->updates++;
        }
    };

    const MgHostApi *host_api{};
    int init_calls{};
    int destroy_calls{};
    bool invoke_ok{};

    MgResult copy_string(const char *value, char *buffer, uint32_t capacity, uint32_t *out)
    {
        if (!out)
            return MG_ERR_NULL_POINTER;
        const uint32_t need = static_cast<uint32_t>(std::strlen(value) + 1);
        *out = need;
        if (!buffer && capacity == 0)
            return MG_OK;
        if (!buffer)
            return MG_ERR_NULL_POINTER;
        if (capacity < need)
            return MG_ERR_BUFFER_TOO_SMALL;
        std::memcpy(buffer, value, need);
        return MG_OK;
    }
    MgResult MG_CALL plugin_info(void *, MgPluginInfoBuffers *b)
    {
        if (!b)
            return MG_ERR_NULL_POINTER;
        MgResult r = copy_string("marmkmt test", b->name, b->name_capacity, b->name_required);
        if (r != MG_OK)
            return r;
        r = copy_string("UTF-8: \xE6\x97\xA5\xE6\x9C\xAC\xE8\xAA\x9E", b->description, b->description_capacity,
                        b->description_required);
        if (r != MG_OK)
            return r;
        return copy_string("tester", b->developer, b->developer_capacity, b->developer_required);
    }
    MgResult MG_CALL create_command(void *, void **out)
    {
        if (!out)
            return MG_ERR_NULL_POINTER;
        *out = new int(1);
        return MG_OK;
    }
    void MG_CALL destroy_command(void *, void *c)
    {
        delete static_cast<int *>(c);
        destroy_calls++;
    }
    MgResult MG_CALL command_name(void *, void *, char *b, uint32_t c, uint32_t *r)
    {
        return copy_string("test command", b, c, r);
    }

    MgResult MG_CALL invoke(void *, void *, MgContext *ctx)
    {
        assert(host_api->context_get_document(ctx, nullptr) == MG_ERR_NULL_POINTER);
        MgDocument *doc{};
        assert(host_api->context_get_document(ctx, &doc) == MG_OK);
        MgChart *chart{};
        MgUndoBuffer *undo{};
        assert(host_api->document_get_chart(doc, &chart) == MG_OK);
        assert(host_api->document_get_undo_buffer(doc, &undo) == MG_OK);
        MgInt tick{};
        assert(host_api->context_get_current_tick(ctx, &tick) == MG_OK && tick == 960);
        assert(host_api->undo_begin_recording(undo) == MG_OK);
        assert(host_api->object_release(undo) == MG_ERR_INVALID_ARGUMENT);
        MgNote *note{};
        assert(host_api->chart_create_note(chart, &note) == MG_OK);
        MgNoteInfo ni{};
        ni.type = MG_NOTE_TYPE_TAP;
        ni.width = 4;
        ni.tick = tick;
        assert(host_api->note_set_info(note, &ni) == MG_OK);
        ni.type = 13;
        assert(host_api->note_set_info(note, &ni) == MG_ERR_INVALID_ARGUMENT);
        assert(host_api->chart_append_note(chart, note) == MG_OK);
        MgInt id{};
        assert(host_api->note_get_id(note, &id) == MG_OK && id == 42);
        MgResult thread_result = MG_OK;
        std::thread worker([&] {
            MgInt count{};
            thread_result = host_api->chart_get_notes_count(chart, &count);
        });
        worker.join();
        assert(thread_result == MG_ERR_WRONG_THREAD);
        assert(host_api->object_release(note) == MG_OK);
        assert(host_api->note_get_id(note, &id) == MG_ERR_INVALID_HANDLE);
        MgEvent *event{};
        assert(host_api->chart_create_event(chart, MG_EVENT_KIND_BPM, &event) == MG_OK);
        MgEventBpmInfo bpm{tick, 180.0};
        assert(host_api->event_bpm_set_info(event, &bpm) == MG_OK);
        bpm.bpm = NAN;
        assert(host_api->event_bpm_set_info(event, &bpm) == MG_ERR_INVALID_ARGUMENT);
        assert(host_api->chart_append_event(chart, event) == MG_OK);
        MgEventKind kind{};
        assert(host_api->event_get_kind(event, &kind) == MG_OK && kind == MG_EVENT_KIND_BPM);
        assert(host_api->object_release(event) == MG_OK);
        assert(host_api->undo_commit_recording(undo) == MG_OK);
        assert(host_api->context_update(ctx) == MG_OK);
        assert(host_api->object_release(undo) == MG_OK);
        assert(host_api->object_release(chart) == MG_OK);
        assert(host_api->object_release(doc) == MG_OK);
        invoke_ok = true;
        return MG_OK;
    }

} // namespace

extern "C" MgResult MG_CALL mg_plugin_init(uint32_t abi, const MgHostApi *host, MgPluginApi *out)
{
    init_calls++;
    if (abi != MG_ABI_VERSION || !host || !out)
        return MG_ERR_UNSUPPORTED_ABI;
    host_api = host;
    assert(host->struct_size == sizeof(MgHostApi));
    assert(host->abi_version == MG_ABI_VERSION);
    out->abi_version = MG_ABI_VERSION;
    out->plugin_data = nullptr;
    out->get_plugin_info = plugin_info;
    out->create_command = create_command;
    out->destroy_command = destroy_command;
    out->get_command_name = command_name;
    out->invoke = invoke;
    return MG_OK;
}

int main()
{
    static_assert(sizeof(MgHostApi) == 456);
    wchar_t name[64]{}, desc[64]{}, developer[64]{};
    MP_PLUGININFO info{0, name, 64, desc, 64, developer, 64};
    MargretePluginGetInfo(&info);
    assert(info.sdkVersion == 2);
    assert(std::wstring(name) == L"marmkmt test");
    assert(std::wstring(desc) == L"UTF-8: 日本語");
    assert(std::wstring(developer) == L"tester");
    assert(init_calls == 1);
    IMargretePluginCommand *command{};
    assert(MargretePluginCommandCreate(&command) == MP_TRUE && command);
    wchar_t command_name_buffer[64]{};
    assert(command->getCommandName(command_name_buffer, 64) == MP_TRUE);
    assert(std::wstring(command_name_buffer) == L"test command");
    FakeContext context;
    assert(command->invoke(&context) == MP_TRUE);
    assert(invoke_ok && context.updates == 1);
    assert(context.document.chart.notes.size() == 1);
    assert(context.document.chart.events.size() == 1);
    assert(!context.document.undo.recording);
    assert(command->release() == 0);
    assert(destroy_calls == 1);
    return 0;
}

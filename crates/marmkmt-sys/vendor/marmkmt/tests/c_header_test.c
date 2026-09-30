#include "marmkmt.h"
#include <stddef.h>

#define ABI_ASSERT(name, expression) typedef char name[(expression) ? 1 : -1]
ABI_ASSERT(note_size, sizeof(MgNoteInfo) == 44);
ABI_ASSERT(note_alignment, __alignof(MgNoteInfo) == 4);
ABI_ASSERT(timeline_size, sizeof(MgEventTimelineSpeedInfo) == 16);
ABI_ASSERT(timeline_speed_offset, offsetof(MgEventTimelineSpeedInfo, speed) == 8);
ABI_ASSERT(note_speed_size, sizeof(MgEventNoteSpeedModifierInfo) == 16);
ABI_ASSERT(note_speed_offset, offsetof(MgEventNoteSpeedModifierInfo, speed) == 8);
ABI_ASSERT(bpm_size, sizeof(MgEventBpmInfo) == 16);
ABI_ASSERT(bpm_offset, offsetof(MgEventBpmInfo, bpm) == 8);
ABI_ASSERT(beat_change_size, sizeof(MgEventBeatChangeInfo) == 12);
ABI_ASSERT(plugin_info_buffers_size, sizeof(MgPluginInfoBuffers) == 80);
ABI_ASSERT(plugin_api_size, sizeof(MgPluginApi) == 56);
ABI_ASSERT(host_api_size, sizeof(MgHostApi) == 456);
ABI_ASSERT(abi_version_value, MG_ABI_VERSION == 1u);
ABI_ASSERT(sdk_version_value, MG_MARGRETE_SDK_VERSION == 2u);
ABI_ASSERT(plugin_info_buffers_alignment, __alignof(MgPluginInfoBuffers) == 8);
ABI_ASSERT(plugin_api_alignment, __alignof(MgPluginApi) == 8);
ABI_ASSERT(host_api_alignment, __alignof(MgHostApi) == 8);
ABI_ASSERT(timeline_alignment, __alignof(MgEventTimelineSpeedInfo) == 8);
ABI_ASSERT(note_speed_alignment, __alignof(MgEventNoteSpeedModifierInfo) == 8);
ABI_ASSERT(bpm_alignment, __alignof(MgEventBpmInfo) == 8);
ABI_ASSERT(beat_change_alignment, __alignof(MgEventBeatChangeInfo) == 4);

#define OFF(type, field) offsetof(type, field)
static const size_t note_offsets[] = {
    OFF(MgNoteInfo, type),         OFF(MgNoteInfo, long_attr),   OFF(MgNoteInfo, direction),   OFF(MgNoteInfo, ex_attr),
    OFF(MgNoteInfo, variation_id), OFF(MgNoteInfo, x),           OFF(MgNoteInfo, width),       OFF(MgNoteInfo, height),
    OFF(MgNoteInfo, tick),         OFF(MgNoteInfo, timeline_id), OFF(MgNoteInfo, option_value)};
static const size_t host_offsets[] = {OFF(MgHostApi, object_add_ref),
                                      OFF(MgHostApi, object_release),
                                      OFF(MgHostApi, report_error),
                                      OFF(MgHostApi, context_get_document),
                                      OFF(MgHostApi, context_get_main_window_handle),
                                      OFF(MgHostApi, context_get_current_tick),
                                      OFF(MgHostApi, context_update),
                                      OFF(MgHostApi, document_get_chart),
                                      OFF(MgHostApi, document_get_undo_buffer),
                                      OFF(MgHostApi, undo_begin_recording),
                                      OFF(MgHostApi, undo_commit_recording),
                                      OFF(MgHostApi, undo_discard_recording),
                                      OFF(MgHostApi, undo_undo),
                                      OFF(MgHostApi, undo_redo),
                                      OFF(MgHostApi, undo_can_undo),
                                      OFF(MgHostApi, undo_can_redo),
                                      OFF(MgHostApi, undo_is_recording),
                                      OFF(MgHostApi, chart_create_note),
                                      OFF(MgHostApi, chart_get_notes_count),
                                      OFF(MgHostApi, chart_get_note),
                                      OFF(MgHostApi, chart_append_note),
                                      OFF(MgHostApi, chart_delete_note),
                                      OFF(MgHostApi, chart_offset_notes),
                                      OFF(MgHostApi, note_get_id),
                                      OFF(MgHostApi, note_get_info),
                                      OFF(MgHostApi, note_set_info),
                                      OFF(MgHostApi, note_get_children_count),
                                      OFF(MgHostApi, note_get_child),
                                      OFF(MgHostApi, note_get_parent),
                                      OFF(MgHostApi, note_append_child),
                                      OFF(MgHostApi, note_delete_child),
                                      OFF(MgHostApi, note_clone),
                                      OFF(MgHostApi, note_replace_with),
                                      OFF(MgHostApi, note_copy_info_to),
                                      OFF(MgHostApi, note_get_base_note),
                                      OFF(MgHostApi, note_offset_child),
                                      OFF(MgHostApi, note_flip_h),
                                      OFF(MgHostApi, chart_create_event),
                                      OFF(MgHostApi, chart_append_event),
                                      OFF(MgHostApi, chart_delete_event),
                                      OFF(MgHostApi, chart_find_event_timeline_speed),
                                      OFF(MgHostApi, chart_find_event_note_speed_modifier),
                                      OFF(MgHostApi, chart_find_event_bpm),
                                      OFF(MgHostApi, chart_find_event_beat_change),
                                      OFF(MgHostApi, event_get_kind),
                                      OFF(MgHostApi, event_get_id),
                                      OFF(MgHostApi, event_timeline_speed_get_info),
                                      OFF(MgHostApi, event_timeline_speed_set_info),
                                      OFF(MgHostApi, event_note_speed_modifier_get_info),
                                      OFF(MgHostApi, event_note_speed_modifier_set_info),
                                      OFF(MgHostApi, event_bpm_get_info),
                                      OFF(MgHostApi, event_bpm_set_info),
                                      OFF(MgHostApi, event_beat_change_get_info),
                                      OFF(MgHostApi, event_beat_change_set_info),
                                      OFF(MgHostApi, event_replace_with),
                                      OFF(MgHostApi, event_copy_info_to)};
static const size_t plugin_info_offsets[] = {
    OFF(MgPluginInfoBuffers, struct_size),          OFF(MgPluginInfoBuffers, name),
    OFF(MgPluginInfoBuffers, name_capacity),        OFF(MgPluginInfoBuffers, name_required),
    OFF(MgPluginInfoBuffers, description),          OFF(MgPluginInfoBuffers, description_capacity),
    OFF(MgPluginInfoBuffers, description_required), OFF(MgPluginInfoBuffers, developer),
    OFF(MgPluginInfoBuffers, developer_capacity),   OFF(MgPluginInfoBuffers, developer_required)};
static const size_t plugin_info_expected[] = {0, 8, 16, 24, 32, 40, 48, 56, 64, 72};
static const size_t plugin_api_offsets[] = {OFF(MgPluginApi, struct_size),      OFF(MgPluginApi, abi_version),
                                            OFF(MgPluginApi, plugin_data),      OFF(MgPluginApi, get_plugin_info),
                                            OFF(MgPluginApi, create_command),   OFF(MgPluginApi, destroy_command),
                                            OFF(MgPluginApi, get_command_name), OFF(MgPluginApi, invoke)};
static const size_t plugin_api_expected[] = {0, 4, 8, 16, 24, 32, 40, 48};

int main(void)
{
    size_t i;
    for (i = 0; i < sizeof(note_offsets) / sizeof(note_offsets[0]); ++i)
        if (note_offsets[i] != i * 4)
            return 2;
    for (i = 0; i < sizeof(host_offsets) / sizeof(host_offsets[0]); ++i)
        if (host_offsets[i] != 8 + i * sizeof(void *))
            return 3;
    for (i = 0; i < sizeof(plugin_info_offsets) / sizeof(plugin_info_offsets[0]); ++i)
        if (plugin_info_offsets[i] != plugin_info_expected[i])
            return 4;
    for (i = 0; i < sizeof(plugin_api_offsets) / sizeof(plugin_api_offsets[0]); ++i)
        if (plugin_api_offsets[i] != plugin_api_expected[i])
            return 5;
    return 0;
}

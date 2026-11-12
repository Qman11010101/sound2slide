mod audio;
mod audit;
mod generate;
mod i18n;
mod mgxc;
mod modal;
mod oss;
mod playback;
mod rainbone;
mod timing;
mod ui;
mod update;

use std::{
    error::Error as StdError,
    fmt,
    panic::{AssertUnwindSafe, catch_unwind},
};

use marmkmt::{
    Command, Context, Error, LongAttribute, NoteInfo, NoteType, Plugin, PluginInfo, Result,
};

use crate::{generate::SlidePoint, i18n::Msg, rainbone::Output, ui::Commit};

struct Sound2SlidePlugin;
struct Sound2SlideCommand;

impl Plugin for Sound2SlidePlugin {
    type Command = Sound2SlideCommand;

    fn info() -> PluginInfo {
        PluginInfo {
            name: "sound2slide".into(),
            description: "音声波形からスライドを生成します".into(),
            developer: "Kjuman Enobikto".into(),
        }
    }

    fn create_command() -> Result<Self::Command> {
        Ok(Sound2SlideCommand)
    }
}

impl Command for Sound2SlideCommand {
    fn name(&self) -> &str {
        "sound2slide"
    }

    fn invoke(&mut self, context: &mut Context<'_>) -> Result<()> {
        let host = modal::ModalHost::block(context.main_window_handle()?).map_err(plugin_err)?;
        let start_tick = context.current_tick()?;
        let chart = context
            .document()
            .and_then(|document| document.chart())
            .ok();
        let lookup = |tick: i32| chart_bpm(chart.as_ref(), tick);
        let lookup_signature = |bar: i32| chart_time_signature(chart.as_ref(), bar);
        let opened = catch_unwind(AssertUnwindSafe(|| {
            ui::open(
                &host,
                start_tick,
                &lookup,
                &lookup_signature,
                chart
                    .as_ref()
                    .and_then(|chart| chart_uses_extended_field(chart).ok())
                    .unwrap_or(false),
            )
        }));
        let commit = match opened {
            Ok(Ok(commit)) => commit,
            Ok(Err(message)) => return Err(plugin_err(message)),
            Err(_) => return Err(plugin_err("画面の表示中に失敗しました")),
        };
        let Some(commit) = commit else {
            return Ok(());
        };
        write_slides(context, &commit)
    }
}

fn chart_uses_extended_field(chart: &marmkmt::Chart<'_>) -> Result<bool> {
    let mut pending = Vec::new();
    for index in 0..chart.notes_count()? {
        pending.push(chart.note(index)?);
    }
    while let Some(note) = pending.pop() {
        let info = note.info()?;
        if info.note_type != NoteType::CLICK
            && info.width > 0
            && (info.x < 0 || i64::from(info.x) + i64::from(info.width) > 16)
        {
            return Ok(true);
        }
        for index in 0..note.children_count()? {
            pending.push(note.child(index)?);
        }
    }
    Ok(false)
}

fn chart_bpm(
    chart: Option<&marmkmt::Chart<'_>>,
    tick: i32,
) -> std::result::Result<Option<f64>, Msg> {
    let Some(chart) = chart else {
        return Err(Msg::NoChart);
    };
    match chart.find_bpm_event(tick) {
        Ok(Some(event)) => {
            let bpm = event.info().map_err(|error| error.to_string())?.bpm;
            if bpm.is_finite() && bpm > 0.0 {
                Ok(Some(bpm))
            } else {
                Err(Msg::InvalidBpm)
            }
        }
        Ok(None) => Ok(None),
        Err(error) => Err(error.to_string().into()),
    }
}

fn chart_time_signature(
    chart: Option<&marmkmt::Chart<'_>>,
    bar: i32,
) -> std::result::Result<Option<[i32; 2]>, Msg> {
    let Some(chart) = chart else {
        return Err(Msg::NoChart);
    };
    let Some(event) = chart
        .find_beat_change_event(bar)
        .map_err(|error| error.to_string())?
    else {
        return Ok(None);
    };
    let info = event.info().map_err(|error| error.to_string())?;
    if !(1..=16).contains(&info.beats_per_bar) || !(1..=480).contains(&info.beat_unit) {
        return Err(Msg::SignatureOutOfRange);
    }
    Ok(Some([info.beats_per_bar, info.beat_unit]))
}

fn write_slides(context: &mut Context<'_>, commit: &Commit) -> Result<()> {
    if commit.slides.is_empty() {
        return Err(plugin_err("追加できるスライドがありません"));
    }
    validate_slide_bounds(&commit.slides)?;
    context.transaction(|edit| {
        let chart = edit.chart();
        for slide in &commit.slides {
            if slide.points.len() < 2 {
                continue;
            }
            write_one(chart, slide, commit.transparent_steps, commit.output)?;
        }
        Ok(())
    })
}

fn validate_slide_bounds(slides: &[generate::Slide]) -> Result<()> {
    for point in slides.iter().flat_map(|slide| &slide.points) {
        if !point.is_within_extended_field() {
            return Err(plugin_err(format!(
                "tick {}のノーツが48レーンの範囲内に収まらないため、追加を中止しました",
                point.tick
            )));
        }
    }
    Ok(())
}

fn write_one<'a>(
    chart: &marmkmt::Chart<'a>,
    slide: &generate::Slide,
    transparent_steps: bool,
    output: Output,
) -> Result<()> {
    let head = chart.create_note()?;
    let mut children = Vec::new();
    let last = slide.points.len() - 1;
    for (index, point) in slide.points.iter().enumerate() {
        let attribute = if index == 0 {
            LongAttribute::BEGIN
        } else if index == last {
            LongAttribute::END
        } else if point.control
            || (transparent_steps && matches!(output, Output::Slide | Output::SlideNoCenterLine))
        {
            LongAttribute::CONTROL
        } else {
            LongAttribute::STEP
        };
        let info = note_info(point, attribute, output);
        if index == 0 {
            head.set_info(&info)?;
        } else {
            let child = chart.create_note()?;
            child.set_info(&info)?;
            head.append_child(&child)?;
            children.push(child);
        }
    }
    chart.append_note(&head)?;
    drop(children);
    Ok(())
}

fn note_info(point: &SlidePoint, attribute: LongAttribute, output: Output) -> NoteInfo {
    let base = NoteInfo {
        note_type: NoteType::SLIDE,
        long_attribute: attribute,
        x: point.x,
        width: point.width,
        tick: point.tick,
        ..NoteInfo::default()
    };
    match output {
        Output::Slide => base,
        Output::SlideNoCenterLine => NoteInfo {
            variation_id: if attribute == LongAttribute::BEGIN {
                35
            } else {
                0
            },
            ..base
        },
        Output::Rainbone {
            axis,
            color,
            height,
        } => {
            let head = attribute == LongAttribute::BEGIN;
            // Margrete keeps the line color and the no-notes mode on the head note only.
            NoteInfo {
                note_type: NoteType::AIR_CRUSH,
                variation_id: if head { color } else { 0 },
                extra_attribute: if head {
                    axis.extra_attribute()
                } else {
                    Default::default()
                },
                height,
                option_value: rainbone::LINE_ONLY,
                ..base
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chart_write_rejects_out_of_field_points_and_invalid_widths_before_editing() {
        let point = |x, width| SlidePoint {
            tick: 123,
            x,
            width,
            control: false,
        };
        let valid = generate::Slide {
            points: vec![point(-16, 1), point(16, 16), point(31, 1)],
        };
        assert!(validate_slide_bounds(std::slice::from_ref(&valid)).is_ok());
        for (x, width) in [
            (-17, 1),
            (32, 1),
            (17, 16),
            (0, 0),
            (0, -1),
            (0, 17),
            (i32::MAX, i32::MAX),
        ] {
            let invalid = generate::Slide {
                points: vec![point(0, 4), point(x, width)],
            };
            let error = validate_slide_bounds(&[valid.clone(), invalid]).unwrap_err();
            assert!(error.to_string().contains("123"));
        }
    }

    #[test]
    fn rainbone_axis_matches_saved_head_attributes_and_preserves_child_attributes() {
        use rainbone::Axis;
        let point = SlidePoint {
            tick: 5760,
            x: -16,
            width: 1,
            control: false,
        };
        for (axis, expected) in [(Axis::X, 0), (Axis::Y, 4), (Axis::Z, 5)] {
            let output = Output::Rainbone {
                axis,
                color: 3,
                height: 80,
            };
            let head = note_info(&point, LongAttribute::BEGIN, output);
            assert_eq!(head.extra_attribute.0, expected);
            assert_eq!(
                (head.x, head.width, head.tick, head.height),
                (-16, 1, 5760, 80)
            );
            assert_eq!((head.variation_id, head.option_value), (3, 0));
            for attribute in [
                LongAttribute::STEP,
                LongAttribute::CONTROL,
                LongAttribute::END,
            ] {
                let child = note_info(&point, attribute, output);
                assert_eq!(
                    (
                        child.extra_attribute.0,
                        child.variation_id,
                        child.option_value
                    ),
                    (0, 0, 0)
                );
            }
        }
        let slide = note_info(&point, LongAttribute::BEGIN, Output::Slide);
        assert_eq!(slide.extra_attribute.0, 0);
    }

    #[test]
    fn no_center_line_matches_nolineslide_saved_by_margrete() {
        // nolineslide.mgxc: the right-hand slide has variation 35 on its head
        // (tick 0, x 8, width 4), and variation 0 on its end (tick 1920, x 12).
        let point = |tick, x| SlidePoint {
            tick,
            x,
            width: 4,
            control: false,
        };
        let head = note_info(
            &point(0, 8),
            LongAttribute::BEGIN,
            Output::SlideNoCenterLine,
        );
        assert_eq!(
            (
                head.note_type,
                head.variation_id,
                head.x,
                head.width,
                head.tick
            ),
            (NoteType::SLIDE, 35, 8, 4, 0)
        );
        for attribute in [
            LongAttribute::STEP,
            LongAttribute::CONTROL,
            LongAttribute::END,
        ] {
            let child = note_info(&point(1920, 12), attribute, Output::SlideNoCenterLine);
            let normal = note_info(&point(1920, 12), attribute, Output::Slide);
            assert_eq!(child.variation_id, 0);
            assert_eq!(child.note_type, normal.note_type);
            assert_eq!(child.long_attribute, normal.long_attribute);
            assert_eq!((child.x, child.width, child.tick), (12, 4, 1920));
            assert_eq!(
                (child.extra_attribute.0, child.height, child.option_value),
                (0, 0, 0)
            );
        }
        assert_eq!(
            note_info(&point(0, 8), LongAttribute::BEGIN, Output::Slide).variation_id,
            0
        );
    }

    #[test]
    fn rainbone_matches_a_line_saved_by_margrete() {
        let output = Output::Rainbone {
            axis: rainbone::Axis::X,
            color: 3,
            height: 80,
        };
        let point = |tick, x| SlidePoint {
            tick,
            x,
            width: 4,
            control: false,
        };
        let head = note_info(&point(0, 12), LongAttribute::BEGIN, output);
        assert_eq!(
            (head.note_type, head.variation_id, head.x, head.width),
            (NoteType::AIR_CRUSH, 3, 12, 4)
        );
        assert_eq!((head.height, head.tick, head.option_value), (80, 0, 0));
        let end = note_info(&point(3360, 4), LongAttribute::END, output);
        assert_eq!(
            (end.note_type, end.variation_id, end.x, end.height, end.tick),
            (NoteType::AIR_CRUSH, 0, 4, 80, 3360)
        );
        let slide = note_info(&point(0, 12), LongAttribute::BEGIN, Output::Slide);
        assert_eq!((slide.note_type, slide.height), (NoteType::SLIDE, 0));
    }
}

#[derive(Debug)]
struct PluginError(String);

impl fmt::Display for PluginError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl StdError for PluginError {}

fn plugin_err(message: impl Into<String>) -> Error {
    Error::plugin(PluginError(message.into()))
}

marmkmt::export_plugin!(Sound2SlidePlugin);

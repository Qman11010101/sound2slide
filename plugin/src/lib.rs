mod audio;
mod generate;
mod i18n;
mod mgxc;
mod modal;
mod oss;
mod playback;
mod timing;
mod ui;

use std::{
    error::Error as StdError,
    fmt,
    panic::{AssertUnwindSafe, catch_unwind},
};

use marmkmt::{
    Command, Context, Error, LongAttribute, NoteInfo, NoteType, Plugin, PluginInfo, Result,
};

use crate::{generate::SlidePoint, i18n::Msg, ui::Commit};

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
            ui::open(&host, start_tick, &lookup, &lookup_signature)
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
    context.transaction(|edit| {
        let chart = edit.chart();
        for slide in &commit.slides {
            if slide.points.len() < 2 {
                continue;
            }
            write_one(chart, slide, commit.transparent_steps)?;
        }
        Ok(())
    })
}

fn write_one<'a>(
    chart: &marmkmt::Chart<'a>,
    slide: &generate::Slide,
    transparent_steps: bool,
) -> Result<()> {
    let head = chart.create_note()?;
    let mut children = Vec::new();
    let last = slide.points.len() - 1;
    for (index, point) in slide.points.iter().enumerate() {
        let attribute = if index == 0 {
            LongAttribute::BEGIN
        } else if index == last {
            LongAttribute::END
        } else if point.control || transparent_steps {
            LongAttribute::CONTROL
        } else {
            LongAttribute::STEP
        };
        let info = note_info(point, attribute);
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

fn note_info(point: &SlidePoint, attribute: LongAttribute) -> NoteInfo {
    NoteInfo {
        note_type: NoteType::SLIDE,
        long_attribute: attribute,
        x: point.x,
        width: point.width,
        tick: point.tick,
        ..NoteInfo::default()
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

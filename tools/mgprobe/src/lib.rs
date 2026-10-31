//! Margrete 検証用プラグイン。
//! %MGPROBE_DIR%\case.txt を読んでノーツを配置し、譜面の全ノーツを dump.txt に書き出す。
//!
//! case.txt の書式（# 以降はコメント）:
//!   MODE load|dump          load: 既存ノーツを全削除してから配置 / dump: 書き出しのみ
//!   BPM <value>             tick 0 の BPM を設定（load 時）
//!   BEAT <num> <den>        小節 0 の拍子を設定（load 時。省略すると 4/4 に戻す）
//!   EV BPM|TIL|NSM <tick> <value>   イベントを追加（TIL は timeline 0）。
//!                           前回 load で追加したイベントは events.txt に記録しておき、次の load で削除する。
//!   <depth> <TYPE> <ATTR> <x> <width> <tick> [height] [option] [variation] [direction] [ex]
//! depth 0 がトップレベル、depth n は直前の depth n-1 のノーツの子になる。
use std::{fmt::Write as _, fs, path::PathBuf};

use marmkmt::{
    BeatChangeInfo, BpmInfo, Chart, Command, Context, Direction, Error, Event, ExtraAttribute,
    LongAttribute, Note, NoteInfo, NoteSpeedModifierInfo, NoteType, Plugin, PluginInfo, Result,
    TimelineSpeedInfo,
};

struct Probe;
struct ProbeCommand;

impl Plugin for Probe {
    type Command = ProbeCommand;
    fn info() -> PluginInfo {
        PluginInfo {
            name: "mgprobe".into(),
            description: "検証用".into(),
            developer: "probe".into(),
        }
    }
    fn create_command() -> Result<Self::Command> {
        Ok(ProbeCommand)
    }
}

fn dir() -> PathBuf {
    PathBuf::from(std::env::var("MGPROBE_DIR").unwrap_or_else(|_| r"C:\mgprobe".into()))
}

fn msg(text: String) -> Error {
    Error::plugin(std::io::Error::other(text))
}

const TYPES: [&str; 12] = [
    "UNKNOWN",
    "TAP",
    "EXTAP",
    "FLICK",
    "DAMAGE",
    "HOLD",
    "SLIDE",
    "AIR",
    "AIR_HOLD",
    "AIR_SLIDE",
    "AIR_CRUSH",
    "CLICK",
];
const ATTRS: [&str; 7] = [
    "NONE",
    "BEGIN",
    "STEP",
    "CONTROL",
    "CURVE",
    "END",
    "END_NO_ACT",
];

fn lookup(table: &[&str], name: &str) -> Result<i32> {
    table
        .iter()
        .position(|v| v.eq_ignore_ascii_case(name))
        .map(|i| i as i32)
        .ok_or_else(|| msg(format!("unknown name {name}")))
}

struct Spec {
    depth: usize,
    info: NoteInfo,
}

struct Ev {
    kind: String,
    tick: i32,
    value: f64,
}

struct Parsed {
    load: bool,
    bpm: Option<f64>,
    beat: (i32, i32),
    events: Vec<Ev>,
    specs: Vec<Spec>,
}

fn parse(text: &str) -> Result<Parsed> {
    let mut load = true;
    let mut bpm = None;
    let mut events = Vec::new();
    let mut beat = (4, 4);
    let mut specs = Vec::new();
    for raw in text.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split_whitespace().collect();
        if f[0].eq_ignore_ascii_case("MODE") {
            load = f.get(1).is_some_and(|m| m.eq_ignore_ascii_case("load"));
            continue;
        }
        if f[0].eq_ignore_ascii_case("EV") {
            let bad = || msg(format!("bad event: {line}"));
            events.push(Ev {
                kind: f.get(1).ok_or_else(bad)?.to_ascii_uppercase(),
                tick: f.get(2).and_then(|v| v.parse().ok()).ok_or_else(bad)?,
                value: f.get(3).and_then(|v| v.parse().ok()).ok_or_else(bad)?,
            });
            continue;
        }
        if f[0].eq_ignore_ascii_case("BEAT") {
            let n = |i: usize| {
                f.get(i)
                    .and_then(|v| v.parse().ok())
                    .ok_or_else(|| msg(format!("bad beat: {line}")))
            };
            beat = (n(1)?, n(2)?);
            continue;
        }
        if f[0].eq_ignore_ascii_case("BPM") {
            bpm = Some(
                f.get(1)
                    .and_then(|v| v.parse().ok())
                    .ok_or_else(|| msg(format!("bad bpm: {line}")))?,
            );
            continue;
        }
        if f.len() < 6 {
            return Err(msg(format!("too few fields: {line}")));
        }
        let num = |i: usize| -> Result<i32> {
            f.get(i).map_or(Ok(0), |v| {
                v.parse().map_err(|_| msg(format!("bad number: {line}")))
            })
        };
        specs.push(Spec {
            depth: f[0]
                .parse()
                .map_err(|_| msg(format!("bad depth: {line}")))?,
            info: NoteInfo {
                note_type: NoteType(lookup(&TYPES, f[1])?),
                long_attribute: LongAttribute(lookup(&ATTRS, f[2])?),
                x: num(3)?,
                width: num(4)?,
                tick: num(5)?,
                height: num(6)?,
                option_value: num(7)?,
                variation_id: num(8)?,
                direction: Direction(num(9)?),
                extra_attribute: ExtraAttribute(num(10)?),
                ..NoteInfo::default()
            },
        });
    }
    Ok(Parsed {
        load,
        bpm,
        beat,
        events,
        specs,
    })
}

fn place<'a>(chart: &Chart<'a>, specs: &[Spec]) -> Result<()> {
    let notes = specs
        .iter()
        .map(|s| {
            let note = chart.create_note()?;
            note.set_info(&s.info)?;
            Ok(note)
        })
        .collect::<Result<Vec<Note<'a>>>>()?;
    // 親を決める: depth n のノーツは直前の depth n-1 のノーツの子。
    let mut stack: Vec<usize> = Vec::new();
    let mut parent = vec![None; specs.len()];
    for (i, s) in specs.iter().enumerate() {
        stack.truncate(s.depth);
        if stack.len() != s.depth {
            return Err(msg(format!("depth jump at line {}", i + 1)));
        }
        parent[i] = stack.last().copied();
        stack.push(i);
    }
    for i in (0..specs.len()).rev() {
        if let Some(p) = parent[i] {
            notes[p].append_child(&notes[i])?;
        }
    }
    for i in 0..specs.len() {
        if parent[i].is_none() {
            chart.append_note(&notes[i])?;
        }
    }
    drop(notes);
    Ok(())
}

fn dump_note(out: &mut String, note: &Note<'_>, depth: usize) -> Result<()> {
    let i = note.info()?;
    let name = |t: &[&str], v: i32| t.get(v as usize).map_or(v.to_string(), |s| s.to_string());
    let _ = writeln!(
        out,
        "{depth} {} {} {} {} {} h={} opt={} var={} dir={} ex={} id={}",
        name(&TYPES, i.note_type.0),
        name(&ATTRS, i.long_attribute.0),
        i.x,
        i.width,
        i.tick,
        i.height,
        i.option_value,
        i.variation_id,
        i.direction.0,
        i.extra_attribute.0,
        note.id()?
    );
    for c in 0..note.children_count()? {
        dump_note(out, &note.child(c)?, depth + 1)?;
    }
    Ok(())
}

impl Command for ProbeCommand {
    fn name(&self) -> &str {
        "mgprobe"
    }
    fn invoke(&mut self, context: &mut Context<'_>) -> Result<()> {
        let dir = dir();
        let text = fs::read_to_string(dir.join("case.txt")).map_err(Error::plugin)?;
        let Parsed {
            load,
            bpm,
            beat,
            events,
            specs,
        } = parse(&text)?;
        let created = fs::read_to_string(dir.join("events.txt")).unwrap_or_default();
        if load {
            context.transaction(|edit| {
                let chart = edit.chart();
                while chart.notes_count()? > 0 {
                    chart.delete_note(chart.note(chart.notes_count()? - 1)?)?;
                }
                let signature = chart.find_beat_change_event(0)?.ok_or(Error::NotFound)?;
                signature.set_info(&BeatChangeInfo {
                    bar: 0,
                    beats_per_bar: beat.0,
                    beat_unit: beat.1,
                })?;
                if let Some(bpm) = bpm {
                    let event = chart.find_bpm_event(0)?.ok_or(Error::NotFound)?;
                    event.set_info(&BpmInfo { tick: 0, bpm })?;
                }
                // 前回追加したイベントを後ろから削除
                for line in created.lines().rev() {
                    let mut f = line.split_whitespace();
                    let (Some(kind), Some(Ok(tick))) = (f.next(), f.next().map(str::parse::<i32>))
                    else {
                        continue;
                    };
                    let event: Option<Event<'_>> = match kind {
                        "BPM" => chart.find_bpm_event(tick)?.map(Into::into),
                        "TIL" => chart.find_timeline_speed_event(tick, 0)?.map(Into::into),
                        "NSM" => chart.find_note_speed_modifier_event(tick)?.map(Into::into),
                        _ => None,
                    };
                    if let Some(event) = event {
                        chart.delete_event(event)?;
                    }
                }
                let mut record = String::new();
                for ev in &events {
                    let event: Event<'_> = match ev.kind.as_str() {
                        "BPM" => {
                            let e = chart.create_bpm_event()?;
                            e.set_info(&BpmInfo {
                                tick: ev.tick,
                                bpm: ev.value,
                            })?;
                            e.into()
                        }
                        "TIL" => {
                            let e = chart.create_timeline_speed_event()?;
                            e.set_info(&TimelineSpeedInfo {
                                timeline_id: 0,
                                tick: ev.tick,
                                speed: ev.value,
                            })?;
                            e.into()
                        }
                        "NSM" => {
                            let e = chart.create_note_speed_modifier_event()?;
                            e.set_info(&NoteSpeedModifierInfo {
                                tick: ev.tick,
                                speed: ev.value,
                            })?;
                            e.into()
                        }
                        other => return Err(msg(format!("unknown event kind {other}"))),
                    };
                    chart.append_event(&event)?;
                    let _ = writeln!(record, "{} {}", ev.kind, ev.tick);
                }
                fs::write(dir.join("events.txt"), record).map_err(Error::plugin)?;
                place(chart, &specs)
            })?;
        }
        let document = context.document()?;
        let chart = document.chart()?;
        let bpm0 = chart.find_bpm_event(0)?.map(|e| e.info()).transpose()?;
        let beat0 = chart
            .find_beat_change_event(0)?
            .map(|e| e.info())
            .transpose()?;
        let mut out = format!(
            "current_tick={} bpm0={:?} beat0={:?}\n",
            context.current_tick()?,
            bpm0.map(|i| i.bpm),
            beat0.map(|i| (i.beats_per_bar, i.beat_unit))
        );
        for ev in &events {
            let found = match ev.kind.as_str() {
                "BPM" => chart
                    .find_bpm_event(ev.tick)?
                    .map(|e| e.info())
                    .transpose()?
                    .map(|i| (i.tick, i.bpm)),
                "TIL" => chart
                    .find_timeline_speed_event(ev.tick, 0)?
                    .map(|e| e.info())
                    .transpose()?
                    .map(|i| (i.tick, i.speed)),
                "NSM" => chart
                    .find_note_speed_modifier_event(ev.tick)?
                    .map(|e| e.info())
                    .transpose()?
                    .map(|i| (i.tick, i.speed)),
                _ => None,
            };
            let _ = writeln!(out, "event {} {} -> {:?}", ev.kind, ev.tick, found);
        }
        for n in 0..chart.notes_count()? {
            dump_note(&mut out, &chart.note(n)?, 0)?;
        }
        fs::write(dir.join("dump.txt"), out).map_err(Error::plugin)?;
        Ok(())
    }
}

marmkmt::export_plugin!(Probe);

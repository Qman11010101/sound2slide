use std::{num::NonZeroU32, sync::Arc, time::Duration};

use rodio::{DeviceSinkBuilder, MixerDeviceSink, Player, Source};

use crate::{audio::Audio, i18n::Msg};

pub(crate) fn preview_range(audio: &Audio, offset_sec: f64, length_sec: f64) -> Option<(f64, f64)> {
    if !offset_sec.is_finite() || !length_sec.is_finite() || audio.sample_rate == 0 {
        return None;
    }
    let end = if length_sec > 0.0 {
        (offset_sec + length_sec).min(audio.duration_secs())
    } else {
        audio.duration_secs()
    };
    (end > offset_sec).then_some((offset_sec, end))
}

pub(crate) struct Playback {
    player: Player,
    _device: MixerDeviceSink,
    pub offset_sec: f64,
    pub length_sec: f64,
    duration_sec: f64,
    start_sec: f64,
}

impl Playback {
    pub fn start(audio: &Audio, offset_sec: f64, length_sec: f64) -> Result<Self, Msg> {
        Self::start_at(audio, offset_sec, length_sec, offset_sec, false)
    }

    pub fn start_at(
        audio: &Audio,
        offset_sec: f64,
        length_sec: f64,
        position_sec: f64,
        paused: bool,
    ) -> Result<Self, Msg> {
        let source = PreviewSource::from_position(audio, offset_sec, length_sec, position_sec)?;
        let duration_sec = source
            .total_duration()
            .expect("finite source")
            .as_secs_f64();
        let device = DeviceSinkBuilder::open_default_sink()
            .map_err(|error| Msg::OpenOutput(error.to_string()))?;
        let player = Player::connect_new(device.mixer());
        if paused {
            player.pause();
        }
        player.append(source);
        Ok(Self {
            player,
            _device: device,
            offset_sec,
            length_sec,
            duration_sec,
            start_sec: position_sec,
        })
    }

    pub fn is_finished(&self) -> bool {
        self.player.empty()
    }

    pub fn is_paused(&self) -> bool {
        self.player.is_paused()
    }

    pub fn toggle_pause(&self) {
        if self.is_paused() {
            self.player.play();
        } else {
            self.player.pause();
        }
    }

    pub fn position_sec(&self) -> f64 {
        self.start_sec
            + if self.is_finished() {
                self.duration_sec
            } else {
                self.player.get_pos().as_secs_f64().min(self.duration_sec)
            }
    }
}

struct PreviewSource {
    samples: Arc<[f32]>,
    sample_rate: NonZeroU32,
    first: i64,
    count: usize,
    index: usize,
}

impl PreviewSource {
    #[cfg(test)]
    fn new(audio: &Audio, offset_sec: f64, length_sec: f64) -> Result<Self, Msg> {
        Self::from_position(audio, offset_sec, length_sec, offset_sec)
    }

    fn from_position(
        audio: &Audio,
        offset_sec: f64,
        length_sec: f64,
        position_sec: f64,
    ) -> Result<Self, Msg> {
        let (start, end) =
            preview_range(audio, offset_sec, length_sec).ok_or(Msg::NoPlaybackRange)?;
        if !position_sec.is_finite() || position_sec < start || position_sec >= end {
            return Err(Msg::PlaybackPositionOutOfRange);
        }
        let start = position_sec;
        let sample_rate = NonZeroU32::new(audio.sample_rate).ok_or(Msg::InvalidSampleRate)?;
        let first = (start * f64::from(audio.sample_rate)).round() as i64;
        let stop = (end * f64::from(audio.sample_rate)).round() as i64;
        let count =
            usize::try_from(stop.saturating_sub(first)).map_err(|_| Msg::PlaybackRangeTooLong)?;
        if count == 0 {
            return Err(Msg::NoPlaybackRange);
        }
        Ok(Self {
            samples: Arc::clone(&audio.samples),
            sample_rate,
            first,
            count,
            index: 0,
        })
    }
}

impl Iterator for PreviewSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.index >= self.count {
            return None;
        }
        let frame = self.first + self.index as i64;
        self.index += 1;
        // Negative offsets include silence before the file begins, matching the preview timeline.
        Some(
            usize::try_from(frame)
                .ok()
                .and_then(|frame| self.samples.get(frame))
                .copied()
                .unwrap_or(0.0),
        )
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.count - self.index;
        (remaining, Some(remaining))
    }
}

impl Source for PreviewSource {
    fn current_span_len(&self) -> Option<usize> {
        Some(self.count - self.index)
    }
    fn channels(&self) -> rodio::ChannelCount {
        rodio::nz!(1)
    }
    fn sample_rate(&self) -> rodio::SampleRate {
        self.sample_rate
    }
    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f64(
            self.count as f64 / f64::from(self.sample_rate.get()),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn audio() -> Audio {
        Audio {
            samples: vec![0.1, 0.2, 0.3, 0.4].into(),
            sample_rate: 4,
        }
    }

    #[test]
    fn seek_plays_from_position_to_the_original_preview_end() {
        let source = PreviewSource::from_position(&audio(), 0.0, 0.75, 0.25).unwrap();
        assert_eq!(source.total_duration(), Some(Duration::from_secs_f64(0.5)));
        assert_eq!(source.collect::<Vec<_>>(), vec![0.2, 0.3]);
        let source = PreviewSource::from_position(&audio(), -0.5, 1.0, -0.25).unwrap();
        assert_eq!(source.collect::<Vec<_>>(), vec![0.0, 0.1, 0.2]);
    }

    #[test]
    fn seek_rejects_positions_outside_the_preview() {
        for position in [-0.25, 0.75, 1.0, f64::NAN, f64::INFINITY] {
            assert!(PreviewSource::from_position(&audio(), 0.0, 0.75, position).is_err());
        }
    }

    #[test]
    fn plays_only_the_selected_range() {
        let source = PreviewSource::new(&audio(), 0.25, 0.5).unwrap();
        assert_eq!(source.total_duration(), Some(Duration::from_secs_f64(0.5)));
        assert_eq!(source.collect::<Vec<_>>(), vec![0.2, 0.3]);
    }

    #[test]
    fn negative_offset_pads_with_silence_and_stops_at_the_preview_end() {
        let source = PreviewSource::new(&audio(), -0.5, 1.0).unwrap();
        assert_eq!(source.collect::<Vec<_>>(), vec![0.0, 0.0, 0.1, 0.2]);
    }

    #[test]
    fn clips_at_the_end_of_the_file() {
        let source = PreviewSource::new(&audio(), 0.75, 10.0).unwrap();
        assert_eq!(source.collect::<Vec<_>>(), vec![0.4]);
    }

    #[test]
    fn rejects_invalid_or_empty_ranges() {
        assert!(PreviewSource::new(&audio(), 1.0, 1.0).is_err());
        assert!(PreviewSource::new(&audio(), f64::NAN, 1.0).is_err());
        assert!(
            PreviewSource::new(
                &Audio {
                    samples: vec![].into(),
                    sample_rate: 0
                },
                0.0,
                1.0
            )
            .is_err()
        );
    }
}

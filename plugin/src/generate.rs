pub(crate) const LANE_COUNT: i32 = 16;
pub(crate) const TICKS_PER_BEAT: i32 = 480;
const MAX_WINDOWS: i32 = 20_000;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SilentZone {
    pub start_sec: f64,
    pub end_sec: f64,
}

impl SilentZone {
    pub fn contains(self, elapsed_sec: f64) -> bool {
        elapsed_sec >= self.start_sec && elapsed_sec < self.end_sec
    }

    fn contains_time(self, time_sec: f64, offset_sec: f64) -> bool {
        time_sec >= offset_sec + self.start_sec && time_sec < offset_sec + self.end_sec
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Settings {
    pub bpm: f64,
    pub time_signature: [i32; 2],
    pub offset_sec: f64,
    pub length_sec: f64,
    pub quantize_ticks: i32,
    pub width: i32,
    pub silence_threshold: f32,
    pub smooth: usize,
    pub remove_silent_points: bool,
    pub max_width: i32,
    pub symmetric_width: bool,
    pub center_ends: bool,
    pub zigzag: bool,
    /// Alternates widths 1 and 2 so the note center, where a rainbone line runs, moves in half lanes.
    pub fine_center: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            bpm: 120.0,
            time_signature: [4, 4],
            offset_sec: 0.0,
            length_sec: 0.0,
            quantize_ticks: 8,
            width: 4,
            silence_threshold: 0.02,
            smooth: 1,
            remove_silent_points: true,
            max_width: 16,
            symmetric_width: false,
            center_ends: true,
            zigzag: false,
            fine_center: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SlidePoint {
    pub tick: i32,
    pub x: i32,
    pub width: i32,
    pub control: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Slide {
    pub points: Vec<SlidePoint>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Generated {
    pub slides: Vec<Slide>,
    pub truncated: bool,
}

pub(crate) fn transparent_slide_note_count(start_tick: i32, end_tick: i32) -> usize {
    let length = end_tick.saturating_sub(start_tick).max(0);
    let unit = if length < 120 {
        120
    } else if length < 240 {
        240
    } else {
        480
    };
    let divisions = (length / unit) as usize;
    if length % unit == 0 {
        divisions.max(1)
    } else {
        divisions + 1
    }
}

pub(crate) fn ticks_per_second(bpm: f64, [numerator, denominator]: [i32; 2]) -> f64 {
    bpm / 60.0 * f64::from(TICKS_PER_BEAT) * f64::from(denominator.clamp(1, 480))
        / f64::from(numerator.clamp(1, 16))
}

pub(crate) fn centered_x(width: i32) -> i32 {
    let width = width.clamp(1, LANE_COUNT);
    (LANE_COUNT - width + 1) / 2
}

pub(crate) fn symmetric_width_from_amplitude(magnitude: f32, max_width: i32) -> (i32, i32) {
    let max_width = max_width.clamp(2, LANE_COUNT);
    let width = (magnitude.clamp(0.0, 1.0) * max_width as f32).round() as i32;
    let width = width.clamp(2, max_width);
    (centered_x(width), width)
}

/// Center of the lane span in half lanes.
const MIDDLE_HALF_LANE: i32 = LANE_COUNT;

/// Converts a center in half lanes to a width 1 note (odd) or a width 2 note (even).
fn note_from_half_lane(center: i32) -> (i32, i32) {
    let center = center.clamp(1, 2 * LANE_COUNT - 1);
    if center % 2 == 1 {
        ((center - 1) / 2, 1)
    } else {
        (center / 2 - 1, 2)
    }
}

fn half_lane_center(x: i32, width: i32) -> i32 {
    2 * x + width
}

/// Farthest the center may move from the middle, in half lanes.
fn half_lane_reach(max_width: i32) -> i32 {
    max_width.clamp(2, LANE_COUNT) - 1
}

pub(crate) fn amplitude_to_fine_note(signed: f32, max_width: i32) -> (i32, i32) {
    let reach = half_lane_reach(max_width) as f32;
    note_from_half_lane(MIDDLE_HALF_LANE + (signed.clamp(-1.0, 1.0) * reach).round() as i32)
}

pub(crate) fn amplitude_to_x(signed: f32, width: i32, max_width: i32) -> i32 {
    let width = width.clamp(1, LANE_COUNT);
    let max_width = max_width.clamp(2, LANE_COUNT).max(width);
    let left = centered_x(max_width);
    let right = left + max_width - width;
    let center = centered_x(width);
    let signed = signed.clamp(-1.0, 1.0);
    let distance = if signed < 0.0 {
        center - left
    } else {
        right - center
    };
    (center as f32 + signed * distance as f32).round() as i32
}

#[cfg(test)]
pub(crate) fn generate(
    samples: &[f32],
    sample_rate: u32,
    start_tick: i32,
    settings: &Settings,
) -> Generated {
    generate_with_silent_zones(samples, sample_rate, start_tick, settings, &[])
}

pub(crate) fn generate_with_silent_zones(
    samples: &[f32],
    sample_rate: u32,
    start_tick: i32,
    settings: &Settings,
    silent_zones: &[SilentZone],
) -> Generated {
    if samples.is_empty() || sample_rate == 0 || settings.bpm <= 0.0 {
        return Generated::default();
    }
    let width = settings.width.clamp(1, LANE_COUNT);
    let quantize = settings.quantize_ticks.max(1);
    let ticks_per_second = ticks_per_second(settings.bpm.max(1.0), settings.time_signature);
    if ticks_per_second <= 0.0 {
        return Generated::default();
    }
    let window = f64::from(quantize) / ticks_per_second;
    if window <= 0.0 {
        return Generated::default();
    }
    let duration = samples.len() as f64 / f64::from(sample_rate);
    let end = if settings.length_sec > 0.0 {
        (settings.offset_sec + settings.length_sec).min(duration)
    } else {
        duration
    };
    let peak = samples
        .iter()
        .enumerate()
        .fold(0.0f32, |peak, (index, sample)| {
            let time = index as f64 / f64::from(sample_rate);
            if silent_zones
                .iter()
                .any(|zone| zone.contains_time(time, settings.offset_sec))
            {
                peak
            } else {
                peak.max(sample.abs())
            }
        })
        .max(1.0e-8);
    let scale = peak;
    let mut slides = Vec::new();
    let mut segment = Vec::new();
    let mut truncated = false;
    let mut index = 0i32;
    loop {
        if index >= MAX_WINDOWS {
            truncated = true;
            break;
        }
        let Some(delta) = index.checked_mul(quantize) else {
            truncated = true;
            break;
        };
        let Some(tick) = start_tick.checked_add(delta) else {
            truncated = true;
            break;
        };
        let elapsed = f64::from(index) * window;
        let forced_center = silent_zones.iter().any(|zone| zone.contains(elapsed));
        let start = settings.offset_sec + elapsed;
        if start >= end || start >= duration {
            break;
        }
        let stop = (start + window).min(end).min(duration);
        index += 1;
        let (window_peak, signed) = if forced_center || stop <= 0.0 {
            (0.0, 0.0)
        } else {
            let Some(stats) = window_stats(
                samples,
                sample_rate,
                start,
                stop,
                settings.offset_sec,
                silent_zones,
            ) else {
                break;
            };
            stats
        };
        let silent = window_peak == 0.0 || window_peak < settings.silence_threshold;
        let signed = if silent {
            0.0
        } else {
            (signed / scale).clamp(-1.0, 1.0)
        };
        segment.push(RawPoint {
            tick,
            signed,
            magnitude: signed.abs(),
            centered: forced_center,
            silent,
        });
    }
    flush(&mut segment, &mut slides, settings, width);
    Generated { slides, truncated }
}

struct RawPoint {
    tick: i32,
    signed: f32,
    magnitude: f32,
    centered: bool,
    silent: bool,
}

fn flush(segment: &mut Vec<RawPoint>, slides: &mut Vec<Slide>, settings: &Settings, width: i32) {
    if segment.len() < 2 {
        segment.clear();
        return;
    }
    let mut values: Vec<f32> = segment.iter().map(|point| point.signed).collect();
    smooth_values(&mut values, settings.smooth);
    stretch_full(&mut values);
    let fixed_width = width.clamp(1, LANE_COUNT);
    let fine = settings.fine_center && !settings.symmetric_width;
    let mut points: Vec<SlidePoint> = segment
        .iter()
        .zip(values)
        .enumerate()
        .filter(|(index, (point, _))| {
            !settings.remove_silent_points
                || !point.silent
                || point.centered
                || *index == 0
                || *index == segment.len() - 1
        })
        .map(|(_, (point, signed))| {
            // Manual zones must remain straight even after smoothing and normalization.
            let signed = if point.centered { 0.0 } else { signed };
            if settings.symmetric_width {
                let (x, point_width) =
                    symmetric_width_from_amplitude(point.magnitude, settings.max_width);
                SlidePoint {
                    tick: point.tick,
                    x,
                    width: point_width,
                    control: false,
                }
            } else if fine {
                let (x, width) = amplitude_to_fine_note(signed, settings.max_width);
                SlidePoint {
                    tick: point.tick,
                    x,
                    width,
                    control: false,
                }
            } else {
                SlidePoint {
                    tick: point.tick,
                    x: amplitude_to_x(signed, fixed_width, settings.max_width),
                    width: fixed_width,
                    control: false,
                }
            }
        })
        .collect();
    points = simplify(points);
    if settings.center_ends && !settings.symmetric_width && !points.is_empty() {
        let last = points.len() - 1;
        for index in [0, last] {
            if fine {
                (points[index].x, points[index].width) = note_from_half_lane(MIDDLE_HALF_LANE);
            } else {
                points[index].x = centered_x(points[index].width);
            }
        }
    }
    if settings.zigzag && !settings.symmetric_width {
        points = if fine {
            insert_opposite_fine_controls(points, settings.max_width)
        } else {
            insert_opposite_controls(points, settings.max_width)
        };
        if settings.remove_silent_points {
            // Mirrored controls can land inside a silent gap even when its sampled points are omitted.
            points.retain(|point| {
                if !point.control {
                    return true;
                }
                let index = segment.partition_point(|raw| raw.tick <= point.tick);
                if index == 0 {
                    return true;
                }
                let raw = &segment[index - 1];
                !raw.silent
                    || raw.centered
                    || point.tick >= raw.tick.saturating_add(settings.quantize_ticks.max(1))
            });
        }
    }
    if points.len() >= 2 && points.last().expect("point").tick > points[0].tick {
        slides.push(Slide { points });
    }
    segment.clear();
}

fn insert_opposite_controls(points: Vec<SlidePoint>, max_width: i32) -> Vec<SlidePoint> {
    if points.len() < 2 {
        return points;
    }
    let mut output = Vec::with_capacity(points.len() * 2);
    for pair in points.windows(2) {
        let current = pair[0];
        let next = pair[1];
        output.push(current);
        let current_center = centered_x(current.width);
        let next_center = centered_x(next.width);
        let same_side = (current.x < current_center && next.x < next_center)
            || (current.x > current_center && next.x > next_center);
        let gap = next.tick.saturating_sub(current.tick);
        if !same_side || gap < 2 {
            continue;
        }
        let max_width = max_width.clamp(2, LANE_COUNT).max(current.width);
        let left = centered_x(max_width);
        let right = left + max_width - current.width;
        let center_x = centered_x(current.width);
        let x = (2 * center_x - current.x).clamp(left, right);
        output.push(SlidePoint {
            tick: current.tick + gap / 2,
            x,
            width: current.width,
            control: true,
        });
    }
    output.push(*points.last().expect("point"));
    output
}

fn insert_opposite_fine_controls(points: Vec<SlidePoint>, max_width: i32) -> Vec<SlidePoint> {
    if points.len() < 2 {
        return points;
    }
    let reach = half_lane_reach(max_width);
    let mut output = Vec::with_capacity(points.len() * 2);
    for pair in points.windows(2) {
        let current = pair[0];
        let next = pair[1];
        output.push(current);
        let side = |point: SlidePoint| {
            (half_lane_center(point.x, point.width) - MIDDLE_HALF_LANE).signum()
        };
        let gap = next.tick.saturating_sub(current.tick);
        if side(current) == 0 || side(current) != side(next) || gap < 2 {
            continue;
        }
        let mirrored = 2 * MIDDLE_HALF_LANE - half_lane_center(current.x, current.width);
        let (x, width) =
            note_from_half_lane(mirrored.clamp(MIDDLE_HALF_LANE - reach, MIDDLE_HALF_LANE + reach));
        output.push(SlidePoint {
            tick: current.tick + gap / 2,
            x,
            width,
            control: true,
        });
    }
    output.push(*points.last().expect("point"));
    output
}

fn stretch_full(values: &mut [f32]) {
    let min = values.iter().copied().fold(f32::MAX, f32::min);
    let max = values.iter().copied().fold(f32::MIN, f32::max);
    let span = max - min;
    if span < 1.0e-6 {
        return;
    }
    for value in values {
        *value = (*value - min) / span * 2.0 - 1.0;
    }
}

fn simplify(points: Vec<SlidePoint>) -> Vec<SlidePoint> {
    if points.len() <= 2 {
        return points;
    }
    let mut simplified = vec![points[0]];
    for index in 1..points.len() - 1 {
        let point = points[index];
        let previous = points[index - 1];
        let next = points[index + 1];
        if (point.x, point.width) != (previous.x, previous.width)
            || (point.x, point.width) != (next.x, next.width)
        {
            simplified.push(point);
        }
    }
    simplified.push(*points.last().expect("point"));
    simplified
}

fn smooth_values(values: &mut [f32], radius: usize) {
    if radius == 0 || values.len() < 2 {
        return;
    }
    let original = values.to_vec();
    for (index, value) in values.iter_mut().enumerate() {
        let start = index.saturating_sub(radius);
        let end = (index + radius + 1).min(original.len());
        let window = &original[start..end];
        *value = window.iter().sum::<f32>() / window.len() as f32;
    }
}

fn window_stats(
    samples: &[f32],
    sample_rate: u32,
    start: f64,
    stop: f64,
    offset_sec: f64,
    silent_zones: &[SilentZone],
) -> Option<(f32, f32)> {
    if sample_rate == 0 || stop <= start {
        return None;
    }
    let first = (start.max(0.0) * f64::from(sample_rate)).floor() as usize;
    let last = (stop.max(0.0) * f64::from(sample_rate)).ceil() as usize;
    if first >= samples.len() || last <= first {
        return None;
    }
    let slice = &samples[first..last.min(samples.len())];
    let mut peak = 0.0f32;
    let mut signed = 0.0f32;
    for (index, sample) in slice.iter().enumerate() {
        let time = (first + index) as f64 / f64::from(sample_rate);
        let sample = if silent_zones
            .iter()
            .any(|zone| zone.contains_time(time, offset_sec))
        {
            0.0
        } else {
            *sample
        };
        let abs = sample.abs();
        if abs >= peak {
            peak = abs;
            signed = sample;
        }
    }
    Some((peak, signed))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(length: f64) -> Settings {
        Settings {
            bpm: 120.0,
            time_signature: [4, 4],
            offset_sec: 0.0,
            length_sec: length,
            quantize_ticks: 480,
            width: 4,
            silence_threshold: 0.02,
            smooth: 0,
            remove_silent_points: false,
            max_width: 16,
            symmetric_width: false,
            center_ends: false,
            zigzag: false,
            fine_center: false,
        }
    }

    #[test]
    fn fine_center_alternates_widths_one_and_two_in_half_lanes() {
        assert_eq!(note_from_half_lane(1), (0, 1));
        assert_eq!(note_from_half_lane(2), (0, 2));
        assert_eq!(note_from_half_lane(16), (7, 2));
        assert_eq!(note_from_half_lane(17), (8, 1));
        assert_eq!(note_from_half_lane(31), (15, 1));
        assert_eq!(amplitude_to_fine_note(-1.0, 16), (0, 1));
        assert_eq!(amplitude_to_fine_note(0.0, 16), (7, 2));
        assert_eq!(amplitude_to_fine_note(1.0, 16), (15, 1));
        assert_eq!(amplitude_to_fine_note(-1.0, 2), (7, 1));
        assert_eq!(amplitude_to_fine_note(1.0, 2), (8, 1));
        let centers: std::collections::BTreeSet<_> = (-150..=150)
            .map(|value| {
                let (x, width) = amplitude_to_fine_note(value as f32 / 150.0, 16);
                half_lane_center(x, width)
            })
            .collect();
        assert_eq!(centers, (1..=31).collect());
    }

    #[test]
    fn fine_center_keeps_ends_and_mirrored_controls_on_half_lanes() {
        let mut options = settings(2.0);
        options.fine_center = true;
        options.center_ends = true;
        options.zigzag = true;
        options.silence_threshold = 0.0;
        let samples: Vec<f32> = (0..2000)
            .map(|index| ((index / 250) as f32 * 0.3).sin() * 0.3 + 0.5)
            .collect();
        options.quantize_ticks = 120;
        let points = &generate(&samples, 1000, 0, &options).slides[0].points;
        assert!(points.iter().all(|point| matches!(point.width, 1 | 2)));
        assert_eq!((points[0].x, points[0].width), (7, 2));
        let last = points.last().unwrap();
        assert_eq!((last.x, last.width), (7, 2));
        assert!(points.iter().any(|point| point.width == 1));
        let controls = insert_opposite_fine_controls(
            vec![
                SlidePoint {
                    tick: 0,
                    x: 2,
                    width: 1,
                    control: false,
                },
                SlidePoint {
                    tick: 8,
                    x: 3,
                    width: 2,
                    control: false,
                },
            ],
            16,
        );
        assert_eq!(controls.len(), 3);
        assert_eq!(half_lane_center(controls[1].x, controls[1].width), 27);
        assert!(controls[1].control);
    }

    #[test]
    fn removing_silent_points_is_enabled_by_default() {
        assert!(Settings::default().remove_silent_points);
    }

    #[test]
    fn silent_gaps_have_no_intermediate_or_mirrored_control_points() {
        let mut samples = vec![0.4; 1000];
        samples.extend(std::iter::repeat_n(0.05, 1500));
        samples.extend(std::iter::repeat_n(0.8, 1000));
        for symmetric_width in [false, true] {
            for max_width in [8, 16] {
                for smooth in [0, 2] {
                    let mut options = settings(3.5);
                    options.silence_threshold = 0.1;
                    options.symmetric_width = symmetric_width;
                    options.max_width = max_width;
                    options.smooth = smooth;
                    options.zigzag = true;
                    options.remove_silent_points = true;
                    let removed = generate(&samples, 1000, 0, &options);
                    let points = &removed.slides[0].points;
                    assert_eq!(points.first().unwrap().tick, 0);
                    assert_eq!(points.last().unwrap().tick, 2880);
                    assert!(
                        points
                            .iter()
                            .all(|point| !(960..2400).contains(&point.tick))
                    );
                    options.remove_silent_points = false;
                    let kept = generate(&samples, 1000, 0, &options);
                    assert!(
                        kept.slides[0]
                            .points
                            .iter()
                            .any(|point| (960..2400).contains(&point.tick))
                    );
                }
            }
        }
    }

    #[test]
    fn removal_preserves_slide_endpoints_and_manual_zone_centerline() {
        let mut options = settings(3.5);
        options.remove_silent_points = true;
        options.zigzag = true;
        options.smooth = 2;
        let silent = generate(&vec![0.0; 3500], 1000, 100, &options);
        assert_eq!(silent.slides[0].points.len(), 2);
        assert_eq!(silent.slides[0].points[0].tick, 100);
        assert_eq!(silent.slides[0].points[1].tick, 2980);
        let manual = generate_with_silent_zones(
            &vec![0.8; 3500],
            1000,
            0,
            &options,
            &[SilentZone {
                start_sec: 1.0,
                end_sec: 2.5,
            }],
        );
        let points = &manual.slides[0].points;
        for tick in [960, 1920] {
            let point = points.iter().find(|point| point.tick == tick).unwrap();
            assert_eq!(point.x, 6);
            assert!(!point.control);
        }
    }

    #[test]
    fn overlapping_zones_mask_samples_relative_to_the_import_offset() {
        let samples: Vec<f32> = (0..400)
            .map(|index| {
                if (120..240).contains(&index) {
                    1.0
                } else {
                    (index as f32 * 0.17).sin() * 0.3
                }
            })
            .collect();
        let zones = [
            SilentZone {
                start_sec: 0.2,
                end_sec: 1.4,
            },
            SilentZone {
                start_sec: 1.0,
                end_sec: 2.0,
            },
        ];
        let mut masked = samples.clone();
        masked[120..300].fill(0.0);
        for index in 0..60 {
            let start = 1.0 + index as f64 * 0.05;
            let stop = start + 0.05;
            assert_eq!(
                window_stats(&samples, 100, start, stop, 1.0, &zones),
                window_stats(&masked, 100, start, stop, 1.0, &[])
            );
        }
        assert!(samples[120..240].iter().all(|sample| *sample == 1.0));
    }

    #[test]
    fn only_manual_zones_override_smoothing_and_normalization() {
        let mut samples = vec![0.4; 1000];
        samples.extend(std::iter::repeat_n(0.0, 1500));
        samples.extend(std::iter::repeat_n(0.8, 1000));
        let zones = [SilentZone {
            start_sec: 1.0,
            end_sec: 2.5,
        }];
        for max_width in [8, 16] {
            let mut options = settings(3.5);
            options.smooth = 2;
            options.max_width = max_width;
            options.zigzag = true;
            let natural = generate(&samples, 1000, 0, &options);
            assert!(
                natural.slides[0]
                    .points
                    .iter()
                    .any(|point| (960..=1920).contains(&point.tick) && point.x != 6)
            );
            let manual = generate_with_silent_zones(&samples, 1000, 0, &options, &zones);
            let zone_points: Vec<_> = manual.slides[0]
                .points
                .iter()
                .filter(|point| (960..=1920).contains(&point.tick))
                .collect();
            assert!(!zone_points.is_empty());
            assert!(
                zone_points
                    .iter()
                    .all(|point| point.x == 6 && !point.control)
            );
        }
    }

    #[test]
    fn zones_follow_the_import_offset_and_can_cover_the_whole_range() {
        let samples = vec![1.0; 400];
        let zones = [SilentZone {
            start_sec: 0.0,
            end_sec: 2.0,
        }];
        for offset_sec in [-1.0, 0.0, 2.0] {
            let mut options = settings(2.0);
            options.offset_sec = offset_sec;
            options.smooth = 4;
            options.silence_threshold = 0.0;
            options.zigzag = true;
            let result = generate_with_silent_zones(&samples, 100, 0, &options, &zones);
            assert_eq!(result.slides.len(), 1);
            assert!(result.slides[0].points.iter().all(|point| point.x == 6));
        }
    }

    #[test]
    fn masking_is_sample_precise_and_end_is_exclusive() {
        let samples = vec![0.1, 0.9, -0.8, 0.7];
        let zones = [SilentZone {
            start_sec: 0.25,
            end_sec: 0.75,
        }];
        assert_eq!(
            window_stats(&samples, 4, 0.0, 1.0, 0.0, &zones),
            Some((0.7, 0.7))
        );
        assert_eq!(
            window_stats(&samples, 4, 0.25, 0.75, 0.0, &zones),
            Some((0.0, 0.0))
        );
        assert_eq!(
            window_stats(&samples, 4, 0.75, 1.0, 0.0, &zones),
            Some((0.7, 0.7))
        );
    }

    #[test]
    fn generation_uses_the_duration_of_the_time_signature() {
        assert_eq!(ticks_per_second(120.0, [4, 4]), 960.0);
        assert_eq!(ticks_per_second(120.0, [3, 4]), 1280.0);
        assert_eq!(ticks_per_second(120.0, [3, 8]), 2560.0);
        let mut options = settings(1.0);
        options.time_signature = [3, 8];
        let generated = generate(&vec![1.0; 1000], 1000, 0, &options);
        assert_eq!(generated.slides[0].points.last().unwrap().tick, 2400);
    }

    #[test]
    fn transparent_slide_counts_like_a_normal_slide() {
        assert_eq!(transparent_slide_note_count(0, 60), 1);
        assert_eq!(transparent_slide_note_count(0, 120), 1);
        assert_eq!(transparent_slide_note_count(0, 239), 1);
        assert_eq!(transparent_slide_note_count(0, 240), 1);
        assert_eq!(transparent_slide_note_count(0, 480), 1);
        assert_eq!(transparent_slide_note_count(0, 500), 2);
        assert_eq!(transparent_slide_note_count(100, 1060), 2);
        assert_eq!(transparent_slide_note_count(0, 960), 2);
    }

    #[test]
    fn amplitude_edges_map_to_lane_edges() {
        assert_eq!(amplitude_to_x(-1.0, 4, 16), 0);
        assert_eq!(amplitude_to_x(0.0, 4, 16), 6);
        assert_eq!(amplitude_to_x(1.0, 4, 16), 12);
        assert_eq!(amplitude_to_x(1.0, 16, 16), 0);
    }

    #[test]
    fn positive_wave_becomes_one_slide_on_the_right() {
        let generated = generate(&vec![1.0; 1000], 1000, 1920, &settings(1.0));
        assert_eq!(generated.slides.len(), 1);
        assert_eq!(
            generated.slides[0].points,
            vec![
                SlidePoint {
                    tick: 1920,
                    x: 12,
                    width: 4,
                    control: false,
                },
                SlidePoint {
                    tick: 2400,
                    x: 12,
                    width: 4,
                    control: false,
                },
            ]
        );
    }

    #[test]
    fn signed_wave_moves_from_left_to_right() {
        let mut samples = vec![-1.0; 500];
        samples.extend(std::iter::repeat_n(1.0, 500));
        let generated = generate(&samples, 1000, 0, &settings(1.0));
        let points = &generated.slides[0].points;
        assert!(points[0].x < points[1].x);
    }

    #[test]
    fn silence_connects_one_slide_and_a_single_window_is_dropped() {
        let mut samples = vec![1.0; 1000];
        samples.extend(std::iter::repeat_n(0.0, 500));
        samples.extend(std::iter::repeat_n(1.0, 1000));
        let generated = generate(&samples, 1000, 100, &settings(2.5));
        assert_eq!(generated.slides.len(), 1);
        let points = &generated.slides[0].points;
        assert_eq!(points[0].tick, 100);
        assert_eq!(points.last().unwrap().tick, 100 + 1920);
        assert_eq!(points.iter().find(|point| point.tick == 1060).unwrap().x, 0);
        assert!(
            generate(&vec![1.0; 400], 1000, 0, &settings(0.4))
                .slides
                .is_empty()
        );
    }

    #[test]
    fn threshold_and_flat_segments_are_respected() {
        let silent = generate(&vec![0.01; 1000], 1000, 0, &settings(1.0));
        assert_eq!(silent.slides.len(), 1);
        assert_eq!(silent.slides[0].points.len(), 2);
        assert!(silent.slides[0].points.iter().all(|point| point.x == 6));
        let generated = generate(&vec![1.0; 1500], 1000, 0, &settings(1.5));
        assert_eq!(generated.slides[0].points.len(), 2);
        assert_eq!(generated.slides[0].points[1].tick, 960);
    }

    #[test]
    fn manual_zone_stays_centered_with_smoothing_normalization_and_zigzag() {
        let mut samples = vec![0.4; 1000];
        samples.extend(std::iter::repeat_n(0.0, 1500));
        samples.extend(std::iter::repeat_n(0.8, 1000));
        let mut options = settings(3.5);
        options.smooth = 2;
        options.zigzag = true;
        let generated = generate_with_silent_zones(
            &samples,
            1000,
            0,
            &options,
            &[SilentZone {
                start_sec: 1.0,
                end_sec: 2.5,
            }],
        );
        assert_eq!(generated.slides.len(), 1);
        let points = &generated.slides[0].points;
        for tick in [960, 1920] {
            let point = points.iter().find(|point| point.tick == tick).unwrap();
            assert_eq!((point.x, point.width), (6, 4));
        }
        assert!(
            points
                .iter()
                .filter(|point| (960..=1920).contains(&point.tick))
                .all(|point| point.x == 6 && !point.control)
        );
    }

    #[test]
    fn silence_including_negative_offset_generates_a_straight_slide() {
        for symmetric_width in [false, true] {
            let mut options = settings(2.0);
            options.offset_sec = -1.0;
            options.symmetric_width = symmetric_width;
            options.zigzag = true;
            let generated = generate(&vec![0.0; 1000], 1000, 0, &options);
            assert_eq!(generated.slides.len(), 1);
            let points = &generated.slides[0].points;
            assert_eq!(points.len(), 2);
            assert_eq!(points[0].tick, 0);
            assert_eq!(points[1].tick, 1440);
            let expected = if symmetric_width { (7, 2) } else { (6, 4) };
            assert!(
                points
                    .iter()
                    .all(|point| (point.x, point.width) == expected)
            );
        }
    }

    #[test]
    fn small_variation_uses_the_full_lane_span() {
        let mut options = settings(1.0);
        options.silence_threshold = 0.0;
        let mut samples = vec![0.05; 500];
        samples.extend(std::iter::repeat_n(0.2, 500));
        let points = &generate(&samples, 1000, 0, &options).slides[0].points;
        assert_eq!(points[0].x, 0);
        assert_eq!(points[1].x, 12);
    }

    #[test]
    fn symmetric_width_expands_equally_from_center() {
        assert_eq!(symmetric_width_from_amplitude(1.0, 8), (4, 8));
        assert_eq!(symmetric_width_from_amplitude(0.0, 8), (7, 2));
        assert_eq!(centered_x(4), 6);
        let mut options = settings(1.0);
        options.symmetric_width = true;
        options.max_width = 8;
        let points = &generate(&vec![1.0; 1000], 1000, 0, &options).slides[0].points;
        assert!(points.iter().all(|point| point.x == 4 && point.width == 8));
    }

    #[test]
    fn same_side_points_insert_a_mirrored_control() {
        let points = insert_opposite_controls(
            vec![
                SlidePoint {
                    tick: 0,
                    x: 0,
                    width: 4,
                    control: false,
                },
                SlidePoint {
                    tick: 8,
                    x: 2,
                    width: 4,
                    control: false,
                },
            ],
            16,
        );
        assert_eq!(points.len(), 3);
        assert_eq!(points[1].tick, 4);
        assert_eq!(points[1].x, 12);
        assert!(points[1].control);
        assert!(!points[0].control);
    }

    #[test]
    fn odd_widths_keep_their_size_and_use_the_ninth_lane_as_center() {
        for width in 1..=16 {
            let x = centered_x(width);
            assert_eq!(x * 2 + width, if width % 2 == 0 { 16 } else { 17 });
            let mut options = settings(1.0);
            options.width = width;
            options.center_ends = true;
            let result = generate(&vec![1.0; 1000], 1000, 0, &options);
            assert!(
                result.slides[0]
                    .points
                    .iter()
                    .all(|point| point.width == width && point.x == x)
            );
        }
        assert_eq!(symmetric_width_from_amplitude(1.0, 5), (6, 5));
        assert_eq!(symmetric_width_from_amplitude(0.5, 5), (7, 3));
    }

    #[test]
    fn symmetric_slides_ignore_normal_waveform_endpoint_and_zigzag_options() {
        let mut options = settings(2.0);
        options.symmetric_width = true;
        options.max_width = 5;
        let samples = vec![0.6; 2000];
        let ordinary_options = generate(&samples, 1000, 0, &options);
        options.center_ends = true;
        options.zigzag = true;
        assert_eq!(generate(&samples, 1000, 0, &options), ordinary_options);
        assert!(
            ordinary_options.slides[0]
                .points
                .iter()
                .all(|point| point.width == 5 && point.x == 6 && !point.control)
        );
    }

    #[test]
    fn maximum_width_limits_the_whole_waveform_and_mirrored_controls() {
        for width in 1..=16 {
            for max_width in 2..=16 {
                let effective_max = max_width.max(width);
                let left = centered_x(effective_max);
                let right = left + effective_max - width;
                assert_eq!(amplitude_to_x(-1.0, width, max_width), left);
                assert_eq!(amplitude_to_x(1.0, width, max_width), right);
                assert_eq!(amplitude_to_x(0.0, width, max_width), centered_x(width));
                let controls = insert_opposite_controls(
                    vec![
                        SlidePoint {
                            tick: 0,
                            x: left,
                            width,
                            control: false,
                        },
                        SlidePoint {
                            tick: 480,
                            x: left,
                            width,
                            control: false,
                        },
                    ],
                    max_width,
                );
                assert!(
                    controls
                        .iter()
                        .all(|point| (left..=right).contains(&point.x))
                );
            }
        }
        let mut options = settings(1.0);
        options.max_width = 9;
        let samples: Vec<_> = vec![-1.0; 500].into_iter().chain(vec![1.0; 500]).collect();
        let result = generate(&samples, 1000, 0, &options);
        assert_eq!(result.slides[0].points[0].x, 4);
        assert_eq!(result.slides[0].points[1].x, 9);
        assert!(result.slides[0].points.iter().all(|point| point.width == 4));
    }

    #[test]
    fn an_odd_width_manual_zone_is_straight_at_the_right_biased_center() {
        let mut options = settings(2.0);
        options.width = 5;
        options.max_width = 9;
        options.zigzag = true;
        let result = generate_with_silent_zones(
            &vec![0.8; 2000],
            1000,
            0,
            &options,
            &[SilentZone {
                start_sec: 0.0,
                end_sec: 2.0,
            }],
        );
        assert!(
            result.slides[0]
                .points
                .iter()
                .all(|point| point.x == 6 && point.width == 5 && !point.control)
        );
    }

    #[test]
    fn center_ends_preserves_odd_width_and_biases_right() {
        let mut options = settings(1.0);
        options.center_ends = true;
        options.width = 5;
        let points = &generate(&vec![1.0; 1000], 1000, 0, &options).slides[0].points;
        assert_eq!(points[0].x, 6);
        assert_eq!(points[0].width, 5);
        assert_eq!(points.last().expect("end").x, 6);
        assert_eq!(points.last().expect("end").width, 5);
    }

    #[test]
    fn one_tick_grid_matches_1920th_notes() {
        let mut options = settings(0.01);
        options.quantize_ticks = 1;
        options.silence_threshold = 0.0;
        let samples: Vec<f32> = (0..20)
            .map(|index| if index % 2 == 0 { 0.2 } else { 0.9 })
            .collect();
        let points = &generate(&samples, 960, 100, &options).slides[0].points;
        assert!(points.len() >= 2);
        assert_eq!(points[1].tick - points[0].tick, 1);
        assert!(points.windows(2).all(|pair| pair[1].tick > pair[0].tick));
    }
}

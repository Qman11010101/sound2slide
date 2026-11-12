use crate::{
    generate::Slide,
    i18n::{Lang, Msg},
};

const BATCH: usize = 4096;
const MAX_DETAILS: usize = 100;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Reason {
    Bounds,
    Start,
    Outside {
        end_tick: i32,
        milliseconds: f64,
        limit: f64,
    },
}

impl Reason {
    pub fn text(&self, lang: Lang) -> String {
        match self {
            Self::Bounds => lang.pick([
                "48レーンの範囲外です（x < -16 または x + 幅 > 32）",
                "Outside the 48-lane field (x < -16 or x + width > 32)",
                "超出48軌範圍（x < -16 或 x + 寬度 > 32）",
                "48레인 범위를 벗어났습니다 (x < -16 또는 x + 너비 > 32)",
            ]).into(),
            Self::Start => lang.pick([
                "始点が中央16レーンに掛かっていません",
                "The start does not overlap the central 16 lanes",
                "起點未與中央16軌重疊",
                "시작점이 중앙 16레인과 겹치지 않습니다",
            ]).into(),
            Self::Outside { end_tick, milliseconds, limit } => lang.pick([
                format!("tick {end_tick}まで中央から外れています：{milliseconds:.1} ms（上限目安 {limit:.0} ms）"),
                format!("Outside the center through tick {end_tick}: {milliseconds:.1} ms (approx. limit {limit:.0} ms)"),
                format!("至tick {end_tick}仍在中央外：{milliseconds:.1} ms（上限約 {limit:.0} ms）"),
                format!("tick {end_tick}까지 중앙 밖: {milliseconds:.1} ms (상한 약 {limit:.0} ms)"),
            ]),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Warning {
    pub slide: usize,
    pub tick: i32,
    pub reason: Reason,
}

pub(crate) struct Audit {
    pub warnings: Vec<Warning>,
    pub count: usize,
    pub error: Option<Msg>,
    pub complete: bool,
    seek: Option<i32>,
    tick: i32,
    bpm: f64,
    slide: usize,
    segment: usize,
    outside: Option<(i32, f64)>,
}

impl Audit {
    pub fn new(slides: &[Slide], rainbone: bool) -> Self {
        let first = slides
            .first()
            .and_then(|s| s.points.first())
            .map(|p| p.tick);
        let mut audit = Self {
            warnings: Vec::new(),
            count: 0,
            error: None,
            complete: rainbone
                || first.is_none()
                || slides
                    .iter()
                    .flat_map(|s| &s.points)
                    .all(|p| covers_center(f64::from(p.x), f64::from(p.x + p.width))),
            seek: first,
            tick: first.unwrap_or(0),
            bpm: 120.0,
            slide: 0,
            segment: 0,
            outside: None,
        };
        for (index, slide) in slides.iter().enumerate() {
            for point in &slide.points {
                if point.x < -16 || i64::from(point.x) + i64::from(point.width) > 32 {
                    audit.warn(index, point.tick, Reason::Bounds);
                }
            }
            if !rainbone
                && let Some(first) = slide.points.first()
                && !covers_center(f64::from(first.x), f64::from(first.x + first.width))
            {
                audit.warn(index, first.tick, Reason::Start);
            }
        }
        audit
    }

    fn warn(&mut self, slide: usize, tick: i32, reason: Reason) {
        self.count += 1;
        if self.warnings.len() < MAX_DETAILS {
            self.warnings.push(Warning {
                slide,
                tick,
                reason,
            });
        }
    }

    pub fn advance(
        &mut self,
        slides: &[Slide],
        mut lookup: impl FnMut(i32) -> Result<Option<f64>, Msg>,
    ) {
        if self.complete {
            return;
        }
        for _ in 0..BATCH {
            if let Some(tick) = self.seek {
                match lookup(tick) {
                    Ok(Some(bpm)) if bpm.is_finite() && bpm > 0.0 => {
                        self.bpm = bpm;
                        self.seek = None;
                    }
                    Ok(None) if tick > 0 => self.seek = Some(tick - 1),
                    Ok(None) => self.seek = None,
                    result => {
                        self.fail(result.err().unwrap_or(Msg::InvalidBpmAt(tick)));
                        return;
                    }
                }
                continue;
            }
            match lookup(self.tick) {
                Ok(Some(bpm)) if bpm.is_finite() && bpm > 0.0 => self.bpm = bpm,
                Ok(None) => {}
                result => {
                    self.fail(result.err().unwrap_or(Msg::InvalidBpmAt(self.tick)));
                    return;
                }
            }
            let points = &slides[self.slide].points;
            let last = points.last().unwrap().tick;
            if self.tick >= points[0].tick {
                while self.segment + 1 < points.len() && points[self.segment + 1].tick <= self.tick
                {
                    self.segment += 1;
                }
                let a = &points[self.segment];
                let b = points.get(self.segment + 1).unwrap_or(a);
                let fraction = if b.tick > a.tick {
                    f64::from(self.tick - a.tick) / f64::from(b.tick - a.tick)
                } else {
                    0.0
                };
                let left = f64::from(a.x) + f64::from(b.x - a.x) * fraction;
                let right =
                    f64::from(a.x + a.width) + f64::from(b.x + b.width - a.x - a.width) * fraction;
                if covers_center(left, right) {
                    self.finish_outside(false);
                } else {
                    let outside = self.outside.get_or_insert((self.tick, 0.0));
                    outside.1 += 60000.0 / (self.bpm * 480.0);
                }
                if self.tick == last {
                    self.finish_outside(true);
                    self.slide += 1;
                    self.segment = 0;
                    if self.slide == slides.len() {
                        self.complete = true;
                        return;
                    }
                }
            }
            let Some(next) = self.tick.checked_add(1) else {
                self.complete = true;
                return;
            };
            self.tick = next;
        }
    }

    fn finish_outside(&mut self, terminal: bool) {
        if let Some((start, milliseconds)) = self.outside.take() {
            let limit = if terminal { 50.0 } else { 100.0 };
            // Margrete boundary behavior is unverified; keep the documented safety margin.
            if milliseconds + 1e-9 >= limit - 10.0 {
                self.warn(
                    self.slide,
                    start,
                    Reason::Outside {
                        end_tick: if terminal { self.tick } else { self.tick - 1 },
                        milliseconds,
                        limit,
                    },
                );
            }
        }
    }

    fn fail(&mut self, error: Msg) {
        self.error = Some(error);
        self.complete = true;
    }
}

fn covers_center(left: f64, right: f64) -> bool {
    right > 0.5 && left < 15.5
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generate::SlidePoint;

    fn slide(points: &[(i32, i32, i32)]) -> Slide {
        Slide {
            points: points
                .iter()
                .map(|&(tick, x, width)| SlidePoint {
                    tick,
                    x,
                    width,
                    control: true,
                })
                .collect(),
        }
    }

    fn check(slides: &[Slide], mut bpm: impl FnMut(i32) -> Result<Option<f64>, Msg>) -> Audit {
        let mut audit = Audit::new(slides, false);
        for _ in 0..100 {
            audit.advance(slides, &mut bpm);
            if audit.complete {
                return audit;
            }
        }
        panic!("audit did not finish");
    }

    fn outside(audit: &Audit) -> Vec<(i32, i32, f64, f64)> {
        audit
            .warnings
            .iter()
            .filter_map(|w| match w.reason {
                Reason::Outside {
                    end_tick,
                    milliseconds,
                    limit,
                } => Some((w.tick, end_tick, milliseconds, limit)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn central_and_partially_overlapping_paths_need_no_timing_lookup() {
        let slides = [slide(&[(0, -3, 4), (500, 15, 4)])];
        let audit = check(&slides, |_| panic!("central paths do not require BPM"));
        assert_eq!(audit.count, 0);
    }

    #[test]
    fn start_outside_warns_even_when_the_path_immediately_returns() {
        let slides = [slide(&[(0, -4, 4), (1, 0, 4)])];
        let audit = check(&slides, |_| Ok(None));
        assert_eq!(audit.count, 1);
        assert_eq!(audit.warnings[0].reason, Reason::Start);
        assert_eq!(audit.warnings[0].tick, 0);
    }

    #[test]
    fn interpolates_both_edges_instead_of_rounding_x_and_width_separately() {
        let slides = [slide(&[(0, -2, 3), (120, -4, 4)])];
        let audit = check(&slides, |_| Ok(None));
        let intervals = outside(&audit);
        assert_eq!(intervals.len(), 1);
        let (start, end, ms, limit) = intervals[0];
        assert_eq!((start, end, limit), (60, 120, 50.0));
        assert!((ms - 61.0 * 60000.0 / (120.0 * 480.0)).abs() < 1e-8);
    }

    #[test]
    fn left_and_right_paths_have_identical_warning_durations() {
        let left = check(&[slide(&[(0, 0, 4), (400, -4, 4)])], |_| Ok(None));
        let right = check(&[slide(&[(0, 12, 4), (400, 16, 4)])], |_| Ok(None));
        assert_eq!(outside(&left), outside(&right));
    }

    #[test]
    fn returns_to_center_reset_the_outside_timer() {
        let slides = [slide(&[
            (0, 0, 4),
            (1, -4, 4),
            (70, -4, 4),
            (71, 0, 4),
            (72, -4, 4),
            (141, -4, 4),
            (142, 0, 4),
        ])];
        assert_eq!(check(&slides, |_| Ok(None)).count, 0);
    }

    #[test]
    fn intermediate_and_terminal_limits_include_the_documented_safety_margin() {
        for (duration, terminal, warnings) in
            [(38, true, 0), (39, true, 1), (86, false, 0), (87, false, 1)]
        {
            let mut points = vec![(0, 0, 4), (1, -4, 4), (duration, -4, 4)];
            if !terminal {
                points.push((duration + 1, 0, 4));
            }
            let audit = check(&[slide(&points)], |_| Ok(None));
            assert_eq!(
                audit.count, warnings,
                "duration={duration}, terminal={terminal}"
            );
        }
    }

    #[test]
    fn bpm_changes_inside_an_outside_interval_use_actual_tick_time() {
        let slides = [slide(&[(0, 0, 4), (1, -4, 4), (480, -4, 4)])];
        let audit = check(&slides, |tick| {
            Ok(match tick {
                0 => Some(120.0),
                241 => Some(240.0),
                _ => None,
            })
        });
        let intervals = outside(&audit);
        assert_eq!(intervals.len(), 1);
        assert!((intervals[0].2 - 375.0).abs() < 1e-8);
    }

    #[test]
    fn finds_the_active_bpm_before_the_generated_range() {
        let slides = [slide(&[(5000, 0, 4), (5001, -4, 4), (5100, -4, 4)])];
        let audit = check(&slides, |tick| Ok((tick == 4800).then_some(240.0)));
        assert!((outside(&audit)[0].2 - 100.0 * 60000.0 / (240.0 * 480.0)).abs() < 1e-8);
    }

    #[test]
    fn rainbone_only_checks_the_full_field_bounds() {
        let valid = [slide(&[(0, -16, 1), (1000, 31, 1)])];
        let mut audit = Audit::new(&valid, true);
        audit.advance(&valid, |_| panic!("rainbone needs no timing lookup"));
        assert!(audit.complete);
        assert_eq!(audit.count, 0);
        let invalid = [slide(&[(0, -17, 1), (1000, 32, 1)])];
        let audit = Audit::new(&invalid, true);
        assert_eq!(audit.count, 2);
        assert!(audit.warnings.iter().all(|w| w.reason == Reason::Bounds));
    }

    #[test]
    fn scanning_yields_and_reports_lookup_failures_without_claiming_success() {
        let slides = [slide(&[(10000, 0, 4), (10100, -4, 4)])];
        let mut audit = Audit::new(&slides, false);
        let mut calls = 0;
        audit.advance(&slides, |_| {
            calls += 1;
            Ok(None)
        });
        assert_eq!(calls, BATCH);
        assert!(!audit.complete);
        audit.advance(&slides, |_| Err(Msg::NoChart));
        assert!(audit.complete);
        assert_eq!(audit.error, Some(Msg::NoChart));
    }

    #[test]
    fn invalid_bpm_does_not_silently_skip_the_time_check() {
        let slides = [slide(&[(0, 0, 4), (100, -4, 4)])];
        for bpm in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            let audit = check(&slides, |_| Ok(Some(bpm)));
            assert_eq!(audit.error, Some(Msg::InvalidBpmAt(0)));
        }
    }

    #[test]
    fn warning_details_are_bounded_while_the_total_count_is_preserved() {
        let slides = [slide(
            &(0..200).map(|tick| (tick, -17, 1)).collect::<Vec<_>>(),
        )];
        let audit = Audit::new(&slides, true);
        assert_eq!(audit.count, 200);
        assert_eq!(audit.warnings.len(), MAX_DETAILS);
    }
}

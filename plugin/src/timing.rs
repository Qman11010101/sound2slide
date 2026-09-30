use crate::generate::TICKS_PER_BEAT;

const TICKS_PER_BAR: i32 = TICKS_PER_BEAT * 4;
const SCAN_BATCH: i32 = 4096;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ChartPosition {
    pub seconds: f64,
    pub bpm: f64,
    pub signature: [i32; 2],
}

struct Change {
    tick: i32,
    bpm: Option<f64>,
    signature: Option<[i32; 2]>,
}

pub(crate) struct PositionScan {
    start_tick: i32,
    next_tick: i32,
    changes: Vec<Change>,
}

impl PositionScan {
    pub fn new(start_tick: i32) -> Result<Self, String> {
        if start_tick < 0 {
            return Err("現在位置が tick 0 より前のため、自動計算できません".into());
        }
        Ok(Self {
            start_tick,
            next_tick: start_tick,
            changes: Vec::new(),
        })
    }

    pub fn advance(
        &mut self,
        mut lookup_bpm: impl FnMut(i32) -> Result<Option<f64>, String>,
        mut lookup_signature: impl FnMut(i32) -> Result<Option<[i32; 2]>, String>,
    ) -> Result<Option<ChartPosition>, String> {
        for _ in 0..SCAN_BATCH {
            let tick = self.next_tick;
            if tick < 0 {
                return Ok(Some(self.position()));
            }
            let bpm = lookup_bpm(tick)?;
            if bpm.is_some_and(|bpm| !bpm.is_finite() || bpm <= 0.0) {
                return Err(format!("tick {tick} のBPMが無効です"));
            }
            // Beat changes are indexed by bar, while Margrete uses 1920 ticks per bar.
            let signature = if tick % TICKS_PER_BAR == 0 {
                lookup_signature(tick / TICKS_PER_BAR)?
            } else {
                None
            };
            if signature.is_some_and(|[numerator, denominator]| {
                !(1..=16).contains(&numerator) || !(1..=480).contains(&denominator)
            }) {
                return Err(format!("tick {tick} の拍子が無効です"));
            }
            if bpm.is_some() || signature.is_some() {
                self.changes.push(Change {
                    tick,
                    bpm,
                    signature,
                });
            }
            self.next_tick -= 1;
        }
        if self.next_tick < 0 {
            Ok(Some(self.position()))
        } else {
            Ok(None)
        }
    }

    fn position(&self) -> ChartPosition {
        let mut result = ChartPosition {
            seconds: 0.0,
            bpm: 120.0,
            signature: [4, 4],
        };
        let mut previous = 0;
        for change in self.changes.iter().rev() {
            result.seconds += elapsed_seconds(change.tick - previous, result.bpm, result.signature);
            previous = change.tick;
            if let Some(bpm) = change.bpm {
                result.bpm = bpm;
            }
            if let Some(signature) = change.signature {
                result.signature = signature;
            }
        }
        result.seconds += elapsed_seconds(self.start_tick - previous, result.bpm, result.signature);
        result
    }
}

fn elapsed_seconds(ticks: i32, bpm: f64, [numerator, denominator]: [i32; 2]) -> f64 {
    f64::from(ticks) / f64::from(TICKS_PER_BAR) * (60.0 / bpm) * f64::from(numerator) * 4.0
        / f64::from(denominator)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn calculate(
        start_tick: i32,
        mut bpm: impl FnMut(i32) -> Result<Option<f64>, String>,
        mut signature: impl FnMut(i32) -> Result<Option<[i32; 2]>, String>,
    ) -> ChartPosition {
        let mut scan = PositionScan::new(start_tick).unwrap();
        loop {
            if let Some(position) = scan.advance(&mut bpm, &mut signature).unwrap() {
                return position;
            }
        }
    }

    #[test]
    fn scans_every_tick_backwards_across_batches_and_uses_defaults() {
        let mut ticks = Vec::new();
        let mut bars = Vec::new();
        let position = calculate(
            5000,
            |tick| {
                ticks.push(tick);
                Ok(None)
            },
            |bar| {
                bars.push(bar);
                Ok(None)
            },
        );
        assert_eq!(ticks, (0..=5000).rev().collect::<Vec<_>>());
        assert_eq!(bars, vec![2, 1, 0]);
        assert!((position.seconds - 5000.0 / 960.0).abs() < 1.0e-10);
        assert_eq!(position.bpm, 120.0);
        assert_eq!(position.signature, [4, 4]);
    }

    #[test]
    fn combines_independent_bpm_and_signature_changes_with_partial_bars() {
        let position = calculate(
            4800,
            |tick| {
                Ok(match tick {
                    0 => Some(120.0),
                    960 => Some(60.0),
                    2400 => Some(180.0),
                    _ => None,
                })
            },
            |bar| {
                Ok(match bar {
                    0 => Some([4, 4]),
                    1 => Some([3, 8]),
                    2 => Some([5, 4]),
                    _ => None,
                })
            },
        );
        assert!((position.seconds - (3.75 + 5.0 / 6.0)).abs() < 1.0e-10);
        assert_eq!(position.bpm, 180.0);
        assert_eq!(position.signature, [5, 4]);
    }

    #[test]
    fn events_at_current_position_apply_without_changing_elapsed_time() {
        let position = calculate(
            1920,
            |tick| Ok((tick == 1920).then_some(240.0)),
            |bar| Ok((bar == 1).then_some([3, 4])),
        );
        assert_eq!(position.seconds, 2.0);
        assert_eq!(position.bpm, 240.0);
        assert_eq!(position.signature, [3, 4]);
        assert_eq!(
            calculate(0, |_| Ok(Some(90.0)), |_| Ok(Some([7, 8]))).seconds,
            0.0
        );
    }

    #[test]
    fn rejects_invalid_events_and_lookup_errors() {
        assert!(PositionScan::new(-1).is_err());
        assert!(
            PositionScan::new(0)
                .unwrap()
                .advance(|_| Ok(Some(0.0)), |_| Ok(None))
                .is_err()
        );
        assert!(
            PositionScan::new(0)
                .unwrap()
                .advance(|_| Ok(None), |_| Ok(Some([4, 0])))
                .is_err()
        );
        assert!(
            PositionScan::new(0)
                .unwrap()
                .advance(|_| Err("fail".into()), |_| Ok(None))
                .is_err()
        );
    }
}

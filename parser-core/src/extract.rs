//! Raw demo data -> model structures. Pure functions, testable without a demo file.

use crate::models::{Round, Team};

/// Round-related game events in the order the demo emitted them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoundEvent {
    /// `begin_new_match`: everything before it (warmup, restarts) is discarded.
    NewMatch(i32),
    Start(i32),
    FreezeEnd(i32),
    End { tick: i32, winner: i32, reason: i32 },
}

/// Splits the match into rounds. A round exists only if it has both a start and an end;
/// a trailing unfinished round (demo cut off) is dropped.
pub fn build_rounds(events: &[RoundEvent]) -> Vec<Round> {
    let mut rounds: Vec<Round> = Vec::new();
    let mut start: Option<i32> = None;
    let mut freeze: Option<i32> = None;
    for ev in events {
        match *ev {
            RoundEvent::NewMatch(t) => {
                rounds.clear();
                start = Some(t);
                freeze = None;
            }
            RoundEvent::Start(t) => {
                start = Some(t);
                freeze = None;
            }
            RoundEvent::FreezeEnd(t) => freeze = Some(t),
            RoundEvent::End { tick, winner, reason } => {
                if let Some(start_tick) = start.take() {
                    rounds.push(Round {
                        number: rounds.len() as u32 + 1,
                        start_tick,
                        freeze_end_tick: freeze.take(),
                        end_tick: tick,
                        winner: Team::from_team_num(winner),
                        reason,
                    });
                }
            }
        }
    }
    rounds
}

#[cfg(test)]
mod tests {
    use super::*;
    use RoundEvent::*;

    fn end(tick: i32, winner: i32) -> RoundEvent {
        End { tick, winner, reason: 9 }
    }

    #[test]
    fn splits_rounds_and_numbers_them() {
        let r = build_rounds(&[
            Start(65), NewMatch(67), FreezeEnd(100), end(500, 2),
            Start(600), FreezeEnd(700), end(1000, 3),
        ]);
        assert_eq!(r.len(), 2);
        // begin_new_match after round_start: the round starts at the later tick.
        assert_eq!((r[0].number, r[0].start_tick, r[0].freeze_end_tick, r[0].end_tick), (1, 67, Some(100), 500));
        assert_eq!(r[0].winner, Some(Team::T));
        assert_eq!((r[1].number, r[1].start_tick), (2, 600));
        assert_eq!(r[1].winner, Some(Team::CT));
    }

    #[test]
    fn new_match_discards_warmup() {
        let r = build_rounds(&[Start(1), FreezeEnd(5), end(10, 2), NewMatch(20), FreezeEnd(30), end(40, 3)]);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].start_tick, 20);
    }

    #[test]
    fn round_without_freeze_end_and_unfinished_tail() {
        let r = build_rounds(&[Start(1), end(10, 3), Start(20), FreezeEnd(25)]);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].freeze_end_tick, None);
    }
}

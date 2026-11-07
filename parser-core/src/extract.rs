//! Raw demo data -> model structures. Pure functions, testable without a demo file.

use crate::models::{InventoryChange, Player, PlayerTrack, Round, Team};
use std::collections::HashMap;

/// Snapshot step in ticks.
pub const SNAPSHOT_STEP: i32 = 4;
const DEFAULT_TICK_RATE: u32 = 64;

/// One player at one snapshot tick, as read from the demo. `None` means the game had no
/// value (e.g. a dead player has no pawn).
#[derive(Debug, Clone, Default)]
pub struct TickRow {
    pub tick: i32,
    pub steam_id: u64,
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub z: Option<f32>,
    pub yaw: Option<f32>,
    pub pitch: Option<f32>,
    pub hp: Option<i32>,
    pub alive: Option<bool>,
    pub team: Option<i32>,
    pub weapon: Option<String>,
    pub money: Option<i32>,
    pub armor: Option<i32>,
    pub helmet: Option<bool>,
    pub defuser: Option<bool>,
    pub inventory: Vec<String>,
    pub spent: Option<i32>,
    pub game_time: Option<f32>,
}

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
                        ticks: Vec::new(),
                        teams: Vec::new(),
                        tracks: Vec::new(),
                        reason,
                    });
                }
            }
        }
    }
    rounds
}

/// Ticks to sample: every `SNAPSHOT_STEP` from each round's start to its end.
pub fn snapshot_ticks(rounds: &[Round]) -> Vec<i32> {
    rounds
        .iter()
        .flat_map(|r| (r.start_tick..=r.end_tick).step_by(SNAPSHOT_STEP as usize))
        .collect()
}

/// Tick rate from `game_time` over the first round (ticks / seconds), rounded.
pub fn derive_tick_rate(rows: &[TickRow], rounds: &[Round]) -> u32 {
    let limit = rounds.first().map_or(i32::MAX, |r| r.end_tick);
    let mut timed = rows.iter().filter(|r| r.tick <= limit).filter_map(|r| Some((r.tick, r.game_time?)));
    let Some(first) = timed.next() else { return DEFAULT_TICK_RATE };
    let Some(last) = timed.last() else { return DEFAULT_TICK_RATE };
    let (dt, dg) = ((last.0 - first.0) as f32, last.1 - first.1);
    if dt <= 0.0 || dg <= 0.0 {
        return DEFAULT_TICK_RATE;
    }
    let rate = (dt / dg).round() as u32;
    if (16..=256).contains(&rate) { rate } else { DEFAULT_TICK_RATE }
}

fn tenths(v: f32) -> i32 {
    (v * 10.0).round() as i32
}

/// Interns names into the weapon dictionary (id = position).
#[derive(Default)]
struct Dict {
    names: Vec<String>,
    ids: HashMap<String, u32>,
}

impl Dict {
    fn id(&mut self, name: &str) -> u32 {
        if let Some(&id) = self.ids.get(name) {
            return id;
        }
        let id = self.names.len() as u32;
        self.names.push(name.to_string());
        self.ids.insert(name.to_string(), id);
        id
    }
}

/// Fills `ticks`, `teams` and `tracks` of every round from the sampled rows and returns the
/// weapon dictionary. `players` must be indexed by `Player::index`.
pub fn fill_snapshots(rounds: &mut [Round], players: &[Player], rows: &[TickRow]) -> Vec<String> {
    let by_steam: HashMap<u64, usize> = players.iter().map(|p| (p.steam_id, p.index as usize)).collect();
    let by_key: HashMap<(i32, usize), &TickRow> = rows
        .iter()
        .filter_map(|r| Some(((r.tick, *by_steam.get(&r.steam_id)?), r)))
        .collect();
    let mut dict = Dict::default();

    for round in rounds.iter_mut() {
        round.ticks = (round.start_tick..=round.end_tick).step_by(SNAPSHOT_STEP as usize).collect();
        round.teams = vec![None; players.len()];
        round.tracks = Vec::with_capacity(players.len());
        // Last snapshot of the buy phase: the end of freeze time, or the round end.
        let buy_idx = round
            .ticks
            .iter()
            .rposition(|&t| round.freeze_end_tick.map_or(true, |f| t <= f))
            .unwrap_or(0);

        for p in 0..players.len() {
            let mut tr = PlayerTrack::default();
            let (mut pos, mut ang) = ([0f32; 3], [0f32; 2]);
            let mut weapon: Option<u32> = None;
            let mut inv: Option<(i32, u8, Vec<u32>)> = None;
            let mut money = 0;
            for (i, &tick) in round.ticks.iter().enumerate() {
                let row = by_key.get(&(tick, p)).copied();
                let alive = row.map_or(false, |r| r.alive.unwrap_or(true) && r.hp.unwrap_or(0) > 0);
                if let Some(r) = row {
                    if let (Some(x), Some(y), Some(z)) = (r.x, r.y, r.z) {
                        pos = [x, y, z];
                    }
                    if let (Some(yaw), Some(pitch)) = (r.yaw, r.pitch) {
                        ang = [yaw, pitch];
                    }
                    if round.teams[p].is_none() {
                        round.teams[p] = r.team.and_then(Team::from_team_num);
                    }
                    money = r.money.unwrap_or(money);
                    if alive {
                        if let Some(w) = &r.weapon {
                            let id = dict.id(w);
                            if weapon != Some(id) {
                                weapon = Some(id);
                                tr.weapon.push((i as u32, id));
                            }
                        }
                        let items: Vec<u32> = r.inventory.iter().map(|n| dict.id(n)).collect();
                        let cur = (
                            r.armor.unwrap_or(0),
                            r.helmet.unwrap_or(false) as u8 | (r.defuser.unwrap_or(false) as u8) << 1,
                            items,
                        );
                        if inv.as_ref() != Some(&cur) {
                            tr.inventory.push(InventoryChange(i as u32, cur.0, cur.1, cur.2.clone()));
                            inv = Some(cur);
                        }
                    }
                    if i == buy_idx {
                        tr.spent = r.spent.unwrap_or(0);
                    }
                }
                tr.x.push(tenths(pos[0]));
                tr.y.push(tenths(pos[1]));
                tr.z.push(tenths(pos[2]));
                tr.yaw.push(tenths(ang[0]));
                tr.pitch.push(tenths(ang[1]));
                tr.hp.push(if alive { row.and_then(|r| r.hp).unwrap_or(0).clamp(1, 100) } else { 0 });
                tr.money.push(money);
            }
            round.tracks.push(tr);
        }
    }
    dict.names
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

    fn player(index: u32, steam_id: u64) -> Player {
        Player { index, steam_id, name: String::new(), final_team: None }
    }

    fn row(tick: i32, steam_id: u64) -> TickRow {
        TickRow { tick, steam_id, ..Default::default() }
    }

    #[test]
    fn snapshots_fill_arrays_and_carry_position_when_dead() {
        let mut rounds = build_rounds(&[Start(0), FreezeEnd(4), end(8, 2)]);
        let players = [player(0, 10), player(1, 20)];
        let alive = |tick, x: f32, w: &str| TickRow {
            x: Some(x), y: Some(0.0), z: Some(1.5), yaw: Some(90.25), pitch: Some(-10.0),
            hp: Some(100), alive: Some(true), team: Some(3), weapon: Some(w.into()),
            money: Some(800), armor: Some(100), helmet: Some(true), inventory: vec!["knife".into(), w.into()],
            spent: Some(if tick == 4 { 650 } else { 0 }), ..row(tick, 10)
        };
        let rows = vec![
            alive(0, 1.0, "AK-47"), alive(4, 2.0, "AK-47"),
            // tick 8: player 10 has no pawn (dead); player 20 never appears.
            TickRow { alive: Some(false), money: Some(800), ..row(8, 10) },
        ];
        let dict = fill_snapshots(&mut rounds, &players, &rows);
        let r = &rounds[0];
        assert_eq!(r.ticks, vec![0, 4, 8]);
        assert_eq!(r.teams, vec![Some(Team::CT), None]);
        let t = &r.tracks[0];
        assert_eq!(t.x, vec![10, 20, 20]); // tenths; carried after death
        assert_eq!(t.z, vec![15, 15, 15]);
        assert_eq!(t.yaw[0], 903); // 90.25 -> 902.5 rounds half away from zero
        assert_eq!(t.hp, vec![100, 100, 0]);
        assert_eq!(t.weapon, vec![(0, 0)]);
        assert_eq!(dict, vec!["AK-47".to_string(), "knife".to_string()]);
        assert_eq!(t.inventory, vec![InventoryChange(0, 100, 1, vec![1, 0])]);
        assert_eq!(t.spent, 650);
        assert_eq!(t.money, vec![800, 800, 800]);
        // The absent player is dead everywhere but arrays still have full length.
        assert_eq!(r.tracks[1].hp, vec![0, 0, 0]);
        assert_eq!(r.tracks[1].x.len(), 3);
    }

    #[test]
    fn tick_rate_from_game_time() {
        let rounds = build_rounds(&[Start(0), end(1000, 2)]);
        let rows: Vec<TickRow> = [0, 640].iter().map(|&t| TickRow { game_time: Some(100.0 + t as f32 / 128.0), ..row(t, 1) }).collect();
        assert_eq!(derive_tick_rate(&rows, &rounds), 128);
        assert_eq!(derive_tick_rate(&[], &rounds), 64);
    }
}

//! JSON contract. Stage 2 defines the top level; rounds are filled in later stages.

use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Team {
    T,
    CT,
}

impl Team {
    /// Source 2 team numbers: 2 = T, 3 = CT. Others (spectator/unassigned) -> None.
    pub fn from_team_num(n: i32) -> Option<Team> {
        match n {
            2 => Some(Team::T),
            3 => Some(Team::CT),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Player {
    /// Index used by every other part of the file (thrower, killer, per-round arrays).
    pub index: u32,
    pub steam_id: u64,
    pub name: String,
    /// Side at the end of the match. Sides swap at halftime, so the side in a given
    /// round is stored per round, not here.
    pub final_team: Option<Team>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Match {
    pub schema_version: u32,
    pub map: String,
    pub tick_rate: u32,
    pub players: Vec<Player>,
    /// Weapon id -> name. Filled in stage 4.
    pub weapons: Vec<String>,
    pub rounds: Vec<Round>,
}

/// One round. Per-tick data (snapshots, grenades, bomb, events) is added in later stages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Round {
    /// 1-based, in play order (overtime included).
    pub number: u32,
    pub start_tick: i32,
    /// End of the freeze time; absent when the round ended before it (e.g. surrender).
    pub freeze_end_tick: Option<i32>,
    pub end_tick: i32,
    pub winner: Option<Team>,
    /// Snapshot ticks: `start_tick`, +4, +8, ... up to `end_tick`. Every per-player array
    /// below has exactly this length (weapon/inventory are change lists instead).
    pub ticks: Vec<i32>,
    /// Side of each player in this round, indexed by `Player::index`.
    pub teams: Vec<Option<Team>>,
    /// Per-player data, indexed by `Player::index`.
    pub tracks: Vec<PlayerTrack>,
    /// Raw `round_end` reason from the game (1 bomb exploded, 7 defused, 8 CTs
    /// eliminated, 9 Ts eliminated, 18 CT surrender, ...). Unknown values pass through.
    pub reason: i32,
}

/// Snapshot data of one player in one round. All arrays are indexed like `Round::ticks`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerTrack {
    /// Position in tenths of a game unit. Dead players keep their last known position.
    pub x: Vec<i32>,
    pub y: Vec<i32>,
    pub z: Vec<i32>,
    /// Tenths of a degree: yaw -1800..=1800, pitch -900..=900.
    pub yaw: Vec<i32>,
    pub pitch: Vec<i32>,
    /// 0..=100, 0 means dead.
    pub hp: Vec<i32>,
    /// Active weapon changes: `[snapshot index, weapon id]`; ids index `Match::weapons`.
    pub weapon: Vec<(u32, u32)>,
    /// Money at each snapshot.
    pub money: Vec<i32>,
    /// Inventory changes, see `InventoryChange`.
    pub inventory: Vec<InventoryChange>,
    /// Money spent during the buy phase (`cash_spent_this_round` at the end of freeze time).
    pub spent: i32,
}

/// `[snapshot index, armor, flags, [item ids]]`. flags: bit 0 = helmet, bit 1 = defuser.
/// Item ids index `Match::weapons`. Recorded on the first snapshot and whenever it changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventoryChange(pub u32, pub i32, pub u8, pub Vec<u32>);

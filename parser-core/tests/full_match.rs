//! Checks on a full official match (NAVI vs Falcons, de_ancient, 24 rounds). The demo is 410 MB
//! and not in git; the tests are skipped without it and slow in debug builds, so run them with
//! `cargo test --release -p parser-core --test full_match`.

mod common;

use parser_core::models::{BombEnd, GrenadeKind, Match, Team};
use std::path::PathBuf;
use std::sync::OnceLock;

fn parsed() -> Option<&'static Match> {
    static M: OnceLock<Option<Match>> = OnceLock::new();
    M.get_or_init(|| {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../samples/natus-vincere-vs-falcons-m1-ancient.dem");
        if !p.exists() {
            eprintln!("skipped: no {}", p.display());
            return None;
        }
        Some(parser_core::parse_file(&p, None).expect("full match parses"))
    })
    .as_ref()
}

#[test]
fn invariants_hold() {
    if let Some(m) = parsed() {
        common::check_invariants(m);
    }
}

#[test]
fn match_facts() {
    let Some(m) = parsed() else { return };
    assert_eq!((m.map.as_str(), m.tick_rate, m.players.len(), m.rounds.len()), ("de_ancient", 64, 10, 24));
    assert!(m.rounds.iter().all(|r| r.winner.is_some() && r.reason != 0), "every round has a winner and a reason");

    // Score of the team that started on T: players with T in round 1.
    let first_t: Vec<usize> = (0..10).filter(|&p| m.rounds[0].teams[p] == Some(Team::T)).collect();
    assert_eq!(first_t.len(), 5);
    let mut score = (0, 0);
    for r in &m.rounds {
        let side = r.teams[first_t[0]].unwrap();
        // The five stay together on one side in every round.
        assert!(first_t.iter().all(|&p| r.teams[p] == Some(side)));
        if r.winner == Some(side) { score.0 += 1 } else { score.1 += 1 }
    }
    assert_eq!(score, (11, 13));
    // Sides swap at the half (MR12): round 12 vs round 13.
    assert_ne!(m.rounds[11].teams[first_t[0]], m.rounds[12].teams[first_t[0]]);
    assert_eq!(m.rounds[0].teams[first_t[0]], m.rounds[11].teams[first_t[0]]);
}

#[test]
fn totals_are_stable() {
    // Regression anchors taken from a run that passed the cross-checks above; they are our own
    // output (no external source), so a change here means behaviour changed, not necessarily a bug.
    let Some(m) = parsed() else { return };
    let kills: u32 = m.stats.per_player.iter().map(|p| p.kills).sum();
    let deaths: u32 = m.stats.per_player.iter().map(|p| p.deaths).sum();
    assert_eq!((kills, deaths), (154, 156));
    let count = |k| m.rounds.iter().flat_map(|r| &r.grenades).filter(|g| g.kind == k).count();
    assert_eq!((count(GrenadeKind::Smoke), count(GrenadeKind::Flash), count(GrenadeKind::He), count(GrenadeKind::Fire)), (170, 156, 99, 135));
    let ends = |e| m.rounds.iter().filter(|r| r.bomb.end_type == Some(e)).count();
    assert_eq!((ends(BombEnd::Exploded), ends(BombEnd::Defused)), (8, 3));
}

//! Invariants on a real demo. Skipped when `samples/test_demo.dem` is absent (demos are not in git).

use std::path::PathBuf;

fn demo() -> Option<PathBuf> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../samples/test_demo.dem");
    p.exists().then_some(p)
}

#[test]
fn snapshots_are_consistent() {
    let Some(path) = demo() else {
        eprintln!("skipped: no samples/test_demo.dem");
        return;
    };
    let m = parser_core::parse_demo::parse(&path).unwrap();
    assert_eq!(m.map, "de_mirage");
    assert_eq!(m.tick_rate, 64);
    assert_eq!(m.players.len(), 10);
    assert_eq!(m.rounds.len(), 10);
    assert!(!m.weapons.is_empty());

    for r in &m.rounds {
        let n = r.ticks.len();
        assert!(n > 1 && r.ticks.windows(2).all(|w| w[1] - w[0] == 4), "round {}", r.number);
        assert_eq!(r.tracks.len(), m.players.len());
        assert_eq!(r.teams.len(), m.players.len());
        for t in &r.tracks {
            for len in [t.x.len(), t.y.len(), t.z.len(), t.yaw.len(), t.pitch.len(), t.hp.len(), t.money.len()] {
                assert_eq!(len, n);
            }
            assert!(t.hp.iter().all(|h| (0..=100).contains(h)));
            assert!(t.yaw.iter().all(|a| (-1800..=1800).contains(a)));
            assert!(t.weapon.iter().all(|(i, w)| (*i as usize) < n && (*w as usize) < m.weapons.len()));
            assert!(t.inventory.iter().all(|c| (c.0 as usize) < n && c.3.iter().all(|&i| (i as usize) < m.weapons.len())));
        }
        // Sides: five on each team in every round of this 5v5 (the last round has a leaver).
        if r.number < 10 {
            assert_eq!(r.teams.iter().filter(|t| **t == Some(parser_core::models::Team::T)).count(), 5);
        }
    }
}

#[test]
fn events_and_bomb_are_consistent() {
    let Some(path) = demo() else { return };
    let m = parser_core::parse_demo::parse(&path).unwrap();
    let np = m.players.len() as u32;
    let mut kills = 0;
    for r in &m.rounds {
        let n = r.ticks.len();
        kills += r.events.kills.len();
        for k in &r.events.kills {
            assert!(k.0 >= r.start_tick && k.2 < np && k.1 < np as i32 && (k.4 as usize) < m.weapons.len());
        }
        assert!(r.events.kills.iter().all(|k| k.1 != k.2 as i32 || m.weapons[k.4 as usize] != "world"), "no world suicides");
        let b = &r.bomb;
        assert!(b.track_x.len() <= n && b.track_x.len() == b.track_y.len() && b.track_x.len() == b.track_z.len());
        assert!(b.carrier.iter().all(|c| (c.0 as usize) < n.max(1)));
        if let (Some(p), Some(e)) = (b.plant_tick, b.end_tick) {
            assert!(p < e);
            assert!(b.end_type.is_some());
        }
        assert_eq!(b.defuser.is_some(), b.end_type == Some(parser_core::models::BombEnd::Defused));
    }
    assert!(kills > 50);
    // Round 2 ends with a defuse, round 6 with an explosion (see round_end reasons 7 and 1).
    assert_eq!(m.rounds[1].bomb.end_type, Some(parser_core::models::BombEnd::Defused));
    assert_eq!(m.rounds[5].bomb.end_type, Some(parser_core::models::BombEnd::Exploded));
}

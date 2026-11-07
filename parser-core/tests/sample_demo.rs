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
    let m = parser_core::parse_file(&path, None).unwrap();
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
    let m = parser_core::parse_file(&path, None).unwrap();
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

#[test]
fn grenades_match_the_demo_events() {
    use parser_core::models::GrenadeKind::*;
    let Some(path) = demo() else { return };
    let m = parser_core::parse_file(&path, None).unwrap();
    let all: Vec<_> = m.rounds.iter().flat_map(|r| r.grenades.iter()).collect();
    let count = |k| all.iter().filter(|g| g.kind == k).count();
    // Detonation event counts in this demo: 40 smokes, 48 flashes, 34 HE, 25 fires.
    assert_eq!((count(Smoke), count(Flash), count(He)), (40, 48, 34));
    assert_eq!(all.iter().filter(|g| g.area.is_some()).count(), 40 + 25);
    for g in &all {
        let n = g.ticks.len();
        assert!(n >= 2 && g.x.len() == n && g.y.len() == n && g.z.len() == n);
        assert_eq!(g.ticks[0], g.throw_tick);
        assert_eq!(*g.ticks.last().unwrap(), g.end_tick);
        assert!(g.ticks.windows(2).all(|w| w[1] > w[0]));
        // One point per tick: contiguous apart from at most a gap before the exact end point.
        assert!(g.ticks.windows(2).all(|w| w[1] - w[0] <= 3));
        if let Some(a) = &g.area {
            assert_eq!(a.start_tick, g.end_tick);
            assert!(a.end_tick.map_or(true, |e| e > a.start_tick));
        }
    }
}

#[test]
fn stats_are_consistent_with_events() {
    let Some(path) = demo() else { return };
    let m = parser_core::parse_file(&path, None).unwrap();
    let s = &m.stats.per_player;
    assert_eq!(s.len(), m.players.len());
    let kill_events: usize = m.rounds.iter().map(|r| r.events.kills.len()).sum();
    // No team kills or suicides in this demo, so every kill event is a kill and a death.
    assert_eq!(s.iter().map(|p| p.kills).sum::<u32>() as usize, kill_events);
    assert_eq!(s.iter().map(|p| p.deaths).sum::<u32>() as usize, kill_events);
    for (i, p) in s.iter().enumerate() {
        assert_eq!(p.player as usize, i);
        assert!(p.rounds >= 9 && p.rounds <= 10);
        assert!((0.0..=100.0).contains(&p.kast_percent) && (0.0..=100.0).contains(&p.hs_percent));
        assert!(p.headshots <= p.kills);
        // Totals equal the sum of the per-round summaries.
        let sum = |f: fn(&parser_core::models::PlayerRound) -> u32| m.rounds.iter().map(|r| f(&r.summary[i])).sum::<u32>();
        assert_eq!((sum(|x| x.kills), sum(|x| x.deaths), sum(|x| x.damage)), (p.kills, p.deaths, p.damage));
        assert!((p.adr - p.damage as f64 / p.rounds as f64).abs() < 0.06);
    }
}

#[test]
fn bad_input_is_an_error_not_a_panic() {
    assert!(parser_core::parse_bytes(b"", None).is_err());
    assert!(parser_core::parse_bytes(&[0u8; 4096], None).is_err());
    assert!(parser_core::parse_bytes(b"PBDEMS2\0 definitely not a demo", None).is_err()); // passes the magic, fails inside the core
    assert!(parser_core::parse_file(std::path::Path::new("/nonexistent/x.dem"), None).is_err());
    // A demo cut in the middle must not panic either (it may fail or return what was read).
    if let Some(path) = demo() {
        let data = std::fs::read(path).unwrap();
        let cut = &data[..data.len() / 3];
        let _ = parser_core::parse_bytes(cut, None); // Ok or Err, but returns
    }
}

#[test]
fn output_matches_the_reference_json() {
    use std::io::Read;
    let Some(path) = demo() else { return };
    let reference = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../samples/test_demo.expected.json.gz");
    let mut expected = String::new();
    flate2::read::GzDecoder::new(std::fs::File::open(reference).unwrap()).read_to_string(&mut expected).unwrap();

    let mut steps = Vec::new();
    let mut progress = |p: parser_core::Progress| steps.push(p);
    let m = parser_core::parse_file(&path, Some(&mut progress)).unwrap();
    let json = parser_core::to_json(&m).unwrap();
    // Regenerate with: cargo run --release -p parser-core --example parse -- samples/test_demo.dem samples/test_demo.expected.json.gz
    assert!(json == expected, "output differs from samples/test_demo.expected.json.gz (len {} vs {}); regenerate it if the change is intended", json.len(), expected.len());

    assert_eq!(steps.first().map(|p| p.percent), Some(0));
    assert_eq!(steps.last().map(|p| (p.stage, p.percent)), Some((parser_core::Stage::Done, 100)));
    assert!(steps.windows(2).all(|w| w[0].percent <= w[1].percent), "progress is monotonic");
    // The JSON parses back into the same model.
    let back: parser_core::Match = serde_json::from_str(&json).unwrap();
    assert_eq!(back.rounds.len(), m.rounds.len());
}

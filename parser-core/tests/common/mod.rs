//! Invariants every parsed demo must satisfy, shared by the sample and full-match tests.
#![allow(dead_code)]

use parser_core::models::{BombEnd, Match, Team};

pub fn check_invariants(m: &Match) {
    let np = m.players.len();
    let nw = m.weapons.len() as u32;
    let valid = |p: i32| (-1..np as i32).contains(&p);
    assert!(m.weapons.iter().all(|w| !w.is_empty()), "empty weapon name");

    for (i, r) in m.rounds.iter().enumerate() {
        let n = r.ticks.len();
        let next = m.rounds.get(i + 1).map_or(r.end_tick, |x| x.start_tick);
        let ctx = format!("round {}", r.number);

        // Snapshots.
        assert_eq!(r.number as usize, i + 1);
        assert_eq!(r.ticks[0], r.start_tick, "{ctx}");
        assert!(r.ticks.windows(2).all(|w| w[1] - w[0] == 4), "{ctx}: step 4");
        assert_eq!((r.tracks.len(), r.teams.len(), r.summary.len()), (np, np, np), "{ctx}");
        for t in &r.tracks {
            for len in [t.x.len(), t.y.len(), t.z.len(), t.yaw.len(), t.pitch.len(), t.hp.len(), t.money.len()] {
                assert_eq!(len, n, "{ctx}: array length");
            }
            assert!(t.hp.iter().all(|h| (0..=100).contains(h)), "{ctx}: hp");
            assert!(t.yaw.iter().all(|a| (-1800..=1800).contains(a)), "{ctx}: yaw");
            assert!(t.pitch.iter().all(|a| (-900..=900).contains(a)), "{ctx}: pitch");
            assert!(t.money.iter().all(|x| (0..=65535).contains(x)), "{ctx}: money");
            assert!(t.x.iter().chain(&t.y).chain(&t.z).all(|c| c.abs() < 327_680), "{ctx}: position");
            assert!(t.weapon.iter().all(|(s, w)| (*s as usize) < n && *w < nw), "{ctx}: weapon refs");
            assert!(t.inventory.iter().all(|c| (c.0 as usize) < n && c.3.iter().all(|&w| w < nw)), "{ctx}: inventory refs");
            // Nobody respawns inside a round.
            if let Some(d) = t.hp.iter().skip(1).position(|&h| h == 0) {
                assert!(t.hp[d + 1..].iter().all(|&h| h == 0), "{ctx}: respawn");
            }
        }

        // Events.
        let mut dead = std::collections::HashSet::new();
        for k in &r.events.kills {
            assert!(valid(k.1) && valid(k.3) && (k.2 as usize) < np && k.4 < nw && k.0 >= r.start_tick, "{ctx}: kill {k:?}");
            assert!(dead.insert(k.2), "{ctx}: player {} died twice", k.2);
        }
        for d in &r.events.damage {
            assert!(valid(d.1) && (d.2 as usize) < np && d.3 < nw && d.6 <= 100, "{ctx}: damage {d:?}");
        }
        assert!(r.events.blinds.iter().all(|b| valid(b.1) && (b.2 as usize) < np));

        // Bomb: a plant may come after the round was already won, but never after the next round starts.
        let b = &r.bomb;
        assert!(b.plant_tick.map_or(true, |t| t >= r.start_tick && t <= next.max(r.end_tick)), "{ctx}: plant tick");
        assert!(b.planter.map_or(true, |p| (p as usize) < np) && b.defuser.map_or(true, |p| (p as usize) < np));
        assert!(b.carrier.iter().all(|c| valid(c.1) && (c.0 as usize) < n.max(1)), "{ctx}: carrier");
        assert!(b.track_x.len() <= n && b.track_x.len() == b.track_y.len() && b.track_x.len() == b.track_z.len());
        assert_eq!(b.defuser.is_some(), b.end_type == Some(BombEnd::Defused), "{ctx}");
        match r.reason {
            1 => assert_eq!(b.end_type, Some(BombEnd::Exploded), "{ctx}: reason 1 = explosion"),
            7 => assert_eq!(b.end_type, Some(BombEnd::Defused), "{ctx}: reason 7 = defuse"),
            _ => assert!(b.end_type.is_none() || r.reason != 8 && r.reason != 9 || b.plant_tick.is_some(), "{ctx}"),
        }

        // Grenades.
        for g in &r.grenades {
            let len = g.ticks.len();
            assert!(len >= 1 && g.x.len() == len && g.y.len() == len && g.z.len() == len, "{ctx}: grenade arrays");
            assert!((g.thrower as usize) < np && g.throw_tick >= r.start_tick && g.throw_tick <= next.max(r.end_tick));
            assert_eq!((g.ticks[0], *g.ticks.last().unwrap()), (g.throw_tick, g.end_tick), "{ctx}: grenade ends");
            assert!(g.ticks.windows(2).all(|w| w[1] > w[0]), "{ctx}: grenade ticks increase");
            if let Some(a) = &g.area {
                assert_eq!(a.start_tick, g.end_tick);
                assert!(a.end_tick.map_or(true, |e| e > a.start_tick));
            }
        }

        // Elimination rounds: the losing side has nobody alive at the end (the final kill may land
        // exactly on the round_end tick, which the last snapshot cannot show, hence "at most one").
        if let (8 | 9, Some(w)) = (r.reason, r.winner) {
            let loser = if w == Team::T { Team::CT } else { Team::T };
            let alive = (0..np).filter(|&p| r.teams[p] == Some(loser) && *r.tracks[p].hp.last().unwrap() > 0).count();
            assert!(alive <= 1, "{ctx}: {alive} players of the eliminated side alive at the end");
        }
    }

    // Statistics agree with the per-round summaries and the events.
    let s = &m.stats.per_player;
    assert_eq!(s.len(), np);
    let kill_events: usize = m.rounds.iter().map(|r| r.events.kills.len()).sum();
    assert_eq!(s.iter().map(|p| p.deaths).sum::<u32>() as usize, kill_events, "every kill event is a death");
    assert!(s.iter().map(|p| p.kills + p.team_kills).sum::<u32>() as usize <= kill_events);
    for (i, p) in s.iter().enumerate() {
        assert_eq!(p.player as usize, i);
        assert!((0.0..=100.0).contains(&p.kast_percent) && (0.0..=100.0).contains(&p.hs_percent));
        assert!(p.headshots <= p.kills);
        let sum = |f: fn(&parser_core::models::PlayerRound) -> u32| m.rounds.iter().map(|r| f(&r.summary[i])).sum::<u32>();
        assert_eq!((sum(|x| x.kills), sum(|x| x.deaths), sum(|x| x.damage), sum(|x| x.assists)), (p.kills, p.deaths, p.damage, p.assists));
    }
}

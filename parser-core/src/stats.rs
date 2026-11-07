//! Per-player statistics from the round events. Pure functions over `Round`s; needs
//! `teams` and `events` to be filled already.

use crate::models::*;

/// A teammate killing the killer within this many seconds trades the death (KAST).
const TRADE_SECONDS: i32 = 5;

fn rounded(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

/// Fills `Round::summary` and returns the match totals. `players` is the player count.
pub fn fill_stats(rounds: &mut [Round], players: usize, tick_rate: u32) -> Stats {
    let mut totals: Vec<PlayerStats> = (0..players)
        .map(|p| PlayerStats {
            player: p as u32,
            rounds: 0,
            kills: 0,
            deaths: 0,
            assists: 0,
            team_kills: 0,
            damage: 0,
            headshots: 0,
            hs_percent: 0.0,
            adr: 0.0,
            kast_percent: 0.0,
            flash_assists: 0,
            enemies_blinded: 0,
            enemy_blind_ms: 0,
        })
        .collect();
    let mut kast_rounds = vec![0u32; players];

    for round in rounds.iter_mut() {
        let mut s = vec![PlayerRound::default(); players];
        let team = |p: usize| round.teams.get(p).copied().flatten();
        // Unknown sides count as enemies so nothing is silently dropped.
        let same_team = |a: usize, b: usize| matches!((team(a), team(b)), (Some(x), Some(y)) if x == y);
        let mut team_kills = vec![0u32; players];
        let (mut blinded, mut blind_ms) = (vec![0u32; players], vec![0u64; players]);

        // Damage, capped by the victim's hp before the hit (hp starts at 100 each round).
        let mut hp = vec![100u32; players];
        for d in &round.events.damage {
            let (a, v) = (d.1, d.2 as usize);
            let before = hp[v];
            hp[v] = d.6;
            if a < 0 || a as usize == v || same_team(a as usize, v) {
                continue;
            }
            s[a as usize].damage += d.4.min(before);
        }

        // Kills, deaths, assists and the data for trades.
        let mut deaths: Vec<(i32, usize, i32)> = Vec::new(); // (tick, victim, killer)
        for k in &round.events.kills {
            let (killer, v, assister) = (k.1, k.2 as usize, k.3);
            s[v].deaths += 1;
            deaths.push((k.0, v, killer));
            if killer >= 0 && killer as usize != v {
                let ki = killer as usize;
                if same_team(ki, v) {
                    team_kills[ki] += 1;
                } else {
                    s[ki].kills += 1;
                    if k.5 & KILL_HEADSHOT != 0 {
                        s[ki].headshots += 1;
                    }
                }
            }
            if assister >= 0 && assister as usize != v && !same_team(assister as usize, v) {
                s[assister as usize].assists += 1;
                if k.5 & KILL_FLASH_ASSIST != 0 {
                    s[assister as usize].flash_assists += 1;
                }
            }
        }

        for b in &round.events.blinds {
            let (t, v) = (b.1, b.2 as usize);
            if t >= 0 && t as usize != v && !same_team(t as usize, v) {
                blinded[t as usize] += 1;
                blind_ms[t as usize] += b.3 as u64;
            }
        }

        // KAST: kill, assist, survived, or traded.
        let window = TRADE_SECONDS * tick_rate as i32;
        let traded = |victim: usize, tick: i32, killer: i32| -> bool {
            killer >= 0
                && deaths.iter().any(|&(t, dv, dk)| {
                    t >= tick && t - tick <= window && dv == killer as usize && dk >= 0 && dk as usize != victim && same_team(dk as usize, victim)
                })
        };
        for p in 0..players {
            if team(p).is_none() {
                continue;
            }
            let died = deaths.iter().find(|d| d.1 == p);
            s[p].kast = s[p].kills > 0
                || s[p].assists > 0
                || died.is_none()
                || died.map_or(false, |&(t, _, k)| traded(p, t, k));
            totals[p].rounds += 1;
            kast_rounds[p] += s[p].kast as u32;
        }

        for p in 0..players {
            let t = &mut totals[p];
            t.kills += s[p].kills;
            t.deaths += s[p].deaths;
            t.assists += s[p].assists;
            t.team_kills += team_kills[p];
            t.damage += s[p].damage;
            t.headshots += s[p].headshots;
            t.flash_assists += s[p].flash_assists;
            t.enemies_blinded += blinded[p];
            t.enemy_blind_ms += blind_ms[p];
        }
        round.summary = s;
    }

    for (p, t) in totals.iter_mut().enumerate() {
        if t.kills > 0 {
            t.hs_percent = rounded(t.headshots as f64 / t.kills as f64 * 100.0);
        }
        if t.rounds > 0 {
            t.adr = rounded(t.damage as f64 / t.rounds as f64);
            t.kast_percent = rounded(kast_rounds[p] as f64 / t.rounds as f64 * 100.0);
        }
    }
    Stats { per_player: totals }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::{build_rounds, RoundEvent};

    // Players 0,1 are CT; 2,3 are T.
    fn round(kills: Vec<Kill>, damage: Vec<Damage>, blinds: Vec<Blind>) -> Round {
        let mut r = build_rounds(&[RoundEvent::Start(0), RoundEvent::End { tick: 2000, winner: 3, reason: 8 }]).remove(0);
        r.teams = vec![Some(Team::CT), Some(Team::CT), Some(Team::T), Some(Team::T)];
        r.events = RoundEvents { kills, damage, blinds };
        r
    }

    #[test]
    fn counts_caps_damage_and_kast() {
        let k = |t, killer, victim, assister, flags| Kill(t, killer, victim, assister, 0, flags);
        let mut rounds = vec![round(
            vec![
                k(100, 0, 2, 1, KILL_HEADSHOT | KILL_FLASH_ASSIST), // 0 kills 2 (hs), 1 flash-assists
                k(200, 0, 1, -1, 0),                                 // teamkill: no kill
                k(300, 0, 3, -1, 0),                                 // 0 kills 3
                k(500, -1, 0, -1, 0),                                // world death of 0
            ],
            vec![
                Damage(90, 0, 2, 0, 70, 0, 30),
                Damage(100, 0, 2, 0, 80, 0, 0), // raw 80 but only 30 left -> 30
                Damage(150, 3, 1, 0, 40, 0, 60),
                Damage(160, 0, 1, 0, 25, 0, 35), // friendly fire: ignored
                Damage(170, 3, 3, 0, 10, 0, 90), // self damage: ignored
            ],
            vec![Blind(80, 1, 2, 1500), Blind(81, 1, 0, 900) /* teammate */],
        )];
        let st = fill_stats(&mut rounds, 4, 64);
        let (p0, p1, p2, p3) = (&st.per_player[0], &st.per_player[1], &st.per_player[2], &st.per_player[3]);
        assert_eq!((p0.kills, p0.headshots, p0.team_kills, p0.deaths), (2, 1, 1, 1));
        assert_eq!(p0.damage, 70 + 30); // 80 raw, capped to the 30 hp left
        assert_eq!((p0.hs_percent, p0.adr), (50.0, 100.0));
        assert_eq!((p1.assists, p1.flash_assists, p1.deaths), (1, 1, 1));
        assert_eq!((p1.enemies_blinded, p1.enemy_blind_ms), (1, 1500)); // teammate flash ignored
        assert_eq!((p3.kills, p3.damage, p3.deaths), (0, 40, 1));
        assert_eq!(p2.deaths, 1);
        let s = &rounds[0].summary;
        assert!(s[0].kast, "has kills");
        assert!(s[1].kast, "has an assist");
        assert!(!s[2].kast, "died, no kill or assist, and the killer was not avenged");
        assert!(!s[3].kast, "same for p3: the killer died to the world, not to a teammate");
        assert_eq!(st.per_player[0].kast_percent, 100.0);
        assert_eq!(st.per_player[2].kast_percent, 0.0);
    }

    #[test]
    fn trade_and_survival() {
        let k = |t, killer, victim| Kill(t, killer, victim, -1, 0, 0);
        // p2 (T) kills p0 (CT) at 100; p1 (CT) kills p2 at 200 (<5 s): p0 traded. p3 survives. p1 killed.
        let mut rounds = vec![round(vec![k(100, 2, 0), k(200, 1, 2), k(2000, 3, 1)], vec![], vec![])];
        // The late kill (tick 2000) is more than 5 s after... irrelevant for p1 (p1 has a kill).
        fill_stats(&mut rounds, 4, 64);
        let s = &rounds[0].summary;
        assert!(s[0].kast, "traded");
        assert!(s[1].kast, "got a kill");
        assert!(s[3].kast, "survived and killed");
        // Without the trade window: killer dies 10 s later.
        let mut late = vec![round(vec![k(100, 2, 0), k(100 + 10 * 64, 1, 2)], vec![], vec![])];
        fill_stats(&mut late, 4, 64);
        assert!(!late[0].summary[0].kast, "avenged too late");
    }
}

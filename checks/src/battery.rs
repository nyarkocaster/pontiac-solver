#[path = "../../app/src-tauri/src/study_core.rs"]
mod study_core;
use study_core::*;
use std::io::Write;

struct Spot { name: &'static str, oop: &'static str, ip: &'static str, board: &'static str, pot: i32, stack: i32, hero: usize, flop_oop: &'static str, flop_ip: &'static str, turn: &'static str, river: &'static str }

fn aggr(v: &NodeView, bucket: Option<usize>) -> f64 {
    let fr = match bucket { Some(b) => &v.buckets[b].freqs, None => &v.action_freqs };
    v.actions.iter().zip(fr).filter(|(a, _)| a.starts_with("Bet") || a.starts_with("Raise") || a.starts_with("All-in")).map(|(_, f)| *f).sum()
}
fn fold(v: &NodeView) -> f64 { if v.actions.first().map(|a| a == "Fold").unwrap_or(false) { v.action_freqs[0] } else { f64::NAN } }

fn main() {
    let rec_bb = "AA:0.25,KK:0.5,QQ-22,AKs:0.5,AQs-A2s,AKo:0.5,AQo-A7o,KQs-K5s,KQo-K9o,QJs-Q7s,QJo-Q9o,JTs-J7s,JTo-J9o,T9s-T7s,T9o,98s-96s,98o,87s-85s,76s-74s,65s-63s,54s-53s,43s";
    let rec_cc = "AA:0.25,KK:0.5,QQ-22,AKs:0.5,AQs-A2s,AKo:0.5,AQo-ATo,KQs-K9s,KQo-KTo,QJs-Q9s,QJo-QTo,JTs-J9s,JTo,T9s-T8s,98s-97s,87s-86s,76s-75s,65s-64s,54s";
    let spots = [
        Spot { name: "BTN open vs rec BB, K72r (hero IP)", oop: rec_bb, ip: "22+,A2s+,A9o+,K8s+,KJo+,Q9s+,QJo,J9s+,T8s+,97s+,87s,76s", board: "Ks7h2d", pot: 31, stack: 185, hero: 1, flop_oop: "33%", flop_ip: "33%, 66%", turn: "75%", river: "75%" },
        Spot { name: "CO open vs rec cold-call, T98 two-tone (hero OOP)", oop: "22+,A4s+,ATo+,K9s+,KJo+,QTs+,QJo,J9s+,T9s,98s,87s", ip: rec_cc, board: "Th9h8c", pot: 33, stack: 185, hero: 0, flop_oop: "33%, 66%", flop_ip: "33%", turn: "75%", river: "75%" },
        Spot { name: "HU SNG 15bb, J83r (hero = button IP)", oop: "99-22,AJs-A2s,K2s+,Q2s+,J4s+,T6s+,96s+,85s+,74s+,64s+,53s+,43s,ATo-A2o,K6o+,Q8o+,J8o+,T8o+,98o,87o", ip: "22+,A2s+,K2s+,Q4s+,J6s+,T6s+,96s+,85s+,75s+,64s+,54s,A2o+,K5o+,Q8o+,J8o+,T8o+,98o", board: "Jh8d3c", pot: 40, stack: 130, hero: 1, flop_oop: "33%", flop_ip: "33%", turn: "50%", river: "75%" },
    ];
    let effects = ["fold_less", "fold_more", "raise_less", "bluff_less", "bet_more", "bet_less"];
    let only: Option<usize> = std::env::args().nth(1).and_then(|x| x.parse().ok());
    let mut log = std::fs::OpenOptions::new().create(true).append(true).open("battery.log").unwrap();
    for (si, sp) in spots.iter().enumerate() {
        if let Some(o) = only { if o != si { continue; } }
        let vill = 1 - sp.hero;
        for eff in effects {
            let spec = StudySpec {
                oop_range: sp.oop.into(), ip_range: sp.ip.into(), board: sp.board.into(), pot: sp.pot, stack: sp.stack,
                oop_flop_bet: sp.flop_oop.into(), ip_flop_bet: sp.flop_ip.into(), turn_bet: sp.turn.into(), river_bet: sp.river.into(),
                raise: "3x".into(), allin_threshold: 1.5, accuracy_pct: 0.75, max_iter: 600,
                leaks: vec![Leak { player: vill, street: "all".into(), effect: eff.into(), strength: 0.6 }], threads: 2, low_memory: true, freeze_rest: std::env::var("NOFREEZE").is_err(), confidence: std::env::var("CONF").ok().and_then(|x| x.parse().ok()).unwrap_or(1.0),
            };
            let t = std::time::Instant::now();
            let out = match solve_inner(&spec, &StudyProgress::default()) { Ok(o) => o, Err(e) => { writeln!(log, "{} | {eff} | ERROR {e}", sp.name).unwrap(); continue; } };
            let s = out.summary.clone();
            let mut st = StudyState::default();
            st.install(out);
            let tol = sp.pot as f64 * 0.01;
            let gain = s.locked_ev[sp.hero] - s.base_ev[sp.hero];
            let inv = if gain >= -tol { "PASS" } else { "FAIL" };
            // hero's first decision: root if hero is OOP, else after villain checks
            let h0: Vec<usize> = if sp.hero == 0 { vec![] } else { vec![0] };
            let (b0, l0) = (st.view(false, &h0).unwrap(), st.view(true, &h0).unwrap());
            // hero facing a villain bet: villain bets at its first decision
            let hb: Vec<usize> = if sp.hero == 0 { vec![0, 1] } else { vec![1] };
            let (bb, lb) = (st.view(false, &hb).unwrap(), st.view(true, &hb).unwrap());
            let line = format!(
                "{} | {eff} | EV hero {:.2} -> {:.2} (gain {:+.2}) invariant {inv} | hero first decision aggr {:.0}%->{:.0}% strong {:.0}->{:.0} medium {:.0}->{:.0} weak {:.0}->{:.0} | hero vs villain bet: fold {:.0}%->{:.0}% raise {:.0}%->{:.0}% | locked nodes {} | {:.0}s",
                sp.name, s.base_ev[sp.hero], s.locked_ev[sp.hero], gain,
                100.0*aggr(&b0,None), 100.0*aggr(&l0,None), 100.0*aggr(&b0,Some(0)), 100.0*aggr(&l0,Some(0)), 100.0*aggr(&b0,Some(1)), 100.0*aggr(&l0,Some(1)), 100.0*aggr(&b0,Some(2)), 100.0*aggr(&l0,Some(2)),
                100.0*fold(&bb), 100.0*fold(&lb), 100.0*aggr(&bb,None), 100.0*aggr(&lb,None), s.locked_nodes, t.elapsed().as_secs_f64());
            println!("{line}");
            writeln!(log, "{line}").unwrap();
            st.reset();
        }
    }
}

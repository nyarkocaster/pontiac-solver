#[path = "../../app/src-tauri/src/study_core.rs"]
mod study_core;
use study_core::*;

fn show(st: &mut StudyState, locked: bool, h: &[usize], tag: &str) {
    let v = st.view(locked, h).unwrap();
    println!("[{tag}] node {:?} player {} actions {:?}", h, v.player, v.actions);
    println!("   overall {:?}", v.action_freqs.iter().map(|x| (x * 100.0).round()).collect::<Vec<_>>());
    for b in &v.buckets {
        println!("   {:22} share {:3.0}% -> {:?}", b.name, b.share * 100.0, b.freqs.iter().map(|x| (x * 100.0).round()).collect::<Vec<_>>());
    }
}

fn main() {
    let case = std::env::args().nth(1).unwrap_or_default();
    let acc: f32 = std::env::args().nth(2).and_then(|x| x.parse().ok()).unwrap_or(0.75);
    let rec_cc = "AA:0.25,KK:0.5,QQ-22,AKs:0.5,AQs-A2s,AKo:0.5,AQo-ATo,KQs-K9s,KQo-KTo,QJs-Q9s,QJo-QTo,JTs-J9s,JTo,T9s-T8s,98s-97s,87s-86s,76s-75s,65s-64s,54s";
    let (spec, nodes): (StudySpec, Vec<(Vec<usize>, &str)>) = match case.as_str() {
        "t98_bluffless" => (StudySpec {
            oop_range: "22+,A4s+,ATo+,K9s+,KJo+,QTs+,QJo,J9s+,T9s,98s,87s".into(), ip_range: rec_cc.into(), board: "Th9h8c".into(),
            pot: 33, stack: 185, oop_flop_bet: "33%, 66%".into(), ip_flop_bet: "33%".into(), turn_bet: "75%".into(), river_bet: "75%".into(),
            raise: "3x".into(), allin_threshold: 1.5, accuracy_pct: acc, max_iter: 2000,
            leaks: vec![Leak { player: 1, street: "all".into(), effect: "bluff_less".into(), strength: 0.6 }], threads: 2, low_memory: true, freeze_rest: std::env::var("NOFREEZE").is_err(), confidence: std::env::var("CONF").ok().and_then(|x| x.parse().ok()).unwrap_or(1.0),
        }, vec![(vec![0], "villain after hero check"), (vec![0, 1], "hero facing villain bet")]),
        _ => (StudySpec {
            oop_range: "99-22,AJs-A2s,K2s+,Q2s+,J4s+,T6s+,96s+,85s+,74s+,64s+,53s+,43s,ATo-A2o,K6o+,Q8o+,J8o+,T8o+,98o,87o".into(),
            ip_range: "22+,A2s+,K2s+,Q4s+,J6s+,T6s+,96s+,85s+,75s+,64s+,54s,A2o+,K5o+,Q8o+,J8o+,T8o+,98o".into(), board: "Jh8d3c".into(),
            pot: 40, stack: 130, oop_flop_bet: "33%".into(), ip_flop_bet: std::env::var("FLOP").unwrap_or("33%".into()), turn_bet: "50%".into(), river_bet: "75%".into(),
            raise: "3x".into(), allin_threshold: 1.5, accuracy_pct: acc, max_iter: 2000,
            leaks: vec![Leak { player: 0, street: "all".into(), effect: "fold_less".into(), strength: 0.6 }], threads: 2, low_memory: true, freeze_rest: std::env::var("NOFREEZE").is_err(), confidence: std::env::var("CONF").ok().and_then(|x| x.parse().ok()).unwrap_or(1.0),
        }, vec![(vec![], "villain root"), (vec![0], "hero after villain check"), (vec![0, 1], "villain facing hero bet")]),
    };
    let out = solve_inner(&spec, &StudyProgress::default()).unwrap();
    let s = out.summary.clone();
    println!("case {case} acc {acc}: base expl {:.2}% locked expl {:.2}% EV base {:?} locked {:?} nodes {}", s.base_expl_pct, s.locked_expl_pct, s.base_ev, s.locked_ev, s.locked_nodes);
    let mut st = StudyState::default();
    st.install(out);
    for (h, tag) in &nodes {
        show(&mut st, false, h, &format!("BASE   {tag}"));
        show(&mut st, true, h, &format!("LOCKED {tag}"));
    }
}

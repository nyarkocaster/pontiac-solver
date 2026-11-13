#[path = "../../app/src-tauri/src/study_core.rs"]
mod study_core;
use study_core::*;
fn main() {
    let leak = std::env::args().nth(1).unwrap_or_default();
    let board = std::env::args().nth(2).unwrap_or("Ks7h2d".into());
    let leaks = if leak.is_empty() { vec![] } else { leak.split(',').map(|e| Leak{player:0, street:"all".into(), effect:e.into(), strength:0.6}).collect() };
    let mut spec = StudySpec {
        oop_range: "AA:0.25,KK:0.5,QQ-22,AKs:0.5,AQs-A2s,AKo:0.5,AQo-A7o,KQs-K5s,KQo-K9o,QJs-Q7s,QJo-Q9o,JTs-J7s,JTo-J9o,T9s-T7s,T9o,98s-96s,98o,87s-85s,76s-74s,65s-63s,54s-53s,43s".into(),
        ip_range: "22+,A2s+,A9o+,K8s+,KJo+,Q9s+,QJo,J9s+,T8s+,97s+,87s,76s".into(),
        board, pot: 31, stack: 185,
        oop_flop_bet: "33%".into(), ip_flop_bet: "33%, 66%".into(), turn_bet: "75%".into(), river_bet: "75%".into(),
        raise: "100%".into(), allin_threshold: 0.0, accuracy_pct: 0.5, max_iter: 1000, leaks, threads: 2, low_memory: false, freeze_rest: true, confidence: 1.0,
    };
    println!("estimate MB: {:?}", memory_estimate(&spec).unwrap());
    // HU_MODE

    if let Ok(st) = std::env::var("HU_STACK") {
        spec.oop_range = "99-22,AJs-A2s,K2s+,Q2s+,J4s+,T6s+,96s+,85s+,74s+,64s+,53s+,43s,ATo-A2o,K6o+,Q8o+,J8o+,T8o+,98o,87o".into();
        spec.ip_range = "22+,A2s+,K2s+,Q4s+,J6s+,T6s+,96s+,85s+,75s+,64s+,54s,A2o+,K5o+,Q8o+,J8o+,T8o+,98o".into();
        spec.pot = 40; spec.stack = st.parse::<i32>().unwrap() * 10 - 20;
        spec.oop_flop_bet = "33%".into(); spec.ip_flop_bet = "33%".into(); spec.turn_bet = "50%".into(); spec.river_bet = "75%".into();
        spec.raise = "3x".into(); spec.allin_threshold = 1.5; spec.accuracy_pct = 1.0; spec.max_iter = 300;
        println!("HU estimate MB: {:?}", memory_estimate(&spec).unwrap());
    }
    let prog = StudyProgress::default();
    spec.low_memory = std::env::var("LOWMEM").is_ok();
    let out = solve_inner(&spec, &prog).unwrap();
    println!("{:?}", out.summary);
    let mut st = StudyState::default();
    st.install(out);
    let mut show = |locked: bool, h: &[usize], tag: &str| {
        let t = std::time::Instant::now();
        let v = st.view(locked, h).unwrap();
        println!("[{tag}] ({:.1}s) {:?} player {} actions {:?} freqs {:?}", t.elapsed().as_secs_f32(), h, v.player, v.actions, v.action_freqs.iter().map(|x| (x*1000.0).round()/10.0).collect::<Vec<_>>());
        for b in &v.buckets { println!("   {} share {:.0}% -> {:?}", b.name, b.share*100.0, b.freqs.iter().map(|x| (x*1000.0).round()/10.0).collect::<Vec<_>>()); }
    };
    show(false, &[0], "base IP after check");
    show(false, &[0,1], "base OOP vs bet");
    show(true, &[0], "LOCK IP after check");
    show(true, &[0,1], "LOCK OOP vs bet");
    show(false, &[0], "base again (cached)");
}

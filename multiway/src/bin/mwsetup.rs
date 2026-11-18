//! Times tree building against solver steps (speed work).
use multiway::study::*;
use std::time::Instant;
fn spec(board: &str, wide: bool) -> Spec3 {
    let w = ["99-22,A9s-A2s,K9s-K2s,Q9s+,J9s+,T8s+,97s+,86s+,75s+,65s,54s,ATo-A8o,KTo+,QTo+,JTo", "TT-22,AJs-A2s,KTs+,QTs+,JTs,T9s,98s,87s,76s,AQo-ATo,KQo,KJo", "AA-22,AKs-A2s,K9s+,Q9s+,J9s+,T8s+,98s,87s,76s,65s,AKo-ATo,KJo+,QJo"];
    let t = ["99-66,A9s-A7s,KTs+,QTs+,JTs,T9s,98s,KQo", "TT-77,AJs-ATs,KQs,QJs,JTs,AQo", "AA-TT,AKs-ATs,KQs,KJs,QJs,AKo,AQo"];
    let r = if wide { w } else { t };
    Spec3 { ranges: [r[0].into(), r[1].into(), r[2].into()], board: board.into(), pot: 30.0, stack: 100.0, bet_sizes: if board.len() == 10 { "33%, 75%".into() } else { "66%".into() },
        raise_sizes: "60%".into(), max_raises: if board.len() == 10 { 3 } else { 2 }, allin_spr: 1.0, accuracy_pct: 0.0, max_iter: 0, leaks: vec![],
        flop_bet_sizes: String::new(), flop_max_raises: 0, turn_bet_sizes: String::new(), turn_max_raises: 0, threads: 0, turn_cards: 1, river_cards: if board.len() == 8 { 0 } else { 1 }, stacks: None, rake_rate: 0.0, rake_cap: 0.0, raise_mults: String::new(), max_seconds: 0.0, ..Default::default() }
}
fn main() {
    let which = std::env::args().nth(1).unwrap_or("flop".into());
    let (board, wide) = match which.as_str() { "river" => ("Kh9s6d4c2s", true), "turn" => ("Kh9s6d4c", true), "narrow" => ("Qs8h7h", false), _ => ("Kh9s6d", true) };
    let t = Instant::now();
    let g = build_game(&spec(board, wide), true).unwrap();
    println!("build {:.2}s, {} nodes, {} streets", t.elapsed().as_secs_f64(), g.nodes.len(), g.streets.len());
    for i in 1..=3u32 { let t = Instant::now(); g.iterate(i); println!("step {i}: {:.2}s", t.elapsed().as_secs_f64()); }
    let t = Instant::now(); let e = g.evaluate(); println!("evaluate {:.2}s EV {:.4}/{:.4}/{:.4}", t.elapsed().as_secs_f64(), e[0].0, e[1].0, e[2].0);
}

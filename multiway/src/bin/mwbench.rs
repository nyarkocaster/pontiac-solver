//! Fixed benchmark: seconds per solver step on a river, a turn and a flop spot, plus a result
//! fingerprint so optimisations can be checked for identical output.
use multiway::study::*;
use std::time::Instant;
fn spec(board: &str, wide: bool) -> Spec3 {
    let w = ["99-22,A9s-A2s,K9s-K2s,Q9s+,J9s+,T8s+,97s+,86s+,75s+,65s,54s,ATo-A8o,KTo+,QTo+,JTo", "TT-22,AJs-A2s,KTs+,QTs+,JTs,T9s,98s,87s,76s,AQo-ATo,KQo,KJo", "AA-22,AKs-A2s,K9s+,Q9s+,J9s+,T8s+,98s,87s,76s,65s,AKo-ATo,KJo+,QJo"];
    let t = ["99-66,A9s-A7s,KTs+,QTs+,JTs,T9s,98s,KQo", "TT-77,AJs-ATs,KQs,QJs,JTs,AQo", "AA-TT,AKs-ATs,KQs,KJs,QJs,AKo,AQo"];
    let r = if wide { w } else { t };
    Spec3 { ranges: [r[0].into(), r[1].into(), r[2].into()], board: board.into(), pot: 30.0, stack: 100.0, bet_sizes: if board.len() == 10 { "33%, 75%".into() } else { "66%".into() },
        raise_sizes: "60%".into(), max_raises: if board.len() == 10 { 3 } else { 2 }, allin_spr: 1.0, accuracy_pct: 0.0, max_iter: 0, leaks: vec![],
        flop_bet_sizes: String::new(), flop_max_raises: 0, turn_bet_sizes: String::new(), turn_max_raises: 0, threads: 0, turn_cards: 1, river_cards: if board.len() == 8 { 0 } else { 1 } }
}
fn main() {
    let only = std::env::args().nth(1).unwrap_or_default();
    for (name, board, wide, iters) in [("river wide", "Kh9s6d4c2s", true, 60u32), ("turn wide", "Kh9s6d4c", true, 8), ("flop narrow", "Qs8h7h", false, 4), ("flop wide", "Kh9s6d", true, 2)] {
        if !only.is_empty() && !name.starts_with(&only) { continue; }
        let g = build_game(&spec(board, wide), true).unwrap();
        let t = Instant::now();
        for i in 1..=iters { g.iterate(i); }
        let secs = t.elapsed().as_secs_f64() / iters as f64;
        let e = g.evaluate();
        println!("{name:12} {:8.3}s per step | work {:.2} | {} nodes | EV {:.4}/{:.4}/{:.4} best {:.4}/{:.4}/{:.4}", secs, g.work_units(), g.nodes.len(), e[0].0, e[1].0, e[2].0, e[0].1, e[1].1, e[2].1);
    }
}

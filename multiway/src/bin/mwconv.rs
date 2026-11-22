//! Accuracy against steps for one spot (speed work).
use multiway::study::*;
fn main() {
    let w = ["99-22,A9s-A2s,K9s-K2s,Q9s+,J9s+,T8s+,97s+,86s+,75s+,65s,54s,ATo-A8o,KTo+,QTo+,JTo", "TT-22,AJs-A2s,KTs+,QTs+,JTs,T9s,98s,87s,76s,AQo-ATo,KQo,KJo", "AA-22,AKs-A2s,K9s+,Q9s+,J9s+,T8s+,98s,87s,76s,65s,AKo-ATo,KJo+,QJo"];
    let board = std::env::args().nth(1).unwrap_or("Kh9s6d4c".into());
    let s = Spec3 { ranges: [w[0].into(), w[1].into(), w[2].into()], board: board.clone(), pot: 30.0, stack: 100.0, bet_sizes: "66%".into(), raise_sizes: "60%".into(), max_raises: 2, allin_spr: 1.0, threads: 0, turn_cards: 1, river_cards: if board.len() == 8 { 0 } else { 1 }, ..Default::default() };
    let g = build_game(&s, true).unwrap();
    let marks = [20u32, 40, 80, 160];
    let t0 = std::time::Instant::now();
    for i in 1..=*marks.last().unwrap() {
        g.iterate(i);
        if marks.contains(&i) { let e = g.evaluate(); let w = (0..3).map(|p| 100.0 * (e[p].1 - e[p].0) / 30.0).fold(0.0, f64::max); println!("step {i}: {w:.3}%  ({:.0}s)", t0.elapsed().as_secs_f64()); }
    }
}

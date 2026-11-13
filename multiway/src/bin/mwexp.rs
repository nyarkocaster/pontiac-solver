//! Experiment: how good are early-street strategies solved against a sample of later cards?
//! Solve a turn spot with sampled rivers, freeze its turn strategies inside the exact game,
//! solve the rivers exactly, then measure how much any player could gain by deviating anywhere.
use multiway::game::*;
use multiway::study::*;
use std::time::Instant;

fn spec(board: &str, river_cards: usize) -> Spec3 {
    Spec3 {
        ranges: [
            "99-22,A9s-A2s,K9s-K2s,Q9s+,J9s+,T8s+,97s+,86s+,75s+,65s,54s,ATo-A8o,KTo+,QTo+,JTo".into(),
            "TT-22,AJs-A2s,KTs+,QTs+,JTs,T9s,98s,87s,76s,AQo-ATo,KQo,KJo".into(),
            "AA-22,AKs-A2s,K9s+,Q9s+,J9s+,T8s+,98s,87s,76s,65s,AKo-ATo,KJo+,QJo".into(),
        ],
        board: board.into(), pot: 30.0, stack: 100.0, bet_sizes: "66%".into(), raise_sizes: "60%".into(),
        max_raises: 2, allin_spr: 1.0, accuracy_pct: 0.25, max_iter: 160, leaks: vec![],
        flop_bet_sizes: String::new(), flop_max_raises: 0, turn_bet_sizes: String::new(), turn_max_raises: 0, threads: 0, turn_cards: 0, river_cards, stacks: None, rake_rate: 0.0, rake_cap: 0.0, raise_mults: String::new(), max_seconds: 0.0, ..Default::default()
    }
}

fn copy_turn(src: &Game, dst: &mut Game, a: usize, b: usize, n: &mut usize) {
    if src.nodes[a].kind != Kind::Decision || src.board_len(&src.nodes[a]) != src.start_len() { return; }
    dst.nodes[b].locked = Some(src.avg_strategy(a));
    *n += 1;
    for k in 0..src.nodes[a].children.len() { let (x, y) = (src.nodes[a].children[k], dst.nodes[b].children[k]); copy_turn(src, dst, x, y, n); }
}

fn root_mix(g: &Game) -> String {
    let s = g.avg_strategy(0); let node = &g.nodes[0]; let nh = g.hands[0].len();
    let mut f = vec![0f64; node.actions.len()]; let mut tot = 0f64;
    for i in 0..nh { let w = g.cfg.ranges[0][g.hands[0][i] as usize] as f64; tot += w; for a in 0..f.len() { f[a] += w * s[a * nh + i] as f64; } }
    node.actions.iter().enumerate().map(|(a, x)| format!("{} {:.1}%", x.label(), 100.0 * f[a] / tot)).collect::<Vec<_>>().join(", ")
}

fn main() {
    let board = std::env::args().nth(1).unwrap_or("Kh9s6d4c".into());
    let prog = Progress3::default();
    for per_rank in [1usize, 2] {
        let t = Instant::now();
        let st = solve3(&spec(&board, per_rank), &prog).unwrap();
        let sg = st.base.as_ref().unwrap(); let sum = st.summary.as_ref().unwrap();
        println!("{} rivers per rank ({} of 48 rivers): solved in {:.0}s, {} steps, in-sample gain {:.2}%/{:.2}%/{:.2}% | seat 1 turn: {}",
            per_rank, sg.samples[&255].len(), t.elapsed().as_secs_f64(), sum.base_iters, sum.base_gain_pct[0], sum.base_gain_pct[1], sum.base_gain_pct[2], root_mix(sg));
        // exact game with the sampled solve's turn strategies frozen
        let mut full = build_game(&spec(&board, 0), true).unwrap();
        let mut n = 0; copy_turn(sg, &mut full, 0, 0, &mut n);
        let t = Instant::now();
        for it in 1..=90 { full.iterate(it); }
        full.br_ignores_locks = true;
        let e = full.evaluate();
        let g: Vec<String> = (0..3).map(|p| format!("{:.2}%", 100.0 * (e[p].1 - e[p].0) / 30.0)).collect();
        println!("   in the exact game ({} turn decisions frozen, rivers re-solved in {:.0}s): any one player could gain {} of the pot | EV {:.2}/{:.2}/{:.2}",
            n, t.elapsed().as_secs_f64(), g.join(" / "), e[0].0, e[1].0, e[2].0);
    }
}

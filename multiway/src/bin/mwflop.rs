//! Flop tools: `mwflop est` prints size/time for a sampled flop solve; `mwflop run <iters> [narrow]` solves.
use multiway::study::*;
use std::time::Instant;

fn spec(narrow: bool, turn_cards: usize, river_cards: usize, max_iter: u32) -> Spec3 {
    let wide = ["99-22,A9s-A2s,K9s-K2s,Q9s+,J9s+,T8s+,97s+,86s+,75s+,65s,54s,ATo-A8o,KTo+,QTo+,JTo", "TT-22,AJs-A2s,KTs+,QTs+,JTs,T9s,98s,87s,76s,AQo-ATo,KQo,KJo", "AA-22,AKs-A2s,K9s+,Q9s+,J9s+,T8s+,98s,87s,76s,65s,AKo-ATo,KJo+,QJo"];
    let tight = ["99-66,A9s-A7s,KTs+,QTs+,JTs,T9s,98s,KQo", "TT-77,AJs-ATs,KQs,QJs,JTs,AQo", "AA-TT,AKs-ATs,KQs,KJs,QJs,AKo,AQo"];
    let r = if narrow { tight } else { wide };
    Spec3 {
        ranges: [r[0].into(), r[1].into(), r[2].into()], board: "Kh9s6d".into(), pot: 30.0, stack: 100.0,
        bet_sizes: "66%".into(), raise_sizes: "60%".into(), max_raises: 2, allin_spr: 1.0, accuracy_pct: 0.5, max_iter, leaks: vec![],
        flop_bet_sizes: String::new(), flop_max_raises: 0, turn_bet_sizes: String::new(), turn_max_raises: 0, threads: 0, turn_cards, river_cards, stacks: None, rake_rate: 0.0, rake_cap: 0.0, raise_mults: String::new(), max_seconds: 0.0, ..Default::default()
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.get(1).map(|s| s.as_str()) == Some("est") {
        for narrow in [false, true] { for (t, r) in [(1, 1), (2, 1), (0, 1), (0, 0)] {
            let t0 = Instant::now();
            let e = estimate(&spec(narrow, t, r, 100)).unwrap();
            println!("{} ranges ({:?} hands), turn cards/rank {}, river cards/rank {}: {} nodes, {:.0} MB, est {:.0}s per step (tree built in {:.1}s)",
                if narrow { "narrow" } else { "wide" }, e.combos, t, r, e.nodes, e.memory_mb, e.secs_per_iter, t0.elapsed().as_secs_f64());
        } }
        return;
    }
    let iters: u32 = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(40);
    let narrow = a.get(3).is_some();
    let sp = spec(narrow, 1, 1, iters);
    let prog = Progress3::default();
    let t0 = Instant::now();
    let mut st = solve3(&sp, &prog).unwrap();
    let s = st.summary.clone().unwrap();
    println!("flop solve: {} nodes, {:.0} MB, {} steps in {:.0}s ({:.1}s per step) | EV {:.2}/{:.2}/{:.2} (sum {:.2}) | gain {:.2}%/{:.2}%/{:.2}%", s.nodes, s.memory_mb, s.base_iters, t0.elapsed().as_secs_f64(),
        s.base_secs / s.base_iters as f64, s.base_ev[0], s.base_ev[1], s.base_ev[2], s.base_ev.iter().sum::<f64>(), s.base_gain_pct[0], s.base_gain_pct[1], s.base_gain_pct[2]);
    let v = st.view(false, &[], true).unwrap();
    println!("seat 1 on the flop: {} | equity {:.0}%/{:.0}%/{:.0}%", v.actions.iter().zip(v.action_freqs.iter()).map(|(a, f)| format!("{a} {:.1}%", 100.0 * f)).collect::<Vec<_>>().join(", "), 100.0 * v.equity[0], 100.0 * v.equity[1], 100.0 * v.equity[2]);
    let c = st.view(false, &[0, 0, 0], true).unwrap();
    println!("after three checks: {} with {} turn cards: {}", c.kind, c.cards.len(), c.cards.iter().map(|x| x.0.clone()).collect::<Vec<_>>().join(" "));
    let t = st.view(false, &[0, 0, 0, c.cards[4].1], true).unwrap();
    println!("turn {}: seat {} | {} | board {:?} | EV sum {:.2} vs pot {:.0}", c.cards[4].0, t.player + 1, t.actions.iter().zip(t.action_freqs.iter()).map(|(a, f)| format!("{a} {:.1}%", 100.0 * f)).collect::<Vec<_>>().join(", "), t.board, t.ev.iter().sum::<f64>(), t.pot);
}

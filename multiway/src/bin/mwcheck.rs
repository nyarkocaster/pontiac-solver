use multiway::cards::*;
use multiway::game::*;
use std::time::Instant;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13; self.0 ^= self.0 >> 7; self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn disjoint(a: (u8, u8), b: (u8, u8)) -> bool { a.0 != b.0 && a.0 != b.1 && a.1 != b.0 && a.1 != b.1 }

/// Checks the fast terminal maths against a slow, obviously-correct triple loop.
fn selftest() -> bool {
    // 5-card evaluator: category counts over all 2,598,960 hands
    let mut counts = [0u32; 9];
    for a in 0..52u8 { for b in a + 1..52 { for c in b + 1..52 { for d in c + 1..52 { for e in d + 1..52 {
        counts[((eval5(&[a, b, c, d, e]) - 1) / 400_000) as usize] += 1;
    } } } } }
    let expect = [1302540, 1098240, 123552, 54912, 10200, 5108, 3744, 624, 40];
    println!("hand evaluator category counts: {}", if counts == expect { "PASS" } else { "FAIL" });
    let mut ok = counts == expect;

    let mut rng = Rng(0x9E3779B97F4A7C15);
    let mut worst = 0f64;
    for trial in 0..6 {
        let boards = ["KsQs7h7d2c", "AhKhQhJhTh", "9c8c7c6c2d", "AsAdKsKd3h", "Td9d4s3c2h", "5s5d5h9c9d"];
        let board = parse_board(boards[trial]).unwrap();
        let mut ranges: [Vec<f32>; 3] = [vec![0.0; NH], vec![0.0; NH], vec![0.0; NH]];
        for p in 0..3 { for i in 0..NH { if rng.next() < 0.11 { ranges[p][i] = (0.05 + rng.next()) as f32; } } }
        let cfg = Config { board, pot: 30.0, stack: 100.0, ranges, flop_bet_sizes: vec![], turn_bet_sizes: vec![0.5], bet_sizes: vec![0.5], raise_sizes: vec![], flop_max_raises: 1, turn_max_raises: 1, max_raises: 1, allin_spr: 0.0, turn_cards: 0, river_cards: 0 };
        let g = Game::new(cfg, false).unwrap();
        let r = &g.cfg.ranges;
        let (pot, inv) = (90.0f64, 20.0f64);
        for hero in 0..3 {
            let (o1, o2) = ((hero + 1) % 3, (hero + 2) % 3);
            let fast3 = g.showdown(0, hero, o1, &r[o1], &r[o2], true, pot, inv);
            let fast2 = g.showdown(0, hero, o1, &r[o1], &r[o2], false, pot, inv);
            let fastr = g.reach_sum(hero, &r[o1], &r[o2]);
            for (k, &h) in g.hands[hero].iter().enumerate() {
                let hc = g.deck.cards[h as usize]; let s = g.streets[0].strength[h as usize];
                let (mut v3, mut v2, mut rs) = (0f64, 0f64, 0f64);
                for &h1 in &g.hands[o1] {
                    let c1 = g.deck.cards[h1 as usize]; if !disjoint(hc, c1) { continue; }
                    let s1 = g.streets[0].strength[h1 as usize];
                    for &h2 in &g.hands[o2] {
                        let c2 = g.deck.cards[h2 as usize]; if !disjoint(hc, c2) || !disjoint(c1, c2) { continue; }
                        let s2 = g.streets[0].strength[h2 as usize];
                        let w = r[o1][h1 as usize] as f64 * r[o2][h2 as usize] as f64;
                        rs += w;
                        let best = s.max(s1).max(s2);
                        let n = (s == best) as u32 + (s1 == best) as u32 + (s2 == best) as u32;
                        v3 += w * (if s == best { pot / n as f64 } else { 0.0 } - inv);
                        v2 += w * (if s > s1 { pot } else if s == s1 { pot / 2.0 } else { 0.0 } - inv);
                    }
                }
                let scale = rs.abs().max(1.0) * pot;
                worst = worst.max((v3 - fast3[k] as f64).abs() / scale).max((v2 - fast2[k] as f64).abs() / scale).max((rs - fastr[k]).abs() / rs.abs().max(1.0));
            }
        }
    }
    let pass = worst < 1e-5;
    println!("terminal maths vs brute force (6 boards, 3 seats, 3-way and 2-way showdowns): worst relative error {:.2e} -> {}", worst, if pass { "PASS" } else { "FAIL" });
    ok &= pass;
    ok
}

use multiway::study::*;

fn spec(leaks: Vec<Leak3>) -> Spec3 {
    Spec3 {
        ranges: [
            "99-22,A9s-A2s,K9s-K2s,Q9s+,J9s+,T8s+,97s+,86s+,75s+,65s,54s,ATo-A8o,KTo+,QTo+,JTo".into(),
            "TT-22,AJs-A2s,KTs+,QTs+,JTs,T9s,98s,87s,76s,AQo-ATo,KQo,KJo".into(),
            "AA-22,AKs-A2s,K9s+,Q9s+,J9s+,T8s+,98s,87s,76s,65s,AKo-ATo,KJo+,QJo".into(),
        ],
        board: "Kh9s6d4c2s".into(), pot: 30.0, stack: 100.0, bet_sizes: "33%, 75%".into(), raise_sizes: "60%".into(),
        max_raises: 3, allin_spr: 1.0, accuracy_pct: 0.1, max_iter: 400, leaks, flop_bet_sizes: String::new(), flop_max_raises: 0, turn_bet_sizes: String::new(), turn_max_raises: 0, threads: 0, turn_cards: 0, river_cards: 0,
    }
}

fn show(st: &mut State3, locked: bool, hist: &[usize], title: &str) -> NodeView3 {
    let v = st.view(locked, hist, true).unwrap();
    println!("  {title}: seat {} | pot {:.0} | {} | EV {:.2}/{:.2}/{:.2} | equity {:.0}%/{:.0}%/{:.0}%", v.player + 1, v.pot,
        v.actions.iter().zip(v.action_freqs.iter()).map(|(a, f)| format!("{a} {:.1}%", 100.0 * f)).collect::<Vec<_>>().join(", "),
        v.ev[0], v.ev[1], v.ev[2], 100.0 * v.equity[0], 100.0 * v.equity[1], 100.0 * v.equity[2]);
    v
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut ok = selftest();
    if args.get(1).map(|s| s.as_str()) == Some("quick") { std::process::exit(if ok { 0 } else { 1 }); }
    if args.get(1).map(|s| s.as_str()) == Some("flop") { ok &= flop_checks(); std::process::exit(if ok { 0 } else { 1 }); }
    let prog = Progress3::default();
    println!("\nNormal solve (no habits):");
    let mut st = solve3(&spec(vec![]), &prog).unwrap();
    let s = st.summary.clone().unwrap();
    println!("  {} nodes, {} iterations, {:.1}s, {:.1} MB | EV {:.2}/{:.2}/{:.2} | could gain by deviating {:.3}%/{:.3}%/{:.3}%",
        s.nodes, s.base_iters, s.base_secs, s.memory_mb, s.base_ev[0], s.base_ev[1], s.base_ev[2], s.base_gain_pct[0], s.base_gain_pct[1], s.base_gain_pct[2]);
    let evsum: f64 = s.base_ev.iter().sum();
    let pass = (evsum - 30.0).abs() < 0.01 && s.base_gain_pct.iter().all(|&g| g <= 0.11);
    println!("  EVs add up to the pot and all three seats are within 0.1% of best play -> {}", if pass { "PASS" } else { "FAIL" }); ok &= pass;
    let root = show(&mut st, false, &[], "seat 1 first action");
    let v = show(&mut st, false, &[0, 0], "after check, check");
    let evs: f64 = v.ev.iter().sum();
    let pass = (evs - v.pot).abs() < 0.02 && (root.equity.iter().sum::<f64>() - 1.0).abs() < 0.01;
    println!("  EVs at an inner node add up to the pot; equities add up to 100% -> {}", if pass { "PASS" } else { "FAIL" }); ok &= pass;

    println!("\nHabit: seat 2 calls too much (60% of folds become calls):");
    let mut st2 = solve3(&spec(vec![Leak3 { player: 1, street: "all".into(), effect: "fold_less".into(), strength: 0.6 }]), &prog).unwrap();
    let s2 = st2.summary.clone().unwrap();
    println!("  locked {} decisions, {} iterations, {:.1}s | EV {:.2}/{:.2}/{:.2} (normal {:.2}/{:.2}/{:.2})", s2.locked_nodes, s2.locked_iters, s2.locked_secs,
        s2.locked_ev[0], s2.locked_ev[1], s2.locked_ev[2], s2.base_ev[0], s2.base_ev[1], s2.base_ev[2]);
    // seat 3's strategy after two checks: weak hands should bluff less against a calling station
    let b = st2.view(false, &[0, 0], true).unwrap(); let l = st2.view(true, &[0, 0], true).unwrap();
    let weak_bet = |v: &NodeView3| 1.0 - v.buckets[2].freqs[0];
    println!("  seat 3 weak hands bet: normal {:.1}% -> vs habit {:.1}%", 100.0 * weak_bet(&b), 100.0 * weak_bet(&l));
    let pass = weak_bet(&l) < weak_bet(&b) && s2.locked_ev[1] < s2.base_ev[1] && (s2.locked_ev.iter().sum::<f64>() - 30.0).abs() < 0.01;
    println!("  bluffs drop against the caller, and the caller's EV falls -> {}", if pass { "PASS" } else { "FAIL" }); ok &= pass;
    ok &= turn_checks();
    ok &= flop_checks();
    std::process::exit(if ok { 0 } else { 1 });
}

/// Turn start: a check-only tree must reproduce brute-force equity over every river card;
/// then a real turn + river solve.
fn turn_checks() -> bool {
    let mut ok = true;
    let mut rng = Rng(0xD1B54A32D192ED03);
    let deck = Deck::new();
    let board = parse_board("Qs8h7h2d").unwrap();
    let mut ranges: [Vec<f32>; 3] = [vec![0.0; NH], vec![0.0; NH], vec![0.0; NH]];
    for p in 0..3 { for i in 0..NH { if rng.next() < 0.035 { ranges[p][i] = (0.05 + rng.next()) as f32; } } }
    let cfg = Config { board: board.clone(), pot: 60.0, stack: 100.0, ranges, flop_bet_sizes: vec![], turn_bet_sizes: vec![], bet_sizes: vec![], raise_sizes: vec![], flop_max_raises: 1, turn_max_raises: 1, max_raises: 1, allin_spr: 0.0, turn_cards: 0, river_cards: 0 };
    let g = Game::new(cfg, true).unwrap();
    let r = &g.cfg.ranges;
    let mut worst = 0f64;
    for hero in 0..3 {
        let (o1, o2) = ((hero + 1) % 3, (hero + 2) % 3);
        let fast = g.walk(0, hero, [&r[0], &r[1], &r[2]], Mode::Average);
        let z = g.reach_sum(hero, &r[o1], &r[o2]);
        for (k, &h) in g.hands[hero].iter().enumerate() {
            let hc = deck.cards[h as usize];
            let mut v = 0f64;
            for &h1 in &g.hands[o1] {
                let c1 = deck.cards[h1 as usize]; if !disjoint(hc, c1) { continue; }
                for &h2 in &g.hands[o2] {
                    let c2 = deck.cards[h2 as usize]; if !disjoint(hc, c2) || !disjoint(c1, c2) { continue; }
                    let w = r[o1][h1 as usize] as f64 * r[o2][h2 as usize] as f64;
                    let mut share = 0f64;
                    for st in &g.streets {
                        let c = st.card;
                        if [hc.0, hc.1, c1.0, c1.1, c2.0, c2.1].contains(&c) { continue; }
                        let (s, s1, s2) = (st.strength[h as usize], st.strength[h1 as usize], st.strength[h2 as usize]);
                        let best = s.max(s1).max(s2);
                        if s == best { share += 1.0 / ((s == best) as u32 + (s1 == best) as u32 + (s2 == best) as u32) as f64; }
                    }
                    v += w * 60.0 * share / 42.0;
                }
            }
            worst = worst.max((v - fast[k] as f64).abs() / (z[k].abs().max(1.0) * 60.0));
        }
    }
    let pass = worst < 1e-5;
    println!("\nTurn: river-card averaging vs brute force over all rivers: worst relative error {:.2e} -> {}", worst, if pass { "PASS" } else { "FAIL" });
    ok &= pass;

    let mut sp = spec(vec![]);
    sp.board = "Kh9s6d4c".into(); sp.bet_sizes = "66%".into(); sp.raise_sizes = "60%".into(); sp.max_raises = 2; sp.accuracy_pct = 0.3; sp.max_iter = 150;
    let e = estimate(&sp).unwrap();
    println!("Turn + river solve (Kh 9s 6d 4c): {} nodes, about {:.0} MB, estimated {:.1}s per step", e.nodes, e.memory_mb, e.secs_per_iter);
    let prog = Progress3::default();
    let mut st = solve3(&sp, &prog).unwrap();
    let s = st.summary.clone().unwrap();
    println!("  {} iterations, {:.1}s ({:.2}s per step) | EV {:.2}/{:.2}/{:.2} | could gain by deviating {:.3}%/{:.3}%/{:.3}%", s.base_iters, s.base_secs, s.base_secs / s.base_iters as f64,
        s.base_ev[0], s.base_ev[1], s.base_ev[2], s.base_gain_pct[0], s.base_gain_pct[1], s.base_gain_pct[2]);
    let root = show(&mut st, false, &[], "seat 1 on the turn");
    let ch = st.view(false, &[0, 0, 0], true).unwrap();
    let card = ch.cards[5].1;
    println!("  after three checks: {} with {} possible river cards; dealing {}", ch.kind, ch.cards.len(), ch.cards[5].0);
    let rv = show(&mut st, false, &[0, 0, 0, card], "seat 1 on the river");
    let pass = (s.base_ev.iter().sum::<f64>() - 30.0).abs() < 0.02 && ch.kind == "chance" && ch.cards.len() == 48
        && (rv.ev.iter().sum::<f64>() - rv.pot).abs() < 0.05 && (root.equity.iter().sum::<f64>() - 1.0).abs() < 0.01 && rv.board.len() == 5;
    println!("  EVs add up to the pot on both streets, equities add up to 100% -> {}", if pass { "PASS" } else { "FAIL" });
    ok &= pass;
    ok
}

/// Flop start with every turn and river card: a check-only tree must reproduce brute-force
/// equity over all two-card runouts. Then the sampled deal must give nearly the same answer.
fn flop_checks() -> bool {
    let mut rng = Rng(0xA0761D6478BD642F);
    let deck = Deck::new();
    let board = parse_board("Qs8h7h").unwrap();
    let mut ranges: [Vec<f32>; 3] = [vec![0.0; NH], vec![0.0; NH], vec![0.0; NH]];
    for p in 0..3 { for i in 0..NH { if rng.next() < 0.012 { ranges[p][i] = (0.05 + rng.next()) as f32; } } }
    let mk = |tc: usize, rc: usize| Config { board: board.clone(), pot: 60.0, stack: 100.0, ranges: ranges.clone(), flop_bet_sizes: vec![], turn_bet_sizes: vec![], bet_sizes: vec![], raise_sizes: vec![],
        flop_max_raises: 1, turn_max_raises: 1, max_raises: 1, allin_spr: 0.0, turn_cards: tc, river_cards: rc };
    let g = Game::new(mk(0, 0), true).unwrap();
    let r = &g.cfg.ranges;
    let mut worst = 0f64; let worst_s;
    for hero in 0..3 {
        let (o1, o2) = ((hero + 1) % 3, (hero + 2) % 3);
        let fast = g.walk(0, hero, [&r[0], &r[1], &r[2]], Mode::Average);
        let z = g.reach_sum(hero, &r[o1], &r[o2]);
        for (k, &h) in g.hands[hero].iter().enumerate() {
            if k >= 6 { continue; }
            let hc = deck.cards[h as usize];
            let mut v = 0f64;
            for &h1 in &g.hands[o1] {
                let c1 = deck.cards[h1 as usize]; if !disjoint(hc, c1) { continue; }
                for &h2 in &g.hands[o2] {
                    let c2 = deck.cards[h2 as usize]; if !disjoint(hc, c2) || !disjoint(c1, c2) { continue; }
                    let w = r[o1][h1 as usize] as f64 * r[o2][h2 as usize] as f64;
                    let used = [board[0], board[1], board[2], hc.0, hc.1, c1.0, c1.1, c2.0, c2.1];
                    let (mut share, mut n) = (0f64, 0f64);
                    for t in 0..52u8 { if used.contains(&t) { continue; } for rv in (t + 1)..52u8 { if used.contains(&rv) { continue; }
                        let e = |x: (u8, u8)| eval7(&[board[0], board[1], board[2], t, rv, x.0, x.1]);
                        let (s, s1, s2) = (e(hc), e(c1), e(c2));
                        let best = s.max(s1).max(s2);
                        if s == best { share += 1.0 / ((s == best) as u32 + (s1 == best) as u32 + (s2 == best) as u32) as f64; }
                        n += 1.0;
                    } }
                    v += w * 60.0 * share / n;
                }
            }
            worst = worst.max((v - fast[k] as f64).abs() / (z[k].abs().max(1.0) * 60.0));
        }
    }
    let pass = worst < 1e-5;
    println!("\nFlop: averaging over every turn and river card vs brute force: worst relative error {:.2e} -> {}", worst, if pass { "PASS" } else { "FAIL" });
    // sampled deal vs every card, with realistic (suit-symmetric) ranges, on a flush-draw flop
    let sp = spec(vec![]);
    let rr = [parse_range(&deck, &sp.ranges[0]).unwrap(), parse_range(&deck, &sp.ranges[1]).unwrap(), parse_range(&deck, &sp.ranges[2]).unwrap()];
    let mk2 = |tc: usize, rc: usize| Game::new(Config { ranges: rr.clone(), ..mk(tc, rc) }, true).unwrap();
    let (gf, gx) = (mk2(0, 0), mk2(1, 1));
    let r2 = &gf.cfg.ranges;
    let (mut tot, mut dif) = (0f64, 0f64);
    for hero in 0..3 {
        let a = gf.walk(0, hero, [&r2[0], &r2[1], &r2[2]], Mode::Average); let b = gx.walk(0, hero, [&r2[0], &r2[1], &r2[2]], Mode::Average);
        for k in 0..a.len() { tot += a[k].abs() as f64; dif += (a[k] - b[k]).abs() as f64; }
    }
    worst_s = dif / tot;
    let pass2 = worst_s < 0.015;
    println!("Flop: sampled turn and river cards ({} of {} runouts) vs every card, average difference in hand values: {:.2}% -> {}", gx.streets.len(), gf.streets.len(), 100.0 * worst_s, if pass2 { "PASS" } else { "FAIL" });
    pass && pass2
}

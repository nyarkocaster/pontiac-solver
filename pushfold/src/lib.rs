//! Heads-up push/fold: the small blind either moves all-in or folds, the big blind calls or folds.
//! Finds the equilibrium for any stack depth. Heads-up a sit-and-go is winner-take-all, so chip
//! values and prize values coincide and no prize-structure model is needed.
use serde::Serialize;

static EQUITY: &[u8] = include_bytes!("../data/preflop_equity_169.bin");
pub const RANKS: &str = "AKQJT98765432";

/// Name of a starting-hand class by chart cell (row = higher rank, AA top-left, suited above the diagonal).
pub fn class_name(i: usize) -> String {
    let (r, c) = (i / 13, i % 13);
    let ch = |k: usize| RANKS.as_bytes()[k] as char;
    if r == c { format!("{}{}", ch(r), ch(c)) } else if c > r { format!("{}{}s", ch(r), ch(c)) } else { format!("{}{}o", ch(c), ch(r)) }
}
fn combos(i: usize) -> f64 { let (r, c) = (i / 13, i % 13); if r == c { 6.0 } else if c > r { 4.0 } else { 12.0 } }

/// All-in equity of class `a` against class `b` (win plus half of ties).
pub fn equity(a: usize, b: usize) -> f64 {
    let k = (a * 169 + b) * 4;
    f32::from_le_bytes([EQUITY[k], EQUITY[k + 1], EQUITY[k + 2], EQUITY[k + 3]]) as f64
}

/// How many hands of class `b` an opponent can hold when we hold one particular hand of class `a`.
fn compat() -> Vec<f64> {
    let cls = |x: u8, y: u8| -> usize {
        let (ra, rb) = ((x / 4) as usize, (y / 4) as usize);
        let (hi, lo) = if ra >= rb { (ra, rb) } else { (rb, ra) };
        let (r_hi, r_lo) = (12 - hi, 12 - lo);
        if hi == lo { r_hi * 13 + r_hi } else if x % 4 == y % 4 { r_hi * 13 + r_lo } else { r_lo * 13 + r_hi }
    };
    let mut hands = vec![];
    for a in 0..52u8 { for b in a + 1..52 { hands.push((a, b, cls(a, b))); } }
    let mut m = vec![0f64; 169 * 169];
    for &(a, b, c1) in &hands { for &(x, y, c2) in &hands { if a != x && a != y && b != x && b != y { m[c1 * 169 + c2] += 1.0; } } }
    for c1 in 0..169 { for c2 in 0..169 { m[c1 * 169 + c2] /= combos(c1); } }
    m
}

#[derive(Serialize, Clone, Debug)]
pub struct PushFold {
    pub stack_bb: f64,
    pub small_blind: f64,
    pub ante: f64,
    /// share of all starting hands the small blind moves in with / the big blind calls with
    pub jam_pct: f64,
    pub call_pct: f64,
    /// by chart cell (169 values, AA first): how often the hand jams / calls
    pub jam: Vec<f64>,
    pub call: Vec<f64>,
    /// by chart cell: what jamming gains over folding (small blind) and calling over folding (big blind), in big blinds
    pub jam_edge_bb: Vec<f64>,
    pub call_edge_bb: Vec<f64>,
    /// average result per hand in big blinds, counted from before the blinds are posted
    pub ev_small_blind_bb: f64,
    pub ev_big_blind_bb: f64,
    pub classes: Vec<String>,
}

/// Equilibrium for an effective stack of `stack_bb` big blinds (before posting), a small blind of
/// `small_blind` big blinds and an ante of `ante` big blinds each.
pub fn solve(stack_bb: f64, small_blind: f64, ante: f64) -> Result<PushFold, String> {
    if !(stack_bb > 1.0 && stack_bb <= 200.0) { return Err("stack must be more than 1 and at most 200 big blinds".into()); }
    if !(small_blind > 0.0 && small_blind < 1.0) { return Err("small blind must be between 0 and 1 big blind".into()); }
    if !(ante >= 0.0 && ante < stack_bb - 1.0) { return Err("ante must be zero or more, and leave chips behind".into()); }
    let cp = compat();
    let eq: Vec<f64> = (0..169 * 169).map(|k| equity(k / 169, k % 169)).collect();
    let s = stack_bb;
    let lose_sb_fold = -(small_blind + ante);
    let lose_bb_fold = -(1.0 + ante);
    let win_steal = 1.0 + ante;
    let (mut jam, mut call) = (vec![1.0f64; 169], vec![0.5f64; 169]);
    let (mut jam_edge, mut call_edge) = (vec![0f64; 169], vec![0f64; 169]);
    let iters = 4000;
    for t in 1..=iters {
        // small blind's best reply to the big blind's average calling range
        for i in 0..169 {
            let (mut ev, mut tot) = (0f64, 0f64);
            for j in 0..169 { let w = cp[i * 169 + j]; tot += w; ev += w * ((1.0 - call[j]) * win_steal + call[j] * s * (2.0 * eq[i * 169 + j] - 1.0)); }
            jam_edge[i] = ev / tot - lose_sb_fold;
        }
        for i in 0..169 { let br = if jam_edge[i] > 0.0 { 1.0 } else { 0.0 }; jam[i] += (br - jam[i]) / (t as f64 + 1.0); }
        // big blind's best reply to the small blind's average jamming range
        for j in 0..169 {
            let (mut ev, mut tot) = (0f64, 0f64);
            for i in 0..169 { let w = cp[j * 169 + i] * jam[i]; tot += w; ev += w * s * (2.0 * eq[j * 169 + i] - 1.0); }
            call_edge[j] = if tot > 0.0 { ev / tot - lose_bb_fold } else { 0.0 };
        }
        for j in 0..169 { let br = if call_edge[j] > 0.0 { 1.0 } else { 0.0 }; call[j] += (br - call[j]) / (t as f64 + 1.0); }
    }
    // tidy the averages: anything within rounding of pure is pure
    for v in jam.iter_mut().chain(call.iter_mut()) { if *v < 0.01 { *v = 0.0 } else if *v > 0.99 { *v = 1.0 } }
    let total: f64 = (0..169).map(combos).sum();
    let jam_pct = (0..169).map(|i| combos(i) * jam[i]).sum::<f64>() / total;
    let call_pct = (0..169).map(|i| combos(i) * call[i]).sum::<f64>() / total;
    let mut ev_sb = 0f64;
    for i in 0..169 {
        let (mut ev, mut tot) = (0f64, 0f64);
        for j in 0..169 { let w = cp[i * 169 + j]; tot += w; ev += w * ((1.0 - call[j]) * win_steal + call[j] * s * (2.0 * eq[i * 169 + j] - 1.0)); }
        ev_sb += combos(i) / total * (jam[i] * ev / tot + (1.0 - jam[i]) * lose_sb_fold);
    }
    let r = |x: f64, d: i32| { let m = 10f64.powi(d); (x * m).round() / m };
    Ok(PushFold {
        stack_bb, small_blind, ante, jam_pct: r(jam_pct, 4), call_pct: r(call_pct, 4),
        jam: jam.iter().map(|&x| r(x, 3)).collect(), call: call.iter().map(|&x| r(x, 3)).collect(),
        jam_edge_bb: jam_edge.iter().map(|&x| r(x, 3)).collect(), call_edge_bb: call_edge.iter().map(|&x| r(x, 3)).collect(),
        ev_small_blind_bb: r(ev_sb, 4), ev_big_blind_bb: r(-ev_sb, 4),
        classes: (0..169).map(class_name).collect(),
    })
}

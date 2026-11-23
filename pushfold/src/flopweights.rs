//! Weights that make a small sample of flops stand in for all 19,600 a hand can see.
//!
//! A sample of 16 flops pairs some ranks far too often and others never: 7-2 offsuit pairs on
//! most of a sample that happens to hold several sevens and deuces, and so looks like a hand
//! worth playing. Here every starting-hand class gets its own weight for each sample flop, chosen
//! so that how the hand connects with the weighted sample (no pair, pair with the high or low
//! card, two pair, trips, overpair, flush and straight draws, and what the board itself looks
//! like) matches how it connects with all flops.
//!
//! Cards are rank * 4 + suit, rank 0 = deuce, as everywhere else in this crate.
use crate::tables::class_of;

pub const NC: usize = 169;
/// Number of connection features.
pub const NF: usize = 32;

/// Names of the features, for reports.
pub const FEATURE_NAMES: [&str; NF] = [
    "no pair", "pair, high card", "pair, low card", "two pair", "trips", "full house or better", "set",
    "pair or better", "two pair or better",
    "top pair", "two overcards", "one overcard", "overpair", "pair, one card above", "pair, two cards above",
    "board paired", "board two-tone", "board one suit", "board ace-high", "board K- or Q-high", "board J- to 9-high", "board connected",
    "flush", "flush draw, both cards", "flush draw, one card", "backdoor flush draw",
    "straight", "eight-out straight draw", "gutshot",
    "kicker plays", "pocket pair, board paired", "board three of a kind",
];

/// How much each feature counts when the weights are fitted.
const LAMBDA: [f64; NF] = [
    4.0, 4.0, 4.0, 4.0, 4.0, 4.0, 4.0,
    4.0, 4.0,
    2.0, 2.0, 2.0, 3.0, 3.0, 3.0,
    2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0,
    3.0, 3.0, 2.0, 1.0,
    3.0, 2.0, 2.0,
    1.0, 1.0, 4.0,
];

/// The sample used up to 0.13.3, in its original order (the first eight were the short sample).
pub const SAMPLE_16: [&str; 16] = ["As7d2c", "Ks8s3d", "QhJh6c", "Ts9s8d", "7s6d5c", "KsKd4c", "8s3d2c", "Jh9h5h",
    "AsKdTc", "Qs7d2c", "9c6c4d", "7s7d4d", "Ah8h5c", "Jd7d4s", "Ts6d2c", "As5d5c"];

/// Samples chosen by the `flopsample` tool: with the fitted weights, every hand class's all-in equity
/// against a random hand, against the strongest 12% of hands and against the weakest half comes out
/// close to the exact figure over all boards (mean error 0.36, 0.65 and 0.45 points for the 24;
/// 0.24, 0.52 and 0.30 for the 32; the 16 above manage 1.2, 1.6 and 1.5).
pub const SAMPLE_24: [&str; 24] = ["JdJc2c", "9c8d4c", "AcJd2c", "KcJd3c", "Qd8d6c", "Qc7c5c", "6c2d2c", "8h7d5c", "8h5d3c", "KcQd6c", "Jd4c2c", "Th4d3c", "Qh9d5c", "6d6c2c", "AdQc4c", "9d6c5c", "Th7d6c", "Ac9c6c", "Qc7d3c", "AdAcTc", "Th6d4c", "7d7c6c", "8h5d4c", "AhQdJc"];
pub const SAMPLE_32: [&str; 32] = ["AdTd3c", "9h6d2c", "AhJdTc", "QdQc9c", "9d6c5c", "6h6d6c", "QhJd2c", "Qd4c2c", "AdJd9c", "Td8c2c", "Qh8d6c", "Tc2d2c", "AcKdKc", "AhQdTc", "Jh7d5c", "8h5d3c", "Th7d6c", "QhJd8c", "Jd8d3c", "Kd6c3c", "Qd9d3c", "AcTdTc", "7h4d3c", "Ac9d8c", "8h8d7c", "8h5d2c", "KhQd5c", "9d9c5c", "Tc5d5c", "KhTd8c", "Kc5d4c", "9h4d4c"];

/// The built-in sample of `n` flops: the chosen 24 or 32 when asked for exactly that many,
/// otherwise the first `n` of the original 16.
pub fn sample(n: usize) -> Vec<&'static str> {
    match n {
        24 => SAMPLE_24.to_vec(),
        32 => SAMPLE_32.to_vec(),
        _ => SAMPLE_16.iter().take(n.min(SAMPLE_16.len())).copied().collect(),
    }
}

pub fn parse_card(s: &str) -> Result<u8, String> {
    let b: Vec<char> = s.chars().collect();
    if b.len() != 2 { return Err(format!("bad card '{s}'")); }
    let r = "23456789TJQKA".find(b[0].to_ascii_uppercase()).ok_or(format!("bad rank in '{s}'"))?;
    let su = "cdhs".find(b[1].to_ascii_lowercase()).ok_or(format!("bad suit in '{s}'"))?;
    Ok((r * 4 + su) as u8)
}
pub fn parse_flop(s: &str) -> Result<[u8; 3], String> {
    let ch: Vec<char> = s.chars().filter(|c| !c.is_whitespace()).collect();
    if ch.len() != 6 { return Err(format!("a flop is three cards, got '{s}'")); }
    let v: Vec<u8> = ch.chunks(2).map(|k| parse_card(&k.iter().collect::<String>())).collect::<Result<_, _>>()?;
    if v[0] == v[1] || v[0] == v[2] || v[1] == v[2] { return Err(format!("repeated card in '{s}'")); }
    Ok([v[0], v[1], v[2]])
}

const PERMS: [[u8; 4]; 24] = [
    [0, 1, 2, 3], [0, 1, 3, 2], [0, 2, 1, 3], [0, 2, 3, 1], [0, 3, 1, 2], [0, 3, 2, 1],
    [1, 0, 2, 3], [1, 0, 3, 2], [1, 2, 0, 3], [1, 2, 3, 0], [1, 3, 0, 2], [1, 3, 2, 0],
    [2, 0, 1, 3], [2, 0, 3, 1], [2, 1, 0, 3], [2, 1, 3, 0], [2, 3, 0, 1], [2, 3, 1, 0],
    [3, 0, 1, 2], [3, 0, 2, 1], [3, 1, 0, 2], [3, 1, 2, 0], [3, 2, 0, 1], [3, 2, 1, 0],
];

/// How many flops a sample flop stands for when suits are swapped around (1 to 24 of the 22,100).
pub fn suit_ways(f: [u8; 3]) -> f64 {
    let mut seen = std::collections::HashSet::new();
    for p in PERMS { let mut g = f.map(|c| (c / 4) * 4 + p[(c % 4) as usize]); g.sort(); seen.insert(g); }
    seen.len() as f64
}

/// The connection features of one two-card hand on one flop (each 0 or 1).
pub fn features(hole: [u8; 2], flop: [u8; 3]) -> [f32; NF] {
    let mut x = [0f32; NF];
    let (ra, rb) = (hole[0] / 4, hole[1] / 4);
    let (hi, lo) = if ra >= rb { (ra, rb) } else { (rb, ra) };
    let br = [flop[0] / 4, flop[1] / 4, flop[2] / 4];
    let bs = [flop[0] % 4, flop[1] % 4, flop[2] % 4];
    let top = *br.iter().max().unwrap();
    let n_hi = br.iter().filter(|&&r| r == hi).count();
    let n_lo = br.iter().filter(|&&r| r == lo).count();
    let board_pair = br[0] == br[1] || br[0] == br[2] || br[1] == br[2];
    let board_trips = br[0] == br[1] && br[1] == br[2];
    if hi == lo {
        // pocket pair
        let above = br.iter().filter(|&&r| r > hi).count();
        x[7] = 1.0;
        if n_hi >= 1 {
            x[6] = 1.0; x[8] = 1.0;
            if n_hi == 2 || board_pair { x[5] = 1.0; }
        } else if board_trips {
            x[5] = 1.0; x[8] = 1.0;
        } else {
            if board_pair { x[30] = 1.0; }
            match above { 0 => x[12] = 1.0, 1 => x[13] = 1.0, 2 => x[14] = 1.0, _ => {} }
        }
    } else {
        // 0 nothing, 1 pair with the high card, 2 pair with the low card, 3 two pair, 4 trips, 5 full house or quads
        let made = match (n_hi, n_lo) { (0, 0) => 0, (1, 0) => 1, (0, 1) => 2, (1, 1) => 3, (2, 0) | (0, 2) => 4, _ => 5 };
        x[made] = 1.0;
        if made >= 1 { x[7] = 1.0; }
        if made >= 3 { x[8] = 1.0; }
        if made == 0 {
            let over = (hi > top) as usize + (lo > top) as usize;
            if over == 2 { x[10] = 1.0; } else if over == 1 { x[11] = 1.0; }
        } else if made <= 3 {
            let paired = if n_hi >= 1 { hi } else { lo };
            if paired == top { x[9] = 1.0; }
            if made <= 2 {
                // the other hole card is a kicker higher than every other board card
                let kicker = if made == 1 { lo } else { hi };
                if br.iter().filter(|&&r| r != paired).all(|&r| kicker > r) { x[29] = 1.0; }
            }
        }
    }
    // the board on its own
    if board_pair { x[15] = 1.0; }
    if board_trips { x[31] = 1.0; }
    let suits_same = (bs[0] == bs[1]) as usize + (bs[0] == bs[2]) as usize + (bs[1] == bs[2]) as usize;
    if suits_same == 1 { x[16] = 1.0; } else if suits_same == 3 { x[17] = 1.0; }
    if top == 12 { x[18] = 1.0; } else if top >= 10 { x[19] = 1.0; } else if top >= 7 { x[20] = 1.0; }
    {
        let mut m = 0u16; for r in br { m |= 1 << (r + 1); if r == 12 { m |= 1; } }
        if !board_pair && (0..10).any(|s| (m >> s & 0x1f).count_ones() == 3) { x[21] = 1.0; }
    }
    // flushes
    let mut sc = [0u8; 4]; for s in bs { sc[s as usize] += 1; }
    let (sa, sb) = (hole[0] % 4, hole[1] % 4);
    if sa == sb {
        match sc[sa as usize] { 3 => x[22] = 1.0, 2 => x[23] = 1.0, 1 => x[25] = 1.0, _ => {} }
    } else if sc[sa as usize] == 3 || sc[sb as usize] == 3 { x[24] = 1.0; }
    // straights
    {
        let mut m = 0u16;
        for r in [ra, rb, br[0], br[1], br[2]] { m |= 1 << (r + 1); if r == 12 { m |= 1; } }
        let made = |m: u16| (0..10).any(|s| (m >> s & 0x1f) == 0x1f);
        if made(m) { x[26] = 1.0; } else {
            let mut outs = 0;
            for r in 0..13u8 { let bit = 1u16 << (r + 1); if m & bit != 0 { continue; } let mm = m | bit | if r == 12 { 1 } else { 0 }; if made(mm) { outs += 1; } }
            if outs >= 2 { x[27] = 1.0; } else if outs == 1 { x[28] = 1.0; }
        }
    }
    x
}

/// One hand of a class: pairs in clubs and diamonds, suited hands in clubs, offsuit high club, low diamond.
pub fn representative(class: usize) -> [u8; 2] {
    let (r, c) = (class / 13, class % 13);
    let rank = |k: usize| (12 - k) as u8;
    if r == c { [rank(r) * 4, rank(r) * 4 + 1] } else if c > r { [rank(r) * 4, rank(c) * 4] } else { [rank(c) * 4, rank(r) * 4 + 1] }
}

/// Every two-card hand, grouped by class.
pub fn hands_by_class() -> Vec<Vec<[u8; 2]>> {
    let mut v = vec![vec![]; NC];
    for a in 0..52u8 { for b in a + 1..52 { v[class_of(a, b)].push([a, b]); } }
    v
}

/// How often each feature is true for a class over all 19,600 flops it can see.
pub fn true_profile(class: usize) -> [f64; NF] {
    let h = representative(class);
    let rest: Vec<u8> = (0..52u8).filter(|c| *c != h[0] && *c != h[1]).collect();
    let mut sum = [0f64; NF];
    let mut n = 0f64;
    for a in 0..rest.len() { for b in a + 1..rest.len() { for c in b + 1..rest.len() {
        let x = features(h, [rest[a], rest[b], rest[c]]);
        for k in 0..NF { sum[k] += x[k] as f64; }
        n += 1.0;
    } } }
    for k in 0..NF { sum[k] /= n; }
    sum
}

/// How often each feature is true for a class on one flop, over the class's hands that the flop
/// leaves possible, and how many such hands there are.
pub fn flop_profile(hands: &[[u8; 2]], flop: [u8; 3]) -> ([f64; NF], f64) {
    let mut sum = [0f64; NF];
    let mut n = 0f64;
    for h in hands {
        if flop.contains(&h[0]) || flop.contains(&h[1]) { continue; }
        let x = features(*h, flop);
        for k in 0..NF { sum[k] += x[k] as f64; }
        n += 1.0;
    }
    if n > 0.0 { for k in 0..NF { sum[k] /= n; } }
    (sum, n)
}

/// Every flop up to a relabelling of suits (1,755 of them), each with the number of real flops it stands for.
pub fn canonical_flops() -> Vec<([u8; 3], f64)> {
    let mut seen = std::collections::BTreeSet::new();
    let mut out = vec![];
    for a in 0..52u8 { for b in a + 1..52 { for c in b + 1..52 {
        let f = [a, b, c];
        let key = canonical_key(f);
        if seen.insert(key) { out.push((key, suit_ways(f))); }
    } } }
    out
}
fn canonical_key(f: [u8; 3]) -> [u8; 3] {
    let mut best = [255u8; 3];
    for p in PERMS { let mut g = f.map(|c| (c / 4) * 4 + p[(c % 4) as usize]); g.sort(); if g < best { best = g; } }
    best
}

/// The fitted weights and what they achieve.
pub struct FlopWeights {
    /// weight[class][flop]: adds to 1 over the flops for every class
    pub weight: Vec<Vec<f64>>,
    /// the weights a plain count gives (each flop by the number of real flops and hands it stands for)
    pub plain: Vec<Vec<f64>>,
    /// feature frequencies: over all flops, under the plain count, under the fitted weights
    pub truth: Vec<[f64; NF]>,
    pub plain_profile: Vec<[f64; NF]>,
    pub fitted_profile: Vec<[f64; NF]>,
}

/// Fits the weights for a sample of flops. `ways`, when given, says how many real flops each one
/// stands for; otherwise that is counted from its suits.
pub fn fit(flops: &[[u8; 3]], ways: Option<&[f64]>) -> FlopWeights {
    let by_class = hands_by_class();
    let ways: Vec<f64> = match ways { Some(w) => w.to_vec(), None => flops.iter().map(|f| suit_ways(*f)).collect() };
    let mut out = FlopWeights { weight: vec![], plain: vec![], truth: vec![], plain_profile: vec![], fitted_profile: vec![] };
    for class in 0..NC {
        let q = true_profile(class);
        let prof: Vec<([f64; NF], f64)> = flops.iter().map(|f| flop_profile(&by_class[class], *f)).collect();
        let refs: Vec<&([f64; NF], f64)> = prof.iter().collect();
        let (base, w) = fit_class(&q, &refs, &ways, 6000);
        let mix = |w: &[f64]| { let mut m = [0f64; NF]; for f in 0..flops.len() { for k in 0..NF { m[k] += w[f] * prof[f].0[k]; } } m };
        out.plain_profile.push(mix(&base));
        out.fitted_profile.push(mix(&w));
        out.truth.push(q);
        out.plain.push(base);
        out.weight.push(w);
    }
    out
}

/// One class: the plain-count weights and the fitted ones, from the class's true feature
/// frequencies `q` and its feature frequencies and hand count on each sample flop.
pub fn fit_class(q: &[f64; NF], prof: &[&([f64; NF], f64)], ways: &[f64], iterations: usize) -> (Vec<f64>, Vec<f64>) {
    let nf = prof.len();
    let mut base: Vec<f64> = (0..nf).map(|f| ways[f] * prof[f].1).collect();
    let total: f64 = base.iter().sum();
    if total <= 0.0 { base = vec![1.0 / nf as f64; nf]; } else { for b in base.iter_mut() { *b /= total; } }
    // Least squares on the feature frequencies, kept close to the plain count where the features
    // leave a choice; multiplicative steps keep every weight positive and the total at 1.
    let mut w = base.clone();
    let (tau, eta) = (0.002, 0.25);
    let mut g = vec![0f64; nf];
    for _ in 0..iterations {
        let mut r = [0f64; NF];
        for f in 0..nf { let wf = w[f]; if wf > 0.0 { for k in 0..NF { r[k] += wf * prof[f].0[k]; } } }
        for k in 0..NF { r[k] = 2.0 * LAMBDA[k] * (r[k] - q[k]); }
        let mut sum = 0.0;
        for f in 0..nf {
            if base[f] <= 0.0 { w[f] = 0.0; continue; }
            let mut s = 0.0;
            for k in 0..NF { s += r[k] * prof[f].0[k]; }
            g[f] = s + tau * ((w[f].max(1e-300) / base[f]).ln() + 1.0);
            w[f] *= (-eta * g[f]).exp(); sum += w[f];
        }
        for f in 0..nf { w[f] /= sum; }
    }
    (base, w)
}

// ---------------------------------------------------------------------------------------------
// Valuing a hand kind by kind
//
// With a couple of dozen flops a hand's value hangs on the one or two where it makes two pair or
// trips: worth ten times an ordinary flop, and present for one hand and absent for its neighbour by
// luck. So the value is built from kinds of flop instead, each at its true frequency over all
// flops, and what a kind is worth to a hand leans on its neighbours where its own flops are few.

/// The kinds of flop for a hand, by rank alone (so they are the same for every suit of a class).
pub const KIND_NAMES: [&str; 5] = ["miss", "pair, high card", "pair, low card", "two pair", "trips or better"];
pub const NK: usize = 5;

/// (sort, high rank, low rank) of a class: sort 0 pair, 1 suited, 2 offsuit; rank 12 = ace.
pub fn class_shape(class: usize) -> (u8, u8, u8) {
    let (r, c) = (class / 13, class % 13);
    if r == c { (0, 12 - r as u8, 12 - r as u8) } else if c > r { (1, 12 - r as u8, 12 - c as u8) } else { (2, 12 - c as u8, 12 - r as u8) }
}
fn class_from(sort: u8, hi: u8, lo: u8) -> usize {
    let (a, b) = (12 - hi as usize, 12 - lo as usize); // chart row and column of the two ranks, ace first
    match sort { 0 => a * 13 + a, 1 => a * 13 + b, _ => b * 13 + a }
}

/// Which kind of flop this is for the class. Pocket pairs: 0 without a set, 4 with a set or better.
pub fn kind_of(class: usize, flop: [u8; 3]) -> usize {
    let (sort, hi, lo) = class_shape(class);
    let br = [flop[0] / 4, flop[1] / 4, flop[2] / 4];
    let n_hi = br.iter().filter(|&&r| r == hi).count();
    if sort == 0 { return if n_hi >= 1 { 4 } else { 0 }; }
    let n_lo = br.iter().filter(|&&r| r == lo).count();
    match (n_hi, n_lo) { (0, 0) => 0, (1, 0) => 1, (0, 1) => 2, (1, 1) => 3, _ => 4 }
}

/// How often each kind comes over all 19,600 flops the class can see.
pub fn kind_truth(class: usize) -> [f64; NK] {
    let h = representative(class);
    let rest: Vec<u8> = (0..52u8).filter(|c| *c != h[0] && *c != h[1]).collect();
    let mut q = [0f64; NK];
    let mut n = 0f64;
    for a in 0..rest.len() { for b in a + 1..rest.len() { for c in b + 1..rest.len() { q[kind_of(class, [rest[a], rest[b], rest[c]])] += 1.0; n += 1.0; } } }
    for k in 0..NK { q[k] /= n; }
    q
}

/// The nearest hands to borrow from: same sort and top card, kicker `dist` ranks either side
/// (pocket pairs: the pairs `dist` ranks either side).
fn neighbours(class: usize, dist: u8) -> Vec<usize> {
    let (sort, hi, lo) = class_shape(class);
    let mut out = vec![];
    if sort == 0 {
        if hi >= dist { out.push(class_from(0, hi - dist, hi - dist)); }
        if hi + dist <= 12 { out.push(class_from(0, hi + dist, hi + dist)); }
    } else {
        if lo >= dist { out.push(class_from(sort, hi, lo - dist)); }
        if lo + dist < hi { out.push(class_from(sort, hi, lo + dist)); }
    }
    out
}

/// What `value_by_kinds` worked out, kept for the record.
pub struct KindValues {
    /// each class's value (NaN for a class with no value on any flop)
    pub value: Vec<f64>,
    /// per class and kind: the average over the class's own flops of that kind (NaN if it has none), and how many there were
    pub own: Vec<[f64; NK]>,
    pub count: Vec<[f64; NK]>,
    /// per class and kind: the value used, after leaning on neighbours
    pub used: Vec<[f64; NK]>,
}

/// Values a line kind by kind. `vals[flop][class]` is the class's value on that flop (NaN where it
/// has none); `weight[class][flop]` the fitted weights. Within a kind the class's own flops are
/// averaged with the fitted weights. That average is then pulled toward the same kind's value for
/// the adjacent-kicker hands, as if the neighbours were `lean` extra flops of evidence: a kind with
/// many flops of its own barely moves, one with a single flop moves most of the way, and one with
/// none takes the neighbours' value outright (looking further along the kickers, then to every
/// hand of the same sort, if the adjacent ones have none either). `lean` 0 only fills the gaps.
pub fn value_by_kinds(flops: &[[u8; 3]], weight: &[Vec<f64>], vals: &[Vec<f64>], lean: f64) -> KindValues {
    let nf = flops.len();
    let mut own = vec![[f64::NAN; NK]; NC];
    let mut count = vec![[0f64; NK]; NC];
    for c in 0..NC {
        let (mut num, mut den, mut plain) = ([0f64; NK], [0f64; NK], [0f64; NK]);
        for f in 0..nf {
            let v = vals[f][c];
            if !v.is_finite() { continue; }
            let k = kind_of(c, flops[f]);
            num[k] += weight[c][f] * v; den[k] += weight[c][f]; plain[k] += v; count[c][k] += 1.0;
        }
        for k in 0..NK { if count[c][k] > 0.0 { own[c][k] = if den[k] > 1e-9 { num[k] / den[k] } else { plain[k] / count[c][k] }; } }
    }
    let mut used = vec![[f64::NAN; NK]; NC];
    let mut value = vec![f64::NAN; NC];
    for c in 0..NC {
        if count[c].iter().all(|&n| n == 0.0) { continue; }
        let q = kind_truth(c);
        let (sort, _, _) = class_shape(c);
        let (mut tot, mut qsum) = (0.0, 0.0);
        for k in 0..NK {
            if q[k] <= 0.0 { continue; }
            // the neighbours' value for this kind: adjacent kickers first, then further out
            let mut near = f64::NAN;
            for dist in 1..12u8 {
                let (mut num, mut den) = (0.0, 0.0);
                for nb in neighbours(c, dist) { if count[nb][k] > 0.0 { num += count[nb][k] * own[nb][k]; den += count[nb][k]; } }
                if den > 0.0 { near = num / den; break; }
                if count[c][k] > 0.0 { break; } // a hand with flops of its own only leans on the adjacent ones
            }
            if !near.is_finite() && count[c][k] == 0.0 {
                let (mut num, mut den) = (0.0, 0.0);
                for o in 0..NC { if class_shape(o).0 == sort && count[o][k] > 0.0 { num += count[o][k] * own[o][k]; den += count[o][k]; } }
                if den > 0.0 { near = num / den; }
            }
            let n = count[c][k];
            let v = if n > 0.0 && near.is_finite() { (n * own[c][k] + lean * near) / (n + lean) } else if n > 0.0 { own[c][k] } else { near };
            if v.is_finite() { used[c][k] = v; tot += q[k] * v; qsum += q[k]; }
        }
        if qsum > 0.0 { value[c] = tot / qsum; }
    }
    KindValues { value, own, count, used }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn features_of_known_hands() {
        let c = |s: &str| parse_card(s).unwrap();
        let f = |h: [&str; 2], b: &str| features([c(h[0]), c(h[1])], parse_flop(b).unwrap());
        let on = |x: &[f32; NF], name: &str| x[FEATURE_NAMES.iter().position(|n| *n == name).unwrap()] == 1.0;
        let x = f(["7c", "2d"], "As7d2c"); assert!(on(&x, "two pair") && !on(&x, "top pair"));
        let x = f(["Ac", "Kd"], "As7d2c"); assert!(on(&x, "pair, high card") && on(&x, "top pair") && on(&x, "kicker plays"));
        let x = f(["9c", "9d"], "As7d2c"); assert!(on(&x, "pair, one card above") && !on(&x, "set"));
        let x = f(["9c", "9d"], "9s7d2c"); assert!(on(&x, "set"));
        let x = f(["Qc", "Qd"], "Ts9s8d"); assert!(on(&x, "overpair") && on(&x, "gutshot"));
        let x = f(["Js", "Ts"], "Ks8s3d"); assert!(on(&x, "flush draw, both cards") && on(&x, "no pair"));
        let x = f(["Jc", "Tc"], "Qs9d2c"); assert!(on(&x, "eight-out straight draw") && on(&x, "backdoor flush draw"));
        let x = f(["6c", "5d"], "9s8d7c"); assert!(on(&x, "straight"));
        let x = f(["Ah", "2d"], "Jh9h5h"); assert!(on(&x, "flush draw, one card"));
        let x = f(["Ac", "5d"], "4s3d2c"); assert!(on(&x, "straight"));
        let x = f(["Kc", "4d"], "KsKd4c"); assert!(on(&x, "full house or better"));
        let x = f(["Kc", "Qd"], "KsKd4c"); assert!(on(&x, "trips"));
    }
    #[test]
    fn true_profiles_are_sane() {
        let name = |n: &str| FEATURE_NAMES.iter().position(|x| *x == n).unwrap();
        // an unpaired hand pairs at least one card on about 32.4% of flops
        let q = true_profile(class_of(parse_card("7c").unwrap(), parse_card("2d").unwrap()));
        assert!((q[name("pair or better")] - 0.324).abs() < 0.03, "{}", q[name("pair or better")]);
        // a pocket pair flops a set or better about 11.8% of the time
        let q = true_profile(class_of(parse_card("8c").unwrap(), parse_card("8d").unwrap()));
        assert!((q[name("set")] - 0.1176).abs() < 0.002, "{}", q[name("set")]);
        // suited cards flop a flush draw 10.9% and a flush 0.84%
        let q = true_profile(class_of(parse_card("Jc").unwrap(), parse_card("Tc").unwrap()));
        assert!((q[name("flush draw, both cards")] - 0.1094).abs() < 0.002);
        assert!((q[name("flush")] - 0.0084).abs() < 0.0005);
    }
    #[test]
    fn built_in_samples_fit() {
        let name = |n: &str| FEATURE_NAMES.iter().position(|x| *x == n).unwrap();
        for n in [16, 24, 32] {
            let flops: Vec<[u8; 3]> = sample(n).iter().map(|f| parse_flop(f).unwrap()).collect();
            assert_eq!(flops.len(), n);
            let fw = fit(&flops, None);
            let c = class_of(parse_card("7c").unwrap(), parse_card("2d").unwrap());
            assert!((fw.weight[c].iter().sum::<f64>() - 1.0).abs() < 1e-9);
            // 7-2 offsuit pairs on a third of all flops, whatever the sample happens to hold
            assert!((fw.fitted_profile[c][name("pair or better")] - 0.324).abs() < 0.02, "{n} flops: {}", fw.fitted_profile[c][name("pair or better")]);
        }
    }
    #[test]
    fn kinds_have_true_frequencies_and_fill_gaps() {
        let c = |a: &str, b: &str| class_of(parse_card(a).unwrap(), parse_card(b).unwrap());
        let q = kind_truth(c("Tc", "5d"));
        assert!((q[0] - 0.6757).abs() < 1e-3 && (q[1] - 0.1448).abs() < 1e-3 && (q[3] - 0.0202).abs() < 1e-3 && (q[4] - 0.0145).abs() < 1e-3, "{q:?}");
        let q = kind_truth(c("8c", "8d"));
        assert!((q[4] - 0.1176).abs() < 1e-3 && q[1] == 0.0);
        assert_eq!(kind_of(c("Tc", "5d"), parse_flop("Th5d2c").unwrap()), 3);
        assert_eq!(kind_of(c("Tc", "5d"), parse_flop("5h5d2c").unwrap()), 4);
        assert_eq!(kind_of(c("8c", "8d"), parse_flop("8h5d2c").unwrap()), 4);
        // neighbours of T5o are T4o and T6o; of 88, 77 and 99
        let names = |v: Vec<usize>| -> Vec<String> { v.into_iter().map(crate::class_name).collect() };
        assert_eq!(names(neighbours(c("Tc", "5d"), 1)), vec!["T4o", "T6o"]);
        assert_eq!(names(neighbours(c("8c", "8d"), 1)), vec!["77", "99"]);
        assert_eq!(names(neighbours(c("Tc", "9c"), 1)), vec!["T8s"]);
        // a made-up line: every hand is worth 1 on a miss, 3 on a pair, 10 on two pair or better.
        // Whatever the sample, every unpaired hand must then come out at the same true-odds value.
        let flops: Vec<[u8; 3]> = sample(24).iter().map(|f| parse_flop(f).unwrap()).collect();
        let fw = fit(&flops, None);
        let vals: Vec<Vec<f64>> = flops.iter().map(|f| (0..NC).map(|h| match kind_of(h, *f) { 0 => 1.0, 1 | 2 => 3.0, _ => 10.0 }).collect()).collect();
        let kv = value_by_kinds(&flops, &fw.weight, &vals, 2.0);
        let want = 0.6757 * 1.0 + 0.2896 * 3.0 + 0.0347 * 10.0;
        for h in 0..NC { if class_shape(h).0 != 0 { assert!((kv.value[h] - want).abs() < 0.01, "{}: {}", crate::class_name(h), kv.value[h]); } }
        // T5o has no two-pair flop in the 24, so that kind was borrowed
        let t5 = c("Tc", "5d");
        assert!(kv.count[t5][3] == 0.0 && (kv.used[t5][3] - 10.0).abs() < 1e-9);
    }
    #[test]
    fn suit_ways_counts() {
        assert_eq!(suit_ways(parse_flop("As7d2c").unwrap()), 24.0);
        assert_eq!(suit_ways(parse_flop("Ks8s3d").unwrap()), 12.0);
        assert_eq!(suit_ways(parse_flop("Jh9h5h").unwrap()), 4.0);
        assert_eq!(suit_ways(parse_flop("KsKd4c").unwrap()), 12.0);
        assert_eq!(suit_ways(parse_flop("7s7d4d").unwrap()), 12.0);
    }
}

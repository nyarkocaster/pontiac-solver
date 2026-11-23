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
pub const NF: usize = 31;

/// Names of the features, for reports.
pub const FEATURE_NAMES: [&str; NF] = [
    "no pair", "pair, high card", "pair, low card", "two pair", "trips", "full house or better", "set",
    "pair or better", "two pair or better",
    "top pair", "two overcards", "one overcard", "overpair", "pair, one card above", "pair, two cards above",
    "board paired", "board two-tone", "board one suit", "board ace-high", "board K- or Q-high", "board J- to 9-high", "board connected",
    "flush", "flush draw, both cards", "flush draw, one card", "backdoor flush draw",
    "straight", "eight-out straight draw", "gutshot",
    "kicker plays", "pocket pair, board paired",
];

/// How much each feature counts when the weights are fitted.
const LAMBDA: [f64; NF] = [
    4.0, 4.0, 4.0, 4.0, 4.0, 4.0, 4.0,
    4.0, 4.0,
    2.0, 2.0, 2.0, 3.0, 3.0, 3.0,
    1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0,
    3.0, 3.0, 2.0, 1.0,
    3.0, 2.0, 2.0,
    1.0, 1.0,
];

/// The sample used up to 0.13.3, in its original order (the first eight were the short sample).
pub const SAMPLE_16: [&str; 16] = ["As7d2c", "Ks8s3d", "QhJh6c", "Ts9s8d", "7s6d5c", "KsKd4c", "8s3d2c", "Jh9h5h",
    "AsKdTc", "Qs7d2c", "9c6c4d", "7s7d4d", "Ah8h5c", "Jd7d4s", "Ts6d2c", "As5d5c"];

/// The built-in sample of `n` flops.
pub fn sample(n: usize) -> Vec<&'static str> {
    SAMPLE_16.iter().take(n.min(SAMPLE_16.len())).copied().collect()
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
    fn suit_ways_counts() {
        assert_eq!(suit_ways(parse_flop("As7d2c").unwrap()), 24.0);
        assert_eq!(suit_ways(parse_flop("Ks8s3d").unwrap()), 12.0);
        assert_eq!(suit_ways(parse_flop("Jh9h5h").unwrap()), 4.0);
        assert_eq!(suit_ways(parse_flop("KsKd4c").unwrap()), 12.0);
        assert_eq!(suit_ways(parse_flop("7s7d4d").unwrap()), 12.0);
    }
}

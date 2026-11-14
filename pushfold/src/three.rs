//! Three-handed push/fold (button, small blind, big blind), in chips or for prize money.
//! Each player has one decision: move all-in (or call all-in) or fold.
use crate::icm::icm;
use crate::tables::*;
use crate::{class_name, equity};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

static THREE: &[u8] = include_bytes!("../data/three_way_169.bin");

#[derive(Clone, Debug, Deserialize)]
pub struct Spot3 {
    /// stacks of button, small blind, big blind in big blinds, before antes and blinds
    pub stacks: [f64; 3],
    pub small_blind: f64,
    /// ante paid by each of the three players
    #[serde(default)]
    pub ante: f64,
    /// big-blind ante: one ante for the table, paid by the big blind
    #[serde(default)]
    pub bb_ante: f64,
    /// prizes, first place first; empty = play for chips
    #[serde(default)]
    pub payouts: Vec<f64>,
    /// stacks of players still in the tournament who are not in this hand
    #[serde(default)]
    pub others: Vec<f64>,
    #[serde(default)]
    pub iterations: usize,
}

#[derive(Serialize, Clone, Debug)]
pub struct Decision { pub id: String, pub title: String, pub pct: f64, pub freq: Vec<f64>, pub edge: Vec<f64>, pub range: String }

#[derive(Serialize, Clone, Debug)]
pub struct PushFold3 {
    pub players: [String; 3],
    pub stacks_bb: [f64; 3],
    pub small_blind: f64, pub ante: f64, pub bb_ante: f64,
    pub payouts: Vec<f64>, pub others_bb: Vec<f64>,
    /// "chips" or "prize money"
    pub playing_for: String,
    /// what `value` and `edge` are measured in
    pub value_unit: String,
    /// average result per player: big blinds won or lost (chips), or expected prize (prize money)
    pub value: [f64; 3],
    /// expected prize before the hand if everyone folded their way to the same stacks (prize money only)
    pub value_before: Option<[f64; 3]>,
    /// the most any one player could gain by changing only their own ranges, in the same unit
    pub max_gain: [f64; 3],
    pub decisions: Vec<Decision>,
    pub classes: Vec<String>,
    pub iterations: usize,
}

struct Game {
    w: Vec<f32>,            // ways to deal (i, j, k)
    ug: [Vec<f32>; 3],      // three-way all-in value per player
    e2: Vec<f32>,           // heads-up equity
    ua: [f64; 3], ub: [f64; 3], ud: [f64; 3],
    uc: [[f64; 3]; 2], ue: [[f64; 3]; 2], uf: [[f64; 3]; 2], // [first-named player wins, second wins]
}

fn finals_3way(r: [f64; 3], dead: f64, order: [usize; 3]) -> [f64; 3] {
    let mut sorted = r; sorted.sort_by(|a, b| b.partial_cmp(a).unwrap());
    let cap = sorted[1];
    let c = [r[0].min(cap), r[1].min(cap), r[2].min(cap)];
    let mut f = [r[0] - c[0], r[1] - c[1], r[2] - c[2]];
    let mut levels = c.to_vec(); levels.sort_by(|a, b| a.partial_cmp(b).unwrap()); levels.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
    let mut prev = 0.0; let mut first = true;
    for &lv in &levels {
        let inpot: Vec<usize> = (0..3).filter(|&p| c[p] >= lv - 1e-12).collect();
        let mut pot = (lv - prev) * inpot.len() as f64;
        if first { pot += dead; first = false; }
        let winner = *order.iter().find(|p| inpot.contains(p)).unwrap();
        f[winner] += pot; prev = lv;
    }
    f
}

impl Spot3 {
    fn build(&self) -> Result<Game, String> {
        let s = self.stacks;
        for (n, &x) in s.iter().enumerate() {
            let need = self.ante + if n == 2 { self.bb_ante + 1.0 } else if n == 1 { self.small_blind } else { 0.0 };
            if !(x > need + 1e-9) { return Err("every stack must cover its ante and blind with chips to spare".into()); }
            if x > 400.0 { return Err("stacks above 400 big blinds are outside what push/fold is for".into()); }
        }
        if !(self.small_blind > 0.0 && self.small_blind <= 1.0) { return Err("the small blind must be above 0 and at most 1 big blind".into()); }
        if self.ante < 0.0 || self.bb_ante < 0.0 { return Err("antes cannot be negative".into()); }
        if self.payouts.iter().any(|&p| p < 0.0) || self.others.iter().any(|&p| p <= 0.0) { return Err("prizes cannot be negative and other stacks must be above zero".into()); }
        if self.payouts.len() > 6 || self.others.len() > 7 { return Err("at most 6 prizes and 7 other stacks".into()); }
        let icm_mode = !self.payouts.is_empty();
        let value = |f: [f64; 3]| -> [f64; 3] {
            if !icm_mode { return [f[0] - s[0], f[1] - s[1], f[2] - s[2]]; }
            let mut all = f.to_vec(); all.extend_from_slice(&self.others);
            let e = icm(&all, &self.payouts); [e[0], e[1], e[2]]
        };
        let dead = 3.0 * self.ante + self.bb_ante;
        let r = [s[0] - self.ante, s[1] - self.ante, s[2] - self.ante - self.bb_ante];
        let sb = self.small_blind;
        let duel = |a: usize, b: usize, extra: [f64; 3], pot: f64| -> [[f64; 3]; 2] {
            let m = r[a].min(r[b]);
            let mut out = [[0.0; 3]; 2];
            for (n, (win, lose)) in [(a, b), (b, a)].into_iter().enumerate() {
                let mut f = [r[0] - extra[0], r[1] - extra[1], r[2] - extra[2]];
                f[win] += m + pot; f[lose] -= m;
                out[n] = value(f);
            }
            out
        };
        let base = tri_base();
        let counts = |t: usize| u16::from_le_bytes([THREE[t * 2], THREE[t * 2 + 1]]) as f32;
        let probs = |t: usize, o: usize| THREE[NTRI * 2 + t * 6 + o] as f32 / 255.0;
        let ord_val: Vec<[f64; 3]> = ORDERS.iter().map(|o| value(finals_3way(r, dead, *o))).collect();
        let n3 = NC * NC * NC;
        let mut w = vec![0f32; n3];
        let mut ug = [vec![0f32; n3], vec![0f32; n3], vec![0f32; n3]];
        for i in 0..NC { for j in 0..NC { for k in 0..NC {
            // sort the three classes, remembering which seat sits at which sorted position
            let mut seat = [(i, 0usize), (j, 1), (k, 2)]; seat.sort();
            let t = base[seat[0].0 * NC + seat[1].0] as usize + (seat[2].0 - seat[1].0);
            let at = (i * NC + j) * NC + k;
            w[at] = counts(t);
            if w[at] == 0.0 { continue; }
            let mut v = [0f64; 3];
            for (o, ord) in ORDERS.iter().enumerate() {
                let p = probs(t, o) as f64; if p == 0.0 { continue; }
                // ORDERS is in sorted positions; turn it into seats
                let seats = [seat[ord[0]].1, seat[ord[1]].1, seat[ord[2]].1];
                let oi = ORDERS.iter().position(|x| *x == seats).unwrap();
                for q in 0..3 { v[q] += p * ord_val[oi][q]; }
            }
            for q in 0..3 { ug[q][at] = v[q] as f32; }
        } } }
        Ok(Game {
            w, ug, e2: (0..NC * NC).map(|x| equity(x / NC, x % NC) as f32).collect(),
            ua: value([r[0], r[1] - sb, r[2] + dead + sb]),
            ub: value([r[0], r[1] + dead + 1.0, r[2] - 1.0]),
            ud: value([r[0] + dead + sb + 1.0, r[1] - sb, r[2] - 1.0]),
            uc: duel(1, 2, [0.0; 3], dead),
            ue: duel(0, 1, [0.0, 0.0, 1.0], dead + 1.0),
            uf: duel(0, 2, [0.0, sb, 0.0], dead + sb),
        })
    }
}

/// Six decision tables of 169 frequencies each: button jam, small blind call (after a button jam),
/// small blind jam (button folded), big blind call vs button, vs small blind, vs both.
type Strat = [Vec<f64>; 6];
/// For every decision and class: value of [fold, commit], weighted by how often the spot comes up.
type Vals = [Vec<[f64; 2]>; 6];

fn values(g: &Game, s: &Strat) -> (Vals, [f64; 3], f64) {
    let mix = |e: f32, u: &[[f64; 3]; 2], p: usize| e as f64 * u[0][p] + (1.0 - e as f64) * u[1][p];
    let parts: Vec<(Vec<[f64; 2]>, Vec<[f64; 2]>, Vec<[f64; 2]>, Vec<[f64; 2]>, Vec<[f64; 2]>, Vec<[f64; 2]>, [f64; 3], f64)> = (0..NC).into_par_iter().map(|i| {
        let z = || vec![[0f64; 2]; NC];
        let (mut btn, mut sbc, mut sbj, mut b1, mut b2, mut b3) = ([0f64; 2], z(), z(), z(), z(), z());
        let (mut ev, mut tot) = ([0f64; 3], 0f64);
        let x = s[0][i];
        for j in 0..NC {
            let (y, zj) = (s[1][j], s[2][j]);
            let eij = g.e2[i * NC + j];
            let ue = [mix(eij, &g.ue, 0), mix(eij, &g.ue, 1), mix(eij, &g.ue, 2)];
            for k in 0..NC {
                let at = (i * NC + j) * NC + k;
                let w = g.w[at] as f64; if w == 0.0 { continue; }
                let (c1, c2, c3) = (s[3][k], s[4][k], s[5][k]);
                let (eik, ejk) = (g.e2[i * NC + k], g.e2[j * NC + k]);
                let uf = [mix(eik, &g.uf, 0), mix(eik, &g.uf, 1), mix(eik, &g.uf, 2)];
                let uc = [mix(ejk, &g.uc, 0), mix(ejk, &g.uc, 1), mix(ejk, &g.uc, 2)];
                let ug = [g.ug[0][at] as f64, g.ug[1][at] as f64, g.ug[2][at] as f64];
                // value to each player of the branches below each decision
                let after_fold = |p: usize| (1.0 - zj) * g.ua[p] + zj * ((1.0 - c2) * g.ub[p] + c2 * uc[p]);
                let sb_folds = |p: usize| (1.0 - c1) * g.ud[p] + c1 * uf[p];
                let sb_calls = |p: usize| (1.0 - c3) * ue[p] + c3 * ug[p];
                let after_jam = |p: usize| (1.0 - y) * sb_folds(p) + y * sb_calls(p);
                btn[0] += w * after_fold(0); btn[1] += w * after_jam(0);
                sbc[j][0] += w * x * sb_folds(1); sbc[j][1] += w * x * sb_calls(1);
                sbj[j][0] += w * (1.0 - x) * g.ua[1]; sbj[j][1] += w * (1.0 - x) * ((1.0 - c2) * g.ub[1] + c2 * uc[1]);
                b1[k][0] += w * x * (1.0 - y) * g.ud[2]; b1[k][1] += w * x * (1.0 - y) * uf[2];
                b2[k][0] += w * (1.0 - x) * zj * g.ub[2]; b2[k][1] += w * (1.0 - x) * zj * uc[2];
                b3[k][0] += w * x * y * ue[2]; b3[k][1] += w * x * y * ug[2];
                for p in 0..3 { ev[p] += w * ((1.0 - x) * after_fold(p) + x * after_jam(p)); }
                tot += w;
            }
        }
        let mut b = z(); b[i] = btn;
        (b, sbc, sbj, b1, b2, b3, ev, tot)
    }).collect();
    let z = || vec![[0f64; 2]; NC];
    let mut v: Vals = [z(), z(), z(), z(), z(), z()];
    let (mut ev, mut tot) = ([0f64; 3], 0f64);
    for p in &parts {
        for (d, src) in [&p.0, &p.1, &p.2, &p.3, &p.4, &p.5].into_iter().enumerate() { for c in 0..NC { v[d][c][0] += src[c][0]; v[d][c][1] += src[c][1]; } }
        for q in 0..3 { ev[q] += p.6[q]; }
        tot += p.7;
    }
    (v, [ev[0] / tot, ev[1] / tot, ev[2] / tot], tot)
}

pub fn solve(spot: &Spot3) -> Result<PushFold3, String> {
    let g = spot.build()?;
    let iters = if spot.iterations == 0 { 200 } else { spot.iterations.clamp(50, 5000) };
    let init = |f: f64| vec![f; NC];
    let mut cur: Strat = [init(0.5), init(0.5), init(0.5), init(0.5), init(0.5), init(0.5)];
    let mut regret: [Vec<[f64; 2]>; 6] = std::array::from_fn(|_| vec![[0f64; 2]; NC]);
    let mut sum: Strat = std::array::from_fn(|_| init(0.0));
    let mut wsum = 0f64;
    for t in 1..=iters {
        let (v, _, _) = values(&g, &cur);
        for d in 0..6 { for c in 0..NC {
            let now = (1.0 - cur[d][c]) * v[d][c][0] + cur[d][c] * v[d][c][1];
            let r = &mut regret[d][c];
            r[0] = (r[0] + v[d][c][0] - now).max(0.0); r[1] = (r[1] + v[d][c][1] - now).max(0.0);
            cur[d][c] = if r[0] + r[1] > 0.0 { r[1] / (r[0] + r[1]) } else { 0.5 };
        } }
        let wt = t as f64;
        for d in 0..6 { for c in 0..NC { sum[d][c] += wt * cur[d][c]; } }
        wsum += wt;
    }
    let mut avg: Strat = std::array::from_fn(|d| sum[d].iter().map(|x| x / wsum).collect());
    for d in 0..6 { for x in avg[d].iter_mut() { if *x < 0.005 { *x = 0.0 } else if *x > 0.995 { *x = 1.0 } } }
    let (v, ev, tot) = values(&g, &avg);
    let owner = [0usize, 1, 1, 2, 2, 2];
    let mut gain = [0f64; 3];
    for d in 0..6 { for c in 0..NC {
        let now = (1.0 - avg[d][c]) * v[d][c][0] + avg[d][c] * v[d][c][1];
        gain[owner[d]] += (v[d][c][0].max(v[d][c][1]) - now) / tot;
    } }
    let combos = |i: usize| { let (r, c) = (i / 13, i % 13); if r == c { 6.0 } else if c > r { 4.0 } else { 12.0 } };
    let icm_mode = !spot.payouts.is_empty();
    let r = |x: f64, dp: i32| { let m = 10f64.powi(dp); (x * m).round() / m };
    let titles = [
        ("btn_jam", "Button moves all-in"), ("sb_call", "Small blind calls the button's all-in"), ("sb_jam", "Small blind moves all-in after the button folds"),
        ("bb_call_btn", "Big blind calls the button (small blind folded)"), ("bb_call_sb", "Big blind calls the small blind (button folded)"), ("bb_call_both", "Big blind calls after the button jams and the small blind calls"),
    ];
    let classes: Vec<String> = (0..NC).map(class_name).collect();
    let decisions = (0..6).map(|d| {
        let pct = (0..NC).map(|c| combos(c) * avg[d][c]).sum::<f64>() / 1326.0;
        // edge: what committing gains over folding, per time the spot comes up
        let edge: Vec<f64> = (0..NC).map(|c| {
            // weight of the spot for this class: recover it from a fold line of known value where possible; otherwise compare totals
            let reach: f64 = match d { 0 => (0..NC * NC).map(|x| g.w[c * NC * NC + x] as f64).sum(), _ => 0.0 };
            if d == 0 { if reach > 0.0 { (v[d][c][1] - v[d][c][0]) / reach } else { 0.0 } } else { v[d][c][1] - v[d][c][0] }
        }).collect();
        let range = classes.iter().zip(&avg[d]).filter(|(_, &f)| f > 0.0).map(|(c, &f)| if f >= 1.0 { c.clone() } else { format!("{c}:{}", r(f, 2)) }).collect::<Vec<_>>().join(",");
        Decision { id: titles[d].0.into(), title: titles[d].1.into(), pct: r(pct, 4), freq: avg[d].iter().map(|&x| r(x, 3)).collect(), edge, range }
    }).collect::<Vec<_>>();
    // edges for the later decisions need the weight of the spot; get it by valuing a "1 for commit, 0 for fold" game
    let mut decisions = decisions;
    {
        let s = &avg;
        let mut reach = [vec![0f64; NC], vec![0f64; NC], vec![0f64; NC], vec![0f64; NC], vec![0f64; NC]];
        for i in 0..NC { for j in 0..NC { for k in 0..NC {
            let w = g.w[(i * NC + j) * NC + k] as f64; if w == 0.0 { continue; }
            let (x, y, zj) = (s[0][i], s[1][j], s[2][j]);
            reach[0][j] += w * x; reach[1][j] += w * (1.0 - x);
            reach[2][k] += w * x * (1.0 - y); reach[3][k] += w * (1.0 - x) * zj; reach[4][k] += w * x * y;
        } } }
        for d in 1..6 { for c in 0..NC { let q = reach[d - 1][c]; decisions[d].edge[c] = if q > 1e-9 { decisions[d].edge[c] / q } else { 0.0 }; } }
        for d in decisions.iter_mut() { for e in d.edge.iter_mut() { *e = r(*e, 4); } }
    }
    let before = if icm_mode {
        let mut all = spot.stacks.to_vec(); all.extend_from_slice(&spot.others);
        let e = icm(&all, &spot.payouts); Some([r(e[0], 4), r(e[1], 4), r(e[2], 4)])
    } else { None };
    Ok(PushFold3 {
        players: ["BTN".into(), "SB".into(), "BB".into()], stacks_bb: spot.stacks, small_blind: spot.small_blind, ante: spot.ante, bb_ante: spot.bb_ante,
        payouts: spot.payouts.clone(), others_bb: spot.others.clone(),
        playing_for: if icm_mode { "prize money".into() } else { "chips".into() },
        value_unit: if icm_mode { "prize, in the units the payouts were given in".into() } else { "big blinds per hand".into() },
        value: [r(ev[0], 4), r(ev[1], 4), r(ev[2], 4)], value_before: before,
        max_gain: [r(gain[0], 5), r(gain[1], 5), r(gain[2], 5)],
        decisions, classes, iterations: iters,
    })
}

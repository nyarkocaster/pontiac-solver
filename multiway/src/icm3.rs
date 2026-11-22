//! Tournament prize-money payoffs (Independent Chip Model) for the three-player engine.
//!
//! In chips, a result is worth the chips won. Here every way a hand can end is worth what the
//! stacks it leaves are worth in prizes, so a player's result depends on the whole finishing
//! order of the showdown, and a player who has folded still cares how the other two finish.
//!
//! Showdown weights are exact, card removal included; the test at the end checks every kind of
//! showdown against a count of every pair of opposing hands.
use crate::cards::NH;
use crate::game::*;
use std::collections::HashMap;

/// Malmuth-Harville: each player's expected prize given the stacks. A player with no chips takes
/// what is left after everyone with chips has placed.
pub fn icm_values(stacks: &[f64], payouts: &[f64]) -> Vec<f64> {
    let n = stacks.len();
    let places = payouts.len().min(n);
    let mut out = vec![0.0; n];
    let mut prob = vec![0.0f64; 1 << n];
    prob[0] = 1.0;
    for mask in 0..(1usize << n) {
        let pm = prob[mask];
        if pm == 0.0 { continue; }
        let k = mask.count_ones() as usize;
        if k >= places { continue; }
        let left: f64 = (0..n).filter(|&i| mask >> i & 1 == 0).map(|i| stacks[i].max(0.0)).sum();
        let free = (0..n).filter(|&i| mask >> i & 1 == 0).count() as f64;
        for i in 0..n {
            if mask >> i & 1 == 1 { continue; }
            let p = if left > 0.0 { stacks[i].max(0.0) / left } else { 1.0 / free };
            if p == 0.0 { continue; }
            out[i] += pm * p * payouts[k];
            prob[mask | 1 << i] += pm * p;
        }
    }
    out
}

/// Settings: the prizes (first place first) and the stacks of players not in this hand.
#[derive(Clone, Debug, Default)]
pub struct IcmCfg { pub payouts: Vec<f64>, pub others: Vec<f64> }

/// Class of a finish from one player's side: 9 * (first opponent vs me) + 3 * (second opponent
/// vs me) + (first opponent vs second), each 0 = lower, 1 = tied, 2 = higher.
pub const CLASSES: usize = 27;

pub struct Icm3 {
    pub cfg: IcmCfg,
    /// prize money per chip, so payoffs keep the scale of chips
    pub per_chip: f64,
    pub prize: f64,
    /// each player's prize value if the starting pot were shared by those still in at the start
    pub base: [f64; 3],
    /// the chips that sharing gives each player (added back so payoffs line up with chip play)
    pub shift: [f64; 3],
    /// per terminal node: index into `util` (u32::MAX elsewhere)
    pub node_util: Vec<u32>,
    /// per distinct ending: payoff for each player in each class, in chip-sized units
    pub util: Vec<[[f64; CLASSES]; 3]>,
    /// per final board: for each card and strength group r, how many hands holding the card are
    /// weaker than r (row width = groups + 3)
    pub below: Vec<(usize, Vec<u8>)>,
}

fn rel(a: u8, b: u8) -> usize { if a < b { 0 } else if a == b { 1 } else { 2 } }

impl Icm3 {
    pub fn new(g: &Game, cfg: IcmCfg) -> Result<Icm3, String> {
        if cfg.payouts.is_empty() || cfg.payouts.iter().any(|x| !x.is_finite() || *x < 0.0) || cfg.payouts.iter().sum::<f64>() <= 0.0 { return Err("prize money: give at least one prize, none negative".into()); }
        if cfg.others.iter().any(|x| !x.is_finite() || *x < 0.0) { return Err("prize money: other players' stacks cannot be negative".into()); }
        if cfg.others.len() > 7 { return Err("prize money: at most 7 players outside the hand".into()); }
        if g.cfg.extra.rake_rate > 0.0 { return Err("prize money cannot be combined with rake".into()); }
        let n = 3 + cfg.others.len();
        let total: f64 = g.stacks.iter().sum::<f64>() + g.cfg.pot + cfg.others.iter().sum::<f64>();
        let prize: f64 = cfg.payouts.iter().take(n).sum();
        let per_chip = prize / total;
        let value = |st: [f64; 3]| -> [f64; 3] { let mut all = st.to_vec(); all.extend(cfg.others.iter().cloned()); let e = icm_values(&all, &cfg.payouts); [e[0], e[1], e[2]] };
        let live = g.cfg.extra.folded.iter().filter(|&&f| !f).count() as f64;
        let mut shift = [0.0; 3]; let mut reference = g.stacks;
        for p in 0..3 { if !g.cfg.extra.folded[p] { shift[p] = g.cfg.pot / live; reference[p] += shift[p]; } }
        let base = value(reference);
        let mut node_util = vec![u32::MAX; g.nodes.len()];
        let mut util: Vec<[[f64; CLASSES]; 3]> = vec![];
        let mut seen: HashMap<Vec<i64>, u32> = HashMap::new();
        for (id, node) in g.nodes.iter().enumerate() {
            if !matches!(node.kind, Kind::FoldEnd | Kind::Showdown) { continue; }
            let mut key: Vec<i64> = node.invested.iter().map(|x| (x * 1e6).round() as i64).collect();
            key.push(node.folded.iter().enumerate().map(|(i, &f)| (f as i64) << i).sum());
            key.push(if node.kind == Kind::FoldEnd { 10 + node.player as i64 } else { 0 });
            if let Some(&k) = seen.get(&key) { node_util[id] = k; continue; }
            let mut table = [[0.0; CLASSES]; 3];
            for code in 0..27u8 {
                let st = [code % 3, code / 3 % 3, code / 9];
                let mut fin = [0.0; 3];
                for p in 0..3 { fin[p] = g.stacks[p] - node.invested[p]; }
                if node.kind == Kind::FoldEnd { fin[node.player] += node.pots[0].0; }
                else {
                    for (amount, mask) in &node.pots {
                        let best = (0..3).filter(|&p| mask[p]).map(|p| st[p]).max();
                        let Some(best) = best else { continue };
                        let winners: Vec<usize> = (0..3).filter(|&p| mask[p] && st[p] == best).collect();
                        for &w in &winners { fin[w] += amount / winners.len() as f64; }
                    }
                }
                let e = value(fin);
                for h in 0..3 {
                    let (o1, o2) = ((h + 1) % 3, (h + 2) % 3);
                    let cls = 9 * rel(st[o1], st[h]) + 3 * rel(st[o2], st[h]) + rel(st[o1], st[o2]);
                    table[h][cls] = (e[h] - base[h]) / per_chip + shift[h];
                }
            }
            seen.insert(key, util.len() as u32);
            node_util[id] = util.len() as u32;
            util.push(table);
        }
        let below = g.streets.iter().map(|sd| {
            let groups = sd.by_card.iter().flat_map(|v| v.iter().map(|x| x.0 as usize)).max().unwrap_or(0);
            let w = groups + 3;
            let mut t = vec![0u8; 52 * w];
            for c in 0..52 { let v = &sd.by_card[c]; let mut k = 0usize; for r in 0..w { while k < v.len() && (v[k].0 as usize) < r { k += 1; } t[c * w + r] = k as u8; } }
            (w, t)
        }).collect();
        Ok(Icm3 { cfg, per_chip, prize, base, shift, node_util, util, below })
    }
    /// Share of the prize pool (percent) for a player whose average result, counted the engine's
    /// way (chips won from the start of the spot, in chip-sized units), is `ev`.
    pub fn share_pct(&self, player: usize, ev: f64) -> f64 { (self.base[player] + (ev - self.shift[player]) * self.per_chip) / self.prize * 100.0 }
    pub fn split_pct(&self, player: usize) -> f64 { self.base[player] / self.prize * 100.0 }
}

/// One player's reach on a final board, arranged for quick "weaker than / tied with / all" sums
/// that leave out hands sharing a card with a given hand.
struct Dist { z: Vec<f64>, cum: Vec<f64>, pre: Vec<f64>, tot: f64 }

const SLOTS: usize = 49;

impl Dist {
    fn new(sd: &Street, z: Vec<f64>) -> Dist {
        let groups = sd.by_card.iter().flat_map(|v| v.iter().map(|x| x.0 as usize)).max().unwrap_or(0);
        let mut cum = vec![0.0; 52 * SLOTS];
        let mut bucket = vec![0.0; groups + 2];
        for c in 0..52 {
            let mut acc = 0.0;
            for (k, &(rank, h)) in sd.by_card[c].iter().enumerate() { let v = z[h as usize]; acc += v; cum[c * SLOTS + k + 1] = acc; bucket[rank as usize] += v; }
        }
        // every hand was added once per card
        let mut pre = vec![0.0; groups + 2];
        for i in 1..groups + 2 { pre[i] = pre[i - 1] + bucket[i - 1] * 0.5; }
        let tot = pre[groups + 1];
        Dist { z, cum, pre, tot }
    }
    /// (weaker than rank s, tied with s, all): hands sharing a card with (a, b) left out.
    /// `own` is the hand index of (a, b); `ia` and `ib` are (lo, hi) positions of s among each card's partners.
    #[inline]
    fn sums(&self, sd: &Street, a: usize, b: usize, s: usize, own: usize, ia: (usize, usize), ib: (usize, usize)) -> (f64, f64, f64) {
        let (ca, cb) = (&self.cum[a * SLOTS..(a + 1) * SLOTS], &self.cum[b * SLOTS..(b + 1) * SLOTS]);
        let lower = self.pre[s] - ca[ia.0] - cb[ib.0];
        let tie = (self.pre[s + 1] - self.pre[s]) - (ca[ia.1] - ca[ia.0]) - (cb[ib.1] - cb[ib.0]) + self.z[own];
        let all = self.tot - ca[sd.by_card[a].len()] - cb[sd.by_card[b].len()] + self.z[own];
        (lower, tie, all)
    }
}

impl Game {
    /// Value for `hero` (dense over hero's hands) of a hand that ends at terminal node `id`,
    /// in prize money. `r` are the three reaches.
    /// The first, slower way of working out `icm_terminal`; kept as a second opinion for the tests.
    pub fn icm_terminal_slow(&self, id: usize, hero: usize, r: [&[f32]; 3]) -> Vec<f32> {
        let icm = self.icm.as_ref().unwrap();
        let node = &self.nodes[id];
        let u = &icm.util[icm.node_util[id] as usize][hero];
        let (o1, o2) = ((hero + 1) % 3, (hero + 2) % 3);
        if node.kind == Kind::FoldEnd { return self.reach_sum(hero, r[o1], r[o2]).iter().map(|&x| (u[13] * x) as f32).collect(); }
        let sd = &self.streets[node.street];
        let f = node.folded;
        let nhh = self.hands[hero].len();
        let mut out = vec![0f32; nhh];
        let dense = |p: usize| -> Vec<f64> { let mut z = vec![0.0; NH]; for &h in &sd.sorted[p] { z[h as usize] = r[p][h as usize] as f64; } z };
        let (z1, z2) = (dense(o1), dense(o2));
        let cls = |x: usize, y: usize, z: usize| 9 * x + 3 * y + z;
        let (bw, bt) = &icm.below[node.street];
        // hands holding card c that are weaker than strength group r; "at most r" is lo(c, r + 1)
        let lo = |c: usize, r: usize| bt[c * bw + r] as usize;
        let at = |c: usize, r: usize| (lo(c, r), lo(c, r + 1));
        let cards = |h: u16| { let (a, b) = self.deck.cards[h as usize]; (a as usize, b as usize, sd.rank[a as usize * W + b as usize] as usize) };
        // weight of `d` on hands strictly between two strength groups that avoid the given cards
        let window = |d: &Dist, from: usize, to: usize, avoid: [usize; 3]| -> f64 {
            if to <= from + 1 { return 0.0; }
            let mut v = d.pre[to] - d.pre[from + 1];
            for (i, &x) in avoid.iter().enumerate() {
                if avoid[..i].contains(&x) { continue; }
                v -= d.cum[x * SLOTS + lo(x, to)] - d.cum[x * SLOTS + lo(x, from + 1)];
                for &y in &avoid[..i] { if y != x { let k = sd.rank[x * W + y] as usize; if k > from && k < to { v += d.z[self.deck.idx[x][y] as usize]; } } }
            }
            v
        };
        let both = !f[o1] && !f[o2];
        let (d1, d2) = (Dist::new(sd, z1.clone()), Dist::new(sd, z2.clone()));
        // for each hand of one opponent: the other's weight below it and above it (hands sharing a card left out)
        let mut order: Option<[Dist; 4]> = None;
        if both {
            let (mut zl2, mut zh2, mut zl1, mut zh1) = (vec![0.0; NH], vec![0.0; NH], vec![0.0; NH], vec![0.0; NH]);
            for &h in &sd.sorted[o2] {
                let w = z2[h as usize]; if w == 0.0 { continue; }
                let (a, b, s) = cards(h);
                let (l, t, all) = d1.sums(sd, a, b, s, h as usize, at(a, s), at(b, s));
                zl2[h as usize] = w * l; zh2[h as usize] = w * (all - l - t);
            }
            for &h in &sd.sorted[o1] {
                let w = z1[h as usize]; if w == 0.0 { continue; }
                let (a, b, s) = cards(h);
                let (l, t, all) = d2.sums(sd, a, b, s, h as usize, at(a, s), at(b, s));
                zl1[h as usize] = w * l; zh1[h as usize] = w * (all - l - t);
            }
            order = Some([Dist::new(sd, zl2), Dist::new(sd, zh2), Dist::new(sd, zl1), Dist::new(sd, zh1)]);
        }
        // The sums above let an opponent hold one of hero's cards in the inner hand. This takes that
        // back out: for each hand of `zo` holding a or b whose strength passes `keep`, its weight
        // times the other opponent's weight (`dother`) strictly between `from(k)` and `to(k)`.
        let overlap = |zo: &Vec<f64>, dother: &Dist, a: usize, b: usize, keep: &dyn Fn(usize) -> bool, span: &dyn Fn(usize) -> (usize, usize)| -> f64 {
            let mut v = 0.0;
            for (card, skip) in [(a, usize::MAX), (b, a)] {
                for &(k, h) in &sd.by_card[card] {
                    let w = zo[h as usize]; if w == 0.0 { continue; }
                    let (x, y) = self.deck.cards[h as usize]; let c = if x as usize == card { y as usize } else { x as usize };
                    if c == skip || !keep(k as usize) { continue; }
                    let (from, to) = span(k as usize);
                    v += w * window(dother, from, to, [a, b, c]);
                }
            }
            v
        };
        if f[hero] {
            // hero is watching: only the order of the other two matters
            let ord = order.as_ref().expect("a showdown needs two players");
            let total = self.reach_sum(hero, r[o1], r[o2]);
            for &h in &sd.sorted[hero] {
                let (a, b, s) = cards(h);
                let (ia, ib) = (at(a, s), at(b, s));
                // first opponent ahead: every hand of the second times the first's weight above it
                let ahead = ord[1].sums(sd, a, b, s, h as usize, ia, ib).2 - overlap(&z1, &d2, a, b, &|_| true, &|k| (0, k));
                let behind = ord[3].sums(sd, a, b, s, h as usize, ia, ib).2 - overlap(&z2, &d1, a, b, &|_| true, &|k| (0, k));
                let pos = self.hpos[hero][h as usize] as usize;
                let tied = total[pos] - ahead - behind;
                out[pos] = (behind * u[cls(0, 0, 0)] + tied * u[cls(0, 0, 1)] + ahead * u[cls(0, 0, 2)]) as f32;
            }
            return out;
        }
        let zq: Vec<f64> = (0..NH).map(|h| z1[h] * z2[h]).collect();
        let dq = Dist::new(sd, zq);
        for &h in &sd.sorted[hero] {
            let (a, b, s) = cards(h);
            let (ia, ib) = (at(a, s), at(b, s));
            // sums over cards of (first opponent's weight on the card) x (second's), by category
            let mut p = [[0.0f64; 3]; 3];
            for c in 0..52 {
                let n = sd.by_card[c].len();
                if n == 0 || c == a || c == b { continue; }
                let (l, hi) = at(c, s);
                let (c1, c2) = (&d1.cum[c * SLOTS..], &d2.cum[c * SLOTS..]);
                let (ha, hb) = (self.deck.idx[a][c] as usize, self.deck.idx[b][c] as usize);
                let (ka, kb) = (sd.rank[a * W + c] as usize, sd.rank[b * W + c] as usize);
                let (la, lb, ea, eb) = ((ka < s) as u8 as f64, (kb < s) as u8 as f64, (ka == s) as u8 as f64, (kb == s) as u8 as f64);
                let x1 = [c1[l] - z1[ha] * la - z1[hb] * lb, (c1[hi] - c1[l]) - z1[ha] * ea - z1[hb] * eb, c1[n] - z1[ha] - z1[hb]];
                let x2 = [c2[l] - z2[ha] * la - z2[hb] * lb, (c2[hi] - c2[l]) - z2[ha] * ea - z2[hb] * eb, c2[n] - z2[ha] - z2[hb]];
                for x in 0..3 { for y in 0..3 { p[x][y] += x1[x] * x2[y]; } }
            }
            let (s1, s2, sq) = (d1.sums(sd, a, b, s, h as usize, ia, ib), d2.sums(sd, a, b, s, h as usize, ia, ib), dq.sums(sd, a, b, s, h as usize, ia, ib));
            let (t1, t2, tq) = ([s1.0, s1.1, s1.2], [s2.0, s2.1, s2.2], [sq.0, sq.1, sq.2]);
            // weight of (first opponent in category x, second in y), x and y: weaker, tied, anything
            let mut g = [[0.0f64; 3]; 3];
            for x in 0..3 { for y in 0..3 {
                let shared = if x == 2 { tq[y] } else if y == 2 || x == y { tq[x] } else { 0.0 };
                g[x][y] = t1[x] * t2[y] - p[x][y] + shared;
            } }
            // turn "anything" into "stronger"
            let mut m = [[0.0f64; 3]; 3];
            for x in 0..2 { for y in 0..2 { m[x][y] = g[x][y]; } }
            for x in 0..2 { m[x][2] = g[x][2] - g[x][0] - g[x][1]; }
            for y in 0..2 { m[2][y] = g[2][y] - g[0][y] - g[1][y]; }
            m[2][2] = g[2][2] - m[0][0] - m[0][1] - m[1][0] - m[1][1] - m[0][2] - m[1][2] - m[2][0] - m[2][1];
            let pos = self.hpos[hero][h as usize] as usize;
            let v = if both {
                let ord = order.as_ref().unwrap();
                let o: Vec<(f64, f64, f64)> = ord.iter().map(|d| d.sums(sd, a, b, s, h as usize, ia, ib)).collect();
                // both weaker than hero: first below second, then first above second; the rest of that weight is a tie between them
                let ll0 = o[0].0 - overlap(&z1, &d2, a, b, &|k| k < s, &|k| (k, s));
                let ll2 = o[2].0 - overlap(&z2, &d1, a, b, &|k| k < s, &|k| (k, s));
                // both stronger than hero
                let hh2 = (o[1].2 - o[1].0 - o[1].1) - overlap(&z1, &d2, a, b, &|k| k > s, &|k| (s, k));
                let hh0 = (o[3].2 - o[3].0 - o[3].1) - overlap(&z2, &d1, a, b, &|k| k > s, &|k| (s, k));
                ll0 * u[cls(0, 0, 0)] + (m[0][0] - ll0 - ll2) * u[cls(0, 0, 1)] + ll2 * u[cls(0, 0, 2)]
                    + hh0 * u[cls(2, 2, 0)] + (m[2][2] - hh0 - hh2) * u[cls(2, 2, 1)] + hh2 * u[cls(2, 2, 2)]
                    + m[0][1] * u[cls(0, 1, 0)] + m[0][2] * u[cls(0, 2, 0)] + m[1][0] * u[cls(1, 0, 2)] + m[1][1] * u[cls(1, 1, 1)]
                    + m[1][2] * u[cls(1, 2, 0)] + m[2][0] * u[cls(2, 0, 2)] + m[2][1] * u[cls(2, 1, 2)]
            } else if f[o2] {
                // only the first opponent shows down; the second's cards still block hands
                let (l, t, all) = (g[0][2], g[1][2], g[2][2]);
                l * u[cls(0, 0, 2)] + t * u[cls(1, 0, 2)] + (all - l - t) * u[cls(2, 0, 2)]
            } else {
                let (l, t, all) = (g[2][0], g[2][1], g[2][2]);
                l * u[cls(0, 0, 0)] + t * u[cls(0, 1, 0)] + (all - l - t) * u[cls(0, 2, 0)]
            };
            out[pos] = v as f32;
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------
// Fast showdowns. Same answers as `icm_terminal_slow` (kept as the reference the tests compare
// against), worked out in sweeps over the hands in strength order, the way the chips engine does.

thread_local! {
    /// Scratch for `Order`: two card-by-card tables per thread.
    static ORD: std::cell::RefCell<[Vec<f32>; 2]> = std::cell::RefCell::new([vec![0f32; 52 * W], vec![0f32; 52 * W]]);
}

/// The nine products for one card: (first opponent lower, tied, any) x (second lower, tied, any).
struct Kern9<'a> {
    s: u16,
    ca1: &'a [f32; W], ca2: &'a [f32; W], cl1: &'a [f32; W], cl2: &'a [f32; W], ct1: &'a [f32; W], ct2: &'a [f32; W],
    r1a: &'a [f32; W], r1b: &'a [f32; W], r2a: &'a [f32; W], r2b: &'a [f32; W], ka: &'a [u16; W], kb: &'a [u16; W],
}
impl Kern9<'_> {
    #[inline(always)]
    fn term(&self, c: usize) -> [f32; 9] {
        let la = if self.ka[c] < self.s { 1.0f32 } else { 0.0 }; let lb = if self.kb[c] < self.s { 1.0f32 } else { 0.0 };
        let ea = if self.ka[c] == self.s { 1.0f32 } else { 0.0 }; let eb = if self.kb[c] == self.s { 1.0f32 } else { 0.0 };
        let (p1a, p1b, p2a, p2b) = (self.r1a[c], self.r1b[c], self.r2a[c], self.r2b[c]);
        let a1 = self.ca1[c] - p1a - p1b; let a2 = self.ca2[c] - p2a - p2b;
        let l1 = self.cl1[c] - p1a * la - p1b * lb; let t1 = self.ct1[c] - p1a * ea - p1b * eb;
        let l2 = self.cl2[c] - p2a * la - p2b * lb; let t2 = self.ct2[c] - p2a * ea - p2b * eb;
        [l1 * l2, l1 * t2, l1 * a2, t1 * l2, t1 * t2, t1 * a2, a1 * l2, a1 * t2, a1 * a2]
    }
}
#[inline(always)]
fn kern9_body<const M: u8>(k: &Kern9) -> [f32; 9] {
    let mut acc = [[0f32; 8]; 9];
    for base in (0..W).step_by(8) {
        let (mut l1, mut t1, mut a1, mut l2, mut t2, mut a2) = ([0f32; 8], [0f32; 8], [0f32; 8], [0f32; 8], [0f32; 8], [0f32; 8]);
        for j in 0..8 {
            let c = base + j;
            let la = if k.ka[c] < k.s { 1.0f32 } else { 0.0 }; let lb = if k.kb[c] < k.s { 1.0f32 } else { 0.0 };
            let ea = if k.ka[c] == k.s { 1.0f32 } else { 0.0 }; let eb = if k.kb[c] == k.s { 1.0f32 } else { 0.0 };
            let (p1a, p1b, p2a, p2b) = (k.r1a[c], k.r1b[c], k.r2a[c], k.r2b[c]);
            a1[j] = k.ca1[c] - p1a - p1b; a2[j] = k.ca2[c] - p2a - p2b;
            l1[j] = k.cl1[c] - p1a * la - p1b * lb; t1[j] = k.ct1[c] - p1a * ea - p1b * eb;
            l2[j] = k.cl2[c] - p2a * la - p2b * lb; t2[j] = k.ct2[c] - p2a * ea - p2b * eb;
        }
        // with one opponent folded only that row or column of the table is read
        if M == 1 { for j in 0..8 { acc[2][j] += l1[j] * a2[j]; acc[5][j] += t1[j] * a2[j]; acc[8][j] += a1[j] * a2[j]; } }
        else if M == 2 { for j in 0..8 { acc[6][j] += a1[j] * l2[j]; acc[7][j] += a1[j] * t2[j]; acc[8][j] += a1[j] * a2[j]; } }
        else {
            for j in 0..8 { acc[0][j] += l1[j] * l2[j]; acc[1][j] += l1[j] * t2[j]; acc[2][j] += l1[j] * a2[j]; }
            for j in 0..8 { acc[3][j] += t1[j] * l2[j]; acc[4][j] += t1[j] * t2[j]; acc[5][j] += t1[j] * a2[j]; }
            for j in 0..8 { acc[6][j] += a1[j] * l2[j]; acc[7][j] += a1[j] * t2[j]; acc[8][j] += a1[j] * a2[j]; }
        }
    }
    let mut out = [0f32; 9];
    for i in 0..9 { out[i] = acc[i].iter().sum(); }
    out
}
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2,fma")]
unsafe fn kern9_avx2<const M: u8>(k: &Kern9) -> [f32; 9] { kern9_body::<M>(k) }
#[inline(always)]
fn kern9<const M: u8>(k: &Kern9) -> [f32; 9] {
    #[cfg(target_arch = "x86_64")]
    { if std::is_x86_feature_detected!("avx2") && std::is_x86_feature_detected!("fma") { return unsafe { kern9_avx2::<M>(k) }; } }
    kern9_body::<M>(k)
}

/// For one hand (y, d) of weight `w` in strength group `g`: per card x, `w` times the other
/// player's weight on hands holding x that come before `g` and share no card with (y, d).
#[inline(always)]
fn pair_body(c: &[f32; W], ry: &[f32; W], rd: &[f32; W], ky: &[u16; W], kd: &[u16; W], g: u16, desc: bool, w: f32, u: &mut [f32; W], ty: &mut [f32; W], td: &mut [f32; W]) {
    for x in 0..W {
        let my = if desc { ky[x] > g } else { ky[x] < g };
        let md = if desc { kd[x] > g } else { kd[x] < g };
        let v = w * (c[x] - if my { ry[x] } else { 0.0 } - if md { rd[x] } else { 0.0 });
        u[x] += v; ty[x] += v; td[x] += v;
    }
}
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2,fma")]
unsafe fn pair_avx2(c: &[f32; W], ry: &[f32; W], rd: &[f32; W], ky: &[u16; W], kd: &[u16; W], g: u16, desc: bool, w: f32, u: &mut [f32; W], ty: &mut [f32; W], td: &mut [f32; W]) { pair_body(c, ry, rd, ky, kd, g, desc, w, u, ty, td) }
/// Adds the result to `u` and to the two table rows `ty` and `td`.
#[inline(always)]
fn pair(c: &[f32; W], ry: &[f32; W], rd: &[f32; W], ky: &[u16; W], kd: &[u16; W], g: u16, desc: bool, w: f32, u: &mut [f32; W], ty: &mut [f32; W], td: &mut [f32; W]) {
    #[cfg(target_arch = "x86_64")]
    { if std::is_x86_feature_detected!("avx2") && std::is_x86_feature_detected!("fma") { return unsafe { pair_avx2(c, ry, rd, ky, kd, g, desc, w, u, ty, td) }; } }
    pair_body(c, ry, rd, ky, kd, g, desc, w, u, ty, td)
}

/// Running totals for "one opponent comes before the other" while sweeping the hands in strength
/// order. Side 0: the first opponent before the second; side 1: the second before the first.
/// "Before" means weaker on a rising sweep and stronger on a falling one.
struct Order<'a> {
    /// per side and card x: weight of pairs (early hand holding x, late hand sharing no card with it)
    u: [[f32; W]; 2],
    /// per side, cards y and x: the same, with the late hand holding y (index y * W + x)
    dt: &'a mut [Vec<f32>; 2],
    /// per side: total of (late hand's weight x early weight before it), and the same by the late hand's cards
    tz: [f64; 2], cz: [[f64; 52]; 2],
    /// weight already swept, per player and card, and in total
    c: [[f32; W]; 2], t: [f64; 2],
}
impl Order<'_> {
    /// Weight of pairs on `side`, both hands already swept, neither holding a or b. A hand (a, b)
    /// itself must not have been swept yet (callers add what it needs).
    #[inline(always)]
    fn read(&self, side: usize, a: usize, b: usize) -> f64 {
        self.tz[side] - self.cz[side][a] - self.cz[side][b] - self.u[side][a] as f64 - self.u[side][b] as f64
            + self.dt[side][b * W + a] as f64 + self.dt[side][a * W + b] as f64
    }
}

impl Game {
    /// Sweeps the two opponents' hands on a final board in strength order (rising, or falling with
    /// `desc`), keeping `Order` up to date. `at_hero` is called for each hero hand when everything
    /// strictly before its strength has been swept; it also gets the opponents' hands tied with it.
    fn order_sweep(&self, sd: &Street, hero: usize, z: [&[f32]; 2], m: [&[f32]; 2], desc: bool, mut at_hero: impl FnMut(Option<HandInfo>, &Order, &[HandInfo], &[HandInfo])) {
        let (o1, o2) = ((hero + 1) % 3, (hero + 2) % 3);
        let key = |e: &HandInfo| -> u16 { if desc { u16::MAX - e.k } else { e.k } };
        let list = |p: usize| -> Vec<HandInfo> { let mut v = sd.info[p].clone(); if desc { v.reverse(); } v };
        let (l1, l2, lh) = (list(o1), list(o2), list(hero));
        ORD.with(|d| {
            let mut d = d.borrow_mut();
            d[0].fill(0.0); d[1].fill(0.0);
            let mut st = Order { u: [[0.0; W]; 2], dt: &mut *d, tz: [0.0; 2], cz: [[0.0; 52]; 2], c: [[0.0; W]; 2], t: [0.0; 2] };
            let (mut i1, mut i2, mut ih) = (0usize, 0usize, 0usize);
            loop {
                let k = [l1.get(i1), l2.get(i2), lh.get(ih)].iter().filter_map(|x| x.map(|e| key(e))).min();
                let Some(k) = k else { break };
                let g = if desc { u16::MAX - k } else { k };
                let (mut e1, mut e2) = (i1, i2);
                while e1 < l1.len() && key(&l1[e1]) == k { e1 += 1; }
                while e2 < l2.len() && key(&l2[e2]) == k { e2 += 1; }
                while ih < lh.len() && key(&lh[ih]) == k { at_hero(Some(lh[ih]), &st, &l1[i1..e1], &l2[i2..e2]); ih += 1; }
                // late hands of this strength against everything before it
                for (side, late, early, range) in [(0usize, 1usize, 0usize, &l2[i2..e2]), (1, 0, 1, &l1[i1..e1])] {
                    for e in range {
                        let w = z[late][e.h as usize]; if w == 0.0 { continue; }
                        let (y, dd) = (e.a as usize, e.b as usize);
                        let zl = w as f64 * (st.t[early] - st.c[early][y] as f64 - st.c[early][dd] as f64);
                        st.tz[side] += zl; st.cz[side][y] += zl; st.cz[side][dd] += zl;
                        // the hand's own two cards take no part: put back what the pass adds there
                        let t = &mut st.dt[side];
                        let keep = [st.u[side][y], st.u[side][dd], t[y * W + y], t[y * W + dd], t[dd * W + y], t[dd * W + dd]];
                        let (lo, hi) = t.split_at_mut(y.max(dd) * W);
                        let (first, second): (&mut [f32; W], &mut [f32; W]) = ((&mut lo[y.min(dd) * W..y.min(dd) * W + W]).try_into().unwrap(), (&mut hi[..W]).try_into().unwrap());
                        pair(&st.c[early], row(m[early], y), row(m[early], dd), rowu(&sd.rank, y), rowu(&sd.rank, dd), g, desc, w, &mut st.u[side], first, second);
                        st.u[side][y] = keep[0]; st.u[side][dd] = keep[1];
                        let t = &mut st.dt[side];
                        t[y * W + y] = keep[2]; t[y * W + dd] = keep[3]; t[dd * W + y] = keep[4]; t[dd * W + dd] = keep[5];
                    }
                }
                for (p, range) in [(0usize, &l1[i1..e1]), (1, &l2[i2..e2])] {
                    for e in range {
                        let w = z[p][e.h as usize]; if w == 0.0 { continue; }
                        st.c[p][e.a as usize] += w; st.c[p][e.b as usize] += w; st.t[p] += w as f64;
                    }
                }
                i1 = e1; i2 = e2;
            }
            // one last call per side so the caller can read the finished totals
            at_hero(None, &st, &[], &[]);
        });
    }

    /// For every hero hand that can reach this showdown: the weight of (first opponent lower /
    /// tied / anything) x (second opponent lower / tied / anything), all three hands sharing no
    /// card. `shows`: 0 both opponents show down, 1 only the first, 2 only the second (then only
    /// the "anything" column or row is filled). `out(position in hero's hand list, table)`.
    fn nine_sweep(&self, sd: &Street, hero: usize, z: [&[f32]; 2], m: [&[f32]; 2], shows: u8, mut out: impl FnMut(usize, [[f64; 3]; 3])) {
        let (o1, o2) = ((hero + 1) % 3, (hero + 2) % 3);
        let (l1, l2, lh) = (&sd.info[o1][..], &sd.info[o2][..], &sd.info[hero][..]);
        let (r1, r2) = (z[0], z[1]);
        let mut ca = [[0f32; W]; 2];
        let (mut ta, mut caq) = ([0f64; 3], [0f64; 52]);
        for e in l1 {
            let h = e.h as usize; let x = r1[h]; if x == 0.0 { continue; }
            let (a, b) = (e.a as usize, e.b as usize);
            let q = x as f64 * r2[h] as f64;
            ta[0] += x as f64; ta[2] += q; ca[0][a] += x; ca[0][b] += x; caq[a] += q; caq[b] += q;
        }
        for e in l2 {
            let h = e.h as usize; let y = r2[h]; if y == 0.0 { continue; }
            let (a, b) = (e.a as usize, e.b as usize);
            ta[1] += y as f64; ca[1][a] += y; ca[1][b] += y;
        }
        let mut cl = [[0f32; W]; 2]; let (mut tl, mut clq) = ([0f64; 3], [0f64; 52]);
        let (mut i1, mut i2, mut ih) = (0usize, 0usize, 0usize);
        while ih < lh.len() {
            let s = lh[ih].k;
            while i1 < l1.len() && l1[i1].k < s {
                let e = l1[i1]; let h = e.h as usize; i1 += 1; let x = r1[h]; if x == 0.0 { continue; }
                let (a, b) = (e.a as usize, e.b as usize);
                let q = x as f64 * r2[h] as f64;
                tl[0] += x as f64; tl[2] += q; cl[0][a] += x; cl[0][b] += x; clq[a] += q; clq[b] += q;
            }
            while i2 < l2.len() && l2[i2].k < s {
                let e = l2[i2]; let h = e.h as usize; i2 += 1; let y = r2[h]; if y == 0.0 { continue; }
                let (a, b) = (e.a as usize, e.b as usize);
                tl[1] += y as f64; cl[1][a] += y; cl[1][b] += y;
            }
            let mut ct = [[0f32; W]; 2]; let (mut tt, mut ctq) = ([0f64; 3], [0f64; 52]);
            let mut j = i1;
            while j < l1.len() && l1[j].k == s {
                let e = l1[j]; let h = e.h as usize; j += 1; let x = r1[h]; if x == 0.0 { continue; }
                let (a, b) = (e.a as usize, e.b as usize);
                let q = x as f64 * r2[h] as f64;
                tt[0] += x as f64; tt[2] += q; ct[0][a] += x; ct[0][b] += x; ctq[a] += q; ctq[b] += q;
            }
            let mut j = i2;
            while j < l2.len() && l2[j].k == s {
                let e = l2[j]; let h = e.h as usize; j += 1; let y = r2[h]; if y == 0.0 { continue; }
                let (a, b) = (e.a as usize, e.b as usize);
                tt[1] += y as f64; ct[1][a] += y; ct[1][b] += y;
            }
            while ih < lh.len() && lh[ih].k == s {
                let e = lh[ih]; let h = e.h as usize; ih += 1;
                let (a, b) = (e.a as usize, e.b as usize);
                let me = [r1[h] as f64, r2[h] as f64, r1[h] as f64 * r2[h] as f64];
                let xa = |k: usize| ta[k] - if k < 2 { ca[k][a] as f64 + ca[k][b] as f64 } else { caq[a] + caq[b] } + me[k];
                let xl = |k: usize| tl[k] - if k < 2 { cl[k][a] as f64 + cl[k][b] as f64 } else { clq[a] + clq[b] };
                let xt = |k: usize| tt[k] - if k < 2 { ct[k][a] as f64 + ct[k][b] as f64 } else { ctq[a] + ctq[b] } + me[k];
                let k = Kern9 { s, ca1: &ca[0], ca2: &ca[1], cl1: &cl[0], cl2: &cl[1], ct1: &ct[0], ct2: &ct[1],
                    r1a: row(m[0], a), r1b: row(m[0], b), r2a: row(m[1], a), r2b: row(m[1], b), ka: rowu(&sd.rank, a), kb: rowu(&sd.rank, b) };
                let mut bb = match shows { 1 => kern9::<1>(&k), 2 => kern9::<2>(&k), _ => kern9::<0>(&k) };
                for c in [a, b] { let t = k.term(c); for i in 0..9 { bb[i] -= t[i]; } }
                let (x1, x2, sq) = ([xl(0), xt(0), xa(0)], [xl(1), xt(1), xa(1)], [xl(2), xt(2), xa(2)]);
                let mut g = [[0f64; 3]; 3];
                for x in 0..3 { for y in 0..3 {
                    let shared = if x == 2 { sq[y] } else if y == 2 || x == y { sq[x] } else { 0.0 };
                    g[x][y] = x1[x] * x2[y] - bb[3 * x + y] as f64 + shared;
                } }
                out(e.pos as usize, g);
            }
        }
    }

    /// Value for `hero` (dense over hero's hands) of a hand that ends at terminal node `id`,
    /// in prize money. `r` are the three reaches.
    pub fn icm_terminal(&self, id: usize, hero: usize, r: [&[f32]; 3]) -> Vec<f32> {
        let icm = self.icm.as_ref().unwrap();
        let node = &self.nodes[id];
        let u = &icm.util[icm.node_util[id] as usize][hero];
        let (o1, o2) = ((hero + 1) % 3, (hero + 2) % 3);
        if node.kind == Kind::FoldEnd { return self.reach_sum(hero, r[o1], r[o2]).iter().map(|&x| (u[13] * x) as f32).collect(); }
        let sd = &self.streets[node.street];
        let f = node.folded;
        let nhh = self.hands[hero].len();
        let cls = |x: usize, y: usize, z: usize| 9 * x + 3 * y + z;
        let total = if f[hero] { self.reach_sum(hero, r[o1], r[o2]) } else { vec![] };
        let mut acc = vec![0f64; nhh];
        let cards = &self.deck.cards;
        TABLES.with(|t| {
            let mut t = t.borrow_mut(); let (m1, m2) = &mut *t;
            for (p, m) in [(o1, &mut *m1), (o2, &mut *m2)] {
                for &h in &sd.sorted[p] { let x = r[p][h as usize]; if x == 0.0 { continue; } let (a, b) = cards[h as usize]; let (a, b) = (a as usize, b as usize); m[a * W + b] = x; m[b * W + a] = x; }
            }
            let (z, m): ([&[f32]; 2], [&[f32]; 2]) = ([r[o1], r[o2]], [&m1[..], &m2[..]]);
            if f[hero] {
                // hero is watching: only the order of the other two matters. `all[p]`: p's weight in total and by card.
                let mut all = ([0f64; 2], [[0f64; 52]; 2]);
                for (p, q) in [(0usize, o1), (1, o2)] { for &h in &sd.sorted[q] { let x = z[p][h as usize] as f64; let (a, b) = cards[h as usize]; all.0[p] += x; all.1[p][a as usize] += x; all.1[p][b as usize] += x; } }
                // per hero hand: what its own cards as an opponent's hand add back, for each side
                let mut extra: Vec<(HandInfo, [f64; 2])> = Vec::with_capacity(sd.sorted[hero].len());
                self.order_sweep(sd, hero, z, m, false, |e, st, tie1, tie2| {
                    let Some(e) = e else {
                        for &(e, x) in &extra {
                            let (a, b) = (e.a as usize, e.b as usize);
                            let behind = st.read(0, a, b) + x[0];
                            let ahead = st.read(1, a, b) + x[1];
                            let pos = e.pos as usize;
                            acc[pos] = behind * u[cls(0, 0, 0)] + (total[pos] - ahead - behind) * u[cls(0, 0, 1)] + ahead * u[cls(0, 0, 2)];
                        }
                        return;
                    };
                    let (a, b, h) = (e.a as usize, e.b as usize, e.h as usize);
                    let own = [z[0][h] as f64, z[1][h] as f64];
                    let mut x = [0f64; 2];
                    if own[0] != 0.0 || own[1] != 0.0 {
                        // weight of player p on hands without a or b: weaker than this hand, and stronger
                        let part = |p: usize, tie: &[HandInfo]| -> (f64, f64) {
                            let lower = st.t[p] - st.c[p][a] as f64 - st.c[p][b] as f64;
                            let mut tied = 0f64;
                            for t in tie { let (c, d) = (t.a as usize, t.b as usize); if c != a && c != b && d != a && d != b { tied += z[p][t.h as usize] as f64; } }
                            (lower, all.0[p] - all.1[p][a] - all.1[p][b] + own[p] - lower - tied)
                        };
                        let (p1, p2) = (part(0, tie1), part(1, tie2));
                        // side 0 (first below second): this hand as the second's late hand, and as the first's early hand
                        x[0] = own[1] * p1.0 + own[0] * p2.1;
                        x[1] = own[0] * p2.0 + own[1] * p1.1;
                    }
                    extra.push((e, x));
                });
            } else if f[o1] || f[o2] {
                // one opponent shows down; the other's cards still block hands
                self.nine_sweep(sd, hero, z, m, if f[o2] { 1 } else { 2 }, |pos, g| {
                    acc[pos] = if f[o2] { let (l, t, all) = (g[0][2], g[1][2], g[2][2]); l * u[cls(0, 0, 2)] + t * u[cls(1, 0, 2)] + (all - l - t) * u[cls(2, 0, 2)] }
                        else { let (l, t, all) = (g[2][0], g[2][1], g[2][2]); l * u[cls(0, 0, 0)] + t * u[cls(0, 1, 0)] + (all - l - t) * u[cls(0, 2, 0)] };
                });
            } else {
                // turn "anything" into "stronger"; the both-weaker and both-stronger corners are split by sub-order below
                let mut corner = vec![(0f64, 0f64); nhh];
                self.nine_sweep(sd, hero, z, m, 0, |pos, g| {
                    let mut mm = [[0.0f64; 3]; 3];
                    for x in 0..2 { for y in 0..2 { mm[x][y] = g[x][y]; } }
                    for x in 0..2 { mm[x][2] = g[x][2] - g[x][0] - g[x][1]; }
                    for y in 0..2 { mm[2][y] = g[2][y] - g[0][y] - g[1][y]; }
                    mm[2][2] = g[2][2] - mm[0][0] - mm[0][1] - mm[1][0] - mm[1][1] - mm[0][2] - mm[1][2] - mm[2][0] - mm[2][1];
                    corner[pos] = (mm[0][0], mm[2][2]);
                    acc[pos] = mm[0][1] * u[cls(0, 1, 0)] + mm[0][2] * u[cls(0, 2, 0)] + mm[1][0] * u[cls(1, 0, 2)] + mm[1][1] * u[cls(1, 1, 1)]
                        + mm[1][2] * u[cls(1, 2, 0)] + mm[2][0] * u[cls(2, 0, 2)] + mm[2][1] * u[cls(2, 1, 2)];
                });
                for desc in [false, true] {
                    self.order_sweep(sd, hero, z, m, desc, |e, st, _, _| {
                        let Some(e) = e else { return };
                        let (a, b, pos) = (e.a as usize, e.b as usize, e.pos as usize);
                        // side 0: first opponent before the second (weaker on the way up, stronger on the way down)
                        let (first, second) = (st.read(0, a, b), st.read(1, a, b));
                        let (base, which) = if desc { (2, corner[pos].1) } else { (0, corner[pos].0) };
                        let (lo, hi) = if desc { (second, first) } else { (first, second) };
                        acc[pos] += lo * u[cls(base, base, 0)] + (which - lo - hi) * u[cls(base, base, 1)] + hi * u[cls(base, base, 2)];
                    });
                }
            }
            for (p, m) in [(o1, &mut *m1), (o2, &mut *m2)] {
                for &h in &sd.sorted[p] { let (a, b) = cards[h as usize]; let (a, b) = (a as usize, b as usize); m[a * W + b] = 0.0; m[b * W + a] = 0.0; }
            }
        });
        acc.iter().map(|&x| x as f32).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::study::*;

    #[test]
    fn icm_values_basics() {
        // one prize: share of chips. Equal stacks: equal shares. Published three-player example.
        let v = icm_values(&[30.0, 10.0], &[100.0]);
        assert!((v[0] - 75.0).abs() < 1e-9 && (v[1] - 25.0).abs() < 1e-9);
        let v = icm_values(&[5.0, 5.0, 5.0], &[50.0, 30.0, 20.0]);
        for x in &v { assert!((x - 100.0 / 3.0).abs() < 1e-9); }
        let v = icm_values(&[50.0, 30.0, 20.0], &[50.0, 30.0, 20.0]);
        assert!((v.iter().sum::<f64>() - 100.0).abs() < 1e-9);
        assert!((v[0] - 38.393).abs() < 0.01 && (v[1] - 32.75).abs() < 0.01 && (v[2] - 28.857).abs() < 0.01, "{v:?}");
        // a player with no chips gets the last prize
        let v = icm_values(&[10.0, 10.0, 0.0], &[50.0, 30.0, 20.0]);
        assert!((v[2] - 20.0).abs() < 1e-9 && (v[0] - 40.0).abs() < 1e-9);
    }

    /// Every kind of showdown against a count of every pair of opposing hands.
    #[test]
    fn showdown_values_match_a_full_count() {
        let spec = Spec3 {
            ranges: ["99-66,A9s-A6s,KTs,QJs,JTo,T9s".into(), "TT-77,AJs-A8s,KQo,KJs,98s,87s".into(), "AA-88,AKs-ATs,KQs,QJs,JTs,T9s,65s".into()],
            board: "Kh9s6d4c2s".into(), pot: 15.0, stack: 100.0, stacks: Some([40.0, 100.0, 65.0]),
            bet_sizes: "50%".into(), raise_sizes: "60%".into(), max_raises: 1, allin_spr: 3.0, threads: 1, turn_cards: 0, river_cards: 0,
            icm_payouts: vec![50.0, 30.0, 20.0], icm_others: vec![30.0, 12.0], ..Default::default()
        };
        let g = build_game(&spec, false).unwrap();
        let icm = g.icm.as_ref().unwrap();
        let mut seed = 0x9E3779B97F4A7C15u64;
        let mut rnd = || { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; (seed >> 40) as f32 / (1u64 << 24) as f32 };
        let mut reach = [vec![0f32; NH], vec![0f32; NH], vec![0f32; NH]];
        for p in 0..3 { for &h in &g.hands[p] { let x = rnd(); reach[p][h as usize] = if x < 0.2 { 0.0 } else { g.cfg.ranges[p][h as usize] * x }; } }
        let mut seen = std::collections::HashSet::new();
        let (mut exact_worst, mut split_worst) = (0f64, 0f64);
        for (id, node) in g.nodes.iter().enumerate() {
            if node.kind != Kind::Showdown { continue; }
            let key = (node.folded, icm.node_util[id]);
            if !seen.insert(key) || seen.len() > 40 { continue; }
            let sd = &g.streets[node.street];
            for hero in 0..3 {
                let (o1, o2) = ((hero + 1) % 3, (hero + 2) % 3);
                let u = &icm.util[icm.node_util[id] as usize][hero];
                let got = g.icm_terminal(id, hero, [&reach[0], &reach[1], &reach[2]]);
                let three = !node.folded[0] && !node.folded[1] && !node.folded[2];
                let mut scale = 0f64; let mut diff = 0f64;
                for &h in &g.hands[hero] {
                    let (a, b) = g.deck.cards[h as usize];
                    let s = sd.strength[h as usize];
                    let mut want = 0f64;
                    if s > 0 {
                        for &h1 in &g.hands[o1] {
                            let (c, d) = g.deck.cards[h1 as usize]; let s1 = sd.strength[h1 as usize];
                            if s1 == 0 || c == a || c == b || d == a || d == b || reach[o1][h1 as usize] == 0.0 { continue; }
                            for &h2 in &g.hands[o2] {
                                let (e, f) = g.deck.cards[h2 as usize]; let s2 = sd.strength[h2 as usize];
                                if s2 == 0 || e == a || e == b || f == a || f == b || e == c || e == d || f == c || f == d { continue; }
                                let w = reach[o1][h1 as usize] as f64 * reach[o2][h2 as usize] as f64;
                                // a folded player's cards count for nothing: give them the weakest hand
                                let (x1, x2, xh) = (if node.folded[o1] { 0 } else { s1 }, if node.folded[o2] { 0 } else { s2 }, if node.folded[hero] { u32::MAX } else { s });
                                let r = |p: u32, q: u32| if p < q { 0 } else if p == q { 1 } else { 2 };
                                want += w * u[9 * r(x1, xh) + 3 * r(x2, xh) + r(x1, x2)];
                            }
                        }
                    }
                    let v = got[g.hpos[hero][h as usize] as usize] as f64;
                    scale = scale.max(want.abs()); diff = diff.max((v - want).abs());
                }
                let rel = diff / scale.max(1e-9);
                // kept apart so a failure says which kind is wrong: three-way showdowns and a watching hero, or the rest
                if three || node.folded[hero] { split_worst = split_worst.max(rel); } else { exact_worst = exact_worst.max(rel); }
            }
        }
        println!("two at showdown: worst {exact_worst:.2e}; three at showdown or hero watching: worst {split_worst:.2e}; endings tried {}", seen.len());
        assert!(seen.len() >= 3);
        assert!(exact_worst < 1e-4, "{exact_worst}");
        assert!(split_worst < 1e-4, "{split_worst}");
    }
    /// The fast showdown code against the slower first version, on every kind of ending of a
    /// tree that starts on the turn (several river cards), with uneven stacks and side pots.
    #[test]
    fn fast_showdowns_match_the_slow_ones() {
        let spec = Spec3 {
            ranges: ["99-22,A9s-A2s,K9s-K2s,Q9s+,J9s+,T8s+,ATo-A8o,KTo+,QTo+".into(), "TT-22,AJs-A2s,KTs+,QTs+,JTs,T9s,98s,AQo-ATo,KQo".into(), "AA-22,AKs-A2s,K9s+,Q9s+,J9s+,T8s+,98s,AKo-ATo,KJo+".into()],
            board: "Kh9s6d4c".into(), pot: 15.0, stack: 100.0, stacks: Some([40.0, 100.0, 65.0]),
            bet_sizes: "50%".into(), raise_sizes: "60%".into(), max_raises: 1, allin_spr: 3.0, threads: 1, turn_cards: 0, river_cards: 1,
            icm_payouts: vec![50.0, 30.0, 20.0], icm_others: vec![30.0, 12.0], ..Default::default()
        };
        let g = build_game(&spec, false).unwrap();
        let icm = g.icm.as_ref().unwrap();
        let mut seed = 0x2545F4914F6CDD1Du64;
        let mut rnd = || { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; (seed >> 40) as f32 / (1u64 << 24) as f32 };
        let mut seen = std::collections::HashSet::new();
        let (mut worst, mut kinds) = (0f64, [0usize; 3]);
        for (id, node) in g.nodes.iter().enumerate() {
            if node.kind != Kind::Showdown { continue; }
            if !seen.insert((node.folded, icm.node_util[id], node.street % 5)) { continue; }
            let sd = &g.streets[node.street];
            let mut reach = [vec![0f32; NH], vec![0f32; NH], vec![0f32; NH]];
            for p in 0..3 { for &h in &sd.sorted[p] { let x = rnd(); reach[p][h as usize] = if x < 0.25 { 0.0 } else { g.cfg.ranges[p][h as usize] * x }; } }
            for hero in 0..3 {
                let r = [&reach[0][..], &reach[1][..], &reach[2][..]];
                let (fast, slow) = (g.icm_terminal(id, hero, r), g.icm_terminal_slow(id, hero, r));
                let scale = slow.iter().fold(0f64, |m, &x| m.max(x.abs() as f64)).max(1e-9);
                for i in 0..fast.len() { worst = worst.max((fast[i] as f64 - slow[i] as f64).abs() / scale); }
                kinds[if node.folded[hero] { 0 } else if node.folded.iter().any(|&f| f) { 1 } else { 2 }] += 1;
            }
        }
        println!("fast against slow: worst {worst:.2e}; hero watching {}, two at showdown {}, three at showdown {}", kinds[0], kinds[1], kinds[2]);
        assert!(kinds.iter().all(|&k| k >= 10), "{kinds:?}");
        assert!(worst < 2e-5, "{worst}");
    }
}

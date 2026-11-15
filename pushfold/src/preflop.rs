//! Heads-up preflop with limps, raises, re-raises and all-ins, for any stack depth.
//!
//! All-in pots are exact (the same all-in table push/fold uses). Pots that see a flop with chips
//! behind are *modelled*, not solved: each player gets their all-in share of the pot, shifted
//! toward the player in position, the last raiser, and hands that play well after the flop, and
//! the shift grows with the chips left behind. The size of that shift was set by solving flops
//! with the postflop engine (see `FlopModel`). Treat the output as a strong starting point, and
//! check the lines that matter in the postflop solver; `lines` hands over the ranges for that.
use crate::{class_name, combos, compat, equity};
use serde::{Deserialize, Serialize};

const N: usize = 169;

/// How pots that reach a flop are valued. Pot shares, as fractions of the pot.
///
/// The defaults were fitted at 20 big blinds by solving the model's own lines on five or six flops
/// with the two-player postflop engine and comparing the button's share of the pot. Limped pots
/// (the common line): model 63.7%, solved 64.5%. Raise-and-call pots: the model was 2 points low
/// in one round and 9 points high in another, as the raising range changed shape; one shift
/// cannot follow that. Against a published solver chart for heads-up tournaments (Poker Academy,
/// small ante): at 20 bb they give limp 73 / raise 25 / all-in 1 / fold 1, this gives
/// 84 / 13 / 2 / 1 without an ante; at 10 bb they give limp 61 / all-in 29 / fold 10, this gives
/// 39 / 39 / 21. So: the right shape, with frequencies that can be 10 to 20 points off.
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct FlopModel {
    /// extra share for the button (in position after the flop), at deep stacks
    pub position: f64,
    /// extra share for the player who made the last raise before the flop, at deep stacks
    pub initiative: f64,
    /// extra share for suited hands, and half as much again for connected ones; offsuit unconnected hands give up half of it
    pub playability: f64,
    /// stack-to-pot ratio at which half of the shifts above apply (none when all the chips are in)
    pub half_spr: f64,
}
impl Default for FlopModel { fn default() -> Self { FlopModel { position: 0.16, initiative: 0.04, playability: 0.02, half_spr: 7.0 } } }

#[derive(Deserialize, Clone, Debug)]
pub struct PreSpec {
    /// the shorter stack, in big blinds, before blinds and antes go in
    pub stack: f64,
    #[serde(default = "half")] pub small_blind: f64,
    /// ante paid by each player, in big blinds
    #[serde(default)] pub ante: f64,
    /// sizes the button may open to, in big blinds (for example [2.0] or [2.0, 2.5])
    #[serde(default = "opens")] pub open_sizes: Vec<f64>,
    #[serde(default = "yes")] pub limp: bool,
    /// a re-raise is this many times the raise it faces; a raise over a limp is this many big blinds
    #[serde(default = "three")] pub reraise: f64,
    #[serde(default)] pub iterations: usize,
    #[serde(default)] pub model: Option<FlopModel>,
}
fn half() -> f64 { 0.5 } fn yes() -> bool { true } fn three() -> f64 { 3.0 } fn opens() -> Vec<f64> { vec![2.0] }

#[derive(Clone, Debug)]
enum Kind { Decision, Fold { winner: usize }, Showdown, Flop { raiser: Option<usize> } }

#[derive(Clone, Debug)]
struct Node { kind: Kind, player: usize, put: [f64; 2], actions: Vec<(String, f64)>, children: Vec<usize>, path: String, regret: Vec<f64>, sum: Vec<f64> }

#[derive(Serialize, Clone, Debug)]
pub struct PreNode {
    pub id: usize, pub path: String, pub player: String, pub situation: String,
    pub actions: Vec<String>, pub amounts_bb: Vec<f64>,
    /// how often each action is taken here, over the range that gets here
    pub freqs: Vec<f64>,
    /// how often the hand reaches this decision
    pub reach: f64,
    /// per action, how often each of the 169 hand classes takes it (AA, AKs, ... 22 chart order)
    pub strategy: Vec<Vec<f64>>,
    /// how much of each class is still in the acting player's range here
    pub range: Vec<f64>,
    /// where each action leads: a decision id, or -1 when the betting ends
    pub next: Vec<i64>,
}
#[derive(Serialize, Clone, Debug)]
pub struct FlopLine { #[serde(skip)] pub leaf: usize, #[serde(skip)] pub reach: [Vec<f64>; 2], pub path: String, pub freq: f64, pub pot_bb: f64, pub behind_bb: f64, pub button_range: String, pub big_blind_range: String, pub button_share: f64,
    /// true when the share comes from solved flops, false when it is the estimate
    pub solved: bool }
#[derive(Serialize, Clone, Debug)]
pub struct Preflop {
    pub stack_bb: f64, pub small_blind: f64, pub ante: f64,
    pub ev_button_bb: f64, pub ev_big_blind_bb: f64,
    /// the most either player could gain by changing strategy, inside this model (bb per hand)
    pub max_gain_bb: f64,
    pub iterations: usize,
    pub nodes: Vec<PreNode>,
    /// every way the hand reaches a flop, with both ranges, ready for the postflop solver
    pub lines: Vec<FlopLine>,
    pub model: FlopModel, pub model_note: String,
    /// accurate mode only: what each round of flop solves did
    #[serde(skip_serializing_if = "Option::is_none")] pub accurate: Option<Vec<PassInfo>>,
    pub classes: Vec<String>,
}

struct Game { nodes: Vec<Node>, stack: f64, ante: f64, cp: Vec<f64>, eq: Vec<f64>, play: Vec<f64>, model: FlopModel,
    /// flop leaves fitted to solved flops: per class, a pot-share offset for the button's hand and
    /// one for the big blind's hand (the button's share of a match-up is equity + button offset - big blind offset)
    solved: std::collections::HashMap<usize, [Vec<f64>; 2]> }

fn playability(i: usize) -> f64 {
    let (r, c) = (i / 13, i % 13);
    if r == c { return 0.25; }
    let (hi, lo) = (r.min(c), r.max(c)); // 0 = ace
    let gap = lo - hi; let suited = c > r;
    let connected = gap <= 2 && hi >= 2; // king-high or lower, at most one gap
    (if suited { 1.0 } else { -0.5 }) + if connected { 0.5 } else { 0.0 }
}

impl Game {
    fn add(&mut self, kind: Kind, player: usize, put: [f64; 2], path: String) -> usize {
        self.nodes.push(Node { kind, player, put, actions: vec![], children: vec![], path, regret: vec![], sum: vec![] });
        self.nodes.len() - 1
    }
    /// `raises`: raises so far; `last`: who raised last; `limped`: the button just called the big blind
    fn build(&mut self, spec: &PreSpec, p: usize, put: [f64; 2], raises: usize, last: Option<usize>, limped: bool, path: String) -> usize {
        let id = self.add(Kind::Decision, p, put, path.clone());
        let o = 1 - p; let s = self.stack - self.ante; // chips that can go in
        let facing = put[o] > put[p] + 1e-9;
        let name = ["SB", "BB"][p];
        let step = |a: &str| if path.is_empty() { format!("{name} {a}") } else { format!("{path} > {name} {a}") };
        let mut acts: Vec<(String, f64, usize)> = vec![];
        if facing {
            let f = self.add(Kind::Fold { winner: o }, p, put, step("fold")); acts.push(("Fold".into(), 0.0, f));
            let mut np = put; np[p] = put[o];
            if put[o] >= s - 1e-9 { let c = self.add(Kind::Showdown, p, np, step("call")); acts.push(("Call".into(), put[o], c)); }
            else if raises == 0 && p == 0 {
                if spec.limp { let c = self.build(spec, 1, np, 0, None, true, step("limp")); acts.push(("Limp".into(), put[o], c)); }
            } else { let c = self.add(Kind::Flop { raiser: last }, p, np, step("call")); acts.push(("Call".into(), put[o], c)); }
        } else {
            let c = self.add(Kind::Flop { raiser: None }, p, put, step("check")); acts.push(("Check".into(), 0.0, c));
        }
        if put[o] < s - 1e-9 {
            // sized raises: opens for the first raise, then one re-raise size, then all-in only
            let sizes: Vec<f64> = match raises {
                0 if !limped => spec.open_sizes.clone(),
                0 => vec![spec.reraise.max(2.0)],
                1 => vec![put[o] * spec.reraise],
                _ => vec![],
            };
            for to in sizes {
                let to = (to * 100.0).round() / 100.0;
                if to <= put[o] * 1.99 - 1e-9 && raises > 0 || to <= put[o] + 1e-9 || to >= 0.4 * s { continue; }
                let mut np = put; np[p] = to;
                let label = if raises == 0 { "Raise" } else { "Re-raise" };
                let c = self.build(spec, o, np, raises + 1, Some(p), false, step(&format!("raise {to}")));
                acts.push((format!("{label} to {to}"), to, c));
            }
            let mut np = put; np[p] = s;
            let c = self.build(spec, o, np, raises + 1, Some(p), false, step("all-in"));
            acts.push(("All-in".into(), s, c));
        }
        let n = &mut self.nodes[id];
        n.actions = acts.iter().map(|a| (a.0.clone(), a.1)).collect(); n.children = acts.iter().map(|a| a.2).collect();
        n.regret = vec![0.0; acts.len() * N]; n.sum = vec![0.0; acts.len() * N];
        id
    }

    /// what `hero` wins or loses, in big blinds, when hero holds class i and the other player class j
    fn payoff(&self, id: usize, hero: usize, i: usize, j: usize) -> f64 {
        let n = &self.nodes[id]; let paid = n.put[hero] + self.ante; let pot = n.put[0] + n.put[1] + 2.0 * self.ante;
        match n.kind {
            Kind::Fold { winner } => if winner == hero { pot - paid } else { -paid },
            Kind::Showdown => self.eq[i * N + j] * pot - paid,
            Kind::Flop { .. } if self.solved.contains_key(&id) => {
                let (b, x) = if hero == 0 { (i, j) } else { (j, i) };
                let share = self.solved_share(id, b, x);
                (if hero == 0 { share } else { 1.0 - share }) * pot - paid
            }
            Kind::Flop { raiser } => {
                let (b, x) = if hero == 0 { (i, j) } else { (j, i) }; // button's class, big blind's class
                let share = self.button_share(n, raiser, b, x);
                (if hero == 0 { share } else { 1.0 - share }) * pot - paid
            }
            Kind::Decision => unreachable!(),
        }
    }
    fn solved_share(&self, id: usize, b: usize, x: usize) -> f64 { let v = &self.solved[&id]; (self.eq[b * N + x] + v[0][b] - v[1][x]).clamp(0.0, 1.0) }
    fn button_share(&self, n: &Node, raiser: Option<usize>, b: usize, x: usize) -> f64 {
        let pot = n.put[0] + n.put[1] + 2.0 * self.ante;
        let spr = (self.stack - self.ante - n.put[0].max(n.put[1])).max(0.0) / pot;
        let depth = spr / (spr + self.model.half_spr);
        let m = &self.model;
        let shift = m.position + m.initiative * match raiser { Some(0) => 1.0, Some(_) => -1.0, None => 0.0 } + m.playability * (self.play[b] - self.play[x]);
        (self.eq[b * N + x] + depth * shift).clamp(0.0, 1.0)
    }

    fn sigma(&self, id: usize, avg: bool) -> Vec<f64> {
        let n = &self.nodes[id]; let na = n.actions.len(); let src = if avg { &n.sum } else { &n.regret };
        let mut s = vec![0.0; na * N];
        for h in 0..N {
            let tot: f64 = (0..na).map(|a| src[a * N + h].max(0.0)).sum();
            for a in 0..na { s[a * N + h] = if tot > 0.0 { src[a * N + h].max(0.0) / tot } else { 1.0 / na as f64 }; }
        }
        s
    }
    /// per-class value for `hero` (summed over the other player's hands, weighted by their reach).
    /// mode 0: one training step; 1: value of the average strategies; 2: hero's best reply to the average.
    fn walk(&mut self, id: usize, hero: usize, mine: &[f64], theirs: &[f64], t: f64, mode: u8) -> Vec<f64> {
        if !matches!(self.nodes[id].kind, Kind::Decision) {
            return (0..N).map(|i| (0..N).map(|j| { let w = self.cp[i * N + j] * theirs[j]; if w > 0.0 { w * self.payoff(id, hero, i, j) } else { 0.0 } }).sum()).collect();
        }
        let (p, na, kids) = { let n = &self.nodes[id]; (n.player, n.actions.len(), n.children.clone()) };
        let s = self.sigma(id, mode != 0);
        if p != hero {
            let mut val = vec![0.0; N];
            for a in 0..na {
                let r: Vec<f64> = (0..N).map(|h| theirs[h] * s[a * N + h]).collect();
                if r.iter().all(|&x| x <= 0.0) { continue; }
                let v = self.walk(kids[a], hero, mine, &r, t, mode);
                for h in 0..N { val[h] += v[h]; }
            }
            return val;
        }
        let vals: Vec<Vec<f64>> = (0..na).map(|a| { let r: Vec<f64> = (0..N).map(|h| mine[h] * s[a * N + h]).collect(); self.walk(kids[a], hero, &r, theirs, t, mode) }).collect();
        let mut val = vec![0.0; N];
        for h in 0..N {
            val[h] = if mode == 2 { (0..na).map(|a| vals[a][h]).fold(f64::MIN, f64::max) } else { (0..na).map(|a| s[a * N + h] * vals[a][h]).sum() };
        }
        if mode == 0 {
            let n = &mut self.nodes[id];
            for a in 0..na { for h in 0..N {
                n.regret[a * N + h] = (n.regret[a * N + h] + vals[a][h] - val[h]).max(0.0);
                n.sum[a * N + h] += t * mine[h] * s[a * N + h];
            } }
        }
        val
    }
    fn total(&self, v: &[f64]) -> f64 { (0..N).map(|i| combos(i) * v[i]).sum::<f64>() / (1326.0 * 1225.0) }
}

/// Range text for the flop solver with at least `floor` of every hand, so each hand gets a value.
pub fn range_with_floor(w: &[f64], floor: f64) -> String {
    (0..N).map(|i| { let x = w[i].max(floor).min(1.0); if x > 0.995 { class_name(i) } else { format!("{}:{:.3}", class_name(i), x) } }).collect::<Vec<_>>().join(",")
}
fn range_text(w: &[f64]) -> String {
    (0..N).filter(|&i| w[i] >= 0.005).map(|i| if w[i] > 0.995 { class_name(i) } else { format!("{}:{:.2}", class_name(i), w[i]) }).collect::<Vec<_>>().join(",")
}

/// One round of accurate mode, for progress reports.
#[derive(Serialize, Clone, Debug)]
pub struct PassInfo {
    pub pass: usize,
    /// the button's first decision after this round's flop values went in
    pub root_actions: Vec<String>, pub root_freqs: Vec<f64>,
    /// per line solved this round: the button's pot share used going in, and what the flop solves gave
    pub lines: Vec<PassLine>,
}
#[derive(Serialize, Clone, Debug)]
pub struct PassLine { pub path: String, pub freq: f64, pub share_before: f64, pub share_solved: f64, pub flops: usize, pub worst_accuracy_pct: f64 }

/// What a flop evaluator hands back for one line: per hand class, the big blinds each player wins
/// from the pot on average (button first, then big blind); how many flops; the worst accuracy reached.
pub type LineValues = ([Vec<f64>; 2], usize, f64);

/// Accurate mode: after each preflop solve, every common line that reaches a flop is handed to
/// `eval`, which solves real flops with those ranges; the values go back in as what each hand is
/// worth at that point, and the preflop is solved again. Ranges and values chase each other, so
/// values are averaged with the previous round's to settle. `floor` keeps a sliver of every hand
/// in the ranges sent out so that every hand gets a value.
pub fn solve_accurate(spec: &PreSpec, passes: usize, min_freq: f64, eval: &mut dyn FnMut(&FlopLine, usize) -> Result<LineValues, String>, report: &mut dyn FnMut(&PassInfo)) -> Result<Preflop, String> {
    let mut solved: std::collections::HashMap<usize, [Vec<f64>; 2]> = Default::default();
    let mut history = vec![];
    let cp = compat(); let eq: Vec<f64> = (0..N * N).map(|k| equity(k / N, k % N)).collect();
    let mut out = solve_with(spec, &solved)?;
    for pass in 1..=passes.max(1) {
        let mut lines = vec![];
        for l in out.lines.iter().filter(|l| l.freq >= min_freq) {
            let (vals, flops, worst) = eval(l, pass)?;
            let fitted = fit_offsets(&cp, &eq, &l.reach, &vals, l.pot_bb);
            let entry = match solved.get(&l.leaf) {
                Some(old) => [0, 1].map(|p| (0..N).map(|i| 0.5 * old[p][i] + 0.5 * fitted[p][i]).collect::<Vec<f64>>()),
                None => fitted,
            };
            // the button's share these flop solves gave, over the ranges that were sent
            let (mut num, mut den) = (0.0, 0.0);
            for i in 0..N { let w = combos(i) * l.reach[0][i]; if vals[0][i].is_finite() { num += w * vals[0][i]; den += w; } }
            lines.push(PassLine { path: l.path.clone(), freq: l.freq, share_before: l.button_share, share_solved: if den > 0.0 { ((num / den / l.pot_bb) * 1e4).round() / 1e4 } else { 0.0 }, flops, worst_accuracy_pct: worst });
            solved.insert(l.leaf, entry);
        }
        out = solve_with(spec, &solved)?;
        let info = PassInfo { pass, root_actions: out.nodes[0].actions.clone(), root_freqs: out.nodes[0].freqs.clone(), lines };
        report(&info);
        history.push(info);
    }
    out.accurate = Some(history);
    out.model_note = "All-in pots are exact. Lines marked solved take their values from real flop solves of a sample of flops, fed back and repeated; rarer lines still use the estimate. The flop solves use one bet size per street and a sample of flops, so small frequency differences are not meaningful.".into();
    Ok(out)
}

/// Turns solved per-hand values into match-up payoffs the preflop solve can use. The button's
/// share of the pot when class `b` meets class `x` is taken to be equity(b, x) + up[b] - down[x];
/// the offsets are chosen so that, against the ranges that were solved, every hand's average
/// comes out at its solved value. Because the match-up equity stays in, the payoffs still move
/// sensibly when the ranges shift in the next preflop solve. Hands that were not in the solved
/// range get the average offset of the hands that were.
fn fit_offsets(cp: &[f64], eq: &[f64], reach: &[Vec<f64>; 2], vals: &[Vec<f64>; 2], pot: f64) -> [Vec<f64>; 2] {
    let have = |p: usize, i: usize| vals[p][i].is_finite() && reach[p][i] > 0.005;
    let (mut up, mut down) = (vec![0.0; N], vec![0.0; N]);
    for _ in 0..40 {
        for b in 0..N {
            if !have(0, b) { continue; }
            let (mut w, mut e, mut d) = (0.0, 0.0, 0.0);
            for x in 0..N { if !have(1, x) { continue; } let k = cp[b * N + x] * reach[1][x]; w += k; e += k * eq[b * N + x]; d += k * down[x]; }
            if w > 0.0 { up[b] = vals[0][b] / pot - e / w + d / w; }
        }
        for x in 0..N {
            if !have(1, x) { continue; }
            let (mut w, mut e, mut u) = (0.0, 0.0, 0.0);
            for b in 0..N { if !have(0, b) { continue; } let k = cp[x * N + b] * reach[0][b]; w += k; e += k * eq[b * N + x]; u += k * up[b]; }
            if w > 0.0 { down[x] = vals[1][x] / pot - 1.0 + e / w + u / w; }
        }
        // only the difference matters; keep the big blind's offsets centred
        let (mut w, mut m) = (0.0, 0.0);
        for x in 0..N { if have(1, x) { let k = combos(x) * reach[1][x]; w += k; m += k * down[x]; } }
        if w > 0.0 { let m = m / w; for x in 0..N { if have(1, x) { down[x] -= m; } } for b in 0..N { if have(0, b) { up[b] -= m; } } }
    }
    for (p, v) in [(0usize, &mut up), (1, &mut down)] {
        let (mut w, mut m) = (0.0, 0.0);
        for i in 0..N { if have(p, i) { let k = combos(i) * reach[p][i]; w += k; m += k * v[i]; } }
        let mean = if w > 0.0 { m / w } else { 0.0 };
        for i in 0..N { if !have(p, i) { v[i] = mean; } else { v[i] = v[i].clamp(-0.6, 0.6); } }
    }
    [up, down]
}

pub fn solve(spec: &PreSpec) -> Result<Preflop, String> { solve_with(spec, &Default::default()) }

fn solve_with(spec: &PreSpec, solved: &std::collections::HashMap<usize, [Vec<f64>; 2]>) -> Result<Preflop, String> {
    if !(spec.stack > 1.5 && spec.stack <= 250.0) { return Err("stack must be more than 1.5 and at most 250 big blinds".into()); }
    if !(spec.small_blind > 0.0 && spec.small_blind < 1.0) { return Err("small blind must be between 0 and 1 big blind".into()); }
    if !(spec.ante >= 0.0 && spec.ante < spec.stack - 1.0) { return Err("ante must be zero or more, and leave chips behind".into()); }
    if spec.open_sizes.iter().any(|&x| !(x >= 2.0 && x <= 10.0)) { return Err("open sizes must be between 2 and 10 big blinds".into()); }
    if !(spec.reraise >= 2.0 && spec.reraise <= 6.0) { return Err("the re-raise multiple must be between 2 and 6".into()); }
    let model = spec.model.clone().unwrap_or_default();
    let mut g = Game { nodes: vec![], stack: spec.stack, ante: spec.ante, cp: compat(), eq: (0..N * N).map(|k| equity(k / N, k % N)).collect(), play: (0..N).map(playability).collect(), model: model.clone(), solved: solved.clone() };
    g.build(spec, 0, [spec.small_blind, 1.0], 0, None, false, String::new());
    let iters = if spec.iterations > 0 { spec.iterations } else { 3000 };
    let ones = vec![1.0; N];
    for t in 1..=iters { for hero in 0..2 { g.walk(0, hero, &ones, &ones, t as f64, 0); } }
    let ev: Vec<f64> = (0..2).map(|h| { let v = g.walk(0, h, &ones, &ones, 0.0, 1); g.total(&v) }).collect();
    let best: Vec<f64> = (0..2).map(|h| { let v = g.walk(0, h, &ones, &ones, 0.0, 2); g.total(&v) }).collect();
    let r = |x: f64, d: i32| { let m = 10f64.powi(d); (x * m).round() / m };

    // read the tree back out: reach of each player at every node under the average strategies
    let mut reach: Vec<[Vec<f64>; 2]> = vec![[vec![], vec![]]; g.nodes.len()];
    reach[0] = [ones.clone(), ones.clone()];
    let mut ids: Vec<i64> = vec![-1; g.nodes.len()]; let mut nodes = vec![]; let mut lines = vec![];
    let share_of = |rc: &[Vec<f64>; 2]| -> f64 { let mut t = 0.0; for i in 0..N { if rc[0][i] <= 0.0 { continue; } for j in 0..N { t += combos(i) * g.cp[i * N + j] * rc[0][i] * rc[1][j]; } } t / (1326.0 * 1225.0) };
    let decisions: Vec<usize> = (0..g.nodes.len()).filter(|&i| matches!(g.nodes[i].kind, Kind::Decision)).collect();
    for (k, &i) in decisions.iter().enumerate() { ids[i] = k as i64; }
    for i in 0..g.nodes.len() {
        let n = g.nodes[i].clone(); let rc = reach[i].clone();
        match n.kind {
            Kind::Decision => {
                let s = g.sigma(i, true); let na = n.actions.len(); let p = n.player;
                for a in 0..na { let mut c = rc.clone(); for h in 0..N { c[p][h] *= s[a * N + h]; } reach[n.children[a]] = c; }
                let here = share_of(&rc);
                let freqs: Vec<f64> = (0..na).map(|a| if here > 0.0 { r(share_of(&reach[n.children[a]]) / here, 4) } else { 0.0 }).collect();
                let facing = n.put[1 - p] > n.put[p] + 1e-9;
                let situation = if n.path.is_empty() { "Small blind (button) acts first".to_string() }
                    else if !facing { "Big blind after a limp".to_string() }
                    else { format!("{} facing {}", ["Small blind", "Big blind"][p], if n.put[1 - p] >= spec.stack - spec.ante - 1e-9 { "an all-in".to_string() } else { format!("a raise to {}", n.put[1 - p]) }) };
                nodes.push(PreNode { id: ids[i] as usize, path: n.path.clone(), player: ["Small blind", "Big blind"][p].into(), situation,
                    actions: n.actions.iter().map(|a| a.0.clone()).collect(), amounts_bb: n.actions.iter().map(|a| a.1).collect(), freqs, reach: r(here, 4),
                    strategy: (0..na).map(|a| (0..N).map(|h| r(s[a * N + h], 3)).collect()).collect(), range: rc[p].iter().map(|&x| r(x, 3)).collect(),
                    next: n.children.iter().map(|&c| ids[c]).collect() });
            }
            Kind::Flop { raiser } => {
                let freq = share_of(&rc); if freq < 0.0005 { continue; }
                let pot = n.put[0] + n.put[1] + 2.0 * spec.ante;
                let (mut num, mut den) = (0.0, 0.0);
                let got = g.solved.get(&i);
                for b in 0..N { if rc[0][b] <= 0.0 { continue; } for x in 0..N { let w = combos(b) * g.cp[b * N + x] * rc[0][b] * rc[1][x]; if w > 0.0 { num += w * match got { Some(_) => g.solved_share(i, b, x), None => g.button_share(&n, raiser, b, x) }; den += w; } } }
                lines.push(FlopLine { leaf: i, reach: rc.clone(), solved: got.is_some(), path: n.path.clone(), freq: r(freq, 4), pot_bb: r(pot, 2), behind_bb: r(spec.stack - spec.ante - n.put[0].max(n.put[1]), 2),
                    button_range: range_text(&rc[0]), big_blind_range: range_text(&rc[1]), button_share: r(if den > 0.0 { num / den } else { 0.0 }, 4) });
            }
            _ => {}
        }
    }
    lines.sort_by(|a, b| b.freq.partial_cmp(&a.freq).unwrap());
    Ok(Preflop {
        stack_bb: spec.stack, small_blind: spec.small_blind, ante: spec.ante, ev_button_bb: r(ev[0], 4), ev_big_blind_bb: r(ev[1], 4),
        max_gain_bb: r((best[0] - ev[0]).max(best[1] - ev[1]), 4), iterations: iters, nodes, lines, model,
        model_note: "All-in pots are exact. Pots that see a flop are estimated, not solved: each hand gets its all-in share of the pot, shifted toward the button, the last raiser and hands that play well after the flop. Solve the lines that matter in the postflop solver.".into(),
        classes: (0..N).map(class_name).collect(), accurate: None,
    })
}

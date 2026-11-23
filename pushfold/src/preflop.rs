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

#[derive(Serialize, Deserialize, Clone, Debug)]
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

#[derive(Serialize, Deserialize, Clone, Debug)]
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
    /// per action, what taking it is worth in big blinds with each of the 169 classes, counted from the
    /// start of the hand, given how both players play from there on (0 for a hand that never gets here).
    /// Two actions a hand mixes between should be worth about the same.
    #[serde(default, skip_serializing_if = "Vec::is_empty")] pub values: Vec<Vec<f64>>,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FlopLine { #[serde(skip)] pub leaf: usize, #[serde(skip)] pub reach: [Vec<f64>; 2], pub path: String, pub freq: f64, pub pot_bb: f64, pub behind_bb: f64, pub button_range: String, pub big_blind_range: String, pub button_share: f64,
    /// true when the share comes from solved flops, false when it is the estimate
    pub solved: bool }
#[derive(Serialize, Deserialize, Clone, Debug)]
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
    /// the strategy at every tree node, kept so the next accurate round can move only part of the way
    #[serde(skip)] pub sigmas: Vec<Vec<f64>>,
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
    fn solved_share(&self, id: usize, b: usize, x: usize) -> f64 { let v = &self.solved[&id]; (self.eq[b * N + x] + v[0][b] - v[1][x]).clamp(-0.5, 1.5) }  // loose bounds only: the fit is linear, and tight ones stop it reproducing the solved values
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
    /// Value for `hero` of every class at node `id` when both players follow the strategies `sig`
    /// (summed over the other player's hands, weighted by their reach), and, at each of hero's
    /// decisions on the way, what each action is worth per hand (`out[node][action][class]`).
    fn value_under(&self, sig: &[Vec<f64>], id: usize, hero: usize, theirs: &[f64], out: &mut Vec<Vec<Vec<f64>>>) -> Vec<f64> {
        let n = &self.nodes[id];
        if !matches!(n.kind, Kind::Decision) {
            return (0..N).map(|i| (0..N).map(|j| { let w = self.cp[i * N + j] * theirs[j]; if w > 0.0 { w * self.payoff(id, hero, i, j) } else { 0.0 } }).sum()).collect();
        }
        let (na, s) = (n.actions.len(), &sig[id]);
        let mut val = vec![0.0; N];
        if n.player != hero {
            for a in 0..na {
                let r: Vec<f64> = (0..N).map(|h| theirs[h] * s[a * N + h]).collect();
                if r.iter().all(|&x| x <= 0.0) { continue; }
                let v = self.value_under(sig, n.children[a], hero, &r, out);
                for h in 0..N { val[h] += v[h]; }
            }
            return val;
        }
        let den: Vec<f64> = (0..N).map(|h| (0..N).map(|j| self.cp[h * N + j] * theirs[j]).sum()).collect();
        let mut rec = vec![vec![0.0; N]; na];
        for a in 0..na {
            let v = self.value_under(sig, n.children[a], hero, theirs, out);
            for h in 0..N { val[h] += s[a * N + h] * v[h]; if den[h] > 1e-9 { rec[a][h] = v[h] / den[h]; } }
        }
        out[id] = rec;
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

const ACCURATE_NOTE: &str = "All-in pots are exact. Lines marked solved take their values from real flop solves of a sample of flops, fed back and repeated; rarer lines still use the estimate. The flop solves use one bet size per street and a sample of flops, so small frequency differences are not meaningful.";

/// One round of accurate mode, for progress reports.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PassInfo {
    pub pass: usize,
    /// the button's first decision after this round's flop values went in
    pub root_actions: Vec<String>, pub root_freqs: Vec<f64>,
    /// per line solved this round: the button's pot share used going in, and what the flop solves gave
    pub lines: Vec<PassLine>,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PassLine { pub path: String, pub freq: f64, pub share_before: f64, pub share_solved: f64, pub flops: usize, pub worst_accuracy_pct: f64 }

/// What a flop evaluator hands back for one line: per hand class, the big blinds each player wins
/// from the pot on average (button first, then big blind); how many flops; the worst accuracy reached.
pub type LineValues = ([Vec<f64>; 2], usize, f64);

/// Accurate mode: after each preflop solve, every common line that reaches a flop is handed to
/// `eval`, which solves real flops with those ranges; the values go back in as what each hand is
/// worth at that point, and the preflop is solved again. Ranges and values chase each other, so
/// values are averaged with the previous round's to settle. `floor` keeps a sliver of every hand
/// in the ranges sent out so that every hand gets a value.
/// Everything an accurate run needs to carry on after the round it has just finished.
#[derive(Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub rounds_done: usize,
    /// fitted flop values by line (leaf id), as used for the last preflop solve
    pub solved: Vec<(usize, [Vec<f64>; 2])>,
    /// the strategy the last preflop solve stepped away from
    pub prev: Vec<Vec<f64>>,
    pub history: Vec<PassInfo>,
}

pub fn solve_accurate(spec: &PreSpec, passes: usize, min_freq: f64, eval: &mut dyn FnMut(&FlopLine, usize) -> Result<LineValues, String>, report: &mut dyn FnMut(&PassInfo, &Preflop)) -> Result<Preflop, String> {
    solve_accurate_from(spec, passes, min_freq, eval, report, None, &mut |_| {})
}

/// As `solve_accurate`, but can start from a checkpoint and hands one out after every round.
/// The preflop solve is deterministic, so a resumed run gives the same answer as an unbroken one.
pub fn solve_accurate_from(spec: &PreSpec, passes: usize, min_freq: f64, eval: &mut dyn FnMut(&FlopLine, usize) -> Result<LineValues, String>, report: &mut dyn FnMut(&PassInfo, &Preflop), resume: Option<Checkpoint>, checkpoint: &mut dyn FnMut(&Checkpoint)) -> Result<Preflop, String> {
    let mut solved: std::collections::HashMap<usize, [Vec<f64>; 2]> = Default::default();
    let mut history = vec![];
    let mut first = 1;
    let mut resumed: Option<Preflop> = None;
    if let Some(c) = resume {
        solved = c.solved.into_iter().collect(); history = c.history; first = c.rounds_done + 1;
        let mut o = solve_with(spec, &solved, Some((&c.prev, 0.35)))?;
        o.accurate = Some(history.clone()); o.model_note = ACCURATE_NOTE.into();
        resumed = Some(o);
    }
    let cp = compat(); let eq: Vec<f64> = (0..N * N).map(|k| equity(k / N, k % N)).collect();
    let mut out = match resumed { Some(o) => o, None => solve_with(spec, &solved, None)? };
    // Each round the flops are solved for the ranges as they stand, and the strategy then moves only
    // part of the way toward the best answer to those values. The values are only right for the
    // ranges they were solved with; small steps keep the two in agreement, and a strategy that no
    // longer moves is one where every hand is doing its best against real flop results.
    let step = 0.35;
    for pass in first..=passes.max(1) {
        let mut lines = vec![];
        for l in out.lines.iter().filter(|l| l.freq >= min_freq) {
            let (mut vals, flops, worst) = eval(l, pass)?;
            // A sample of flops pairs each hand a little more often than the full deck does, so the two
            // players' solved values add up to slightly more than the pot. Scale both back so they add up.
            {
                let mut share = [0.0f64; 2];
                for p in 0..2 { let (mut num, mut den) = (0.0, 0.0);
                    for i in 0..N { if !(vals[p][i].is_finite() && l.reach[p][i] > 0.005) { continue; }
                        let w: f64 = (0..N).filter(|&j| vals[1 - p][j].is_finite() && l.reach[1 - p][j] > 0.005).map(|j| cp[i * N + j] * l.reach[1 - p][j]).sum();
                        let m = combos(i) * l.reach[p][i] * w; num += m * vals[p][i]; den += m; }
                    share[p] = if den > 0.0 { num / den / l.pot_bb } else { 0.5 }; }
                let total = share[0] + share[1];
                if total > 0.5 { for p in 0..2 { for v in vals[p].iter_mut() { *v /= total; } } }
            }
            let entry = fit_offsets(&cp, &eq, &l.reach, &vals, l.pot_bb);
            if std::env::var("PONTIAC_FIT_DEBUG").is_ok() {
                // does the fit give back what was solved, for each side, and do the two sides' solved shares add up?
                let mut tot = [0.0f64; 4]; let mut wsum = [0.0f64; 2];
                for b in 0..N { if !(vals[0][b].is_finite() && l.reach[0][b] > 0.005) { continue; }
                    let (mut w, mut sh) = (0.0, 0.0);
                    for x in 0..N { if !(vals[1][x].is_finite() && l.reach[1][x] > 0.005) { continue; } let k = cp[b * N + x] * l.reach[1][x]; w += k; sh += k * (eq[b * N + x] + entry[0][b] - entry[1][x]); }
                    let m = combos(b) * l.reach[0][b] * w; tot[0] += m * sh / w.max(1e-12); tot[1] += m * vals[0][b] / l.pot_bb; wsum[0] += m; }
                for x in 0..N { if !(vals[1][x].is_finite() && l.reach[1][x] > 0.005) { continue; }
                    let (mut w, mut sh) = (0.0, 0.0);
                    for b in 0..N { if !(vals[0][b].is_finite() && l.reach[0][b] > 0.005) { continue; } let k = cp[x * N + b] * l.reach[0][b]; w += k; sh += k * (1.0 - eq[b * N + x] - entry[0][b] + entry[1][x]); }
                    let m = combos(x) * l.reach[1][x] * w; tot[2] += m * sh / w.max(1e-12); tot[3] += m * vals[1][x] / l.pot_bb; wsum[1] += m; }
                eprintln!("FIT {}: button fitted {:.4} solved {:.4} | big blind fitted {:.4} solved {:.4} | solved shares add to {:.4}", l.path, tot[0] / wsum[0], tot[1] / wsum[0], tot[2] / wsum[1], tot[3] / wsum[1], tot[1] / wsum[0] + tot[3] / wsum[1]);
            }
            // the button's share these flop solves gave, over the ranges that were sent
            let (mut num, mut den) = (0.0, 0.0);
            for i in 0..N { let w = combos(i) * l.reach[0][i]; if vals[0][i].is_finite() { num += w * vals[0][i]; den += w; } }
            lines.push(PassLine { path: l.path.clone(), freq: l.freq, share_before: l.button_share, share_solved: if den > 0.0 { ((num / den / l.pot_bb) * 1e4).round() / 1e4 } else { 0.0 }, flops, worst_accuracy_pct: worst });
            solved.insert(l.leaf, entry);
        }
        let prev = std::mem::take(&mut out.sigmas);
        out = solve_with(spec, &solved, Some((&prev, step)))?;
        let info = PassInfo { pass, root_actions: out.nodes[0].actions.clone(), root_freqs: out.nodes[0].freqs.clone(), lines };
        history.push(info.clone());
        // hand over the whole answer as it stands, so a run that stops later loses one round, not everything
        out.accurate = Some(history.clone());
        out.model_note = ACCURATE_NOTE.into();
        report(&info, &out);
        let mut sv: Vec<(usize, [Vec<f64>; 2])> = solved.iter().map(|(k, v)| (*k, v.clone())).collect(); sv.sort_by_key(|x| x.0);
        checkpoint(&Checkpoint { rounds_done: pass, solved: sv, prev, history: history.clone() });
    }
    out.accurate = Some(history);
    out.model_note = ACCURATE_NOTE.into();
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
        for i in 0..N { if !have(p, i) { v[i] = mean; } else { v[i] = v[i].clamp(-2.0, 2.0); } }
    }
    [up, down]
}

pub fn solve(spec: &PreSpec) -> Result<Preflop, String> { solve_with(spec, &Default::default(), None) }

/// `blend`: the previous strategy and how far to move from it toward the new solve (1 = all the way).
fn solve_with(spec: &PreSpec, solved: &std::collections::HashMap<usize, [Vec<f64>; 2]>, blend: Option<(&[Vec<f64>], f64)>) -> Result<Preflop, String> {
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
    let mut sigmas: Vec<Vec<f64>> = vec![vec![]; g.nodes.len()];
    for i in 0..g.nodes.len() {
        if !matches!(g.nodes[i].kind, Kind::Decision) { continue; }
        let new = g.sigma(i, true);
        sigmas[i] = match blend { Some((old, step)) if old.len() == g.nodes.len() && old[i].len() == new.len() => new.iter().zip(&old[i]).map(|(n, o)| o + step * (n - o)).collect(), _ => new };
    }
    // what each action is worth to each hand under the strategies being reported
    let mut action_values: Vec<Vec<Vec<f64>>> = vec![vec![]; g.nodes.len()];
    for hero in 0..2 { g.value_under(&sigmas, 0, hero, &ones, &mut action_values); }
    let decisions: Vec<usize> = (0..g.nodes.len()).filter(|&i| matches!(g.nodes[i].kind, Kind::Decision)).collect();
    for (k, &i) in decisions.iter().enumerate() { ids[i] = k as i64; }
    for i in 0..g.nodes.len() {
        let n = g.nodes[i].clone(); let rc = reach[i].clone();
        match n.kind {
            Kind::Decision => {
                let s = sigmas[i].clone(); let na = n.actions.len(); let p = n.player;
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
                    next: n.children.iter().map(|&c| ids[c]).collect(),
                    values: action_values[i].iter().map(|v| v.iter().map(|&x| r(x, 4)).collect()).collect() });
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
        classes: (0..N).map(class_name).collect(), accurate: None, sigmas,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    /// With limps and small raises switched off the tree is push or fold, and must give the push/fold answer.
    #[test]
    fn all_in_only_reproduces_push_fold() {
        for stack in [8.0, 12.0, 20.0] {
            let spec: PreSpec = serde_json::from_value(serde_json::json!({"stack": stack, "open_sizes": [], "limp": false, "iterations": 4000})).unwrap();
            let p = solve(&spec).unwrap();
            let pf = crate::solve(stack, 0.5, 0.0).unwrap();
            let root = &p.nodes[0];
            let jam = root.actions.iter().position(|a| a == "All-in").expect("all-in at the root");
            assert_eq!(root.actions.len(), 2, "{:?}", root.actions);
            assert!((root.freqs[jam] - pf.jam_pct).abs() < 0.01, "{stack} bb: jam {} vs push/fold {}", root.freqs[jam], pf.jam_pct);
            let call = p.nodes.iter().find(|n| n.path == "SB all-in").expect("big blind's reply");
            let c = call.actions.iter().position(|a| a == "Call").unwrap();
            assert!((call.freqs[c] - pf.call_pct).abs() < 0.01, "{stack} bb: call {} vs push/fold {}", call.freqs[c], pf.call_pct);
            assert!((p.ev_button_bb - pf.ev_small_blind_bb).abs() < 0.005, "{stack} bb: EV {} vs {}", p.ev_button_bb, pf.ev_small_blind_bb);
        }
    }
    /// If the "flop solver" is the estimate itself, accurate mode must land back on the plain answer
    /// and its predictions must match what the solver then reports.
    #[test]
    fn accurate_mode_recovers_a_known_payoff() {
        let spec: PreSpec = serde_json::from_str(r#"{"stack":20,"iterations":1500}"#).unwrap();
        let plain = solve(&spec).unwrap();
        let cp = compat(); let eq: Vec<f64> = (0..N * N).map(|k| equity(k / N, k % N)).collect();
        let play: Vec<f64> = (0..N).map(playability).collect(); let m = FlopModel::default();
        let mut eval = |l: &FlopLine, _pass: usize| -> Result<LineValues, String> {
            let spr = l.behind_bb / l.pot_bb; let depth = spr / (spr + m.half_spr);
            let raiser = l.path.rsplit(" > ").find(|s| s.contains("raise")).map(|s| if s.starts_with("SB") { 1.0 } else { -1.0 }).unwrap_or(0.0);
            let share = |b: usize, x: usize| (eq[b * N + x] + depth * (m.position + m.initiative * raiser + m.playability * (play[b] - play[x]))).clamp(0.0, 1.0);
            let mut v = [vec![f64::NAN; N], vec![f64::NAN; N]];
            for b in 0..N { if l.reach[0][b] < 0.005 { continue; } let (mut w, mut s) = (0.0, 0.0); for x in 0..N { if l.reach[1][x] < 0.005 { continue; } let k = cp[b * N + x] * l.reach[1][x]; w += k; s += k * share(b, x); } if w > 0.0 { v[0][b] = s / w * l.pot_bb; } }
            for x in 0..N { if l.reach[1][x] < 0.005 { continue; } let (mut w, mut s) = (0.0, 0.0); for b in 0..N { if l.reach[0][b] < 0.005 { continue; } let k = cp[x * N + b] * l.reach[0][b]; w += k; s += k * (1.0 - share(b, x)); } if w > 0.0 { v[1][x] = s / w * l.pot_bb; } }
            Ok((v, 1, 0.0))
        };
        let mut log = vec![];
        let acc = solve_accurate(&spec, 4, 0.02, &mut eval, &mut |i: &PassInfo, _so_far: &Preflop| log.push(i.clone())).unwrap();
        for i in &log { println!("pass {}: {:?} {:?}", i.pass, i.root_freqs, i.lines.iter().map(|l| (l.path.clone(), l.share_before, l.share_solved)).collect::<Vec<_>>()); }
        println!("plain {:?}", plain.nodes[0].freqs);
        for (a, b) in acc.nodes[0].freqs.iter().zip(&plain.nodes[0].freqs) { assert!((a - b).abs() < 0.05, "{a} vs {b}"); }
        for l in &log.last().unwrap().lines { assert!((l.share_before - l.share_solved).abs() < 0.02, "{} {} {}", l.path, l.share_before, l.share_solved); }
    }

    /// A run stopped after two rounds and resumed must end exactly where an unbroken run ends.
    #[test]
    fn accurate_mode_resumes_exactly() {
        let spec: PreSpec = serde_json::from_str(r#"{"stack":15,"iterations":600}"#).unwrap();
        let cp = compat(); let eq: Vec<f64> = (0..N * N).map(|k| equity(k / N, k % N)).collect();
        let mut eval = |l: &FlopLine, _pass: usize| -> Result<LineValues, String> {
            let share = |b: usize, x: usize| (eq[b * N + x] + 0.08).clamp(0.0, 1.0);
            let mut v = [vec![f64::NAN; N], vec![f64::NAN; N]];
            for b in 0..N { if l.reach[0][b] < 0.005 { continue; } let (mut w, mut s) = (0.0, 0.0); for x in 0..N { if l.reach[1][x] < 0.005 { continue; } let k = cp[b * N + x] * l.reach[1][x]; w += k; s += k * share(b, x); } if w > 0.0 { v[0][b] = s / w * l.pot_bb; } }
            for x in 0..N { if l.reach[1][x] < 0.005 { continue; } let (mut w, mut s) = (0.0, 0.0); for b in 0..N { if l.reach[0][b] < 0.005 { continue; } let k = cp[x * N + b] * l.reach[0][b]; w += k; s += k * (1.0 - share(b, x)); } if w > 0.0 { v[1][x] = s / w * l.pot_bb; } }
            Ok((v, 1, 0.0))
        };
        let whole = solve_accurate(&spec, 4, 0.02, &mut eval, &mut |_, _| {}).unwrap();
        let mut saved: Option<String> = None;
        solve_accurate_from(&spec, 2, 0.02, &mut eval, &mut |_, _| {}, None, &mut |c| saved = Some(serde_json::to_string(c).unwrap())).unwrap();
        let c: Checkpoint = serde_json::from_str(&saved.unwrap()).unwrap();
        assert_eq!(c.rounds_done, 2);
        let mut rounds = vec![];
        let rest = solve_accurate_from(&spec, 4, 0.02, &mut eval, &mut |i, _| rounds.push(i.pass), Some(c), &mut |_| {}).unwrap();
        assert_eq!(rounds, vec![3, 4]);
        assert_eq!(rest.accurate.as_ref().unwrap().len(), 4);
        for (a, b) in rest.nodes.iter().zip(&whole.nodes) { for (x, y) in a.freqs.iter().zip(&b.freqs) { assert!((x - y).abs() < 1e-9, "{x} vs {y}"); } }
    }
}

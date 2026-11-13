//! App-facing layer for the three-way river solver: spec in, summary and node views out.
//! Mirrors the two-player "study" API so the interface can treat both the same way.
use crate::cards::*;
use crate::game::*;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};
use std::time::Instant;

#[derive(Default)]
pub struct Progress3 {
    /// 0 idle, 1 normal solve, 2 habit solve, 4 done, 5 error
    pub phase: AtomicU8,
    pub iter: AtomicU32,
    pub max_iter: AtomicU32,
    pub gain_milli_pct: AtomicU32,
    pub cancel: AtomicBool,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Leak3 {
    pub player: usize,  // 0 = first to act, 2 = last to act
    #[serde(default = "all_streets")]
    pub street: String, // "turn" | "river" | "all"
    pub effect: String, // fold_less | fold_more | raise_less | bet_more | bet_less | bluff_less
    pub strength: f32,  // 0..1
}

fn all_streets() -> String { "all".into() }

#[derive(Deserialize, Clone, Debug, Default)]
pub struct Spec3 {
    pub ranges: [String; 3],
    pub board: String,
    pub pot: f64,
    pub stack: f64,
    /// earlier-street bet sizes and raise caps; default to the next street's settings
    #[serde(default)]
    pub flop_bet_sizes: String,
    #[serde(default)]
    pub flop_max_raises: usize,
    #[serde(default)]
    pub turn_bet_sizes: String,
    #[serde(default)]
    pub turn_max_raises: usize,
    /// cards per rank dealt on the turn / river: 0 = every card (exact), 1 or 2 = a balanced sample
    #[serde(default)]
    pub turn_cards: usize,
    #[serde(default)]
    pub river_cards: usize,
    pub bet_sizes: String,
    pub raise_sizes: String,
    pub max_raises: usize,
    /// solver threads; 0 = all cores
    #[serde(default)]
    pub threads: usize,
    /// stack behind per player (in acting order); default: everyone has `stack`
    #[serde(default)]
    pub stacks: Option<[f64; 3]>,
    /// players (in acting order) who folded before the spot starts; their chips stay in the pot
    #[serde(default)]
    pub folded: [bool; 3],
    /// rake as a share of the pot (0.05 = 5%) and its cap in chips (0 = no cap)
    #[serde(default)]
    pub rake_rate: f64,
    #[serde(default)]
    pub rake_cap: f64,
    /// raise sizes as multiples of the bet being raised, e.g. "2.5, 3"
    #[serde(default)]
    pub raise_mults: String,
    /// stop after this many seconds and keep the result so far (0 = no limit)
    #[serde(default)]
    pub max_seconds: f64,
    /// player names in acting order, used in node paths and locks
    #[serde(default)]
    pub names: Option<[String; 3]>,
    #[serde(default)]
    pub node_locks: Vec<NodeLock3>,
    /// chips per unit when a path gives a size in big blinds (e.g. "bet1.5bb")
    #[serde(default)]
    pub chips_per_bb: f64,
    pub allin_spr: f64,
    pub accuracy_pct: f64,
    pub max_iter: u32,
    #[serde(default)]
    pub leaks: Vec<Leak3>,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct Summary3 {
    pub base_ev: [f64; 3],
    pub base_gain_pct: [f64; 3],
    pub base_secs: f64,
    pub base_iters: u32,
    pub locked: bool,
    pub locked_ev: [f64; 3],
    pub locked_gain_pct: [f64; 3],
    pub locked_secs: f64,
    pub locked_iters: u32,
    pub locked_nodes: usize,
    pub combos: [usize; 3],
    pub nodes: usize,
    pub memory_mb: f64,
    pub cancelled: bool,
    pub error: Option<String>,
}

#[derive(Serialize, Default, Clone)]
pub struct Cell3 { /// combos of this class the board leaves possible
    pub full: f64, pub weight: f64, pub freqs: Vec<f64>, pub equity: f64, pub ev: f64 }
#[derive(Serialize, Default, Clone)]
pub struct Bucket3 { pub name: String, pub share: f64, pub freqs: Vec<f64> }
#[derive(Serialize, Default, Clone)]
pub struct Combo3 { pub hand: String, pub cell: usize, pub weight: f64, pub freqs: Vec<f64>, pub equity: f64, pub ev: f64 }

#[derive(Serialize, Default, Clone)]
pub struct NodeView3 {
    pub kind: String, // "player" | "chance" | "terminal"
    pub board: Vec<String>,
    pub street: String, // "turn" | "river"
    pub cards: Vec<(String, usize)>, // chance only: river cards that can come
    pub result: String, // terminal only: "fold" | "showdown"
    pub winner: usize,  // terminal fold only
    pub player: usize,
    pub pot: f64,
    pub invested: [f64; 3],
    pub folded: [bool; 3],
    pub stack: f64,
    pub actions: Vec<String>,
    pub action_amounts: Vec<f64>,
    pub action_freqs: Vec<f64>,
    pub grid: Vec<Cell3>,
    pub buckets: Vec<Bucket3>,
    pub combos: Vec<Combo3>,
    pub ev: [f64; 3],
    pub equity: [f64; 3],
    pub range_combos: [f64; 3],
    /// stack each player still has behind
    pub stacks: [f64; 3],
    /// short action names for paths: check, call, fold, allin, bet0.66, raise2.5
    pub action_names: Vec<String>,
    /// how often play reaches this node
    pub line_frequency: f64,
    /// true inside an exact turn re-solve
    pub exact: bool,
    pub locked: bool,
    /// false when EVs were skipped to answer quickly
    pub has_ev: bool,
    /// chance only: how many cards could really come (more than `cards` when the solve used a sample)
    pub cards_possible: usize,
}

#[derive(Default)]
pub struct State3 {
    pub base: Option<Game>,
    pub locked: Option<Game>,
    pub summary: Option<Summary3>,
    /// what was asked for, kept so a turn can be re-solved with the same settings
    pub spec: Option<Spec3>,
    /// an exact turn solve grafted onto a flop solve
    pub sub: Option<Sub>,
}

/// An exact turn + river solve started from one line and turn card of a flop solve.
pub struct Sub {
    /// whether it continues the habit solve (true) or the normal solve
    pub for_locked: bool,
    /// the flop line followed by the turn card
    pub prefix: Vec<usize>,
    pub base: Game,
    pub locked: Option<Game>,
    pub summary: Summary3,
}

/// A fixed strategy at one decision, by action name.
#[derive(Deserialize, Clone, Debug, Default)]
pub struct NodeLock3 {
    pub node: String,
    #[serde(default)]
    pub player: Option<String>,
    /// range text for the hands the lock covers; none = every hand
    #[serde(default)]
    pub hands: Option<String>,
    pub action_freqs: std::collections::BTreeMap<String, f64>,
}

fn parse_mults(s: &str) -> Result<Vec<f64>, String> {
    let mut v = vec![];
    for t in s.split(',') {
        let t = t.trim().trim_end_matches('x');
        if t.is_empty() { continue; }
        let x: f64 = t.parse().map_err(|_| format!("bad raise size '{t}'"))?;
        if x <= 1.0 || x > 100.0 { return Err(format!("raise multiple '{t}' must be above 1")); }
        v.push(x);
    }
    Ok(v)
}

fn parse_sizes(s: &str) -> Result<Vec<f64>, String> {
    let mut v = vec![];
    for t in s.split(',') {
        let t = t.trim().trim_end_matches('%');
        if t.is_empty() { continue; }
        let x: f64 = t.parse().map_err(|_| format!("bad bet size '{t}'"))?;
        if x <= 0.0 || x > 1000.0 { return Err(format!("bet size '{t}' is out of range")); }
        v.push(x / 100.0);
    }
    Ok(v)
}

pub fn build_game(spec: &Spec3, allocate: bool) -> Result<Game, String> {
    let deck = Deck::new();
    let board = parse_board(&spec.board)?;
    for i in 0..board.len() { for j in 0..i { if board[i] == board[j] { return Err("the board repeats a card".into()); } } }
    if spec.pot <= 0.0 { return Err("the pot must be above zero".into()); }
    if spec.folded.iter().filter(|&&f| f).count() > 1 { return Err("at most one player can have folded already".into()); }
    if spec.stacks.map_or(spec.stack <= 0.0, |st| st.iter().any(|&x| x <= 0.0)) { return Err("every stack must be above zero".into()); }
    let ranges = [parse_range(&deck, &spec.ranges[0])?, parse_range(&deck, &spec.ranges[1])?, parse_range(&deck, &spec.ranges[2])?];
    let river_bets = parse_sizes(&spec.bet_sizes)?;
    let turn_bets = if spec.turn_bet_sizes.trim().is_empty() { river_bets.clone() } else { parse_sizes(&spec.turn_bet_sizes)? };
    let flop_bets = if spec.flop_bet_sizes.trim().is_empty() { turn_bets.clone() } else { parse_sizes(&spec.flop_bet_sizes)? };
    let max_raises = spec.max_raises.clamp(1, 4);
    let turn_max = if spec.turn_max_raises == 0 { max_raises } else { spec.turn_max_raises.clamp(1, 4) };
    Game::new(Config {
        board, pot: spec.pot, stack: spec.stack, ranges,
        flop_bet_sizes: flop_bets, turn_bet_sizes: turn_bets, bet_sizes: river_bets, raise_sizes: parse_sizes(&spec.raise_sizes)?,
        flop_max_raises: if spec.flop_max_raises == 0 { turn_max } else { spec.flop_max_raises.clamp(1, 4) },
        turn_max_raises: turn_max, max_raises, allin_spr: spec.allin_spr.max(0.0),
        turn_cards: spec.turn_cards, river_cards: spec.river_cards,
        extra: Extra { stacks: spec.stacks, folded: spec.folded, rake_rate: spec.rake_rate.max(0.0), rake_cap: spec.rake_cap.max(0.0), raise_mults: parse_mults(&spec.raise_mults)? },
    }, allocate)
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct Estimate3 {
    pub nodes: usize,
    pub memory_mb: f64,
    pub combos: [usize; 3],
    /// rough seconds per solver step on this machine with the chosen thread count
    pub secs_per_iter: f64,
    pub turn: bool,
    /// 3 = flop, 4 = turn, 5 = river
    pub start: usize,
}

fn thread_count(spec: &Spec3) -> usize {
    let all = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    if spec.threads == 0 { all } else { spec.threads.min(all).max(1) }
}

/// Size and speed estimate for a spec; reserves no solver memory.
pub fn estimate(spec: &Spec3) -> Result<Estimate3, String> {
    let g = build_game(spec, false)?;
    let solves = if spec.leaks.is_empty() { 1.0 } else { 2.0 };
    let threads = if g.start_len() < 5 { thread_count(spec) as f64 } else { 1.0 };
    Ok(Estimate3 {
        nodes: g.nodes.len(), memory_mb: g.memory_bytes() as f64 * solves / 1e6 + 10.0,
        combos: [g.hands[0].len(), g.hands[1].len(), g.hands[2].len()],
        secs_per_iter: (g.work_units() * SECS_PER_WORK_UNIT + g.nodes.len() as f64 * SECS_PER_NODE) / (threads * 0.8).max(1.0), turn: g.start_len() < 5, start: g.start_len(),
    })
}

/// Measured on a mid-range laptop core: seconds per million (showdown x hand) evaluations per step.
const SECS_PER_WORK_UNIT: f64 = 0.028;
/// Fixed cost of visiting a tree node three times per step.
const SECS_PER_NODE: f64 = 1.0e-6;

fn gains(game: &Game) -> ([f64; 3], [f64; 3]) {
    let e = game.evaluate();
    let pot = game.cfg.pot;
    ([e[0].0, e[1].0, e[2].0], [100.0 * (e[0].1 - e[0].0) / pot, 100.0 * (e[1].1 - e[1].0) / pot, 100.0 * (e[2].1 - e[2].0) / pot])
}

fn run(game: &Game, spec: &Spec3, prog: &Progress3) -> (u32, [f64; 3], [f64; 3], bool) {
    let max_iter = spec.max_iter.max(20);
    prog.iter.store(0, Ordering::Relaxed);
    prog.max_iter.store(max_iter, Ordering::Relaxed);
    prog.gain_milli_pct.store(100_000, Ordering::Relaxed);
    let mut t = 0u32;
    let started = Instant::now();
    loop {
        t += 1;
        game.iterate(t);
        prog.iter.store(t, Ordering::Relaxed);
        let out_of_time = spec.max_seconds > 0.0 && started.elapsed().as_secs_f64() >= spec.max_seconds;
        let cancelled = prog.cancel.load(Ordering::Relaxed) || out_of_time;
        let every = if game.start_len() == 3 { 20 } else { 10 };
        if t % every == 0 || t >= max_iter || cancelled {
            let (ev, g) = gains(game);
            let worst = g.iter().cloned().fold(0.0, f64::max);
            prog.gain_milli_pct.store((worst * 1000.0).clamp(0.0, 1e9) as u32, Ordering::Relaxed);
            if (worst <= spec.accuracy_pct && t >= 20) || t >= max_iter || cancelled { return (t, ev, g, cancelled); }
        }
    }
}

fn apply_leak(strat: &mut [f32], actions: &[Action], n: usize, leak: &Leak3, equity: Option<&[f32]>) {
    let x = leak.strength.clamp(0.0, 1.0);
    let pick = |pred: &dyn Fn(&Action) -> bool| -> Vec<usize> { actions.iter().enumerate().filter(|(_, a)| pred(a)).map(|(i, _)| i).collect() };
    let fold = pick(&|a| matches!(a, Action::Fold));
    let call = pick(&|a| matches!(a, Action::Call));
    let check = pick(&|a| matches!(a, Action::Check));
    let aggr = pick(&|a| matches!(a, Action::Bet(_) | Action::Raise(_) | Action::AllIn(_)));
    let (facing, first) = (!fold.is_empty(), !check.is_empty());
    for h in 0..n {
        let g = |a: usize| a * n + h;
        let mut drain = |from: &[usize], to: usize, strat: &mut [f32]| { for &a in from { let m = strat[g(a)] * x; strat[g(a)] -= m; strat[g(to)] += m; } };
        match leak.effect.as_str() {
            "fold_less" if facing && !call.is_empty() => drain(&fold, call[0], strat),
            "fold_more" if facing => { let from: Vec<usize> = call.iter().chain(aggr.iter()).cloned().collect(); drain(&from, fold[0], strat) }
            "raise_less" if facing && !call.is_empty() => drain(&aggr, call[0], strat),
            "bet_less" if first => drain(&aggr, check[0], strat),
            "bluff_less" if first => { if equity.map(|e| e[h] < 0.33).unwrap_or(false) { drain(&aggr, check[0], strat) } }
            "bet_more" if first && !aggr.is_empty() => {
                let moved = strat[g(check[0])] * x;
                let tot: f32 = aggr.iter().map(|&a| strat[g(a)]).sum();
                strat[g(check[0])] -= moved;
                if tot > 1e-6 { for &a in &aggr { let share = strat[g(a)] / tot; strat[g(a)] += moved * share; } } else { strat[g(aggr[0])] += moved; }
            }
            _ => {}
        }
    }
}

/// Copies the habit players' solved strategies into `target` as fixed strategies, with the
/// habits applied on top ("a normal player plus this leak").
fn lock_walk(base: &Game, target: &mut Game, id: usize, r: &[Vec<f32>; 3], leaks: &[Leak3], count: &mut usize) {
    let node = &base.nodes[id];
    if node.kind == Kind::Chance {
        for (k, &card) in node.cards.iter().enumerate() {
            let nr = base.without_card([&r[0], &r[1], &r[2]], card);
            lock_walk(base, target, node.children[k], &nr, leaks, count);
        }
        return;
    }
    if node.kind != Kind::Decision { return; }
    let p = node.player; let na = node.actions.len(); let nh = base.hands[p].len();
    let sigma = base.avg_strategy(id);
    let street = match base.board_len(node) { 3 => "flop", 4 => "turn", _ => "river" };
    if leaks.iter().any(|l| l.player == p) {
        let mine: Vec<&Leak3> = leaks.iter().filter(|l| l.player == p && (l.street == "all" || l.street == street)).collect();
        let mut s = sigma.clone();
        let eq = if mine.iter().any(|l| l.effect == "bluff_less") { Some(base.equity(node.dealt, p, [&r[0], &r[1], &r[2]], node.folded)) } else { None };
        for l in mine { apply_leak(&mut s, &node.actions, nh, l, eq.as_deref()); }
        target.set_lock(id, s);
        *count += 1;
    }
    for a in 0..na {
        let mut nr = r.clone();
        let mut any = false;
        for (k, &h) in base.hands[p].iter().enumerate() { nr[p][h as usize] *= sigma[a * nh + k]; any |= nr[p][h as usize] > 0.0; }
        if !any && base.board_len(node) < 5 {
            // nobody takes this turn action: lock what lies below without walking every river
            freeze_below(base, target, node.children[a], leaks, count);
            continue;
        }
        lock_walk(base, target, node.children[a], &nr, leaks, count);
    }
}

/// Locks the habit players' decisions below `id` to their solved strategies, with no habit changes.
fn freeze_below(base: &Game, target: &mut Game, id: usize, leaks: &[Leak3], count: &mut usize) {
    let node = &base.nodes[id];
    if node.kind == Kind::Decision && leaks.iter().any(|l| l.player == node.player) {
        target.set_lock(id, base.avg_strategy(id));
        *count += 1;
    }
    for &c in &node.children { freeze_below(base, target, c, leaks, count); }
}

pub fn names_of(spec: &Spec3) -> [String; 3] {
    spec.names.clone().unwrap_or(["P1".to_string(), "P2".to_string(), "P3".to_string()])
}

/// Short, path-friendly names for the actions at a decision.
pub fn action_names(game: &Game, id: usize) -> Vec<String> {
    let node = &game.nodes[id];
    let maxinv = (0..3).filter(|&q| !node.folded[q]).map(|q| node.invested[q]).fold(0.0, f64::max);
    let pot = game.cfg.pot + node.invested.iter().sum::<f64>();
    let r = |x: f64, d: i32| { let m = 10f64.powi(d); (x * m).round() / m };
    let mut names: Vec<String> = node.actions.iter().map(|a| match a {
        Action::Fold => "fold".into(), Action::Check => "check".into(), Action::Call => "call".into(), Action::AllIn(_) => "allin".into(),
        Action::Bet(t) => format!("bet{}", r((t - maxinv) / pot, 2)),
        Action::Raise(t) => format!("raise{}", r((t - node.base) / (maxinv - node.base).max(1e-9), 1)),
    }).collect();
    for i in 0..names.len() {
        if names.iter().filter(|n| **n == names[i]).count() > 1 {
            if let Action::Bet(t) | Action::Raise(t) = node.actions[i] { names[i] = format!("{}={}", if matches!(node.actions[i], Action::Bet(_)) { "bet" } else { "raise" }, t); }
        }
    }
    names
}

/// Follows a path such as "flop/BB_check/CO_bet0.66/BTN_call/turn=7h" and returns the history
/// (action indexes, and card numbers at deals) plus the node reached.
pub fn resolve_path(game: &Game, names: &[String; 3], path: &str, chips_per_bb: f64) -> Result<(Vec<usize>, usize), String> {
    let mut id = 0usize;
    let mut hist = vec![];
    for tok in path.split('/').map(|t| t.trim()).filter(|t| !t.is_empty()) {
        let low = tok.to_ascii_lowercase();
        if ["flop", "turn", "river", "root"].contains(&low.as_str()) { continue; }
        let node = &game.nodes[id];
        match node.kind {
            Kind::FoldEnd | Kind::Showdown => return Err(format!("path '{path}': the hand is already over before '{tok}'")),
            Kind::Chance => {
                let cs = low.split(['=', ':']).last().unwrap_or("");
                let card = parse_card(cs).map_err(|_| format!("path '{path}': expected a card such as turn=7h, got '{tok}'"))?;
                let k = node.cards.iter().position(|&c| c == card).ok_or_else(|| format!("path '{path}': card {cs} is not part of this solve; it covers: {}",
                    node.cards.iter().map(|&c| card_str(c)).collect::<Vec<_>>().join(" ")))?;
                hist.push(card as usize); id = node.children[k];
            }
            Kind::Decision => {
                let me = node.player;
                let mut spec = tok;
                for (i, n) in names.iter().enumerate() {
                    if let Some(rest) = tok.strip_prefix(&format!("{n}_")) {
                        if i != me { return Err(format!("path '{path}': it is {}'s turn at '{tok}', not {n}'s", names[me])); }
                        spec = rest;
                    }
                }
                let spec = spec.to_ascii_lowercase();
                let canon = action_names(game, id);
                let maxinv = (0..3).filter(|&q| !node.folded[q]).map(|q| node.invested[q]).fold(0.0, f64::max);
                let pot = game.cfg.pot + node.invested.iter().sum::<f64>();
                let facing = maxinv > node.invested[me] + 1e-9;
                let idx = if let Some(i) = canon.iter().position(|c| *c == spec) { Some(i) }
                    else if let Some(n) = spec.strip_prefix('#') { n.parse::<usize>().ok().filter(|&i| i < canon.len()) }
                    else {
                        let kind = spec.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.' || c == '=' || c == 'b' || c == 'x' || c == '%');
                        let numtxt: String = spec.chars().filter(|c| c.is_ascii_digit() || *c == '.').collect();
                        let want: Option<f64> = numtxt.parse().ok();
                        let in_bb = spec.contains("bb") && chips_per_bb > 0.0;
                        let mut best: Option<(usize, f64)> = None;
                        for (i, a) in node.actions.iter().enumerate() {
                            let size = |t: f64| if in_bb { t / chips_per_bb } else if facing { (t - node.base) / (maxinv - node.base).max(1e-9) } else { (t - maxinv) / pot };
                            let val = match (kind, a) {
                                ("allin", Action::AllIn(_)) | ("jam", Action::AllIn(_)) | ("shove", Action::AllIn(_)) => f64::NAN,
                                ("bet", Action::Bet(t)) | ("raise", Action::Raise(t)) => size(*t),
                                ("bet", Action::AllIn(t)) if !facing => size(*t),
                                ("raise", Action::AllIn(t)) if facing => size(*t),
                                _ => continue,
                            };
                            if val.is_nan() { best = Some((i, 0.0)); break; }
                            let d = match want { Some(w) => { let d = (w - val).abs(); if d > 0.25 * w.max(0.01) { continue; } d } None => i as f64 * 1e-9 + if matches!(a, Action::AllIn(_)) { 1.0 } else { 0.0 } };
                            if best.map_or(true, |b| d < b.1) { best = Some((i, d)); }
                        }
                        best.map(|b| b.0)
                    };
                let i = idx.ok_or_else(|| format!("path '{path}': '{tok}' is not available here; choices are: {}", canon.join(", ")))?;
                hist.push(i); id = node.children[i];
            }
        }
    }
    Ok((hist, id))
}

/// The frequency a lock fixes for each action at a node (None = not listed).
fn fixed_actions(game: &Game, id: usize, lock: &NodeLock3) -> Result<Vec<Option<f64>>, String> {
    let node = &game.nodes[id];
    let canon = action_names(game, id);
    let mut fixed: Vec<Option<f64>> = vec![None; canon.len()];
    for (k, &f) in &lock.action_freqs {
        if !(0.0..=1.0).contains(&f) { return Err("node_locks: frequencies must be between 0 and 1".into()); }
        let key = k.to_ascii_lowercase();
        let idx: Vec<usize> = (0..canon.len()).filter(|&a| canon[a] == key || match (&node.actions[a], key.as_str()) {
            (Action::Bet(_), "bet") | (Action::Raise(_), "raise") | (Action::AllIn(_), "jam") | (Action::AllIn(_), "shove") => true,
            (Action::Bet(_) | Action::Raise(_) | Action::AllIn(_), "aggressive") => true,
            _ => false,
        }).collect();
        if idx.is_empty() { return Err(format!("node_locks: '{k}' is not an action at '{}'; choices are: {} (or bet, raise, aggressive)", lock.node, canon.join(", "))); }
        for &a in &idx { fixed[a] = Some(f / idx.len() as f64); }
    }
    let named: f64 = fixed.iter().flatten().sum();
    if named > 1.0 + 1e-6 { return Err("node_locks: the given frequencies add up to more than 1".into()); }
    if fixed.iter().all(|f| f.is_some()) && (named - 1.0).abs() > 0.01 { return Err("node_locks: when every action is listed the frequencies must add up to 1".into()); }
    Ok(fixed)
}

/// The strategy a lock fixes (action * hands + hand); hands it does not cover are left at zero (free).
/// Unlisted actions share what is left in the proportions of `baseline` when given, else equally.
fn lock_strategy(game: &Game, id: usize, lock: &NodeLock3, baseline: Option<&[f32]>) -> Result<Vec<f32>, String> {
    let node = &game.nodes[id];
    let p = node.player; let nh = game.hands[p].len(); let na = node.actions.len();
    let fixed = fixed_actions(game, id, lock)?;
    let selected: Vec<bool> = match lock.hands.as_deref() {
        None | Some("") | Some("all") => vec![true; nh],
        Some(r) => { let w = parse_range(&game.deck, r).map_err(|e| format!("node_locks hands: {e}"))?; game.hands[p].iter().map(|&h| w[h as usize] > 0.0).collect() }
    };
    let named: f64 = fixed.iter().flatten().sum();
    let free: Vec<usize> = (0..na).filter(|&a| fixed[a].is_none()).collect();
    let mut out = vec![0f32; na * nh];
    for h in 0..nh {
        if !selected[h] { continue; }
        let base: Vec<f64> = free.iter().map(|&a| baseline.map(|b| b[a * nh + h] as f64).unwrap_or(1.0)).collect();
        let bs: f64 = base.iter().sum();
        for a in 0..na { if let Some(f) = fixed[a] { out[a * nh + h] = f as f32; } }
        for (j, &a) in free.iter().enumerate() { out[a * nh + h] = ((1.0 - named) * if bs > 1e-9 { base[j] / bs } else { 1.0 / free.len() as f64 }) as f32; }
        if (0..na).all(|a| out[a * nh + h] <= 0.0) { out[h] = 1e-6; }
    }
    Ok(out)
}

/// Lays `over` on top of an existing lock at a node, hand by hand.
fn merge_lock(game: &mut Game, id: usize, over: Vec<f32>) {
    let nh = game.hands[game.nodes[id].player].len(); let na = game.nodes[id].actions.len();
    let mut cur = game.nodes[id].locked.clone().unwrap_or_else(|| vec![0f32; na * nh]);
    for h in 0..nh { if (0..na).any(|a| over[a * nh + h] > 0.0) { for a in 0..na { cur[a * nh + h] = over[a * nh + h]; } } }
    game.set_lock(id, cur);
}

/// Solves `cfg`. With habits, or locks that leave actions unlisted, a normal solve runs first and
/// the returned pair is (normal, Some(locked)); otherwise (the one solve, None).
fn solve_cfg(cfg: &Config, spec: &Spec3, leaks: &[Leak3], locks: &[NodeLock3], prog: &Progress3, pool: &rayon::ThreadPool) -> Result<(Game, Option<Game>, Summary3), String> {
    let names = names_of(spec);
    let mut base = Game::new(cfg.clone(), true)?;
    let mut sum = Summary3 { nodes: base.nodes.len(), combos: [base.hands[0].len(), base.hands[1].len(), base.hands[2].len()], ..Default::default() };
    // where each lock sits, and whether any of them needs the normal solve
    let mut placed: Vec<(usize, &NodeLock3)> = vec![];
    let mut two_pass = !leaks.is_empty();
    for l in locks {
        let (_, id) = resolve_path(&base, &names, &l.node, spec.chips_per_bb)?;
        if base.nodes[id].kind != Kind::Decision { return Err(format!("node_locks: '{}' is not a decision", l.node)); }
        if let Some(p) = &l.player { if *p != names[base.nodes[id].player] { return Err(format!("node_locks: at '{}' it is {}'s decision, not {p}'s", l.node, names[base.nodes[id].player])); } }
        if fixed_actions(&base, id, l)?.iter().any(|f| f.is_none()) { two_pass = true; }
        placed.push((id, l));
    }
    if !two_pass {
        for (id, l) in &placed { let st = lock_strategy(&base, *id, l, None)?; merge_lock(&mut base, *id, st); }
        sum.locked_nodes = placed.len();
    }
    prog.phase.store(1, Ordering::Relaxed);
    prog.iter.store(0, Ordering::Relaxed);
    let t0 = Instant::now();
    let (it, ev, g, cancelled) = pool.install(|| run(&base, spec, prog));
    sum.base_iters = it; sum.base_ev = ev; sum.base_gain_pct = g; sum.base_secs = t0.elapsed().as_secs_f64();
    sum.memory_mb = base.memory_bytes() as f64 / 1e6;
    sum.cancelled = cancelled;
    let mut locked = None;
    if two_pass && !cancelled {
        prog.phase.store(2, Ordering::Relaxed);
        prog.iter.store(0, Ordering::Relaxed);
        let mut lg = Game::new(cfg.clone(), true)?;
        let r = base.cfg.ranges.clone();
        let mut count = 0;
        if !leaks.is_empty() { pool.install(|| lock_walk(&base, &mut lg, 0, &r, leaks, &mut count)); }
        for (id, l) in &placed {
            let st = lock_strategy(&base, *id, l, Some(&base.avg_strategy(*id)))?;
            if lg.nodes[*id].locked.is_none() { count += 1; }
            merge_lock(&mut lg, *id, st);
        }
        let t1 = Instant::now();
        let (it, ev, g, cancelled) = pool.install(|| run(&lg, spec, prog));
        sum.locked = true; sum.locked_iters = it; sum.locked_ev = ev; sum.locked_gain_pct = g;
        sum.locked_secs = t1.elapsed().as_secs_f64(); sum.locked_nodes = count; sum.cancelled = cancelled;
        sum.memory_mb += lg.memory_bytes() as f64 / 1e6;
        locked = Some(lg);
    }
    Ok((base, locked, sum))
}

pub fn solve3(spec: &Spec3, prog: &Progress3) -> Result<State3, String> {
    prog.cancel.store(false, Ordering::Relaxed);
    for l in &spec.leaks { if l.player > 2 { return Err("habit player must be 0, 1 or 2".into()); } }
    let pool = rayon::ThreadPoolBuilder::new().num_threads(thread_count(spec)).build().map_err(|e| e.to_string())?;
    let cfg = build_game(spec, false)?.cfg;
    let (base, locked, sum) = solve_cfg(&cfg, spec, &spec.leaks, &spec.node_locks, prog, &pool)?;
    Ok(State3 { base: Some(base), locked, summary: Some(sum), spec: Some(spec.clone()), sub: None })
}

/// Walks a history and returns the node reached and everyone's reach there.
fn reach_at(game: &Game, history: &[usize]) -> Result<(usize, [Vec<f32>; 3]), String> {
    let mut r = game.cfg.ranges.clone();
    let mut id = 0usize;
    for &a in history {
        let node = &game.nodes[id];
        match node.kind {
            Kind::Chance => {
                let k = node.cards.iter().position(|&c| c as usize == a).ok_or("that card is not part of this solve")?;
                r = game.without_card([&r[0], &r[1], &r[2]], a as u8);
                id = node.children[k];
            }
            Kind::Decision if a < node.actions.len() => {
                let p = node.player; let nh = game.hands[p].len();
                let s = game.avg_strategy(id);
                for (k, &h) in game.hands[p].iter().enumerate() { r[p][h as usize] *= s[a * nh + k]; }
                id = node.children[a];
            }
            _ => return Err("that line does not exist in this tree".into()),
        }
    }
    Ok((id, r))
}

/// After a flop solve: solves the turn and river exactly (every river card) for one flop line and
/// any turn card, starting from the ranges that reach it. Works when a player has already folded.
pub fn resolve_turn(st: &State3, for_locked: bool, history: &[usize], card: u8, prog: &Progress3) -> Result<Sub, String> {
    let spec = st.spec.as_ref().ok_or("solve a spot first")?;
    let g = if for_locked { st.locked.as_ref() } else { st.base.as_ref() }.ok_or("solve a spot first")?;
    let (id, r) = reach_at(g, history)?;
    let node = &g.nodes[id];
    if node.kind != Kind::Chance || g.board_len(node) != 3 { return Err("an exact turn solve starts where the flop betting ends".into()); }
    if g.cfg.board.contains(&card) { return Err("that card is already on the board".into()); }
    let nr = g.without_card([&r[0], &r[1], &r[2]], card);
    for p in 0..3 { if !node.folded[p] && nr[p].iter().all(|&x| x <= 0.0) { return Err("this line is never played in the flop solve, so there are no ranges to continue from".into()); } }
    let mut cfg = g.cfg.clone();
    cfg.board.push(card);
    cfg.pot = g.cfg.pot + node.invested.iter().sum::<f64>();
    let mut stacks = [0.0; 3];
    for p in 0..3 { stacks[p] = (g.stacks[p] - node.invested[p]).max(0.0); }
    cfg.extra.stacks = Some(stacks);
    cfg.extra.folded = node.folded;
    cfg.river_cards = 0;
    cfg.ranges = nr;
    let mut sp = spec.clone();
    sp.accuracy_pct = sp.accuracy_pct.min(0.3); sp.max_iter = 300; sp.max_seconds = 0.0;
    prog.cancel.store(false, Ordering::Relaxed);
    prog.phase.store(1, Ordering::Relaxed); prog.iter.store(0, Ordering::Relaxed); prog.max_iter.store(sp.max_iter as u32, Ordering::Relaxed); prog.gain_milli_pct.store(100_000, Ordering::Relaxed);
    let pool = rayon::ThreadPoolBuilder::new().num_threads(thread_count(spec)).build().map_err(|e| e.to_string())?;
    // habits carry on into the turn and river when continuing the habit solve
    let leaks: Vec<Leak3> = if for_locked { spec.leaks.clone() } else { vec![] };
    let (base, locked, summary) = solve_cfg(&cfg, &sp, &leaks, &[], prog, &pool)?;
    let mut prefix = history.to_vec(); prefix.push(card as usize);
    Ok(Sub { for_locked, prefix, base, locked, summary })
}

pub fn grid_index(cards: (u8, u8)) -> usize {
    let (a, b) = cards;
    let (ra, rb) = ((a / 4) as usize, (b / 4) as usize);
    let (hi, lo) = if ra >= rb { (ra, rb) } else { (rb, ra) };
    let (r_hi, r_lo) = (12 - hi, 12 - lo);
    if hi == lo { r_hi * 13 + r_hi } else if a % 4 == b % 4 { r_hi * 13 + r_lo } else { r_lo * 13 + r_hi }
}

/// 169-cell chart (AA top-left) of how much of each hand class a range text contains.
pub fn range_grid(range: &str) -> Result<Vec<f32>, String> {
    let deck = Deck::new();
    let w = parse_range(&deck, range)?;
    let mut sum = vec![0f32; 169]; let mut cnt = vec![0f32; 169];
    for (i, &c) in deck.cards.iter().enumerate() { let g = grid_index(c); sum[g] += w[i]; cnt[g] += 1.0; }
    Ok(sum.iter().zip(cnt.iter()).map(|(s, c)| s / c).collect())
}

fn hand_name(c: (u8, u8)) -> String { format!("{}{}", card_str(c.1), card_str(c.0)) }

pub fn node_view(game: &Game, history: &[usize], with_ev: bool) -> Result<NodeView3, String> {
    let mut r = game.cfg.ranges.clone();
    let mut id = 0usize;
    let mut board: Vec<String> = game.cfg.board.iter().map(|&c| card_str(c)).collect();
    let mut line_freq = 1.0f64;
    for &a in history {
        let node = &game.nodes[id];
        if node.kind == Kind::Chance {
            let k = node.cards.iter().position(|&c| c as usize == a).ok_or("that card is not part of this solve")?;
            r = game.without_card([&r[0], &r[1], &r[2]], a as u8);
            board.push(card_str(a as u8));
            id = node.children[k];
            continue;
        }
        if node.kind != Kind::Decision || a >= node.actions.len() { return Err("that line does not exist in this tree".into()); }
        let p = node.player; let nh = game.hands[p].len();
        let s = game.avg_strategy(id);
        let (mut before, mut after) = (0f64, 0f64);
        for (k, &h) in game.hands[p].iter().enumerate() { before += r[p][h as usize] as f64; r[p][h as usize] *= s[a * nh + k]; after += r[p][h as usize] as f64; }
        line_freq *= if before > 0.0 { after / before } else { 0.0 };
        id = game.nodes[id].children[a];
    }
    let (kind, inv, folded, player, dealt, blen) = { let n = &game.nodes[id]; (n.kind, n.invested, n.folded, n.player, n.dealt, game.board_len(n)) };
    let mut v = NodeView3 { pot: game.cfg.pot + inv.iter().sum::<f64>(), invested: inv, folded, stack: game.cfg.stack, board, has_ev: with_ev, line_frequency: line_freq,
        stacks: [(game.stacks[0] - inv[0]).max(0.0), (game.stacks[1] - inv[1]).max(0.0), (game.stacks[2] - inv[2]).max(0.0)],
        street: match blen { 3 => "flop".into(), 4 => "turn".into(), _ => "river".into() }, ..Default::default() };
    let mut per_hand: Vec<(Vec<f64>, Vec<f32>, Vec<f32>)> = vec![]; // (weight, ev, equity) per player
    for p in 0..3 {
        v.range_combos[p] = r[p].iter().map(|&x| x as f64).sum();
        let z = game.reach_sum(p, &r[(p + 1) % 3], &r[(p + 2) % 3]);
        let cfv = if with_ev { game.walk(id, p, [&r[0], &r[1], &r[2]], Mode::Average) } else { vec![0.0; game.hands[p].len()] };
        let eq = game.equity(dealt, p, [&r[0], &r[1], &r[2]], folded);
        let nh = game.hands[p].len();
        let mut w = vec![0f64; nh]; let mut evh = vec![0f32; nh];
        let (mut zs, mut es, mut qs) = (0f64, 0f64, 0f64);
        for (k, &h) in game.hands[p].iter().enumerate() {
            let rw = r[p][h as usize] as f64;
            w[k] = rw * z[k];
            evh[k] = if z[k] > 0.0 { (cfv[k] as f64 / z[k] + inv[p]) as f32 } else { 0.0 };
            zs += w[k]; es += rw * cfv[k] as f64; qs += w[k] * eq[k] as f64;
        }
        if zs > 0.0 && !folded[p] { v.ev[p] = es / zs + inv[p]; v.equity[p] = qs / zs; }
        // rescale weights so they add up to the number of combos still in the range
        let scale = if zs > 0.0 { v.range_combos[p] / zs } else { 0.0 };
        for x in w.iter_mut() { *x *= scale; }
        per_hand.push((w, evh, eq));
    }
    match kind {
        Kind::FoldEnd => { v.kind = "terminal".into(); v.result = "fold".into(); v.winner = player; return Ok(v); }
        Kind::Showdown => { v.kind = "terminal".into(); v.result = "showdown".into(); return Ok(v); }
        Kind::Chance => {
            v.kind = "chance".into();
            v.cards = game.nodes[id].cards.iter().map(|&c| (card_str(c), c as usize)).collect();
            v.cards_possible = 52 - blen;
            return Ok(v);
        }
        Kind::Decision => {}
    }
    v.kind = "player".into();
    v.player = player;
    v.action_names = action_names(game, id);
    let node = &game.nodes[id];
    v.locked = node.locked.is_some();
    for a in &node.actions {
        v.actions.push(a.label());
        v.action_amounts.push(match a { Action::Bet(x) | Action::Raise(x) | Action::AllIn(x) => *x, _ => 0.0 });
    }
    let na = node.actions.len(); let nh = game.hands[player].len();
    let s = game.avg_strategy(id);
    let (w, evh, eq) = &per_hand[player];
    let total: f64 = w.iter().sum();
    let mut grid: Vec<Cell3> = (0..169).map(|_| Cell3 { freqs: vec![0.0; na], ..Default::default() }).collect();
    let names = ["Strong (65%+ equity)", "Medium (35-65%)", "Weak (under 35%)"];
    let mut buckets: Vec<Bucket3> = names.iter().map(|n| Bucket3 { name: n.to_string(), share: 0.0, freqs: vec![0.0; na] }).collect();
    v.action_freqs = vec![0.0; na];
    let mut shown: Vec<u8> = game.cfg.board.clone(); for &c in &dealt { if c != 255 { shown.push(c); } }
    for &c in game.deck.cards.iter() { if !shown.contains(&c.0) && !shown.contains(&c.1) { grid[grid_index(c)].full += 1.0; } }
    for k in 0..nh {
        let wt = w[k]; if wt <= 1e-9 { continue; }
        let cards = game.deck.cards[game.hands[player][k] as usize];
        let gi = grid_index(cards);
        let fr: Vec<f64> = (0..na).map(|a| s[a * nh + k] as f64).collect();
        let b = if eq[k] >= 0.65 { 0 } else if eq[k] >= 0.35 { 1 } else { 2 };
        let c = &mut grid[gi];
        c.weight += wt; c.equity += eq[k] as f64 * wt; c.ev += evh[k] as f64 * wt;
        buckets[b].share += wt;
        for a in 0..na { c.freqs[a] += fr[a] * wt; buckets[b].freqs[a] += fr[a] * wt; v.action_freqs[a] += fr[a] * wt; }
        if wt > 0.0005 { v.combos.push(Combo3 { hand: hand_name(cards), cell: gi, weight: wt, freqs: fr, equity: eq[k] as f64, ev: evh[k] as f64 }); }
    }
    for c in grid.iter_mut() { if c.weight > 0.0 { c.equity /= c.weight; c.ev /= c.weight; for f in c.freqs.iter_mut() { *f /= c.weight; } } }
    for b in buckets.iter_mut() { if b.share > 0.0 { for f in b.freqs.iter_mut() { *f /= b.share; } } b.share = if total > 0.0 { b.share / total } else { 0.0 }; }
    if total > 0.0 { for f in v.action_freqs.iter_mut() { *f /= total; } }
    v.grid = grid; v.buckets = buckets;
    Ok(v)
}

impl State3 {
    /// Memory held by the solves kept for browsing (released when the next solve starts).
    pub fn held_bytes(&self) -> u64 {
        let sub = self.sub.as_ref().map_or(0, |s| s.base.memory_bytes() + s.locked.as_ref().map_or(0, |g| g.memory_bytes())) as u64;
        [self.base.as_ref(), self.locked.as_ref()].iter().flatten().map(|g| g.memory_bytes() as u64).sum::<u64>() + sub
    }

    pub fn view(&self, locked: bool, history: &[usize], with_ev: bool) -> Result<NodeView3, String> {
        // inside an exact turn re-solve, answer from it
        if let Some(sub) = &self.sub {
            let n = sub.prefix.len();
            if sub.for_locked == locked && history.len() >= n && history[..n] == sub.prefix[..] {
                let g = if locked { sub.locked.as_ref().unwrap_or(&sub.base) } else { &sub.base };
                let mut v = node_view(g, &history[n..], with_ev)?;
                v.exact = true;
                return Ok(v);
            }
        }
        let g = if locked { self.locked.as_ref() } else { self.base.as_ref() };
        node_view(g.ok_or("solve a spot first")?, history, with_ev)
    }
}

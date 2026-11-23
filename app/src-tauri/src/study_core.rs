// Guided "Study" backend: one-call solve of a baseline spot, optional
// leak profile applied as node locks across the whole tree, and simple
// beginner-friendly node views (action mix, 13x13 grid, strength buckets).
//
// Copyright (C) 2026 contributors. Licensed under AGPL-3.0-or-later,
// same as the upstream desktop-postflop / postflop-solver projects.

use postflop_solver::*;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};

use std::time::Instant;
use std::collections::HashMap;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::PathBuf;

#[cfg(feature = "zstd")]
const SAVE_LEVEL: Option<i32> = Some(3);
#[cfg(not(feature = "zstd"))]
const SAVE_LEVEL: Option<i32> = None;

#[derive(Default)]
pub struct StudyProgress {
    pub phase: AtomicU8, // 0 idle, 1 baseline, 2 building locks, 3 locked solve, 4 done, 5 error/cancelled
    pub iter: AtomicU32,
    pub max_iter: AtomicU32,
    pub expl_milli_pct: AtomicU32, // exploitability in 1/1000 of a percent of the pot
    pub cancel: AtomicBool,
}

#[derive(Default)]
pub struct StudyState {
    pub base: Option<PostFlopGame>,
    pub locked: Option<PostFlopGame>,
    pub summary: Option<StudySummary>,
    pub base_file: Option<PathBuf>,
    pub locked_file: Option<PathBuf>,
    pub swap_mode: bool,
    pub cache: HashMap<(bool, Vec<usize>), NodeView>,
    pub confidence: f32,
}

/// Per-process scratch folder for parked solves, so two open copies never collide.
/// Folders left behind by copies that are no longer running are removed after a day.
pub fn work_dir() -> PathBuf {
    let root = std::env::temp_dir().join("pontiac-solver");
    let d = root.join(format!("run-{}", std::process::id()));
    if !d.exists() {
        if let Ok(rd) = std::fs::read_dir(&root) {
            for e in rd.flatten() {
                let old = e
                    .metadata()
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.elapsed().ok())
                    .map(|age| age.as_secs() > 24 * 3600)
                    .unwrap_or(false);
                if old {
                    let _ = std::fs::remove_dir_all(e.path());
                }
            }
        }
    }
    let _ = std::fs::create_dir_all(&d);
    d
}

impl StudyState {
    /// Memory held by the solves kept for browsing (released when the next solve starts).
    pub fn held_bytes(&self) -> u64 {
        [self.base.as_ref(), self.locked.as_ref()].iter().flatten().map(|g| g.memory_usage().1).sum()
    }

    pub fn reset(&mut self) {
        self.base = None;
        self.locked = None;
        self.summary = None;
        self.base_file = None;
        self.locked_file = None;
        self.swap_mode = false;
        self.cache.clear();
        if let Ok(rd) = std::fs::read_dir(work_dir()) {
            for e in rd.flatten() {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }

    pub fn install(&mut self, out: SolveOutcome) {
        self.base = out.base;
        self.locked = out.locked;
        self.base_file = out.base_file;
        self.swap_mode = out.summary.swap_mode;
        self.confidence = out.confidence;
        self.summary = Some(out.summary);
        self.cache.clear();
    }

    /// Returns the requested game, loading it from disk if needed. In memory-saver
    /// mode only one game is kept in memory, the other is parked on disk.
    pub fn game(&mut self, locked: bool) -> Result<&mut PostFlopGame, String> {
        let have = if locked { self.locked.is_some() } else { self.base.is_some() };
        if !have {
            let file = if locked { self.locked_file.clone() } else { self.base_file.clone() };
            let file = file.ok_or("No solved game yet")?;
            if self.swap_mode {
                // park the other game first
                let other = if locked { self.base.take() } else { self.locked.take() };
                if let Some(g) = other {
                    let other_file = if locked { &mut self.base_file } else { &mut self.locked_file };
                    if other_file.is_none() {
                        let path = work_dir().join(if locked { "base.bin" } else { "locked.bin" });
                        save_data_to_file(&g, "", &path, None).map_err(|e| format!("Could not park a solve on disk: {e}"))?;
                        *other_file = Some(path);
                    }
                    drop(g);
                }
            }
            let (mut g, _): (PostFlopGame, String) =
                load_data_from_file(&file, None).map_err(|e| format!("Could not reload a solve from disk: {e}"))?;
            // prize-money payoffs are not saved to disk either
            if let Some(p) = self.summary.as_ref().filter(|x| !x.payoffs.is_empty()) { g.set_custom_payoffs(p.payoffs.clone())?; }
            if locked {
                // soft-lock weights are not saved to disk; restore them
                g.set_all_locking_weights(if self.confidence > 0.0 { self.confidence } else { 1.0 });
            }
            if locked { self.locked = Some(g) } else { self.base = Some(g) }
        }
        Ok(if locked { self.locked.as_mut().unwrap() } else { self.base.as_mut().unwrap() })
    }

    pub fn view(&mut self, locked: bool, history: &[usize]) -> Result<NodeView, String> {
        let key = (locked, history.to_vec());
        if let Some(v) = self.cache.get(&key) {
            return Ok(v.clone());
        }
        let v = node_view(self.game(locked)?, history)?;
        self.cache.insert(key, v.clone());
        Ok(v)
    }
}

pub struct SolveOutcome {
    pub confidence: f32,
    pub base: Option<PostFlopGame>,
    pub locked: Option<PostFlopGame>,
    pub base_file: Option<PathBuf>,
    pub summary: StudySummary,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Leak {
    pub player: usize,  // 0 = OOP, 1 = IP
    pub street: String, // "flop" | "turn" | "river" | "all"
    pub effect: String, // fold_less | fold_more | raise_less | bet_more | bet_less | bluff_less
    pub strength: f32,  // 0..1, share of the targeted frequency that is moved
}

#[derive(Deserialize, Clone, Debug)]
pub struct StudySpec {
    pub oop_range: String,
    pub ip_range: String,
    pub board: String,
    pub pot: i32,
    pub stack: i32,
    pub oop_flop_bet: String,
    pub ip_flop_bet: String,
    pub turn_bet: String,
    pub river_bet: String,
    pub raise: String,
    pub allin_threshold: f64,
    pub accuracy_pct: f32,
    pub max_iter: u32,
    pub leaks: Vec<Leak>,
    pub threads: usize,
    #[serde(default)]
    pub low_memory: bool,
    #[serde(default = "default_true")]
    pub freeze_rest: bool,
    /// How much the opponent sticks to the locked model (1 = always, a full exploit; lower values
    /// let them play well part of the time, giving a more robust "restricted Nash response").
    #[serde(default = "default_one")]
    pub confidence: f32,
    /// Play for tournament prize money instead of chips (two-player pot, other players sitting out).
    #[serde(default)]
    pub icm: Option<IcmSpec>,
}

/// Prize-money setup, in the same chip units as `pot` and `stack`.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct IcmSpec {
    /// prizes, first place first (any unit)
    pub payouts: Vec<f64>,
    /// stacks behind at the start of the spot; 0 means the effective stack
    #[serde(default)]
    pub oop_stack: f64,
    #[serde(default)]
    pub ip_stack: f64,
    /// stacks of players still in the tournament who are not in this hand
    #[serde(default)]
    pub others: Vec<f64>,
}

/// What the solve means in prize money: shares of the prize pool, in percent, [out of position, in position].
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct IcmView {
    pub base_share: [f64; 2],
    pub locked_share: [f64; 2],
    pub split: [f64; 2],
    pub give_up: [f64; 2],
}

pub fn pot_icm(spec: &StudySpec) -> Result<Option<pushfold::icm::PotIcm>, String> {
    let Some(i) = &spec.icm else { return Ok(None) };
    let eff = spec.stack as f64;
    let behind = [if i.oop_stack > 0.0 { i.oop_stack } else { eff }, if i.ip_stack > 0.0 { i.ip_stack } else { eff }];
    if (behind[0].min(behind[1]) - eff).abs() > 0.5 { return Err("Prize money: the smaller of the two players' stacks must equal the stack behind set for the spot.".into()); }
    pushfold::icm::PotIcm::new(behind, i.others.clone(), i.payouts.clone(), spec.pot as f64).map(Some)
}

fn default_one() -> f32 {
    1.0
}

fn default_true() -> bool {
    true
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct StudySummary {
    pub base_ev: [f64; 2],
    pub base_expl_pct: f64,
    pub base_secs: f64,
    pub locked: bool,
    pub locked_ev: [f64; 2],
    pub locked_expl_pct: f64,
    pub locked_secs: f64,
    pub locked_nodes: usize,
    pub memory_mb: f64,
    pub swap_mode: bool,
    pub error: Option<String>,
    /// prize-money payoffs in force (empty when playing for chips); kept so a reloaded game gets them back
    #[serde(default)]
    pub payoffs: Vec<(i32, [[f64; 3]; 2])>,
    #[serde(default)]
    pub icm: Option<IcmView>,
}

#[derive(Serialize, Default, Clone)]
pub struct Cell {
    pub full: f64,         // combos of this class the board leaves possible
    pub reach: f64,        // combos of this class still in the range here
    pub weight: f64,       // reach-weighted combos in this class (0 = not in range)
    pub freqs: Vec<f64>,   // action mix (sum 1) for player nodes
    pub equity: f64,
    pub ev: f64,
}

#[derive(Serialize, Default, Clone)]
pub struct Bucket {
    pub name: String,
    pub share: f64,      // share of the acting range
    pub freqs: Vec<f64>, // action mix within the bucket
}

#[derive(Serialize, Default, Clone)]
pub struct NodeView {
    pub kind: String, // "player" | "chance" | "terminal"
    pub player: usize,
    pub board: Vec<String>,
    pub pot: i32,
    pub bets: [i32; 2],
    pub stack: i32,
    pub actions: Vec<String>,
    pub action_amounts: Vec<i32>,
    pub action_freqs: Vec<f64>,
    pub grid: Vec<Cell>, // 169 cells, row-major AA..22 like a standard chart
    pub buckets: Vec<Bucket>,
    pub ev: [f64; 2],
    pub equity: [f64; 2],
    pub cards: Vec<(String, usize)>, // chance node: dealable cards (label, id)
    pub locked: bool,
    pub range_combos: [f64; 2],
    /// per player: (chance to win, weight) for every hand still in range, for equity graphs
    pub curves: Vec<Vec<[f32; 2]>>,
    /// acting player's exact hands still in range
    pub combos: Vec<Combo>,
}

#[derive(Serialize, Default, Clone)]
pub struct Combo { pub hand: String, pub cell: usize, pub weight: f64, pub freqs: Vec<f64>, pub equity: f64, pub ev: f64 }

fn parse_board(board: &str) -> Result<(  [Card; 3], Card, Card, BoardState), String> {
    let s: String = board.chars().filter(|c| !c.is_whitespace()).collect();
    if s.len() < 6 || s.len() % 2 != 0 || s.len() > 10 {
        return Err("Board must have 3 to 5 cards, e.g. Ks7h2d".into());
    }
    let flop = flop_from_str(&s[..6]).map_err(|e| e.to_string())?;
    let turn = if s.len() >= 8 { card_from_str(&s[6..8]).map_err(|e| e.to_string())? } else { NOT_DEALT };
    let river = if s.len() == 10 { card_from_str(&s[8..10]).map_err(|e| e.to_string())? } else { NOT_DEALT };
    let state = match s.len() { 6 => BoardState::Flop, 8 => BoardState::Turn, _ => BoardState::River };
    Ok((flop, turn, river, state))
}

pub fn build_game(spec: &StudySpec) -> Result<PostFlopGame, String> {
    let (flop, turn, river, state) = parse_board(&spec.board)?;
    let oop: Range = spec.oop_range.parse().map_err(|e: String| format!("Out-of-position range: {e}"))?;
    let ip: Range = spec.ip_range.parse().map_err(|e: String| format!("In-position range: {e}"))?;
    let card_config = CardConfig { range: [oop, ip], flop, turn, river };
    let bs = |b: &str| BetSizeOptions::try_from((b, spec.raise.as_str())).map_err(|e| format!("Bet sizes '{b}': {e}"));
    let tree_config = TreeConfig {
        initial_state: state,
        starting_pot: spec.pot,
        effective_stack: spec.stack,
        rake_rate: 0.0,
        rake_cap: 0.0,
        flop_bet_sizes: [bs(&spec.oop_flop_bet)?, bs(&spec.ip_flop_bet)?],
        turn_bet_sizes: [bs(&spec.turn_bet)?, bs(&spec.turn_bet)?],
        river_bet_sizes: [bs(&spec.river_bet)?, bs(&spec.river_bet)?],
        turn_donk_sizes: None,
        river_donk_sizes: None,
        add_allin_threshold: spec.allin_threshold,
        force_allin_threshold: 0.15,
        merging_threshold: 0.1,
    };
    let tree = ActionTree::new(tree_config)?;
    let mut game = PostFlopGame::with_config(card_config, tree)?;
    if let Some(icm) = pot_icm(spec)? { game.set_custom_payoffs(icm.table(&game.terminal_amounts()))?; }
    Ok(game)
}

fn run_solve(game: &PostFlopGame, spec: &StudySpec, prog: &StudyProgress) -> f32 {
    let target = spec.pot as f32 * spec.accuracy_pct / 100.0;
    prog.max_iter.store(spec.max_iter, Ordering::Relaxed);
    prog.expl_milli_pct.store(u32::MAX, Ordering::Relaxed); // "not measured yet"
    let mut expl = f32::MAX;
    for i in 0..spec.max_iter {
        if prog.cancel.load(Ordering::Relaxed) {
            break;
        }
        solve_step(game, i);
        prog.iter.store(i + 1, Ordering::Relaxed);
        if (i + 1) % 10 == 0 || i + 1 == spec.max_iter {
            expl = compute_exploitability(game);
            prog.expl_milli_pct
                .store((expl / spec.pot as f32 * 100_000.0).max(0.0) as u32, Ordering::Relaxed);
            if expl <= target {
                break;
            }
        }
    }
    expl
}

fn root_ev(game: &mut PostFlopGame) -> [f64; 2] {
    game.back_to_root();
    game.cache_normalized_weights();
    let mut out = [0.0; 2];
    for p in 0..2 {
        let ev = game.expected_values(p);
        out[p] = weighted_average_f(&ev, game.normalized_weights(p));
    }
    out
}

fn weighted_average_f(v: &[f32], w: &[f32]) -> f64 {
    let (mut s, mut t) = (0.0f64, 0.0f64);
    for (&a, &b) in v.iter().zip(w) {
        s += a as f64 * b as f64;
        t += b as f64;
    }
    if t > 0.0 { s / t } else { 0.0 }
}

fn street_name(board_len: usize) -> &'static str {
    match board_len { 3 => "flop", 4 => "turn", _ => "river" }
}

/// Apply one leak to a per-hand strategy (layout action * n + hand). Returns true if changed.
pub fn apply_leak(strat: &mut [f32], actions: &[Action], n: usize, leak: &Leak, equity: Option<&[f32]>) -> bool {
    let x = leak.strength.clamp(0.0, 1.0);
    let facing_bet = actions.iter().any(|a| matches!(a, Action::Fold));
    let first = actions.iter().any(|a| matches!(a, Action::Check));
    let idx = |pred: &dyn Fn(&Action) -> bool| -> Vec<usize> {
        actions.iter().enumerate().filter(|(_, a)| pred(a)).map(|(i, _)| i).collect()
    };
    let fold = idx(&|a| matches!(a, Action::Fold));
    let call = idx(&|a| matches!(a, Action::Call));
    let check = idx(&|a| matches!(a, Action::Check));
    let aggr = idx(&|a| matches!(a, Action::Bet(_) | Action::Raise(_) | Action::AllIn(_)));
    let mut changed = false;
    for h in 0..n {
        let g = |a: usize| a * n + h;
        match leak.effect.as_str() {
            "fold_less" if facing_bet && !call.is_empty() => {
                let moved = strat[g(fold[0])] * x;
                strat[g(fold[0])] -= moved;
                strat[g(call[0])] += moved;
                changed |= moved > 0.0;
            }
            "fold_more" if facing_bet => {
                let mut moved = 0.0;
                for &a in call.iter().chain(aggr.iter()) {
                    let m = strat[g(a)] * x;
                    strat[g(a)] -= m;
                    moved += m;
                }
                strat[g(fold[0])] += moved;
                changed |= moved > 0.0;
            }
            "raise_less" if facing_bet && !call.is_empty() => {
                let mut moved = 0.0;
                for &a in &aggr {
                    let m = strat[g(a)] * x;
                    strat[g(a)] -= m;
                    moved += m;
                }
                strat[g(call[0])] += moved;
                changed |= moved > 0.0;
            }
            "bet_less" if first => {
                let mut moved = 0.0;
                for &a in &aggr {
                    let m = strat[g(a)] * x;
                    strat[g(a)] -= m;
                    moved += m;
                }
                strat[g(check[0])] += moved;
                changed |= moved > 0.0;
            }
            "bluff_less" if first => {
                // only hands that are behind (< 45% equity vs the opponent's current range)
                let behind = equity.map(|e| e[h] < 0.45).unwrap_or(false);
                if behind {
                    let mut moved = 0.0;
                    for &a in &aggr {
                        let m = strat[g(a)] * x;
                        strat[g(a)] -= m;
                        moved += m;
                    }
                    strat[g(check[0])] += moved;
                    changed |= moved > 0.0;
                }
            }
            "bet_more" if first && !aggr.is_empty() => {
                let moved = strat[g(check[0])] * x;
                if moved > 0.0 {
                    let bet_total: f32 = aggr.iter().map(|&a| strat[g(a)]).sum();
                    strat[g(check[0])] -= moved;
                    if bet_total > 1e-6 {
                        for &a in &aggr {
                            let share = strat[g(a)] / bet_total;
                            strat[g(a)] += moved * share;
                        }
                    } else {
                        strat[g(aggr[0])] += moved; // smallest size
                    }
                    changed = true;
                }
            }
            _ => {}
        }
    }
    changed
}

/// Walk the solved baseline tree and compute lock targets for every node a leak touches.
/// Walks the solved baseline and emits a lock for every decision the habits touch. With
/// `freeze_rest`, every other decision of the habit player is locked to its baseline strategy,
/// so the opponent plays "normally plus the habit" instead of re-optimising around it.
pub fn collect_locks(base: &mut PostFlopGame, leaks: &[Leak], freeze_rest: bool, mut sink: impl FnMut(&[usize], &[f32])) -> usize {
    let mut count = 0usize;
    let mut stack: Vec<Vec<usize>> = vec![Vec::new()];
    let needs_equity = leaks.iter().any(|l| l.effect == "bluff_less");
    while let Some(hist) = stack.pop() {
        base.apply_history(&hist);
        if base.is_terminal_node() {
            continue;
        }
        if base.is_chance_node() {
            for a in base.available_actions() {
                if let Action::Chance(card) = a {
                    let mut h = hist.clone();
                    h.push(card as usize);
                    stack.push(h);
                }
            }
            continue;
        }
        let player = base.current_player();
        let street = street_name(base.current_board().len());
        let actions = base.available_actions();
        let relevant: Vec<&Leak> = leaks
            .iter()
            .filter(|l| l.player == player && (l.street == "all" || l.street == street))
            .collect();
        let is_leaker = leaks.iter().any(|l| l.player == player);
        if is_leaker && (freeze_rest || !relevant.is_empty()) {
            let n = base.private_cards(player).len();
            let mut strat = base.strategy();
            let equity = if needs_equity {
                base.cache_normalized_weights();
                Some(base.equity(player))
            } else {
                None
            };
            let mut changed = false;
            for l in relevant {
                changed |= apply_leak(&mut strat, &actions, n, l, equity.as_deref());
            }
            if changed || freeze_rest {
                // keep a tiny positive floor so every hand counts as locked
                for v in strat.iter_mut() {
                    if *v < 0.0 { *v = 0.0; }
                }
                for h in 0..n {
                    let s: f32 = (0..actions.len()).map(|a| strat[a * n + h]).sum();
                    if s <= 0.0 { strat[h] = 1.0; }
                }
                sink(&hist, &strat);
                count += 1;
            }
        }
        for i in 0..actions.len() {
            let mut h = hist.clone();
            h.push(i);
            stack.push(h);
        }
    }
    base.back_to_root();
    count
}

/// Rough peak memory need in MB: (normal mode, memory-saver mode).
pub fn memory_estimate(spec: &StudySpec) -> Result<(f64, f64), String> {
    let game = build_game(spec)?;
    let (uncompressed, compressed) = game.memory_usage();
    let one = compressed as f64 / 1_048_576.0;
    if spec.leaks.is_empty() {
        return Ok((one * 1.1, one * 1.1));
    }
    // stored lock targets: f32 per hand/action at the villain's locked nodes
    let any_all = spec.leaks.iter().any(|l| l.street == "all");
    let lock_share = if any_all || spec.freeze_rest { 0.25 } else { 0.15 };
    let locks = uncompressed as f64 / 1_048_576.0 * lock_share;
    Ok(((2.0 * one + locks) * 1.15, (one + locks) * 1.15))
}

fn write_lock(w: &mut impl Write, hist: &[usize], strat: &[f32]) -> std::io::Result<()> {
    w.write_all(&(hist.len() as u16).to_le_bytes())?;
    for &h in hist {
        w.write_all(&[h as u8])?;
    }
    w.write_all(&(strat.len() as u32).to_le_bytes())?;
    for &x in strat {
        let q = (x.clamp(0.0, 1.0) * 65535.0).round() as u16;
        w.write_all(&q.to_le_bytes())?;
    }
    Ok(())
}

fn read_lock(r: &mut impl Read) -> std::io::Result<Option<(Vec<usize>, Vec<f32>)>> {
    let mut b2 = [0u8; 2];
    match r.read_exact(&mut b2) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let n = u16::from_le_bytes(b2) as usize;
    let mut hb = vec![0u8; n];
    r.read_exact(&mut hb)?;
    let mut b4 = [0u8; 4];
    r.read_exact(&mut b4)?;
    let m = u32::from_le_bytes(b4) as usize;
    let mut sb = vec![0u8; 2 * m];
    r.read_exact(&mut sb)?;
    let strat = sb.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]]) as f32 / 65535.0).collect();
    Ok(Some((hb.into_iter().map(|x| x as usize).collect(), strat)))
}

/// Bet sizes chosen for a spot by `pick_sizes`.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct SizePick {
    /// "flop", "turn" or "river": the street the spot starts on, the only one sizes are chosen for
    pub street: String,
    /// the spec fields to overwrite with the chosen sizes, e.g. {"oop_flop_bet": "33%, 100%"}
    pub overrides: std::collections::BTreeMap<String, String>,
    /// per player [out of position, in position]: each offered size (percent of pot) and how often it was used
    pub used: [Vec<(f64, f64)>; 2],
    pub chosen: [Vec<f64>; 2],
    pub secs: f64,
    pub expl_pct: f64,
    pub memory_mb: f64,
}

fn pct_list(v: &[f64]) -> String { v.iter().map(|x| format!("{}%", (x * 10.0).round() / 10.0)).collect::<Vec<_>>().join(", ") }

/// Offers `offered` (percent of pot) as bet sizes on the spot's first street, solves that roughly with
/// one size on later streets, and keeps the `keep` sizes each player uses most the first time they can
/// bet. On the flop each player gets their own sizes; turn and river sizes are shared in a `StudySpec`,
/// so there the two players' use is added together.
pub fn pick_sizes(spec: &StudySpec, offered: &[f64], keep: usize, max_memory_mb: f64, prog: &StudyProgress) -> Result<SizePick, String> {
    if offered.len() <= keep { return Err("Give more sizes to try than sizes to keep.".into()); }
    prog.cancel.store(false, Ordering::Relaxed);
    prog.iter.store(0, Ordering::Relaxed);
    prog.phase.store(1, Ordering::Relaxed);
    let ncards = spec.board.chars().filter(|c| !c.is_whitespace()).count() / 2;
    let street = match ncards { 3 => "flop", 4 => "turn", _ => "river" };
    let one = |s: &str| -> String { let v: Vec<&str> = s.split(',').map(|x| x.trim()).filter(|x| !x.is_empty()).collect(); if v.is_empty() { String::new() } else { v[v.len() / 2].to_string() } };
    let mut scout = spec.clone();
    scout.leaks = vec![]; scout.low_memory = false;
    scout.accuracy_pct = (spec.accuracy_pct * 2.0).max(0.5);
    let menu = pct_list(offered);
    match street {
        "flop" => { scout.oop_flop_bet = menu.clone(); scout.ip_flop_bet = menu; scout.turn_bet = one(&spec.turn_bet); scout.river_bet = one(&spec.river_bet); }
        "turn" => { scout.turn_bet = menu; scout.river_bet = one(&spec.river_bet); }
        _ => { scout.river_bet = menu; }
    }
    let pool = rayon::ThreadPoolBuilder::new().num_threads(spec.threads).build().map_err(|e| e.to_string())?;
    pool.install(|| {
        let mut game = build_game(&scout)?;
        let (_, mem) = game.memory_usage();
        let memory_mb = mem as f64 / 1_048_576.0;
        if max_memory_mb > 0.0 && memory_mb > max_memory_mb {
            return Err(format!("Trying {} bet sizes on this spot needs about {:.1} GB of memory, more than is free. Untick \"Pick the bet sizes for me\", or use a shorter stack.", offered.len(), memory_mb / 1024.0));
        }
        game.allocate_memory(true);
        let t = Instant::now();
        let expl = run_solve(&game, &scout, prog);
        if prog.cancel.load(Ordering::Relaxed) { return Err("Solve cancelled".into()); }
        finalize(&mut game);
        let mut pick = SizePick { street: street.into(), secs: t.elapsed().as_secs_f64(), expl_pct: (expl / spec.pot as f32 * 100.0) as f64, memory_mb, ..Default::default() };
        // the first time each player can bet: first to act at the start, the other after a check
        let root = node_view(&mut game, &[])?;
        let check = root.actions.iter().position(|a| a.starts_with("Check"));
        let views = [Some(root), match check { Some(c) => Some(node_view(&mut game, &[c])?), None => None }];
        for (i, v) in views.iter().enumerate() {
            let Some(v) = v else { continue };
            if v.kind != "player" { continue; }
            for (a, f) in v.actions.iter().zip(&v.action_freqs) {
                let Some(x) = a.strip_prefix("Bet ").and_then(|x| x.trim().parse::<f64>().ok()) else { continue };
                let share = x / v.pot as f64 * 100.0;
                let near = offered.iter().copied().min_by(|p, q| (p - share).abs().total_cmp(&(q - share).abs())).unwrap();
                pick.used[i].push((if (near - share).abs() <= 3.0 { near } else { (share * 10.0).round() / 10.0 }, *f as f64));
            }
        }
        let choose = |used: &[(f64, f64)]| -> Vec<f64> {
            let mut u = used.to_vec();
            u.sort_by(|a, b| b.1.total_cmp(&a.1));
            let total: f64 = u.iter().map(|x| x.1).sum();
            // a size carrying under a twentieth of the betting is not worth its cost
            let mut kept: Vec<f64> = u.iter().enumerate().filter(|(k, x)| *k == 0 || (*k < keep && x.1 >= total * 0.05)).map(|(_, x)| x.0).collect();
            if kept.is_empty() { kept = vec![offered[offered.len() / 2]]; }
            kept.sort_by(|a, b| a.total_cmp(b));
            kept
        };
        if street == "flop" {
            pick.chosen = [choose(&pick.used[0]), choose(&pick.used[1])];
            pick.overrides.insert("oop_flop_bet".into(), pct_list(&pick.chosen[0]));
            pick.overrides.insert("ip_flop_bet".into(), pct_list(&pick.chosen[1]));
        } else {
            let mut both: Vec<(f64, f64)> = vec![];
            for u in pick.used.iter() { for (x, f) in u { match both.iter_mut().find(|b| b.0 == *x) { Some(b) => b.1 += f, None => both.push((*x, *f)) } } }
            let c = choose(&both);
            pick.overrides.insert(format!("{street}_bet"), pct_list(&c));
            pick.chosen = [c.clone(), c];
        }
        Ok(pick)
    })
}

pub fn solve_inner(spec: &StudySpec, prog: &StudyProgress) -> Result<SolveOutcome, String> {
    prog.cancel.store(false, Ordering::Relaxed);
    prog.iter.store(0, Ordering::Relaxed);
    prog.phase.store(1, Ordering::Relaxed);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(spec.threads)
        .build()
        .map_err(|e| e.to_string())?;
    pool.install(|| {
        let mut summary = StudySummary::default();
        let mut base = build_game(spec)?;
        let (_, mem) = base.memory_usage();
        summary.memory_mb = mem as f64 / 1_048_576.0;
        base.allocate_memory(true);
        let t = Instant::now();
        let expl = run_solve(&base, spec, prog);
        if prog.cancel.load(Ordering::Relaxed) {
            return Err("Solve cancelled".into());
        }
        finalize(&mut base);
        summary.base_secs = t.elapsed().as_secs_f64();
        summary.base_expl_pct = (expl / spec.pot as f32 * 100.0) as f64;
        summary.base_ev = root_ev(&mut base);
        if let Some(icm) = pot_icm(spec)? {
            summary.payoffs = icm.table(&base.terminal_amounts());
            let sh = [icm.share_pct(0, summary.base_ev[0]), icm.share_pct(1, summary.base_ev[1])];
            summary.icm = Some(IcmView { base_share: sh, locked_share: sh, split: [icm.split_pct(0), icm.split_pct(1)], give_up: [icm.give_up_pct(0), icm.give_up_pct(1)] });
        }

        if spec.leaks.is_empty() {
            return Ok(SolveOutcome { confidence: 1.0, base: Some(base), locked: None, base_file: None, summary });
        }
        prog.phase.store(2, Ordering::Relaxed);
        let mut base_slot = Some(base);
        let mut base_file = None;
        let mut g;
        if spec.low_memory {
            // 1) stream the lock targets to disk, 2) park the baseline on disk and free it,
            // 3) build the exploit game and read the locks back in
            let dir = work_dir();
            let lock_path = dir.join("locks.bin");
            let base_path = dir.join("base.bin");
            {
                let f = std::fs::File::create(&lock_path).map_err(|e| e.to_string())?;
                let mut w = BufWriter::new(f);
                let mut err = None;
                let b = base_slot.as_mut().unwrap();
                summary.locked_nodes = collect_locks(b, &spec.leaks, spec.freeze_rest, |hist, strat| {
                    if err.is_none() {
                        if let Err(e) = write_lock(&mut w, hist, strat) { err = Some(e.to_string()); }
                    }
                });
                if let Some(e) = err { return Err(format!("Could not write lock file: {e}")); }
                w.flush().map_err(|e| e.to_string())?;
            }
            save_data_to_file(base_slot.as_ref().unwrap(), "", &base_path, None)
                .map_err(|e| format!("Could not park the baseline on disk: {e}"))?;
            base_slot = None; // free the baseline
            base_file = Some(base_path);
            g = build_game(spec)?;
            g.allocate_memory(true);
            let f = std::fs::File::open(&lock_path).map_err(|e| e.to_string())?;
            let mut r = BufReader::new(f);
            while let Some((hist, strat)) = read_lock(&mut r).map_err(|e| e.to_string())? {
                g.apply_history(&hist);
                g.lock_current_strategy_with_weight(&strat, spec.confidence);
            }
            let _ = std::fs::remove_file(&lock_path);
            summary.swap_mode = true;
        } else {
            g = build_game(spec)?;
            g.allocate_memory(true);
            let b = base_slot.as_mut().unwrap();
            summary.locked_nodes = collect_locks(b, &spec.leaks, spec.freeze_rest, |hist, strat| {
                g.apply_history(hist);
                g.lock_current_strategy_with_weight(strat, spec.confidence);
            });
        }
        g.back_to_root();
        prog.phase.store(3, Ordering::Relaxed);
        prog.iter.store(0, Ordering::Relaxed);
        let t = Instant::now();
        let expl = run_solve(&g, spec, prog);
        if prog.cancel.load(Ordering::Relaxed) {
            return Err("Solve cancelled".into());
        }
        finalize(&mut g);
        summary.locked = true;
        summary.locked_secs = t.elapsed().as_secs_f64();
        summary.locked_expl_pct = (expl / spec.pot as f32 * 100.0) as f64;
        summary.locked_ev = root_ev(&mut g);
        if let (Some(icm), Some(v)) = (pot_icm(spec)?, summary.icm.as_mut()) { v.locked_share = [icm.share_pct(0, summary.locked_ev[0]), icm.share_pct(1, summary.locked_ev[1])]; }
        Ok(SolveOutcome { confidence: spec.confidence, base: base_slot, locked: Some(g), base_file, summary })
    })
}

const RANKS: &[u8] = b"23456789TJQKA";

fn grid_index(cards: (u8, u8)) -> usize {
    let (a, b) = cards;
    let (ra, rb) = ((a / 4) as usize, (b / 4) as usize);
    let (hi, lo) = if ra >= rb { (ra, rb) } else { (rb, ra) };
    let suited = a % 4 == b % 4;
    // row/col 0 = Ace
    let (r_hi, r_lo) = (12 - hi, 12 - lo);
    if hi == lo {
        r_hi * 13 + r_hi
    } else if suited {
        r_hi * 13 + r_lo
    } else {
        r_lo * 13 + r_hi
    }
}

fn action_label(a: &Action) -> (String, i32) {
    match a {
        Action::Fold => ("Fold".into(), 0),
        Action::Check => ("Check".into(), 0),
        Action::Call => ("Call".into(), 0),
        Action::Bet(x) => (format!("Bet {x}"), *x),
        Action::Raise(x) => (format!("Raise to {x}"), *x),
        Action::AllIn(x) => (format!("All-in {x}"), *x),
        Action::Chance(c) => (card_to_string(*c).unwrap_or_default(), 0),
        _ => ("?".into(), 0),
    }
}

pub fn node_view(game: &mut PostFlopGame, history: &[usize]) -> Result<NodeView, String> {
    game.back_to_root();
    game.apply_history(history);
    let mut v = NodeView::default();
    v.board = game.current_board().iter().map(|&c| card_to_string(c).unwrap_or_default()).collect();
    let tb = game.total_bet_amount();
    v.bets = tb;
    let base_pot = game.tree_config().starting_pot;
    v.pot = base_pot + 2 * tb[0].min(tb[1]);
    v.stack = game.tree_config().effective_stack;
    if game.is_terminal_node() {
        v.kind = "terminal".into();
        return Ok(v);
    }
    game.cache_normalized_weights();
    for p in 0..2 {
        let w = game.normalized_weights(p).to_vec();
        v.range_combos[p] = w.iter().map(|&x| x as f64).sum();
        let eq = game.equity(p);
        let ev = game.expected_values(p);
        v.equity[p] = weighted_average_f(&eq, &w);
        v.ev[p] = weighted_average_f(&ev, &w);
        v.curves.push(eq.iter().zip(&w).filter(|(_, &x)| x > 0.0).map(|(&e, &x)| [e, x]).collect());
    }
    if game.is_chance_node() {
        v.kind = "chance".into();
        let mask = game.possible_cards();
        for c in 0..52u8 {
            if mask & (1u64 << c) != 0 {
                v.cards.push((card_to_string(c).unwrap_or_default(), c as usize));
            }
        }
        return Ok(v);
    }
    v.kind = "player".into();
    let p = game.current_player();
    v.player = p;
    v.locked = game.current_locking_strategy().is_some();
    let actions = game.available_actions();
    for a in &actions {
        let (l, amt) = action_label(a);
        v.actions.push(l);
        v.action_amounts.push(amt);
    }
    let n = game.private_cards(p).len();
    let cards = game.private_cards(p).to_vec();
    let strat = game.strategy();
    let w = game.normalized_weights(p).to_vec();
    let eq = game.equity(p);
    let ev = game.expected_values(p);
    let na = actions.len();
    let raw = game.weights(p).to_vec();
    let board_now = game.current_board();
    let total: f64 = w.iter().map(|&x| x as f64).sum();
    v.action_freqs = (0..na)
        .map(|a| if total > 0.0 { (0..n).map(|h| strat[a * n + h] as f64 * w[h] as f64).sum::<f64>() / total } else { 0.0 })
        .collect();
    let mut grid: Vec<Cell> = (0..169).map(|_| Cell { freqs: vec![0.0; na], ..Default::default() }).collect();
    let bucket_names = ["Strong (65%+ equity)", "Medium (40-65%)", "Weak (under 40%)"];
    let mut buckets: Vec<Bucket> = bucket_names
        .iter()
        .map(|s| Bucket { name: s.to_string(), share: 0.0, freqs: vec![0.0; na] })
        .collect();
    for h in 0..n {
        let wt = w[h] as f64;
        if wt <= 0.0 { continue; }
        let gi = grid_index(cards[h]);
        {
            let (lo, hi) = if cards[h].0 / 4 >= cards[h].1 / 4 { (cards[h].1, cards[h].0) } else { (cards[h].0, cards[h].1) };
            v.combos.push(Combo {
                hand: format!("{}{}", card_to_string(hi).unwrap_or_default(), card_to_string(lo).unwrap_or_default()),
                cell: gi, weight: raw[h] as f64, freqs: (0..na).map(|a| strat[a * n + h] as f64).collect(), equity: eq[h] as f64, ev: ev[h] as f64,
            });
        }
        let c = &mut grid[gi];
        c.weight += wt;
        c.reach += raw[h] as f64;
        c.equity += eq[h] as f64 * wt;
        c.ev += ev[h] as f64 * wt;
        let b = if eq[h] >= 0.65 { 0 } else if eq[h] >= 0.40 { 1 } else { 2 };
        buckets[b].share += wt;
        for a in 0..na {
            let s = strat[a * n + h] as f64 * wt;
            c.freqs[a] += s;
            buckets[b].freqs[a] += s;
        }
    }
    for a in 0..52u8 {
        for b in (a + 1)..52u8 {
            if !board_now.contains(&a) && !board_now.contains(&b) { grid[grid_index((a, b))].full += 1.0; }
        }
    }
    for c in grid.iter_mut() {
        if c.weight > 0.0 {
            c.equity /= c.weight;
            c.ev /= c.weight;
            for f in c.freqs.iter_mut() { *f /= c.weight; }
        }
    }
    for b in buckets.iter_mut() {
        if b.share > 0.0 {
            for f in b.freqs.iter_mut() { *f /= b.share; }
        }
        b.share = if total > 0.0 { b.share / total } else { 0.0 };
    }
    let _ = RANKS;
    v.grid = grid;
    v.buckets = buckets;
    Ok(v)
}

#[derive(Serialize, Default, Clone)]
pub struct HandView {
    pub player: usize,
    pub acting: bool,
    pub in_range: bool,
    pub blocked: bool,
    pub freqs: Vec<f64>,
    pub equity: f64,
    pub ev: f64,
}

/// Exact numbers for one specific hand (e.g. "AsKd") at a node.
pub fn hand_view(game: &mut PostFlopGame, history: &[usize], player: usize, hand: &str) -> Result<HandView, String> {
    let h = hand.trim();
    if h.len() != 4 {
        return Err("Enter a hand like AsKd".into());
    }
    let c1 = card_from_str(&h[..2]).map_err(|e| e.to_string())?;
    let c2 = card_from_str(&h[2..]).map_err(|e| e.to_string())?;
    if c1 == c2 {
        return Err("Pick two different cards".into());
    }
    let key = if c1 < c2 { (c1, c2) } else { (c2, c1) };
    game.back_to_root();
    game.apply_history(history);
    let mut v = HandView { player, ..Default::default() };
    let board = game.current_board();
    if board.contains(&c1) || board.contains(&c2) {
        v.blocked = true;
        return Ok(v);
    }
    if game.is_terminal_node() {
        return Ok(v);
    }
    let idx = game.private_cards(player).iter().position(|&p| p == key || p == (key.1, key.0));
    let Some(i) = idx else { return Ok(v) };
    game.cache_normalized_weights();
    if game.normalized_weights(player)[i] <= 0.0 {
        return Ok(v);
    }
    v.in_range = true;
    v.equity = game.equity(player)[i] as f64;
    v.ev = game.expected_values(player)[i] as f64;
    if !game.is_chance_node() && game.current_player() == player {
        v.acting = true;
        let n = game.private_cards(player).len();
        let s = game.strategy();
        let na = game.available_actions().len();
        v.freqs = (0..na).map(|a| s[a * n + i] as f64).collect();
    }
    Ok(v)
}

// ---------- reviewing a hand that was played ----------

/// One thing that happened after the flop in the hand being reviewed.
#[derive(Deserialize, Clone, Debug)]
pub struct ReviewStep {
    /// "action" or "card"
    pub kind: String,
    /// who acted: 0 = out of position, 1 = in position
    #[serde(default)]
    pub who: usize,
    /// fold | check | call | bet | raise
    #[serde(default)]
    pub verb: String,
    /// bets and raises: the player's total on this street after the action, in the solve's chips
    #[serde(default)]
    pub to: f64,
    /// cards: "7h"
    #[serde(default)]
    pub card: String,
}

/// One decision of the hand set against the solve.
#[derive(Serialize, Clone, Debug, Default)]
pub struct ReviewRow {
    /// the line that leads to this decision (for jumping there on the result page)
    pub history: Vec<usize>,
    pub street: String,
    pub board: Vec<String>,
    pub pot: f64,
    /// whether this was the reviewed player's decision
    pub hero: bool,
    pub actions: Vec<String>,
    pub amounts: Vec<i32>,
    /// the action in the solve that stands for what was done
    pub chosen: usize,
    /// what was done, as the hand history has it, when the solve has no action of exactly that size
    pub played_as: Option<String>,
    /// hero rows: how often the solve takes each action with this exact hand, and what each is worth
    pub freqs: Vec<f64>,
    pub evs: Vec<f64>,
    pub best: usize,
    /// what the action taken gives up against the best one, in the solve's chips (0 when it is the best or tied)
    pub loss: f64,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct Review {
    pub rows: Vec<ReviewRow>,
    pub total_loss: f64,
    /// set when the hand leaves what the solve covers before its end, with the reason
    pub stopped: Option<String>,
}

/// Follows a played hand through the solve and prices each of `hero`'s decisions for the exact
/// hand held: the value of every action available, how often the solve takes it, and what the
/// action actually taken gave up.
pub fn review(game: &mut PostFlopGame, hero: usize, hand: &str, steps: &[ReviewStep]) -> Result<Review, String> {
    let h = hand.trim();
    if h.len() != 4 { return Err("The review needs the two cards that were held, like AsKd.".into()); }
    let c1 = card_from_str(&h[..2]).map_err(|e| e.to_string())?;
    let c2 = card_from_str(&h[2..]).map_err(|e| e.to_string())?;
    let key = if c1 < c2 { (c1, c2) } else { (c2, c1) };
    let mut out = Review::default();
    let mut history: Vec<usize> = vec![];
    game.back_to_root();
    for st in steps {
        if game.is_terminal_node() { out.stopped = Some("The solve ends the hand before the hand history does (a fold or an all-in call in the solve).".into()); break; }
        if st.kind == "card" {
            let card = card_from_str(st.card.trim()).map_err(|_| format!("'{}' is not a card", st.card))?;
            if !game.is_chance_node() { out.stopped = Some("The hand history deals a card where the solve still has betting to do: the two disagree on the bets before it.".into()); break; }
            if card == c1 || card == c2 || game.possible_cards() & (1u64 << card) == 0 { out.stopped = Some(format!("The {} cannot come in this solve (it is in a hand or on the board).", st.card)); break; }
            history.push(card as usize);
            game.play(card as usize);
            continue;
        }
        if game.is_chance_node() { out.stopped = Some("The hand history has more betting where the solve deals the next card: the two disagree on the bets before it.".into()); break; }
        let player = game.current_player();
        if player != st.who.min(1) { out.stopped = Some("The hand history and the solve disagree about whose turn it is. This happens when a bet in the hand was much smaller or larger than any size in the solve.".into()); break; }
        let acts = game.available_actions();
        let labels: Vec<(String, i32)> = acts.iter().map(action_label).collect();
        // which action in the solve stands for what was done
        let find = |want: &dyn Fn(&Action) -> bool| acts.iter().position(|a| want(a));
        let (chosen, played_as) = match st.verb.as_str() {
            "fold" => (find(&|a| matches!(a, Action::Fold)), None),
            "check" => (find(&|a| matches!(a, Action::Check)), None),
            "call" => (find(&|a| matches!(a, Action::Call)), None),
            _ => {
                let mut best: Option<(usize, f64)> = None;
                for (i, a) in acts.iter().enumerate() {
                    let amt = match a { Action::Bet(x) | Action::Raise(x) | Action::AllIn(x) => *x as f64, _ => continue };
                    let d = (amt - st.to).abs();
                    if best.map_or(true, |b| d < b.1) { best = Some((i, d)); }
                }
                match best {
                    Some((i, d)) => (Some(i), if d > 0.5 + 0.02 * st.to.abs() { Some(format!("{} to {}", if st.verb == "raise" { "raise" } else { "bet" }, st.to)) } else { None }),
                    None => (None, None),
                }
            }
        };
        let Some(chosen) = chosen else { out.stopped = Some(format!("The solve has no way to {} at this point, so the review stops here.", if st.verb == "bet" || st.verb == "raise" { "bet or raise" } else { st.verb.as_str() })); break };
        let mut row = ReviewRow {
            history: history.clone(), street: ["flop", "turn", "river"][game.current_board().len().clamp(3, 5) - 3].to_string(),
            board: game.current_board().iter().map(|&c| card_to_string(c).unwrap_or_default()).collect(),
            pot: (game.tree_config().starting_pot + game.total_bet_amount().iter().sum::<i32>()) as f64,
            hero: player == hero, actions: labels.iter().map(|l| l.0.clone()).collect(), amounts: labels.iter().map(|l| l.1).collect(),
            chosen, played_as, ..Default::default()
        };
        if player == hero {
            let idx = game.private_cards(player).iter().position(|&p| p == key || p == (key.1, key.0));
            game.cache_normalized_weights();
            let Some(i) = idx else {
                out.stopped = Some("This hand is not in the range the solve gives this player, so its decisions cannot be priced. Widen the range in the setup to include it.".into());
                break;
            };
            // The hand itself may have no weight left (the solve would have played it differently
            // earlier); its actions can still be priced as long as some of the player's hands get here.
            let w = game.normalized_weights(player);
            if !w.iter().any(|&x| x > 0.0) {
                out.stopped = Some("The solve never gets to this point: an action before it is one the solve never takes, so there is nothing sound to compare with from here on.".into());
                break;
            }
            if w[i] <= 0.0 {
                out.stopped = Some("From here on the solve no longer holds this hand: at an earlier decision it always plays it another way, and a solve cannot price a hand it never has. To study this line anyway, lock that earlier decision to what was done and solve again.".into());
                break;
            }
            let n = game.private_cards(player).len(); let na = acts.len();
            let s = game.strategy(); let ev = game.expected_values_detail(player);
            row.freqs = (0..na).map(|a| s[a * n + i] as f64).collect();
            row.evs = (0..na).map(|a| ev[a * n + i] as f64).collect();
            row.best = (0..na).max_by(|&a, &b| row.evs[a].partial_cmp(&row.evs[b]).unwrap_or(std::cmp::Ordering::Equal)).unwrap_or(0);
            row.loss = (row.evs[row.best] - row.evs[chosen]).max(0.0);
            out.total_loss += row.loss;
        }
        out.rows.push(row);
        history.push(chosen);
        game.play(chosen);
    }
    Ok(out)
}

/// 13x13 weights (0..1) for a range string, chart layout (row/col 0 = Ace, suited above the diagonal).
pub fn range_grid(range: &str) -> Result<Vec<f32>, String> {
    let r: Range = range.parse().map_err(|e: String| e)?;
    let mut w = vec![0.0; 169];
    for row in 0..13usize {
        for col in 0..13usize {
            let (r1, r2) = (12 - row as u8, 12 - col as u8);
            w[row * 13 + col] = match row.cmp(&col) {
                std::cmp::Ordering::Equal => r.get_weight_pair(r1),
                std::cmp::Ordering::Less => r.get_weight_suited(r1, r2),
                std::cmp::Ordering::Greater => r.get_weight_offsuit(r1, r2),
            };
        }
    }
    Ok(w)
}

impl StudyState {
    pub fn hand(&mut self, locked: bool, history: &[usize], player: usize, hand: &str) -> Result<HandView, String> {
        hand_view(self.game(locked)?, history, player, hand)
    }
    pub fn review(&mut self, locked: bool, hero: usize, hand: &str, steps: &[ReviewStep]) -> Result<Review, String> {
        review(self.game(locked)?, hero, hand, steps)
    }
}


// ---------- hand-made locks on top of a finished solve ----------

/// "This player does this here": a fixed action mix for the whole range at one decision, or for
/// some chart squares only (the other hands stay free to adjust).
#[derive(Deserialize, Clone, Debug)]
pub struct ManualLock {
    pub history: Vec<usize>,
    #[serde(default)]
    pub cells: Vec<usize>,
    pub freqs: Vec<f32>,
}

/// Solves the spot again with the given locks (and the spec's habits, if any) in place, reusing the
/// normal solve already in hand. The normal solve is handed back whatever happens.
pub fn relock(mut base: PostFlopGame, spec: &StudySpec, locks: &[ManualLock], prog: &StudyProgress, mut summary: StudySummary) -> (PostFlopGame, Result<(PostFlopGame, StudySummary, f32), String>) {
    let weight = if spec.leaks.is_empty() { 1.0 } else { spec.confidence };
    let pool = match rayon::ThreadPoolBuilder::new().num_threads(spec.threads).build() { Ok(p) => p, Err(e) => return (base, Err(e.to_string())) };
    let out = pool.install(|| -> Result<(PostFlopGame, StudySummary, f32), String> {
        prog.cancel.store(false, Ordering::Relaxed);
        prog.iter.store(0, Ordering::Relaxed);
        prog.phase.store(2, Ordering::Relaxed);
        let mut g = build_game(spec)?;
        g.allocate_memory(true);
        let mut n = 0;
        if !spec.leaks.is_empty() {
            n += collect_locks(&mut base, &spec.leaks, spec.freeze_rest, |hist, strat| {
                g.apply_history(hist);
                g.lock_current_strategy_with_weight(strat, weight);
            });
        }
        for l in locks {
            base.apply_history(&l.history);
            if base.is_terminal_node() || base.is_chance_node() { return Err("A lock points at a place where nobody acts.".into()); }
            let player = base.current_player();
            let hands = base.private_cards(player).to_vec();
            let na = base.available_actions().len();
            if l.freqs.len() != na { return Err("A lock has the wrong number of actions for its decision.".into()); }
            let total: f32 = l.freqs.iter().sum();
            if l.freqs.iter().any(|f| !(0.0..=1.0001).contains(f)) || (total - 1.0).abs() > 0.02 { return Err("The percentages in a lock must add up to 100.".into()); }
            let mut strat = vec![0.0f32; na * hands.len()];
            for (h, &cards) in hands.iter().enumerate() {
                if l.cells.is_empty() || l.cells.contains(&grid_index(cards)) {
                    for a in 0..na { strat[a * hands.len() + h] = l.freqs[a] / total; }
                }
            }
            g.apply_history(&l.history);
            // a later lock at the same decision adds to an earlier one, hand by hand
            if let Some(prev) = g.current_locking_strategy() {
                for h in 0..hands.len() {
                    if (0..na).all(|a| strat[a * hands.len() + h] == 0.0) { for a in 0..na { strat[a * hands.len() + h] = prev[a * hands.len() + h]; } }
                }
            }
            g.lock_current_strategy_with_weight(&strat, weight);
            n += 1;
        }
        base.back_to_root();
        g.back_to_root();
        prog.phase.store(3, Ordering::Relaxed);
        let t = Instant::now();
        let expl = run_solve(&g, spec, prog);
        if prog.cancel.load(Ordering::Relaxed) { return Err("Solve cancelled".into()); }
        finalize(&mut g);
        summary.locked = true;
        summary.locked_secs = t.elapsed().as_secs_f64();
        summary.locked_expl_pct = (expl / spec.pot as f32 * 100.0) as f64;
        summary.locked_ev = root_ev(&mut g);
        if let (Some(icm), Some(v)) = (pot_icm(spec)?, summary.icm.as_mut()) { v.locked_share = [icm.share_pct(0, summary.locked_ev[0]), icm.share_pct(1, summary.locked_ev[1])]; }
        summary.locked_nodes = n;
        summary.swap_mode = false;
        Ok((g, summary, weight))
    });
    base.back_to_root();
    (base, out)
}

impl StudyState {
    /// Takes the normal solve out for a re-solve with locks; `finish_relock` puts things back.
    pub fn begin_relock(&mut self) -> Result<(PostFlopGame, StudySummary), String> {
        let summary = self.summary.clone().ok_or("No solved game yet")?;
        self.game(false)?;
        self.locked = None;
        self.locked_file = None;
        self.cache.clear();
        Ok((self.base.take().unwrap(), summary))
    }
    pub fn finish_relock(&mut self, base: PostFlopGame, out: Result<(PostFlopGame, StudySummary, f32), String>) -> StudySummary {
        self.base = Some(base);
        self.swap_mode = false;
        match out {
            Ok((g, summary, weight)) => { self.locked = Some(g); self.confidence = weight; self.summary = Some(summary.clone()); summary }
            Err(e) => {
                let mut s = self.summary.clone().unwrap_or_default();
                s.locked = false; s.locked_nodes = 0;
                self.summary = Some(s.clone());
                s.error = Some(e);
                s
            }
        }
    }
}

// ---------- saved solves ----------

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LibraryEntry {
    pub id: String,
    pub name: String,
    pub saved_at: u64,
    /// "two" or "three" players. Three-player entries made before 0.13.4 keep the spot only
    /// (`meta.solved` is absent or false) and are solved again when opened.
    pub kind: String,
    pub bytes: u64,
    #[serde(default)]
    pub summary: Option<StudySummary>,
    #[serde(default)]
    pub confidence: f32,
    /// whatever the screen needs to show the spot again
    pub meta: serde_json::Value,
}

pub fn library_dir() -> PathBuf {
    let root = std::env::var_os("PONTIAC_LIBRARY").map(PathBuf::from)
        .or_else(|| std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("PontiacSolver").join("library")))
        .or_else(|| std::env::var_os("HOME").map(|d| PathBuf::from(d).join(".local/share/pontiac-solver/library")))
        .unwrap_or_else(|| std::env::temp_dir().join("pontiac-solver-library"));
    let _ = std::fs::create_dir_all(&root);
    root
}

fn safe_id(id: &str) -> Result<PathBuf, String> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') { return Err("Unknown saved solve".into()); }
    Ok(library_dir().join(id))
}

pub fn library_list() -> Vec<LibraryEntry> {
    let mut out: Vec<LibraryEntry> = std::fs::read_dir(library_dir()).into_iter().flatten().flatten()
        .filter_map(|e| std::fs::read_to_string(e.path().join("entry.json")).ok())
        .filter_map(|t| serde_json::from_str(&t).ok()).collect();
    out.sort_by(|a, b| b.saved_at.cmp(&a.saved_at));
    out
}

pub fn library_delete(id: &str) -> Result<(), String> {
    std::fs::remove_dir_all(safe_id(id)?).map_err(|e| format!("Could not delete it: {e}"))
}

fn new_entry(name: &str, kind: &str, meta: serde_json::Value) -> Result<(LibraryEntry, PathBuf), String> {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let id = format!("s{now}");
    let dir = safe_id(&id)?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Could not create the library folder: {e}"))?;
    Ok((LibraryEntry { id, name: name.trim().chars().take(120).collect(), saved_at: (now / 1000) as u64, kind: kind.into(), bytes: 0, summary: None, confidence: 1.0, meta }, dir))
}

fn write_entry(e: &LibraryEntry, dir: &std::path::Path) -> Result<(), String> {
    std::fs::write(dir.join("entry.json"), serde_json::to_string(e).map_err(|x| x.to_string())?).map_err(|x| format!("Could not write to the library: {x}"))
}

/// Keeps a three-player spot. When a finished solve is given it is stored whole and reopens
/// without solving (`meta.solved` says so); otherwise only the spot is kept.
pub fn library_save_three(st: Option<&multiway::study::State3>, name: &str, mut meta: serde_json::Value) -> Result<LibraryEntry, String> {
    let solved = st.map_or(false, |s| s.base.is_some() && s.summary.is_some());
    if let Some(m) = meta.as_object_mut() { m.insert("solved".into(), serde_json::Value::Bool(solved)); }
    let (mut e, dir) = new_entry(name, "three", meta)?;
    let result = (|| -> Result<(), String> {
        if let (true, Some(st)) = (solved, st) {
            let target = dir.join("solve.p3");
            multiway::save::save_file(st, "", &target.to_string_lossy()).map_err(|x| format!("Could not save the solve: {x}"))?;
            e.bytes = std::fs::metadata(&target).map(|m| m.len()).unwrap_or(0);
        }
        write_entry(&e, &dir)
    })();
    if let Err(x) = result { let _ = std::fs::remove_dir_all(&dir); return Err(x); }
    Ok(e)
}

/// Reads a stored three-player solve back.
pub fn library_open_three(id: &str) -> Result<multiway::study::State3, String> {
    let file = safe_id(id)?.join("solve.p3");
    if !file.exists() { return Err("That entry keeps the spot only; it has to be solved again.".into()); }
    Ok(multiway::save::load_file(&file.to_string_lossy()).map_err(|x| format!("That saved solve could not be opened: {x}"))?.0)
}

impl StudyState {
    /// Writes the solve on screen (and its locked version, if any) to the library.
    pub fn library_save(&mut self, name: &str, meta: serde_json::Value) -> Result<LibraryEntry, String> {
        let summary = self.summary.clone().ok_or("No solved game to save")?;
        let (mut e, dir) = new_entry(name, "two", meta)?;
        let result = (|| -> Result<(), String> {
            for locked in [false, true] {
                if locked && !summary.locked { continue; }
                let target = dir.join(if locked { "locked.bin" } else { "base.bin" });
                let (mem, file) = if locked { (self.locked.as_ref(), self.locked_file.as_ref()) } else { (self.base.as_ref(), self.base_file.as_ref()) };
                match (mem, file) {
                    (Some(g), _) => save_data_to_file(g, "", &target, SAVE_LEVEL).map_err(|x| format!("Could not save the solve: {x}"))?,
                    (None, Some(f)) => { std::fs::copy(f, &target).map_err(|x| format!("Could not save the solve: {x}"))?; }
                    (None, None) => return Err("No solved game to save".into()),
                }
                e.bytes += std::fs::metadata(&target).map(|m| m.len()).unwrap_or(0);
            }
            e.summary = Some(summary.clone());
            e.confidence = self.confidence;
            write_entry(&e, &dir)
        })();
        if let Err(x) = result { let _ = std::fs::remove_dir_all(&dir); return Err(x); }
        Ok(e)
    }

    /// Points this screen at a saved solve; the games are read from disk when first looked at.
    pub fn library_open(&mut self, id: &str) -> Result<LibraryEntry, String> {
        let dir = safe_id(id)?;
        let text = std::fs::read_to_string(dir.join("entry.json")).map_err(|_| "That saved solve is missing.".to_string())?;
        let e: LibraryEntry = serde_json::from_str(&text).map_err(|x| x.to_string())?;
        let mut summary = e.summary.clone().ok_or("That entry has no solve stored.")?;
        if !dir.join("base.bin").exists() { return Err("That saved solve is missing its file.".into()); }
        self.reset();
        summary.swap_mode = false;
        self.base_file = Some(dir.join("base.bin"));
        self.locked_file = if summary.locked { Some(dir.join("locked.bin")) } else { None };
        self.confidence = e.confidence;
        self.summary = Some(summary);
        self.game(false)?; // fail now, not later, if the file cannot be read
        Ok(e)
    }
}


/// Solves one spot and returns, per hand class (AA, AKs ... 22 chart order), the chips each
/// player wins from the pot on average at the start: [out of position, in position], plus the
/// accuracy reached as a percent of the pot. Classes with no hands possible come back as NaN.
/// The third item is how likely each class is on this board, per player (shares adding to 1):
/// hands that share a card with the board are rarer, and they are the ones that hit it, so averages
/// across boards must be weighted by this or strong hands get counted too often.
pub fn class_values(spec: &StudySpec, prog: &StudyProgress) -> Result<([Vec<f64>; 2], f64, [Vec<f64>; 2]), String> {
    let out = solve_inner(spec, prog)?;
    let mut game = out.base.ok_or("no solve came back")?;
    game.back_to_root();
    game.cache_normalized_weights();
    let mut vals = [vec![f64::NAN; 169], vec![f64::NAN; 169]];
    let mut mass = [vec![0f64; 169], vec![0f64; 169]];
    for p in 0..2 {
        let ev = game.expected_values(p);
        let w = game.normalized_weights(p).to_vec();
        let cards = game.private_cards(p).to_vec();
        let (mut num, mut den) = (vec![0f64; 169], vec![0f64; 169]);
        for h in 0..cards.len() { let c = grid_index(cards[h]); num[c] += w[h] as f64 * ev[h] as f64; den[c] += w[h] as f64; }
        let total: f64 = den.iter().sum();
        for c in 0..169 { if den[c] > 0.0 { vals[p][c] = num[c] / den[c]; mass[p][c] = den[c] / total; } }
        if std::env::var("PONTIAC_FIT_DEBUG").is_ok() {
            let tw: f64 = w.iter().map(|&x| x as f64).sum(); let te: f64 = (0..cards.len()).map(|h| w[h] as f64 * ev[h] as f64).sum();
            eprintln!("ROOT player {p}: mean ev {:.3} of pot {} ({} hands, total weight {:.3})", te / tw, spec.pot, cards.len(), tw);
        }
    }
    Ok((vals, out.summary.base_expl_pct, mass))
}

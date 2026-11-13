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
}

fn default_one() -> f32 {
    1.0
}

fn default_true() -> bool {
    true
}

#[derive(Serialize, Clone, Debug, Default)]
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
}

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
    PostFlopGame::with_config(card_config, tree)
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
}

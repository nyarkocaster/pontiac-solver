//! Pontiac Solver library files.
//!
//! A whole flop solve is far too big to share (about 1 GB for a heads-up 20 bb flop). A library
//! file keeps only what cannot be worked out again quickly: the flop strategy of both players and
//! the value of every hand at every flop decision. That is a few hundred kilobytes. The turn and
//! river are solved again on the reader's machine when a turn card is chosen, starting from the
//! ranges the flop strategy leaves each player with; that takes seconds.
//!
//! [`pack`] makes a [`LibFile`] from a solved flop game. [`LibGame`] opens one and behaves like a
//! solved game: the flop is read from the file, and playing a turn card solves that turn.
//! Both it and the engine's own `PostFlopGame` implement [`Solved`], so reports can be written once.
//!
//! AGPL-3.0-or-later.

use postflop_solver::*;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::time::Instant;

pub const MAGIC: &[u8; 8] = b"PNTCLIB\x01";
/// Bumped when a reader of the previous number could no longer read the file.
pub const FORMAT: u64 = 1;

// ------------------------------------------------------------------ what a solved game can be asked

/// Reading a solved game. Implemented by the engine's `PostFlopGame` (a whole solve in memory) and
/// by [`LibGame`] (a library file plus turns solved on demand). Histories are the engine's: an
/// action's position in `available_actions`, or a card number where a card is dealt.
pub trait Solved {
    fn back_to_root(&mut self);
    fn history(&self) -> Vec<usize>;
    fn play(&mut self, action: usize);
    fn apply_history(&mut self, history: &[usize]) { self.back_to_root(); for &a in history { self.play(a); } }
    fn is_terminal_node(&self) -> bool;
    fn is_chance_node(&self) -> bool;
    fn current_player(&self) -> usize;
    fn available_actions(&self) -> Vec<Action>;
    fn possible_cards(&self) -> u64;
    fn current_board(&self) -> Vec<u8>;
    /// chips each player has put in since the start of the spot
    fn total_bet_amount(&self) -> [i32; 2];
    fn starting_pot(&self) -> i32;
    fn effective_stack(&self) -> i32;
    /// the hands the per-hand lists below refer to at the current node
    fn private_cards(&self, player: usize) -> Vec<(u8, u8)>;
    fn cache_normalized_weights(&mut self);
    fn weights(&self, player: usize) -> Vec<f32>;
    fn normalized_weights(&self, player: usize) -> Vec<f32>;
    fn equity(&self, player: usize) -> Vec<f32>;
    fn expected_values(&self, player: usize) -> Vec<f32>;
    fn expected_values_detail(&self, player: usize) -> Vec<f32>;
    fn strategy(&self) -> Vec<f32>;
    fn is_locked(&self) -> bool;
    /// Called before a card is dealt with `play`. A game that has to do work for a card (a library
    /// file solving a turn) does it here, so a failure comes back as an error.
    fn prepare_card(&mut self, _card: usize) -> Result<(), String> { Ok(()) }
}

impl Solved for PostFlopGame {
    fn back_to_root(&mut self) { PostFlopGame::back_to_root(self) }
    fn history(&self) -> Vec<usize> { PostFlopGame::history(self).to_vec() }
    fn play(&mut self, action: usize) { PostFlopGame::play(self, action) }
    fn apply_history(&mut self, history: &[usize]) { PostFlopGame::apply_history(self, history) }
    fn is_terminal_node(&self) -> bool { PostFlopGame::is_terminal_node(self) }
    fn is_chance_node(&self) -> bool { PostFlopGame::is_chance_node(self) }
    fn current_player(&self) -> usize { PostFlopGame::current_player(self) }
    fn available_actions(&self) -> Vec<Action> { PostFlopGame::available_actions(self) }
    fn possible_cards(&self) -> u64 { PostFlopGame::possible_cards(self) }
    fn current_board(&self) -> Vec<u8> { PostFlopGame::current_board(self) }
    fn total_bet_amount(&self) -> [i32; 2] { PostFlopGame::total_bet_amount(self) }
    fn starting_pot(&self) -> i32 { self.tree_config().starting_pot }
    fn effective_stack(&self) -> i32 { self.tree_config().effective_stack }
    fn private_cards(&self, player: usize) -> Vec<(u8, u8)> { PostFlopGame::private_cards(self, player).to_vec() }
    fn cache_normalized_weights(&mut self) { PostFlopGame::cache_normalized_weights(self) }
    fn weights(&self, player: usize) -> Vec<f32> { PostFlopGame::weights(self, player).to_vec() }
    fn normalized_weights(&self, player: usize) -> Vec<f32> { PostFlopGame::normalized_weights(self, player).to_vec() }
    fn equity(&self, player: usize) -> Vec<f32> { PostFlopGame::equity(self, player) }
    fn expected_values(&self, player: usize) -> Vec<f32> { PostFlopGame::expected_values(self, player) }
    fn expected_values_detail(&self, player: usize) -> Vec<f32> { PostFlopGame::expected_values_detail(self, player) }
    fn strategy(&self) -> Vec<f32> { PostFlopGame::strategy(self) }
    fn is_locked(&self) -> bool { self.current_locking_strategy().is_some() }
}

// ------------------------------------------------------------------ the file in memory

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind { Decision, Deal, End }

/// What the whole solve said about one turn card, kept so a re-solve can be checked against it and
/// so every card can be compared without solving any of them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CardInfo {
    pub card: u8,
    /// each player's value over their whole range on this card [out of position, in position]
    pub ev: [f32; 2],
    /// each player's chance to win on this card
    pub equity: [f32; 2],
    /// how often the first player to act bets on this card
    pub bet: f32,
}

/// One place on the flop: a decision, the point where the turn card is dealt, or the end of the hand.
#[derive(Clone, Debug)]
pub struct LibNode {
    pub kind: Kind,
    /// who decides here (decisions only)
    pub player: usize,
    pub bets: [i32; 2],
    /// where this node hangs: (parent, which of the parent's actions)
    pub parent: Option<(usize, usize)>,
    pub actions: Vec<Action>,
    pub children: Vec<usize>,
    /// decisions: how often each hand takes each action, action * hands + hand
    pub strategy: Vec<f32>,
    /// decisions: what each action is worth to each of the deciding player's hands, same layout
    pub detail: Vec<f32>,
    /// the value of each hand of the player whose value cannot be read off the parent: at the
    /// first decision the player who is not deciding; where a card is dealt, the player who did not act last
    pub ev_other: Vec<f32>,
    /// which player `ev_other` is about
    pub other: usize,
    /// deal: one entry per card that can come
    pub cards: Vec<CardInfo>,
}

#[derive(Clone, Debug)]
pub struct LibFile {
    /// Readable without unpacking the rest: what the spot is and how it was solved.
    pub header: Value,
    pub tree: TreeConfig,
    pub flop: [u8; 3],
    pub hands: [Vec<(u8, u8)>; 2],
    /// the ranges the spot started with, one weight per hand
    pub initial: [Vec<f32>; 2],
    pub nodes: Vec<LibNode>,
}

// ------------------------------------------------------------------ bet tree as plain JSON

fn size_json(b: &BetSize) -> Value {
    match *b {
        BetSize::PotRelative(x) => json!({"pot": x}),
        BetSize::PrevBetRelative(x) => json!({"prev": x}),
        BetSize::Additive(a, cap) => json!({"add": [a, cap]}),
        BetSize::Geometric(n, max) => json!({"geo": [n, if max.is_finite() { json!(max) } else { Value::Null }]}),
        BetSize::AllIn => json!("allin"),
    }
}

fn size_from(v: &Value) -> Result<BetSize, String> {
    let bad = || "the file's bet sizes could not be read".to_string();
    if v.as_str() == Some("allin") { return Ok(BetSize::AllIn); }
    if let Some(x) = v.get("pot").and_then(|x| x.as_f64()) { return Ok(BetSize::PotRelative(x)); }
    if let Some(x) = v.get("prev").and_then(|x| x.as_f64()) { return Ok(BetSize::PrevBetRelative(x)); }
    if let Some(a) = v.get("add").and_then(|x| x.as_array()) { return Ok(BetSize::Additive(a.first().and_then(|x| x.as_i64()).ok_or_else(bad)? as i32, a.get(1).and_then(|x| x.as_i64()).ok_or_else(bad)? as i32)); }
    if let Some(a) = v.get("geo").and_then(|x| x.as_array()) { return Ok(BetSize::Geometric(a.first().and_then(|x| x.as_i64()).ok_or_else(bad)? as i32, a.get(1).and_then(|x| x.as_f64()).unwrap_or(f64::INFINITY))); }
    Err(bad())
}

fn sizes_json(v: &[BetSize]) -> Value { Value::Array(v.iter().map(size_json).collect()) }
fn sizes_from(v: Option<&Value>) -> Result<Vec<BetSize>, String> { v.and_then(|x| x.as_array()).ok_or("the file's bet sizes could not be read")?.iter().map(size_from).collect() }

fn tree_json(t: &TreeConfig) -> Value {
    let street = |s: &[BetSizeOptions; 2]| json!([{"bet": sizes_json(&s[0].bet), "raise": sizes_json(&s[0].raise)}, {"bet": sizes_json(&s[1].bet), "raise": sizes_json(&s[1].raise)}]);
    json!({
        "starting_pot": t.starting_pot, "effective_stack": t.effective_stack, "rake_rate": t.rake_rate, "rake_cap": t.rake_cap,
        "flop": street(&t.flop_bet_sizes), "turn": street(&t.turn_bet_sizes), "river": street(&t.river_bet_sizes),
        "river_donk": t.river_donk_sizes.as_ref().map(|d| sizes_json(&d.donk)),
        "add_allin_threshold": t.add_allin_threshold, "force_allin_threshold": t.force_allin_threshold, "merging_threshold": t.merging_threshold,
    })
}

fn tree_from(v: &Value) -> Result<TreeConfig, String> {
    let bad = "the file's bet tree could not be read";
    let street = |k: &str| -> Result<[BetSizeOptions; 2], String> {
        let a = v.get(k).and_then(|x| x.as_array()).filter(|a| a.len() == 2).ok_or(bad)?;
        let one = |x: &Value| -> Result<BetSizeOptions, String> { Ok(BetSizeOptions { bet: sizes_from(x.get("bet"))?, raise: sizes_from(x.get("raise"))? }) };
        Ok([one(&a[0])?, one(&a[1])?])
    };
    let num = |k: &str| v.get(k).and_then(|x| x.as_f64()).ok_or(bad);
    Ok(TreeConfig {
        initial_state: BoardState::Flop,
        starting_pot: num("starting_pot")? as i32, effective_stack: num("effective_stack")? as i32,
        rake_rate: num("rake_rate")?, rake_cap: num("rake_cap")?,
        flop_bet_sizes: street("flop")?, turn_bet_sizes: street("turn")?, river_bet_sizes: street("river")?,
        turn_donk_sizes: None,
        river_donk_sizes: match v.get("river_donk") { None | Some(Value::Null) => None, Some(x) => Some(DonkSizeOptions { donk: sizes_from(Some(x))? }) },
        add_allin_threshold: num("add_allin_threshold")?, force_allin_threshold: num("force_allin_threshold")?, merging_threshold: num("merging_threshold")?,
    })
}

// ------------------------------------------------------------------ making a file from a solve

fn range_average(v: &[f32], w: &[f32]) -> f32 {
    let (mut s, mut t) = (0f64, 0f64);
    for (&a, &b) in v.iter().zip(w) { s += a as f64 * b as f64; t += b as f64; }
    if t > 0.0 { (s / t) as f32 } else { 0.0 }
}

/// Reads the flop out of a solved game. `header` is kept as given, with the bet tree and a few
/// basics added. The game must be a finished, ordinary solve that starts on the flop.
pub fn pack(game: &mut PostFlopGame, mut header: Value) -> Result<LibFile, String> {
    let tree = game.tree_config().clone();
    let cards = game.card_config().clone();
    if tree.initial_state != BoardState::Flop || cards.turn != NOT_DEALT { return Err("A library file holds a flop spot: this solve starts on the turn or river, which is small enough to keep whole.".into()); }
    if tree.turn_donk_sizes.is_some() { return Err("A library file cannot hold a spot with separate turn lead (donk) sizes yet: the turn is solved again on its own, where those sizes would not apply.".into()); }
    if game.has_custom_payoffs() { return Err("A library file cannot hold a prize-money (ICM) solve yet.".into()); }
    game.back_to_root();
    let hands = [PostFlopGame::private_cards(game, 0).to_vec(), PostFlopGame::private_cards(game, 1).to_vec()];
    let initial = [PostFlopGame::weights(game, 0).to_vec(), PostFlopGame::weights(game, 1).to_vec()];
    let mut nodes: Vec<LibNode> = vec![];
    let mut hist: Vec<usize> = vec![];
    pack_node(game, &mut hist, None, &mut nodes)?;
    game.back_to_root();
    if !header.is_object() { header = json!({}); }
    let h = header.as_object_mut().unwrap();
    h.insert("format".into(), json!(FORMAT));
    h.insert("kind".into(), json!("pontiac library file: heads-up flop solve, flop strategy and values only"));
    h.insert("board".into(), json!(cards.flop.iter().map(|&c| card_to_string(c).unwrap_or_default()).collect::<String>()));
    h.insert("tree".into(), tree_json(&tree));
    h.insert("flop_nodes".into(), json!(nodes.len()));
    h.insert("hands".into(), json!([hands[0].len(), hands[1].len()]));
    Ok(LibFile { header, tree, flop: cards.flop, hands, initial, nodes })
}

fn pack_node(game: &mut PostFlopGame, hist: &mut Vec<usize>, parent: Option<(usize, usize)>, nodes: &mut Vec<LibNode>) -> Result<usize, String> {
    PostFlopGame::apply_history(game, hist);
    let id = nodes.len();
    let bets = PostFlopGame::total_bet_amount(game);
    let last_actor = parent.map(|(p, _)| nodes[p].player);
    let mut node = LibNode { kind: Kind::End, player: 0, bets, parent, actions: vec![], children: vec![], strategy: vec![], detail: vec![], ev_other: vec![], other: 0, cards: vec![] };
    if PostFlopGame::is_terminal_node(game) { nodes.push(node); return Ok(id); }
    PostFlopGame::cache_normalized_weights(game);
    if PostFlopGame::is_chance_node(game) {
        node.kind = Kind::Deal;
        node.other = 1 - last_actor.ok_or("the solve deals a card before anyone acts")?;
        node.ev_other = PostFlopGame::expected_values(game, node.other);
        let mask = PostFlopGame::possible_cards(game);
        for card in 0..52u8 {
            if mask & (1u64 << card) == 0 { continue; }
            PostFlopGame::apply_history(game, hist);
            PostFlopGame::play(game, card as usize);
            if PostFlopGame::is_terminal_node(game) || PostFlopGame::is_chance_node(game) { return Err("the solve has a turn with nobody to act".into()); }
            PostFlopGame::cache_normalized_weights(game);
            let mut info = CardInfo { card, ..Default::default() };
            for p in 0..2 {
                let w = PostFlopGame::normalized_weights(game, p).to_vec();
                info.ev[p] = range_average(&PostFlopGame::expected_values(game, p), &w);
                info.equity[p] = range_average(&PostFlopGame::equity(game, p), &w);
            }
            let me = PostFlopGame::current_player(game);
            let acts = PostFlopGame::available_actions(game);
            let (st, w) = (PostFlopGame::strategy(game), PostFlopGame::normalized_weights(game, me).to_vec());
            let n = w.len();
            let (mut b, mut t) = (0f64, 0f64);
            for h in 0..n { t += w[h] as f64; for (a, act) in acts.iter().enumerate() { if matches!(act, Action::Bet(_) | Action::Raise(_) | Action::AllIn(_)) { b += w[h] as f64 * st[a * n + h] as f64; } } }
            info.bet = if t > 0.0 { (b / t) as f32 } else { 0.0 };
            node.cards.push(info);
        }
        nodes.push(node);
        return Ok(id);
    }
    node.kind = Kind::Decision;
    node.player = PostFlopGame::current_player(game);
    node.actions = PostFlopGame::available_actions(game);
    node.strategy = PostFlopGame::strategy(game);
    node.detail = PostFlopGame::expected_values_detail(game, node.player);
    if parent.is_none() { node.other = 1 - node.player; node.ev_other = PostFlopGame::expected_values(game, node.other); }
    let na = node.actions.len();
    nodes.push(node);
    for a in 0..na {
        hist.push(a);
        let child = pack_node(game, hist, Some((id, a)), nodes)?;
        hist.pop();
        nodes[id].children.push(child);
    }
    Ok(id)
}

// ------------------------------------------------------------------ bytes

fn put_u16s(out: &mut Vec<u8>, v: &[u16]) { out.extend(v.iter().map(|x| (x >> 8) as u8)); out.extend(v.iter().map(|x| *x as u8)); }

/// Values for the hands marked in `mask`, as 16-bit steps between the smallest and largest.
fn put_values(out: &mut Vec<u8>, v: &[f32], mask: &[bool]) {
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for (x, m) in v.iter().zip(mask) { if *m && x.is_finite() { lo = lo.min(*x); hi = hi.max(*x); } }
    if lo > hi { lo = 0.0; hi = 0.0; }
    out.extend(lo.to_le_bytes()); out.extend(hi.to_le_bytes());
    let span = hi - lo;
    let q: Vec<u16> = v.iter().zip(mask).filter(|(_, m)| **m).map(|(x, _)| if span > 0.0 && x.is_finite() { (((x - lo) / span) as f64 * 65535.0).round().clamp(0.0, 65535.0) as u16 } else { 0 }).collect();
    put_u16s(out, &q);
}

struct Reader<'a> { b: &'a [u8], at: usize }
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        if self.at + n > self.b.len() { return Err("the library file is cut short or damaged".into()); }
        let s = &self.b[self.at..self.at + n]; self.at += n; Ok(s)
    }
    fn u8(&mut self) -> Result<u8, String> { Ok(self.take(1)?[0]) }
    fn u16(&mut self) -> Result<u16, String> { let s = self.take(2)?; Ok(u16::from_le_bytes([s[0], s[1]])) }
    fn i32(&mut self) -> Result<i32, String> { let s = self.take(4)?; Ok(i32::from_le_bytes([s[0], s[1], s[2], s[3]])) }
    fn f32(&mut self) -> Result<f32, String> { let s = self.take(4)?; Ok(f32::from_le_bytes([s[0], s[1], s[2], s[3]])) }
    fn u16s(&mut self, n: usize) -> Result<Vec<u16>, String> { let s = self.take(2 * n)?; Ok((0..n).map(|i| ((s[i] as u16) << 8) | s[n + i] as u16).collect()) }
    fn values(&mut self, mask: &[bool]) -> Result<Vec<f32>, String> {
        let (lo, hi) = (self.f32()?, self.f32()?);
        let k = mask.iter().filter(|m| **m).count();
        let q = self.u16s(k)?;
        let mut it = q.into_iter();
        Ok(mask.iter().map(|m| if *m { lo + (hi - lo) * (it.next().unwrap() as f32 / 65535.0) } else { 0.0 }).collect())
    }
}

fn action_code(a: &Action) -> (u8, i32) {
    match *a { Action::Fold => (1, 0), Action::Check => (2, 0), Action::Call => (3, 0), Action::Bet(x) => (4, x), Action::Raise(x) => (5, x), Action::AllIn(x) => (6, x), _ => (0, 0) }
}
fn action_from(code: u8, x: i32) -> Result<Action, String> {
    Ok(match code { 1 => Action::Fold, 2 => Action::Check, 3 => Action::Call, 4 => Action::Bet(x), 5 => Action::Raise(x), 6 => Action::AllIn(x), _ => return Err("the library file is damaged".into()) })
}

/// Action frequencies as whole steps, each action measured against its own largest value over the
/// hands still in range. A rarely taken action keeps its shape that way: which hands take it most
/// is kept as finely as for a common action, so the range that reaches a rare line stays usable.
fn quantize(strategy: &[f32], mask: &[bool], na: usize, n: usize, q: u32) -> (Vec<f32>, Vec<u32>) {
    let mut scales = vec![0f32; na]; let mut steps = vec![0u32; na * n];
    for a in 0..na {
        let top = (0..n).filter(|&h| mask[h]).map(|h| strategy[a * n + h]).filter(|x| x.is_finite()).fold(0f32, f32::max);
        scales[a] = top;
        if top > 0.0 { for h in 0..n { if mask[h] { let x = strategy[a * n + h]; steps[a * n + h] = if x.is_finite() && x > 0.0 { ((x / top) as f64 * q as f64).round().min(q as f64) as u32 } else { 0 }; } } }
    }
    (scales, steps)
}

/// The strategy a reader gets back from `quantize`'s output: per hand the frequencies add up to 1.
fn dequantize(scales: &[f32], steps: &[u32], mask: &[bool], na: usize, n: usize, q: u32) -> Vec<f32> {
    let mut out = vec![0f32; na * n];
    for h in 0..n {
        if !mask[h] { continue; }
        let mut sum = 0f32;
        for a in 0..na { let x = scales[a] * (steps[a * n + h] as f32 / q as f32); out[a * n + h] = x; sum += x; }
        if sum > 0.0 { for a in 0..na { out[a * n + h] /= sum; } } else { for a in 0..na { out[a * n + h] = 1.0 / na as f32; } }
    }
    out
}

impl LibFile {
    /// The file as bytes. `strategy_bits` is 8 or 16: how finely action frequencies are kept.
    pub fn to_bytes(&self, strategy_bits: u8) -> Result<Vec<u8>, String> {
        if strategy_bits != 8 && strategy_bits != 16 { return Err("strategy_bits must be 8 or 16".into()); }
        if self.nodes.is_empty() || self.nodes[0].kind != Kind::Decision { return Err("nothing to save".into()); }
        let mut body: Vec<u8> = vec![];
        body.push(strategy_bits);
        body.extend(self.flop);
        for p in 0..2 { body.extend((self.hands[p].len() as u16).to_le_bytes()); }
        for p in 0..2 { for &(a, b) in &self.hands[p] { body.push(a); body.push(b); } }
        for p in 0..2 { for &w in &self.initial[p] { body.extend(w.to_le_bytes()); } }
        self.put_node(&mut body, 0, &self.initial.clone(), strategy_bits);
        let mut header = self.header.clone();
        if let Some(h) = header.as_object_mut() { h.insert("strategy_bits".into(), json!(strategy_bits)); }
        let head = serde_json::to_vec(&header).map_err(|e| e.to_string())?;
        let packed = zstd::stream::encode_all(&body[..], 19).map_err(|e| e.to_string())?;
        let mut out = Vec::with_capacity(packed.len() + head.len() + 24);
        out.extend(MAGIC);
        out.extend((head.len() as u32).to_le_bytes());
        out.extend(&head);
        out.extend((body.len() as u32).to_le_bytes());
        out.extend(&packed);
        Ok(out)
    }

    fn put_node(&self, out: &mut Vec<u8>, id: usize, reach: &[Vec<f32>; 2], bits: u8) {
        let node = &self.nodes[id];
        out.push(match node.kind { Kind::Decision => 0, Kind::Deal => 1, Kind::End => 2 });
        out.extend(node.bets[0].to_le_bytes()); out.extend(node.bets[1].to_le_bytes());
        let live = |p: usize| -> Vec<bool> { reach[p].iter().map(|&w| w > 0.0).collect() };
        match node.kind {
            Kind::End => {}
            Kind::Deal => {
                out.push(node.other as u8);
                put_values(out, &node.ev_other, &live(node.other));
                out.push(node.cards.len() as u8);
                for c in &node.cards { out.push(c.card); for x in [c.ev[0], c.ev[1], c.equity[0], c.equity[1], c.bet] { out.extend(x.to_le_bytes()); } }
            }
            Kind::Decision => {
                let (p, na, n) = (node.player, node.actions.len(), self.hands[node.player].len());
                out.push(p as u8); out.push(na as u8);
                for a in &node.actions { let (c, x) = action_code(a); out.push(c); out.extend(x.to_le_bytes()); }
                if id == 0 { out.push(node.other as u8); put_values(out, &node.ev_other, &live(node.other)); }
                let mask = live(p);
                let q = if bits == 8 { 255u32 } else { 65535 };
                let (scales, steps) = quantize(&node.strategy, &mask, na, n, q);
                for a in 0..na {
                    out.extend(scales[a].to_le_bytes());
                    let col: Vec<u16> = (0..n).filter(|&h| mask[h]).map(|h| steps[a * n + h] as u16).collect();
                    if bits == 8 { out.extend(col.iter().map(|x| *x as u8)); } else { put_u16s(out, &col); }
                }
                for a in 0..na {
                    if matches!(node.actions[a], Action::Fold) { continue; }
                    put_values(out, &node.detail[a * n..(a + 1) * n], &mask);
                }
                let kept = dequantize(&scales, &steps, &mask, na, n, q);
                for a in 0..na {
                    let mut r = reach.clone();
                    for h in 0..n { r[p][h] = reach[p][h] * kept[a * n + h]; }
                    self.put_node(out, node.children[a], &r, bits);
                }
            }
        }
    }

    /// Reads only the header: what the spot is, without unpacking the strategies.
    pub fn read_header(mut input: impl Read) -> Result<Value, String> {
        let mut m = [0u8; 12];
        input.read_exact(&mut m).map_err(|_| "this is not a Pontiac library file".to_string())?;
        if &m[..8] != MAGIC { return Err("this is not a Pontiac library file".into()); }
        let n = u32::from_le_bytes([m[8], m[9], m[10], m[11]]) as usize;
        if n > 16 << 20 { return Err("the library file is damaged".into()); }
        let mut head = vec![0u8; n];
        input.read_exact(&mut head).map_err(|_| "the library file is cut short or damaged".to_string())?;
        let header: Value = serde_json::from_slice(&head).map_err(|_| "the library file's header could not be read".to_string())?;
        if header.get("format").and_then(|x| x.as_u64()).unwrap_or(u64::MAX) > FORMAT { return Err("this library file was made by a newer version of Pontiac".into()); }
        Ok(header)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<LibFile, String> {
        let header = Self::read_header(bytes)?;
        let n = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
        let rest = &bytes[12 + n..];
        if rest.len() < 4 { return Err("the library file is cut short or damaged".into()); }
        let size = u32::from_le_bytes([rest[0], rest[1], rest[2], rest[3]]) as usize;
        if size > 256 << 20 { return Err("the library file is damaged".into()); }
        let body = zstd::stream::decode_all(&rest[4..]).map_err(|_| "the library file is cut short or damaged".to_string())?;
        if body.len() != size { return Err("the library file is cut short or damaged".into()); }
        let tree = tree_from(header.get("tree").ok_or("the library file has no bet tree")?)?;
        let mut r = Reader { b: &body, at: 0 };
        let bits = r.u8()?;
        if bits != 8 && bits != 16 { return Err("the library file is damaged".into()); }
        let f = r.take(3)?; let flop = [f[0], f[1], f[2]];
        let counts = [r.u16()? as usize, r.u16()? as usize];
        let mut hands: [Vec<(u8, u8)>; 2] = [vec![], vec![]];
        for p in 0..2 { for _ in 0..counts[p] { let s = r.take(2)?; if s[0] >= 52 || s[1] >= 52 || s[0] == s[1] { return Err("the library file is damaged".into()); } hands[p].push((s[0], s[1])); } }
        let mut initial: [Vec<f32>; 2] = [vec![], vec![]];
        for p in 0..2 { for _ in 0..counts[p] { initial[p].push(r.f32()?); } }
        let mut file = LibFile { header, tree, flop, hands, initial: initial.clone(), nodes: vec![] };
        file.get_node(&mut r, None, &initial, bits, 0)?;
        if file.nodes.is_empty() || file.nodes[0].kind != Kind::Decision { return Err("the library file is damaged".into()); }
        Ok(file)
    }

    fn get_node(&mut self, r: &mut Reader, parent: Option<(usize, usize)>, reach: &[Vec<f32>; 2], bits: u8, depth: usize) -> Result<usize, String> {
        if depth > 64 || self.nodes.len() > 100_000 { return Err("the library file is damaged".into()); }
        let id = self.nodes.len();
        let kind = match r.u8()? { 0 => Kind::Decision, 1 => Kind::Deal, 2 => Kind::End, _ => return Err("the library file is damaged".into()) };
        let bets = [r.i32()?, r.i32()?];
        let mut node = LibNode { kind, player: 0, bets, parent, actions: vec![], children: vec![], strategy: vec![], detail: vec![], ev_other: vec![], other: 0, cards: vec![] };
        let live = |p: usize| -> Vec<bool> { reach[p].iter().map(|&w| w > 0.0).collect() };
        match kind {
            Kind::End => { self.nodes.push(node); }
            Kind::Deal => {
                node.other = (r.u8()? & 1) as usize;
                node.ev_other = r.values(&live(node.other))?;
                for _ in 0..r.u8()? { let card = r.u8()?; node.cards.push(CardInfo { card, ev: [r.f32()?, r.f32()?], equity: [r.f32()?, r.f32()?], bet: r.f32()? }); }
                self.nodes.push(node);
            }
            Kind::Decision => {
                let p = (r.u8()? & 1) as usize; let na = r.u8()? as usize;
                if na == 0 || na > 32 { return Err("the library file is damaged".into()); }
                let n = self.hands[p].len();
                node.player = p;
                for _ in 0..na { let c = r.u8()?; let x = r.i32()?; node.actions.push(action_from(c, x)?); }
                if id == 0 { node.other = (r.u8()? & 1) as usize; node.ev_other = r.values(&live(node.other))?; }
                let mask = live(p);
                let k = mask.iter().filter(|m| **m).count();
                let q = if bits == 8 { 255u32 } else { 65535 };
                let mut steps = vec![0u32; na * n]; let mut scales = vec![0f32; na];
                for a in 0..na {
                    scales[a] = r.f32()?;
                    if !(0.0..=1.0001).contains(&scales[a]) { return Err("the library file is damaged".into()); }
                    let col: Vec<u16> = if bits == 8 { r.take(k)?.iter().map(|x| *x as u16).collect() } else { r.u16s(k)? };
                    let mut it = col.into_iter();
                    for h in 0..n { if mask[h] { steps[a * n + h] = it.next().unwrap() as u32; } }
                }
                node.strategy = dequantize(&scales, &steps, &mask, na, n, q);
                node.detail = vec![0.0; na * n];
                for a in 0..na {
                    if matches!(node.actions[a], Action::Fold) { continue; }
                    let v = r.values(&mask)?;
                    node.detail[a * n..(a + 1) * n].copy_from_slice(&v);
                }
                let kept = node.strategy.clone();
                self.nodes.push(node);
                for a in 0..na {
                    let mut rc = reach.clone();
                    for h in 0..n { rc[p][h] = reach[p][h] * kept[a * n + h]; }
                    let child = self.get_node(r, Some((id, a)), &rc, bits, depth + 1)?;
                    self.nodes[id].children.push(child);
                }
            }
        }
        Ok(id)
    }

    pub fn save(&self, path: &str, strategy_bits: u8) -> Result<u64, String> {
        let bytes = self.to_bytes(strategy_bits)?;
        // written beside, then renamed, so a reader never sees half a file
        let tmp = format!("{path}.part");
        std::fs::File::create(&tmp).and_then(|mut f| f.write_all(&bytes)).map_err(|e| format!("could not write {path}: {e}"))?;
        std::fs::rename(&tmp, path).map_err(|e| format!("could not write {path}: {e}"))?;
        Ok(bytes.len() as u64)
    }

    pub fn load(path: &str) -> Result<LibFile, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("could not open {path}: {e}"))?;
        Self::from_bytes(&bytes)
    }

    /// The file as a reader will see it: what `from_bytes(to_bytes(..))` gives.
    pub fn as_saved(&self, strategy_bits: u8) -> Result<LibFile, String> { Self::from_bytes(&self.to_bytes(strategy_bits)?) }
}

/// True when the bytes start like a library file.
pub fn is_library_file(first_bytes: &[u8]) -> bool { first_bytes.len() >= 8 && &first_bytes[..8] == MAGIC }

// ------------------------------------------------------------------ chance to win without a solve

/// Each hand's chance to win against the other player's range on a board of 3, 4 or 5 cards.
/// Hands are matched by their cards; hands with no weight get 0.
pub fn equities(board: &[u8], hands: &[Vec<(u8, u8)>; 2], weights: &[Vec<f32>; 2]) -> Result<[Vec<f32>; 2], String> {
    let mut ranges = [Range::new(), Range::new()];
    for p in 0..2 { for (h, &(a, b)) in hands[p].iter().enumerate() { let w = weights[p][h]; if w > 0.0 && !board.contains(&a) && !board.contains(&b) { ranges[p].set_weight_by_cards(a, b, w.min(1.0)); } } }
    if ranges[0].is_empty() || ranges[1].is_empty() { return Ok([vec![0.0; hands[0].len()], vec![0.0; hands[1].len()]]); }
    let card = CardConfig { range: ranges, flop: [board[0], board[1], board[2]], turn: board.get(3).copied().unwrap_or(NOT_DEALT), river: board.get(4).copied().unwrap_or(NOT_DEALT) };
    // a tree with no bets at all: only the cards matter here
    let tree = TreeConfig { initial_state: match board.len() { 3 => BoardState::Flop, 4 => BoardState::Turn, _ => BoardState::River }, starting_pot: 2, effective_stack: 2, ..Default::default() };
    let mut game = PostFlopGame::with_config(card, ActionTree::new(tree)?)?;
    PostFlopGame::cache_normalized_weights(&mut game);
    let mut out = [vec![0.0; hands[0].len()], vec![0.0; hands[1].len()]];
    for p in 0..2 {
        let eq = PostFlopGame::equity(&game, p);
        let sub = PostFlopGame::private_cards(&game, p);
        let mut at = 0;
        for (h, hand) in hands[p].iter().enumerate() { if at < sub.len() && sub[at] == *hand { out[p][h] = eq[at]; at += 1; } }
    }
    Ok(out)
}

// ------------------------------------------------------------------ solving a turn again

#[derive(Clone, Debug)]
pub struct ResolveOpts {
    /// stop when the turn is solved this closely, as a percent of the pot when the turn card comes
    pub accuracy_pct: f32,
    pub max_seconds: f64,
    pub max_iterations: u32,
}
impl Default for ResolveOpts { fn default() -> Self { ResolveOpts { accuracy_pct: 0.1, max_seconds: 60.0, max_iterations: 1000 } } }

/// How a turn solve went, and how it compares with what the whole solve said about the same card.
#[derive(Clone, Debug, Default)]
pub struct ResolveInfo {
    pub card: u8,
    pub seconds: f64,
    pub iterations: u32,
    pub accuracy_pct: f64,
    pub memory_mb: f64,
    pub stopped: &'static str,
    pub pot: i32,
    /// each player's value over their range: from the whole solve, and from this turn solve
    pub stored_ev: [f64; 2],
    pub ev: [f64; 2],
    /// how often the first player bets: whole solve, and this turn solve
    pub stored_bet: f64,
    pub bet: f64,
}

/// A turn (with its river) solved from a library file's flop line.
pub struct Sub { pub node: usize, pub card: u8, pub game: PostFlopGame, pub info: ResolveInfo, used: u64 }

/// Called every ten steps with (steps done, how close in percent of the pot, seconds); return false to stop.
pub type Progress<'a> = &'a (dyn Fn(u32, f32, f64) -> bool + Sync);

/// A library file opened for reading. The flop comes from the file; a turn card is solved when
/// first played (or beforehand with [`LibGame::solve_turn`]) and kept for as long as memory allows.
pub struct LibGame {
    pub file: LibFile,
    pub opts: ResolveOpts,
    /// how many solved turns to keep before the least recently used is dropped
    pub keep: usize,
    same: [Vec<u16>; 2],
    node: usize,
    reach: [Vec<f32>; 2],
    hist: Vec<usize>,
    /// index into `subs` once a turn card has been played
    sub: Option<usize>,
    subs: Vec<Sub>,
    clock: u64,
    norm: Option<[Vec<f32>; 2]>,
    eq_cache: std::cell::RefCell<std::collections::HashMap<usize, [Vec<f32>; 2]>>,
}

impl LibGame {
    pub fn new(file: LibFile) -> LibGame {
        let mut same: [Vec<u16>; 2] = [vec![], vec![]];
        for p in 0..2 { same[p] = file.hands[p].iter().map(|h| file.hands[1 - p].binary_search(h).map_or(u16::MAX, |i| i as u16)).collect(); }
        let reach = file.initial.clone();
        LibGame { file, opts: ResolveOpts::default(), keep: 4, same, node: 0, reach, hist: vec![], sub: None, subs: vec![], clock: 0, norm: None, eq_cache: Default::default() }
    }

    pub fn open(path: &str) -> Result<LibGame, String> { Ok(LibGame::new(LibFile::load(path)?)) }

    /// True when the current node is on a turn or river that was solved here.
    pub fn in_turn(&self) -> bool { self.sub.is_some() }

    /// How the turn solve under the current node went (None on the flop).
    pub fn turn_info(&self) -> Option<&ResolveInfo> { self.sub.map(|i| &self.subs[i].info) }

    /// The solved turn under the current node, as an ordinary game positioned at the current node.
    pub fn turn_game(&self) -> Option<&PostFlopGame> { self.sub.map(|i| &self.subs[i].game) }

    /// Memory held by the solved turns.
    pub fn held_bytes(&self) -> u64 { self.subs.iter().map(|s| s.game.memory_usage().0).sum() }

    /// Drops every solved turn.
    pub fn forget_turns(&mut self) { self.back_to_root(); self.subs.clear(); }

    /// Where a flop history leads in the file, with each player's range there. The history must stay on the flop.
    fn follow(&self, flop_history: &[usize]) -> Result<(usize, [Vec<f32>; 2]), String> {
        let (mut node, mut reach) = (0usize, self.file.initial.clone());
        for &a in flop_history {
            let nd = &self.file.nodes[node];
            if nd.kind != Kind::Decision || a >= nd.children.len() { return Err("that line is not in this library file".into()); }
            let n = self.file.hands[nd.player].len();
            for h in 0..n { reach[nd.player][h] *= nd.strategy[a * n + h]; }
            node = nd.children[a];
        }
        Ok((node, reach))
    }

    /// Whether a history (from the start of the spot) needs a turn that has not been solved yet.
    /// Returns the flop part of the history and the card when it does.
    pub fn needs_turn(&self, history: &[usize]) -> Result<Option<(Vec<usize>, u8)>, String> {
        let mut node = 0usize;
        for (i, &a) in history.iter().enumerate() {
            let nd = &self.file.nodes[node];
            match nd.kind {
                Kind::End => return Err("that line is not in this library file".into()),
                Kind::Deal => return Ok(if self.subs.iter().any(|s| s.node == node && s.card as usize == a) { None } else { Some((history[..i].to_vec(), a as u8)) }),
                Kind::Decision => { if a >= nd.children.len() { return Err("that line is not in this library file".into()); } node = nd.children[a]; }
            }
        }
        Ok(None)
    }

    /// Solves the turn `card` after the flop line `flop_history` and keeps it. Does nothing when it
    /// is already held. Returns how the solve went.
    pub fn solve_turn(&mut self, flop_history: &[usize], card: u8, opts: &ResolveOpts, progress: Option<Progress>) -> Result<ResolveInfo, String> {
        let (node, reach) = self.follow(flop_history)?;
        if let Some(s) = self.subs.iter().find(|s| s.node == node && s.card == card) { return Ok(s.info.clone()); }
        let sub = resolve(&self.file, node, &reach, card, opts, progress)?;
        let info = sub.info.clone();
        self.back_to_root();
        while self.subs.len() >= self.keep.max(1) {
            let oldest = (0..self.subs.len()).min_by_key(|&i| self.subs[i].used).unwrap();
            self.subs.remove(oldest);
        }
        self.subs.push(sub);
        Ok(info)
    }

    fn flop_node(&self) -> &LibNode { &self.file.nodes[self.node] }

    fn flop_normalized(&self) -> [Vec<f32>; 2] {
        let mut sum = [0f64; 2]; let mut minus = [[0f64; 52]; 2];
        for p in 0..2 { for (h, &(a, b)) in self.file.hands[p].iter().enumerate() { let w = self.reach[p][h] as f64; sum[p] += w; minus[p][a as usize] += w; minus[p][b as usize] += w; } }
        let mut out = [vec![], vec![]];
        for p in 0..2 {
            out[p] = self.file.hands[p].iter().enumerate().map(|(h, &(a, b))| {
                let s = self.same[p][h];
                let twin = if s == u16::MAX { 0.0 } else { self.reach[1 - p][s as usize] as f64 };
                self.reach[p][h] * (sum[1 - p] + twin - minus[1 - p][a as usize] - minus[1 - p][b as usize]) as f32
            }).collect();
        }
        out
    }

    /// Value of each of `player`'s hands at the current flop node (0 for hands no longer in range).
    fn flop_values(&self, player: usize) -> Vec<f32> {
        let node = self.flop_node();
        let n = self.file.hands[player].len();
        let live = |v: Vec<f32>| -> Vec<f32> { v.into_iter().zip(&self.reach[player]).map(|(x, &w)| if w > 0.0 { x } else { 0.0 }).collect() };
        if node.kind == Kind::Decision && node.player == player {
            let na = node.actions.len();
            return (0..n).map(|h| if self.reach[player][h] > 0.0 { (0..na).map(|a| node.strategy[a * n + h] * node.detail[a * n + h]).sum() } else { 0.0 }).collect();
        }
        if let Some((parent, a)) = node.parent {
            let up = &self.file.nodes[parent];
            if up.player == player { return live(up.detail[a * n..(a + 1) * n].to_vec()); }
        }
        if node.other == player && node.ev_other.len() == n { return live(node.ev_other.clone()); }
        vec![0.0; n]
    }
}

/// Builds and solves the turn `card` reached at deal node `node` of `file`, with the ranges `reach`.
pub fn resolve(file: &LibFile, node: usize, reach: &[Vec<f32>; 2], card: u8, opts: &ResolveOpts, progress: Option<Progress>) -> Result<Sub, String> {
    let nd = file.nodes.get(node).filter(|n| n.kind == Kind::Deal).ok_or("no card is dealt at that point of the hand")?;
    let stored = nd.cards.iter().find(|c| c.card == card).ok_or("that card cannot come here")?;
    if nd.bets[0] != nd.bets[1] { return Err("the library file is damaged".into()); }
    let mut ranges = [Range::new(), Range::new()];
    for p in 0..2 { for (h, &(a, b)) in file.hands[p].iter().enumerate() { let w = reach[p][h]; if w > 0.0 && a != card && b != card { ranges[p].set_weight_by_cards(a, b, w.min(1.0)); } } }
    if ranges[0].is_empty() || ranges[1].is_empty() { return Err("This line is never played in the solve, so there is no range left to solve the turn with.".into()); }
    let mut tree = file.tree.clone();
    tree.initial_state = BoardState::Turn;
    tree.starting_pot = file.tree.starting_pot + nd.bets[0] + nd.bets[1];
    tree.effective_stack = file.tree.effective_stack - nd.bets[0];
    tree.turn_donk_sizes = None;
    if tree.effective_stack <= 0 { return Err("both players are all-in here: there is nothing left to solve".into()); }
    let pot = tree.starting_pot;
    let cards = CardConfig { range: ranges, flop: file.flop, turn: card, river: NOT_DEALT };
    let mut game = PostFlopGame::with_config(cards, ActionTree::new(tree)?)?;
    let (full, small) = game.memory_usage();
    let compress = full > 1500 << 20;
    game.allocate_memory(compress);
    let t0 = Instant::now();
    let target = pot as f32 * opts.accuracy_pct / 100.0;
    let (mut expl, mut it, mut stopped) = (f32::MAX, 0u32, "max_iterations");
    while it < opts.max_iterations.max(1) {
        solve_step(&game, it); it += 1;
        let out_of_time = t0.elapsed().as_secs_f64() >= opts.max_seconds;
        if it % 10 == 0 || it == opts.max_iterations || out_of_time {
            expl = compute_exploitability(&game);
            if expl <= target { stopped = "target_reached"; break; }
            if let Some(f) = progress { if !f(it, expl / pot as f32 * 100.0, t0.elapsed().as_secs_f64()) { stopped = "cancelled"; break; } }
        }
        if out_of_time { stopped = "max_seconds"; break; }
    }
    if stopped == "cancelled" { return Err("Solve cancelled".into()); }
    finalize(&mut game);
    let mut info = ResolveInfo { card, seconds: t0.elapsed().as_secs_f64(), iterations: it, accuracy_pct: (expl / pot as f32 * 100.0) as f64, memory_mb: (if compress { small } else { full }) as f64 / 1048576.0, stopped, pot,
        stored_ev: [stored.ev[0] as f64, stored.ev[1] as f64], stored_bet: stored.bet as f64, ..Default::default() };
    PostFlopGame::back_to_root(&mut game);
    PostFlopGame::cache_normalized_weights(&mut game);
    for p in 0..2 { info.ev[p] = range_average(&PostFlopGame::expected_values(&game, p), PostFlopGame::normalized_weights(&game, p)) as f64; }
    {
        let me = PostFlopGame::current_player(&game);
        let acts = PostFlopGame::available_actions(&game);
        let (st, w) = (PostFlopGame::strategy(&game), PostFlopGame::normalized_weights(&game, me).to_vec());
        let n = w.len();
        let (mut b, mut t) = (0f64, 0f64);
        for h in 0..n { t += w[h] as f64; for (a, act) in acts.iter().enumerate() { if matches!(act, Action::Bet(_) | Action::Raise(_) | Action::AllIn(_)) { b += w[h] as f64 * st[a * n + h] as f64; } } }
        info.bet = if t > 0.0 { b / t } else { 0.0 };
    }
    Ok(Sub { node, card, game, info, used: 0 })
}

impl Solved for LibGame {
    fn back_to_root(&mut self) {
        self.node = 0; self.reach = self.file.initial.clone(); self.hist.clear(); self.sub = None; self.norm = None;
    }
    fn history(&self) -> Vec<usize> { self.hist.clone() }

    /// Plays an action or deals a card. Dealing a turn card that has not been solved yet solves it
    /// here with `self.opts`, which can take a while; a failure to solve panics, so callers that
    /// want an error instead should use `solve_turn` first.
    fn play(&mut self, action: usize) {
        if let Some(i) = self.sub { PostFlopGame::play(&mut self.subs[i].game, action); self.hist.push(action); return; }
        let kind = self.file.nodes[self.node].kind;
        match kind {
            Kind::End => panic!("the hand is over"),
            Kind::Decision => {
                let nd = &self.file.nodes[self.node];
                let (p, n) = (nd.player, self.file.hands[nd.player].len());
                for h in 0..n { self.reach[p][h] *= nd.strategy[action * n + h]; }
                self.node = nd.children[action];
            }
            Kind::Deal => {
                let (card, node) = (action as u8, self.node);
                if !self.subs.iter().any(|s| s.node == node && s.card == card) {
                    let sub = resolve(&self.file, node, &self.reach, card, &self.opts, None).unwrap_or_else(|e| panic!("{e}"));
                    while self.subs.len() >= self.keep.max(1) { let oldest = (0..self.subs.len()).min_by_key(|&i| self.subs[i].used).unwrap(); self.subs.remove(oldest); }
                    self.subs.push(sub);
                }
                let i = self.subs.iter().position(|s| s.node == node && s.card == card).unwrap();
                self.clock += 1; self.subs[i].used = self.clock;
                PostFlopGame::back_to_root(&mut self.subs[i].game);
                self.sub = Some(i);
            }
        }
        self.norm = None;
        self.hist.push(action);
    }

    fn is_terminal_node(&self) -> bool { match self.sub { Some(i) => PostFlopGame::is_terminal_node(&self.subs[i].game), None => self.flop_node().kind == Kind::End } }
    fn is_chance_node(&self) -> bool { match self.sub { Some(i) => PostFlopGame::is_chance_node(&self.subs[i].game), None => self.flop_node().kind == Kind::Deal } }
    fn current_player(&self) -> usize { match self.sub { Some(i) => PostFlopGame::current_player(&self.subs[i].game), None => self.flop_node().player } }
    fn available_actions(&self) -> Vec<Action> {
        match self.sub {
            Some(i) => PostFlopGame::available_actions(&self.subs[i].game),
            None => { let nd = self.flop_node(); if nd.kind == Kind::Deal { nd.cards.iter().map(|c| Action::Chance(c.card)).collect() } else { nd.actions.clone() } }
        }
    }
    fn possible_cards(&self) -> u64 {
        match self.sub {
            Some(i) => PostFlopGame::possible_cards(&self.subs[i].game),
            None => { let nd = self.flop_node(); if nd.kind == Kind::Deal { nd.cards.iter().fold(0u64, |m, c| m | (1u64 << c.card)) } else { 0 } }
        }
    }
    fn current_board(&self) -> Vec<u8> { match self.sub { Some(i) => PostFlopGame::current_board(&self.subs[i].game), None => self.file.flop.to_vec() } }
    fn total_bet_amount(&self) -> [i32; 2] {
        match self.sub {
            Some(i) => { let (t, b) = (PostFlopGame::total_bet_amount(&self.subs[i].game), self.file.nodes[self.subs[i].node].bets); [t[0] + b[0], t[1] + b[1]] }
            None => self.flop_node().bets,
        }
    }
    fn starting_pot(&self) -> i32 { self.file.tree.starting_pot }
    fn effective_stack(&self) -> i32 { self.file.tree.effective_stack }
    fn private_cards(&self, player: usize) -> Vec<(u8, u8)> { match self.sub { Some(i) => PostFlopGame::private_cards(&self.subs[i].game, player).to_vec(), None => self.file.hands[player].clone() } }
    fn cache_normalized_weights(&mut self) {
        match self.sub {
            Some(i) => PostFlopGame::cache_normalized_weights(&mut self.subs[i].game),
            None => { if self.norm.is_none() { self.norm = Some(self.flop_normalized()); } }
        }
    }
    fn weights(&self, player: usize) -> Vec<f32> { match self.sub { Some(i) => PostFlopGame::weights(&self.subs[i].game, player).to_vec(), None => self.reach[player].clone() } }
    fn normalized_weights(&self, player: usize) -> Vec<f32> {
        match self.sub {
            Some(i) => PostFlopGame::normalized_weights(&self.subs[i].game, player).to_vec(),
            None => match &self.norm { Some(n) => n[player].clone(), None => self.flop_normalized()[player].clone() },
        }
    }
    fn equity(&self, player: usize) -> Vec<f32> {
        match self.sub {
            Some(i) => PostFlopGame::equity(&self.subs[i].game, player),
            None => self.flop_equities()[player].clone(),
        }
    }
    fn expected_values(&self, player: usize) -> Vec<f32> { match self.sub { Some(i) => PostFlopGame::expected_values(&self.subs[i].game, player), None => self.flop_values(player) } }
    fn expected_values_detail(&self, player: usize) -> Vec<f32> {
        match self.sub {
            Some(i) => PostFlopGame::expected_values_detail(&self.subs[i].game, player),
            None => {
                let nd = self.flop_node();
                if nd.kind == Kind::Decision && nd.player == player {
                    let n = self.file.hands[player].len();
                    (0..nd.detail.len()).map(|i| if self.reach[player][i % n] > 0.0 { nd.detail[i] } else { 0.0 }).collect()
                } else { self.flop_values(player) }
            }
        }
    }
    fn strategy(&self) -> Vec<f32> { match self.sub { Some(i) => PostFlopGame::strategy(&self.subs[i].game), None => self.flop_node().strategy.clone() } }
    fn is_locked(&self) -> bool { false }
    fn prepare_card(&mut self, card: usize) -> Result<(), String> {
        if self.sub.is_some() || self.flop_node().kind != Kind::Deal { return Ok(()); }
        let (node, card) = (self.node, card as u8);
        if self.subs.iter().any(|s| s.node == node && s.card == card) { return Ok(()); }
        let sub = resolve(&self.file, node, &self.reach, card, &self.opts, None)?;
        while self.subs.len() >= self.keep.max(1) { let oldest = (0..self.subs.len()).min_by_key(|&i| self.subs[i].used).unwrap(); self.subs.remove(oldest); }
        self.subs.push(sub);
        Ok(())
    }
}

impl LibGame {
    /// Both players' chance to win at the current flop node, worked out once per node and kept.
    pub fn flop_equities(&self) -> [Vec<f32>; 2] {
        if let Some(e) = self.eq_cache.borrow().get(&self.node) { return e.clone(); }
        let e = equities(&self.file.flop, &self.file.hands, &self.reach).unwrap_or_else(|_| [vec![0.0; self.file.hands[0].len()], vec![0.0; self.file.hands[1].len()]]);
        self.eq_cache.borrow_mut().insert(self.node, e.clone());
        e
    }
    /// What the whole solve said about each card at the current deal node (empty elsewhere).
    pub fn card_infos(&self) -> Vec<CardInfo> { if self.sub.is_none() && self.flop_node().kind == Kind::Deal { self.flop_node().cards.clone() } else { vec![] } }
}

impl LibGame {
    /// Writes the solved turn under the current node as an ordinary whole solve that starts on the
    /// turn. The current node moves back to the start of the spot.
    pub fn save_turn(&mut self, path: &str, memo: &str) -> Result<(), String> {
        let i = self.sub.ok_or("the path does not reach a turn card")?;
        PostFlopGame::back_to_root(&mut self.subs[i].game);
        let out = save_data_to_file(&self.subs[i].game, memo, path, None);
        self.back_to_root();
        out
    }
}

impl ResolveInfo {
    /// The turn solve in plain numbers, chips divided by `chips_per_bb`.
    pub fn to_json(&self, names: &[String; 2], chips_per_bb: f64) -> Value {
        let r = |x: f64, d: i32| { let m = 10f64.powi(d); (x * m).round() / m };
        json!({
            "card": card_to_string(self.card).unwrap_or_default(),
            "seconds": r(self.seconds, 2), "iterations": self.iterations, "exploitability_pct": r(self.accuracy_pct, 3), "stopped_because": self.stopped, "memory_mb": r(self.memory_mb, 1),
            "pot_bb": r(self.pot as f64 / chips_per_bb, 2),
            "ev_bb": {names[0].clone(): r(self.ev[0] / chips_per_bb, 3), names[1].clone(): r(self.ev[1] / chips_per_bb, 3)},
            "whole_solve_ev_bb": {names[0].clone(): r(self.stored_ev[0] / chips_per_bb, 3), names[1].clone(): r(self.stored_ev[1] / chips_per_bb, 3)},
            "ev_difference_pct_of_pot": r((self.ev[0] - self.stored_ev[0]).abs() / self.pot.max(1) as f64 * 100.0, 2),
            "first_bet_frequency": r(self.bet, 4), "whole_solve_first_bet_frequency": r(self.stored_bet, 4),
            "note": "This turn was solved here from the ranges the stored flop strategy leaves each player. whole_solve_* are what the original flop solve said about the same card, kept in the file for comparison.",
        })
    }
}

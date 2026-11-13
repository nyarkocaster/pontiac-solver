//! Three-player game starting on the flop, turn or river: betting tree, exact terminal
//! evaluation with card removal, and a multithreaded DCFR solver.
use crate::cards::*;
use rayon::prelude::*;
use std::cell::UnsafeCell;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub enum Action { Fold, Check, Call, Bet(f64), Raise(f64), AllIn(f64) }
impl Action {
    pub fn label(&self) -> String {
        match self {
            Action::Fold => "Fold".into(), Action::Check => "Check".into(), Action::Call => "Call".into(),
            Action::Bet(x) => format!("Bet {:.0}", x), Action::Raise(x) => format!("Raise to {:.0}", x),
            Action::AllIn(x) => format!("All-in {:.0}", x),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind { Decision, FoldEnd, Showdown, Chance }

pub const NO_STREET: usize = usize::MAX;

#[derive(Default)]
pub struct NodeData {
    pub regrets: Vec<f32>,   // actions x hands[player]
    pub strat_sum: Vec<f32>, // actions x hands[player]
}

pub struct Node {
    pub kind: Kind,
    /// decision: who acts. fold end: who wins.
    pub player: usize,
    pub actions: Vec<Action>,
    /// decision: one per action. chance: one per entry of `cards`.
    pub children: Vec<usize>,
    /// chance only: the cards that can come, and how many real cards each one stands for
    pub cards: Vec<u8>,
    pub weights: Vec<f32>,
    /// chance only: cards not on the board before this deal
    pub remaining: f64,
    /// cards dealt inside the tree to reach this node: [turn, river], 255 = not dealt here
    pub dealt: [u8; 2],
    /// index into `Game::streets` of the final board this node sits on (NO_STREET before the river card)
    pub street: usize,
    /// chips each player has put in since the start of the tree
    pub invested: [f64; 3],
    pub folded: [bool; 3],
    pub data: UnsafeCell<NodeData>,
    /// fixed strategy (actions x hands[player]); the solver never changes it
    pub locked: Option<Vec<f32>>,
}

#[derive(Clone)]
pub struct Config {
    /// 3 cards (flop), 4 cards (turn) or 5 cards (river)
    pub board: Vec<u8>,
    pub pot: f64,
    pub stack: f64,
    pub ranges: [Vec<f32>; 3],
    /// pot-fraction bet sizes on each street
    pub flop_bet_sizes: Vec<f64>,
    pub turn_bet_sizes: Vec<f64>,
    pub bet_sizes: Vec<f64>,
    pub raise_sizes: Vec<f64>,
    pub flop_max_raises: usize,
    pub turn_max_raises: usize,
    pub max_raises: usize,
    /// how many cards per rank to deal on the turn / river (0 = every card, exact)
    pub turn_cards: usize,
    pub river_cards: usize,
    /// all-in is offered when the remaining stack is at most this multiple of the pot
    pub allin_spr: f64,
}

/// Showdown ordering for one final (five-card) board.
pub struct Street {
    pub card: u8,              // the river card dealt (255 when the tree starts on the river)
    pub strength: Vec<u32>,    // per hand index, 0 = blocked by the board
    /// strength order by card pair: rank[a * W + b] = 1 + index of the hand's strength group (0 = no such hand)
    pub rank: Vec<u16>,
    /// each player's hands that are possible on this board, weakest first
    pub sorted: [Vec<u16>; 3],
}

/// Row width of the card-by-card tables (52 cards padded to a multiple of 8 for vector maths).
pub const W: usize = 56;

pub struct Game {
    pub deck: Deck,
    pub cfg: Config,
    pub nodes: Vec<Node>,
    pub streets: Vec<Street>,
    /// final-board index by (turn card, river card) dealt in the tree
    pub street_of: HashMap<(u8, u8), usize>,
    /// the cards dealt after a given board prefix (key: turn card dealt in the tree, 255 = none yet)
    pub samples: HashMap<u8, Vec<(u8, f32)>>,
    pub valid: Vec<u16>,          // hands not blocked by the starting board
    pub card_hands: Vec<Vec<u16>>, // per card: valid hands containing it
    pub hands: [Vec<u16>; 3],     // each player's hands with non-zero weight
    pub hpos: [Vec<u32>; 3],      // hand index -> position in hands[p] (u32::MAX if absent)
    pub allocated: bool,
    /// unique per game; keeps per-thread scratch memory from leaking between games
    pub id: u64,
    /// when set, best-response checks may deviate at locked nodes too (for testing)
    pub br_ignores_locks: bool,
}

// Solver threads only ever touch disjoint river subtrees at the same time.
unsafe impl Sync for Game {}
unsafe impl Sync for Node {}

#[derive(Clone, Copy, PartialEq)]
pub enum Mode { Train(u32), Average, BestResponse }

#[derive(Clone)]
struct St { inv: [f64; 3], folded: [bool; 3], acted: [bool; 3], raises: usize, last_inc: f64 }

fn make_street(deck: &Deck, board: &[u8], card: u8, hands: &[Vec<u16>; 3]) -> Street {
    let mut strength = vec![0u32; NH];
    for (i, &(a, b)) in deck.cards.iter().enumerate() {
        if board.contains(&a) || board.contains(&b) { continue; }
        strength[i] = eval7(&[board[0], board[1], board[2], board[3], board[4], a, b]);
    }
    let mut order: Vec<u16> = (0..NH as u16).filter(|&i| strength[i as usize] > 0).collect();
    order.sort_by_key(|&i| strength[i as usize]);
    let mut rank = vec![0u16; 52 * W];
    let (mut gi, mut last) = (0u16, 0u32);
    for &h in &order {
        if strength[h as usize] != last { gi += 1; last = strength[h as usize]; }
        let (a, b) = deck.cards[h as usize];
        rank[a as usize * W + b as usize] = gi; rank[b as usize * W + a as usize] = gi;
    }
    let mut sorted: [Vec<u16>; 3] = Default::default();
    for p in 0..3 {
        sorted[p] = hands[p].iter().cloned().filter(|&h| strength[h as usize] > 0).collect();
        sorted[p].sort_by_key(|&h| strength[h as usize]);
    }
    Street { card, strength, rank, sorted }
}

thread_local! {
    /// Two zeroed card-by-card tables reused by every showdown on this thread.
    static TABLES: std::cell::RefCell<(Vec<f32>, Vec<f32>)> = std::cell::RefCell::new((vec![0f32; 52 * W], vec![0f32; 52 * W]));
    /// Spare reach vectors per player, valid for one game only (tagged with its id).
    static POOL: std::cell::RefCell<(u64, [Vec<Vec<f32>>; 3])> = std::cell::RefCell::new((0, Default::default()));
}
static NEXT_GAME_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// True when one more card of this suit changes what flushes are possible (two or more on board).
pub fn hot_suit(board: &[u8], suit: u8) -> bool { board.iter().filter(|&&c| c % 4 == suit).count() >= 2 }

/// Picks the cards a chance node deals. With `per_rank` = 0 every card comes (exact). Otherwise
/// every card of a flush-relevant suit comes, and for the other suits `per_rank` cards of each
/// rank stand in for the rest, spread evenly over those suits.
pub fn sample_cards(board: &[u8], per_rank: usize) -> Vec<(u8, f32)> {
    let avail: Vec<u8> = (0..52u8).filter(|c| !board.contains(c)).collect();
    if per_rank == 0 || per_rank >= 4 { return avail.iter().map(|&c| (c, 1.0)).collect(); }
    let mut out: Vec<(u8, f32)> = avail.iter().filter(|&&c| hot_suit(board, c % 4)).map(|&c| (c, 1.0)).collect();
    let cold: Vec<u8> = avail.iter().cloned().filter(|&c| !hot_suit(board, c % 4)).collect();
    let mut suit_left = [0f64; 4];
    for &c in &cold { suit_left[(c % 4) as usize] += 1.0; }
    let total = cold.len() as f64;
    let mut given = [0f64; 4];
    let mut picked = 0f64;
    for rank in (0..13u8).rev() {
        let mut here: Vec<u8> = cold.iter().cloned().filter(|c| c / 4 == rank).collect();
        if here.is_empty() { continue; }
        let k = per_rank.min(here.len());
        let w = here.len() as f32 / k as f32;
        for _ in 0..k {
            // take the suit that is furthest behind its fair share
            picked += 1.0;
            let (mut best, mut best_def) = (0usize, f64::MIN);
            for (i, c) in here.iter().enumerate() {
                let su = (c % 4) as usize;
                let def = suit_left[su] / total * picked - given[su];
                if def > best_def { best_def = def; best = i; }
            }
            let c = here.remove(best);
            given[(c % 4) as usize] += 1.0;
            out.push((c, w));
        }
    }
    out.sort_by(|a, b| b.0.cmp(&a.0));
    out
}

#[inline(always)]
fn row(m: &[f32], a: usize) -> &[f32; W] { m[a * W..a * W + W].try_into().unwrap() }
#[inline(always)]
fn rowu(m: &[u16], a: usize) -> &[u16; W] { m[a * W..a * W + W].try_into().unwrap() }

/// Inputs for the per-hand sum over cards at a showdown.
struct Kern<'a> {
    s: u16,
    ca1: &'a [f32; W], ca2: &'a [f32; W], cl1: &'a [f32; W], cl2: &'a [f32; W], ct1: &'a [f32; W], ct2: &'a [f32; W],
    r1a: &'a [f32; W], r1b: &'a [f32; W], r2a: &'a [f32; W], r2b: &'a [f32; W], ka: &'a [u16; W], kb: &'a [u16; W],
}
impl Kern<'_> {
    /// The five products for card `c`: all*all, then (lower*lower, tie*lower, lower*tie, tie*tie)
    /// for a three-way showdown or (lower*all, tie*all) for a two-way one.
    #[inline(always)]
    fn term(&self, c: usize, three_way: bool) -> [f32; 5] {
        let la = if self.ka[c] < self.s { 1.0f32 } else { 0.0 }; let lb = if self.kb[c] < self.s { 1.0f32 } else { 0.0 };
        let ea = if self.ka[c] == self.s { 1.0f32 } else { 0.0 }; let eb = if self.kb[c] == self.s { 1.0f32 } else { 0.0 };
        let (p1a, p1b, p2a, p2b) = (self.r1a[c], self.r1b[c], self.r2a[c], self.r2b[c]);
        let a1 = self.ca1[c] - p1a - p1b; let a2 = self.ca2[c] - p2a - p2b;
        let l1 = self.cl1[c] - p1a * la - p1b * lb; let t1 = self.ct1[c] - p1a * ea - p1b * eb;
        if three_way {
            let l2 = self.cl2[c] - p2a * la - p2b * lb; let t2 = self.ct2[c] - p2a * ea - p2b * eb;
            [a1 * a2, l1 * l2, t1 * l2, l1 * t2, t1 * t2]
        } else { [a1 * a2, l1 * a2, t1 * a2, 0.0, 0.0] }
    }
}

impl Kern<'_> {
    /// As `term`, for when no opponent hand ties the hero hand: only all*all and lower*lower
    /// (three-way) or lower*all (two-way) can be non-zero.
    #[inline(always)]
    fn term_no_ties(&self, c: usize, three_way: bool) -> [f32; 2] {
        let la = if self.ka[c] < self.s { 1.0f32 } else { 0.0 }; let lb = if self.kb[c] < self.s { 1.0f32 } else { 0.0 };
        let (p1a, p1b, p2a, p2b) = (self.r1a[c], self.r1b[c], self.r2a[c], self.r2b[c]);
        let a1 = self.ca1[c] - p1a - p1b; let a2 = self.ca2[c] - p2a - p2b;
        let l1 = self.cl1[c] - p1a * la - p1b * lb;
        if three_way { let l2 = self.cl2[c] - p2a * la - p2b * lb; [a1 * a2, l1 * l2] } else { [a1 * a2, l1 * a2] }
    }
}
#[inline(always)]
fn kern_nt_body(k: &Kern, three_way: bool) -> [f32; 2] {
    let mut acc = [[0f32; 8]; 2];
    for base in (0..W).step_by(8) { for j in 0..8 { let t = k.term_no_ties(base + j, three_way); acc[0][j] += t[0]; acc[1][j] += t[1]; } }
    [acc[0].iter().sum(), acc[1].iter().sum()]
}
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2,fma")]
unsafe fn kern_nt_avx2(k: &Kern, three_way: bool) -> [f32; 2] { kern_nt_body(k, three_way) }
#[inline(always)]
fn kern_nt(k: &Kern, three_way: bool) -> [f32; 2] {
    #[cfg(target_arch = "x86_64")]
    { if std::is_x86_feature_detected!("avx2") && std::is_x86_feature_detected!("fma") { return unsafe { kern_nt_avx2(k, three_way) }; } }
    kern_nt_body(k, three_way)
}

#[inline(always)]
fn kern_body(k: &Kern, three_way: bool) -> [f32; 5] {
    // eight independent partial sums per output so the compiler can use vector instructions
    let mut acc = [[0f32; 8]; 5];
    for base in (0..W).step_by(8) {
        for j in 0..8 {
            let t = k.term(base + j, three_way);
            for i in 0..5 { acc[i][j] += t[i]; }
        }
    }
    let mut out = [0f32; 5];
    for i in 0..5 { out[i] = acc[i].iter().sum(); }
    out
}
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2,fma")]
unsafe fn kern_avx2(k: &Kern, three_way: bool) -> [f32; 5] { kern_body(k, three_way) }
#[inline(always)]
fn kern(k: &Kern, three_way: bool) -> [f32; 5] {
    #[cfg(target_arch = "x86_64")]
    { if std::is_x86_feature_detected!("avx2") && std::is_x86_feature_detected!("fma") { return unsafe { kern_avx2(k, three_way) }; } }
    kern_body(k, three_way)
}

#[inline(always)]
fn kern_all_body(c1: &[f32; W], c2: &[f32; W], r1a: &[f32; W], r1b: &[f32; W], r2a: &[f32; W], r2b: &[f32; W]) -> f32 {
    let mut acc = [0f32; 8];
    for base in (0..W).step_by(8) { for j in 0..8 { let c = base + j; acc[j] += (c1[c] - r1a[c] - r1b[c]) * (c2[c] - r2a[c] - r2b[c]); } }
    acc.iter().sum()
}
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2,fma")]
unsafe fn kern_all_avx2(c1: &[f32; W], c2: &[f32; W], r1a: &[f32; W], r1b: &[f32; W], r2a: &[f32; W], r2b: &[f32; W]) -> f32 { kern_all_body(c1, c2, r1a, r1b, r2a, r2b) }
#[inline(always)]
fn kern_all(c1: &[f32; W], c2: &[f32; W], r1a: &[f32; W], r1b: &[f32; W], r2a: &[f32; W], r2b: &[f32; W]) -> f32 {
    #[cfg(target_arch = "x86_64")]
    { if std::is_x86_feature_detected!("avx2") && std::is_x86_feature_detected!("fma") { return unsafe { kern_all_avx2(c1, c2, r1a, r1b, r2a, r2b) }; } }
    kern_all_body(c1, c2, r1a, r1b, r2a, r2b)
}

struct Ctx { board: Vec<u8>, dealt: [u8; 2], street: usize }

impl Game {
    /// Builds the tree. With `allocate` false no solver memory is reserved (for size estimates).
    pub fn new(mut cfg: Config, allocate: bool) -> Result<Self, String> {
        if cfg.board.len() < 3 || cfg.board.len() > 5 { return Err("the board needs 3 cards (flop), 4 (turn) or 5 (river)".into()); }
        let deck = Deck::new();
        let valid: Vec<u16> = (0..NH as u16).filter(|&i| { let (a, b) = deck.cards[i as usize]; !cfg.board.contains(&a) && !cfg.board.contains(&b) }).collect();
        let mut card_hands = vec![vec![]; 52];
        for &h in &valid { let (a, b) = deck.cards[h as usize]; card_hands[a as usize].push(h); card_hands[b as usize].push(h); }
        let mut hands: [Vec<u16>; 3] = Default::default();
        let mut hpos: [Vec<u32>; 3] = [vec![u32::MAX; NH], vec![u32::MAX; NH], vec![u32::MAX; NH]];
        let mut is_valid = vec![false; NH]; for &h in &valid { is_valid[h as usize] = true; }
        for p in 0..3 {
            for i in 0..NH {
                if !is_valid[i] { cfg.ranges[p][i] = 0.0; }
                if cfg.ranges[p][i] > 0.0 { hpos[p][i] = hands[p].len() as u32; hands[p].push(i as u16); }
            }
            if hands[p].is_empty() { return Err(format!("player {} has an empty range", p + 1)); }
        }
        let mut g = Game { deck, cfg, nodes: vec![], streets: vec![], street_of: HashMap::new(), samples: HashMap::new(), valid, card_hands, hands, hpos, allocated: allocate, br_ignores_locks: false,
            id: NEXT_GAME_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed) };
        // deal structure: which turn and river cards exist in this tree
        let b0 = g.cfg.board.clone();
        match b0.len() {
            5 => { let st = make_street(&g.deck, &b0, 255, &g.hands); g.streets.push(st); g.street_of.insert((255, 255), 0); }
            4 => { g.add_rivers(&b0, 255); }
            _ => {
                let turns = sample_cards(&b0, g.cfg.turn_cards);
                for &(t, _) in &turns { let mut b = b0.clone(); b.push(t); g.add_rivers(&b, t); }
                g.samples.insert(254, turns);
            }
        }
        let st = St { inv: [0.0; 3], folded: [false; 3], acted: [false; 3], raises: 0, last_inc: 0.0 };
        let street = if b0.len() == 5 { 0 } else { NO_STREET };
        g.build(st, 0, &Ctx { board: b0, dealt: [255, 255], street });
        Ok(g)
    }

    fn add_rivers(&mut self, board4: &[u8], turn: u8) {
        let rivers = sample_cards(board4, self.cfg.river_cards);
        for &(r, _) in &rivers {
            let mut b = board4.to_vec(); b.push(r);
            self.street_of.insert((turn, r), self.streets.len());
            let st = make_street(&self.deck, &b, r, &self.hands);
            self.streets.push(st);
        }
        self.samples.insert(turn, rivers);
    }

    /// 3 = the tree starts on the flop, 4 = turn, 5 = river.
    pub fn start_len(&self) -> usize { self.cfg.board.len() }
    pub fn starts_on_turn(&self) -> bool { self.cfg.board.len() == 4 }
    /// How many board cards are out at a node (3, 4 or 5).
    pub fn board_len(&self, node: &Node) -> usize { self.cfg.board.len() + node.dealt.iter().filter(|&&c| c != 255).count() }

    fn push(&mut self, kind: Kind, player: usize, st: &St, cx: &Ctx) -> usize {
        self.nodes.push(Node { kind, player, actions: vec![], children: vec![], cards: vec![], weights: vec![], remaining: 0.0, dealt: cx.dealt, street: cx.street,
            invested: st.inv, folded: st.folded, data: UnsafeCell::new(NodeData::default()), locked: None });
        self.nodes.len() - 1
    }

    /// Builds the node reached in state `st` with `first` the first candidate to act.
    fn build(&mut self, st: St, first: usize, cx: &Ctx) -> usize {
        let active = (0..3).filter(|&q| !st.folded[q]).count();
        if active == 1 { return self.push(Kind::FoldEnd, (0..3).find(|&q| !st.folded[q]).unwrap(), &st, cx); }
        let maxinv = st.inv.iter().cloned().fold(0.0, f64::max);
        let stack = self.cfg.stack;
        let mut actor = None;
        for k in 0..3 {
            let q = (first + k) % 3;
            if !st.folded[q] && (!st.acted[q] || st.inv[q] < maxinv - 1e-9) && st.inv[q] < stack - 1e-9 { actor = Some(q); break; }
        }
        let blen = cx.board.len();
        let p = match actor {
            Some(p) => p,
            None if blen < 5 => {
                // betting on this street is over: deal the next card
                let id = self.push(Kind::Chance, 0, &st, cx);
                let key = if blen == 3 { 254 } else { cx.dealt[0] };
                let deal = self.samples[&key].clone();
                let mut children = vec![];
                for &(c, _) in &deal {
                    let ns = St { inv: st.inv, folded: st.folded, acted: [false; 3], raises: 0, last_inc: 0.0 };
                    let mut b = cx.board.clone(); b.push(c);
                    let ncx = if blen == 3 { Ctx { board: b, dealt: [c, 255], street: NO_STREET } }
                        else { Ctx { board: b, dealt: [cx.dealt[0], c], street: self.street_of[&(cx.dealt[0], c)] } };
                    children.push(self.build(ns, 0, &ncx));
                }
                let node = &mut self.nodes[id];
                node.cards = deal.iter().map(|x| x.0).collect(); node.weights = deal.iter().map(|x| x.1).collect();
                node.remaining = (52 - blen) as f64; node.children = children;
                return id;
            }
            None => return self.push(Kind::Showdown, 0, &st, cx),
        };
        let id = self.push(Kind::Decision, p, &st, cx);
        let pot_now = self.cfg.pot + st.inv.iter().sum::<f64>();
        let to_call = maxinv - st.inv[p];
        let mut acts: Vec<Action> = vec![];
        let (cap, sizes) = match blen { 3 => (self.cfg.flop_max_raises, self.cfg.flop_bet_sizes.clone()), 4 => (self.cfg.turn_max_raises, self.cfg.turn_bet_sizes.clone()), _ => (self.cfg.max_raises, self.cfg.bet_sizes.clone()) };
        let can_raise = st.raises < cap && maxinv < stack - 1e-9;
        if to_call > 1e-9 {
            acts.push(Action::Fold); acts.push(Action::Call);
            if can_raise {
                let mut allin = false;
                for &f in &self.cfg.raise_sizes.clone() {
                    let mut t = maxinv + f * (pot_now + to_call);
                    t = t.max(maxinv + st.last_inc).round();
                    if t >= stack * 0.85 { allin = true; } else if !acts.contains(&Action::Raise(t)) { acts.push(Action::Raise(t)); }
                }
                if allin || (stack - maxinv) <= self.cfg.allin_spr * pot_now { acts.push(Action::AllIn(stack)); }
            }
        } else {
            acts.push(Action::Check);
            if can_raise {
                let mut allin = false;
                for &f in &sizes {
                    let t = (maxinv + (f * pot_now).max(1.0)).round();
                    if t >= stack * 0.85 { allin = true; } else if !acts.contains(&Action::Bet(t)) { acts.push(Action::Bet(t)); }
                }
                if allin || (stack - maxinv) <= self.cfg.allin_spr * pot_now { acts.push(Action::AllIn(stack)); }
            }
        }
        let mut children = vec![];
        for a in &acts {
            let mut s = st.clone();
            s.acted[p] = true;
            match a {
                Action::Fold => s.folded[p] = true,
                Action::Check => {}
                Action::Call => s.inv[p] = maxinv,
                Action::Bet(t) | Action::Raise(t) | Action::AllIn(t) => { s.last_inc = t - maxinv; s.inv[p] = *t; s.raises += 1; }
            }
            children.push(self.build(s, (p + 1) % 3, cx));
        }
        let n = acts.len() * self.hands[p].len();
        let alloc = self.allocated;
        let node = &mut self.nodes[id];
        node.actions = acts; node.children = children;
        if alloc { *node.data.get_mut() = NodeData { regrets: vec![0.0; n], strat_sum: vec![0.0; n] }; }
        id
    }

    /// Bytes of solver memory the tree needs (whether or not it has been reserved).
    pub fn memory_bytes(&self) -> usize {
        self.nodes.iter().filter(|n| n.kind == Kind::Decision).map(|n| n.actions.len() * self.hands[n.player].len() * 8 + n.locked.as_ref().map_or(0, |l| l.len() * 4)).sum::<usize>()
            + self.nodes.len() * 160
    }

    /// Rough work per solver step: showdown evaluations times hands, in millions.
    pub fn work_units(&self) -> f64 {
        let s = self.nodes.iter().filter(|n| n.kind == Kind::Showdown).count() as f64;
        let h = (self.hands[0].len() + self.hands[1].len() + self.hands[2].len()) as f64;
        s * h / 1e6
    }

    /// A reach vector for player `p` from this thread's spare list. Entries outside the
    /// player's own hands are zero; the caller must overwrite every one of the player's hands.
    fn take(&self, p: usize) -> Vec<f32> {
        POOL.with(|c| {
            let mut c = c.borrow_mut();
            if c.0 != self.id { c.0 = self.id; for q in 0..3 { c.1[q].clear(); } }
            c.1[p].pop()
        }).unwrap_or_else(|| vec![0f32; NH])
    }
    fn give(&self, p: usize, v: Vec<f32>) {
        POOL.with(|c| { let mut c = c.borrow_mut(); if c.0 == self.id { c.1[p].push(v); } });
    }

    /// Sum over all opponent hand pairs (h1, h2) that share no card with each other or with
    /// the hero hand, of r1[h1]*r2[h2]. Returned per hero hand (dense over `hero`'s hand list).
    /// `r1` must be the reach of player hero+1 and `r2` of player hero+2 (mod 3).
    pub fn reach_sum(&self, hero: usize, r1: &[f32], r2: &[f32]) -> Vec<f64> {
        let (o1, o2) = ((hero + 1) % 3, (hero + 2) % 3);
        TABLES.with(|t| {
            let mut t = t.borrow_mut(); let (m1, m2) = &mut *t;
            let (mut t1, mut t2, mut tq) = (0f64, 0f64, 0f64);
            let (mut c1, mut c2) = ([0f32; W], [0f32; W]);
            let mut cq = [0f64; 52];
            for &h in &self.hands[o1] {
                let h = h as usize; let x = r1[h]; if x == 0.0 { continue; }
                let (a, b) = self.deck.cards[h]; let (a, b) = (a as usize, b as usize);
                let q = x as f64 * r2[h] as f64;
                t1 += x as f64; tq += q; c1[a] += x; c1[b] += x; cq[a] += q; cq[b] += q;
                m1[a * W + b] = x; m1[b * W + a] = x;
            }
            for &h in &self.hands[o2] {
                let h = h as usize; let y = r2[h]; if y == 0.0 { continue; }
                let (a, b) = self.deck.cards[h]; let (a, b) = (a as usize, b as usize);
                t2 += y as f64; c2[a] += y; c2[b] += y;
                m2[a * W + b] = y; m2[b * W + a] = y;
            }
            let out = self.hands[hero].iter().map(|&h| {
                let h = h as usize; let (a, b) = self.deck.cards[h]; let (a, b) = (a as usize, b as usize);
                let x1 = t1 - c1[a] as f64 - c1[b] as f64 + r1[h] as f64;
                let x2 = t2 - c2[a] as f64 - c2[b] as f64 + r2[h] as f64;
                let (r1a, r1b, r2a, r2b) = (row(m1, a), row(m1, b), row(m2, a), row(m2, b));
                let mut bsum = kern_all(&c1, &c2, r1a, r1b, r2a, r2b) as f64;
                for c in [a, b] { bsum -= ((c1[c] - r1a[c] - r1b[c]) * (c2[c] - r2a[c] - r2b[c])) as f64; }
                let cc = tq - cq[a] - cq[b] + r1[h] as f64 * r2[h] as f64;
                x1 * x2 - bsum + cc
            }).collect();
            for &h in &self.hands[o1] { let (a, b) = self.deck.cards[h as usize]; let (a, b) = (a as usize, b as usize); m1[a * W + b] = 0.0; m1[b * W + a] = 0.0; }
            for &h in &self.hands[o2] { let (a, b) = self.deck.cards[h as usize]; let (a, b) = (a as usize, b as usize); m2[a * W + b] = 0.0; m2[b * W + a] = 0.0; }
            out
        })
    }

    /// Hero's counterfactual value at a showdown on final board `street`. `p1` (reach `r1`) is an
    /// opponent who is in the showdown; the other opponent (reach `r2`) is in it only if `three_way`.
    pub fn showdown(&self, street: usize, hero: usize, p1: usize, r1: &[f32], r2: &[f32], three_way: bool, pot: f64, invested: f64) -> Vec<f32> {
        let sd = &self.streets[street];
        let p2 = 3 - hero - p1;
        let (l1, l2, lh) = (&sd.sorted[p1], &sd.sorted[p2], &sd.sorted[hero]);
        let cards = &self.deck.cards;
        let rk = |h: u16| -> u16 { let (a, b) = cards[h as usize]; sd.rank[a as usize * W + b as usize] };
        let mut out = vec![0f32; self.hands[hero].len()];
        TABLES.with(|t| {
            let mut t = t.borrow_mut(); let (m1, m2) = &mut *t;
            // card-by-card reach tables and per-card sums over everything ("all")
            let mut ca = [[0f32; W]; 2];
            let (mut ta, mut caq) = ([0f64; 3], [0f64; 52]);
            for &h in l1 {
                let h = h as usize; let x = r1[h]; if x == 0.0 { continue; }
                let (a, b) = cards[h]; let (a, b) = (a as usize, b as usize);
                let q = x as f64 * r2[h] as f64;
                ta[0] += x as f64; ta[2] += q; ca[0][a] += x; ca[0][b] += x; caq[a] += q; caq[b] += q;
                m1[a * W + b] = x; m1[b * W + a] = x;
            }
            for &h in l2 {
                let h = h as usize; let y = r2[h]; if y == 0.0 { continue; }
                let (a, b) = cards[h]; let (a, b) = (a as usize, b as usize);
                ta[1] += y as f64; ca[1][a] += y; ca[1][b] += y;
                m2[a * W + b] = y; m2[b * W + a] = y;
            }
            // running sums over opponent hands strictly weaker than the hero hand ("lower") and tied with it
            let mut cl = [[0f32; W]; 2]; let (mut tl, mut clq) = ([0f64; 3], [0f64; 52]);
            let (mut i1, mut i2, mut ih) = (0usize, 0usize, 0usize);
            while ih < lh.len() {
                let s = rk(lh[ih]);
                while i1 < l1.len() && rk(l1[i1]) < s {
                    let h = l1[i1] as usize; i1 += 1; let x = r1[h]; if x == 0.0 { continue; }
                    let (a, b) = cards[h]; let (a, b) = (a as usize, b as usize);
                    let q = x as f64 * r2[h] as f64;
                    tl[0] += x as f64; tl[2] += q; cl[0][a] += x; cl[0][b] += x; clq[a] += q; clq[b] += q;
                }
                while i2 < l2.len() && rk(l2[i2]) < s {
                    let h = l2[i2] as usize; i2 += 1; let y = r2[h]; if y == 0.0 { continue; }
                    let (a, b) = cards[h]; let (a, b) = (a as usize, b as usize);
                    tl[1] += y as f64; cl[1][a] += y; cl[1][b] += y;
                }
                let mut ct = [[0f32; W]; 2]; let (mut tt, mut ctq) = ([0f64; 3], [0f64; 52]);
                let mut j = i1;
                while j < l1.len() && rk(l1[j]) == s {
                    let h = l1[j] as usize; j += 1; let x = r1[h]; if x == 0.0 { continue; }
                    let (a, b) = cards[h]; let (a, b) = (a as usize, b as usize);
                    let q = x as f64 * r2[h] as f64;
                    tt[0] += x as f64; tt[2] += q; ct[0][a] += x; ct[0][b] += x; ctq[a] += q; ctq[b] += q;
                }
                let mut j = i2;
                while j < l2.len() && rk(l2[j]) == s {
                    let h = l2[j] as usize; j += 1; let y = r2[h]; if y == 0.0 { continue; }
                    let (a, b) = cards[h]; let (a, b) = (a as usize, b as usize);
                    tt[1] += y as f64; ct[1][a] += y; ct[1][b] += y;
                }
                while ih < lh.len() && rk(lh[ih]) == s {
                    let h = lh[ih] as usize; ih += 1;
                    let pos = self.hpos[hero][h];
                    let (a, b) = cards[h]; let (a, b) = (a as usize, b as usize);
                    let me = [r1[h] as f64, r2[h] as f64, r1[h] as f64 * r2[h] as f64];
                    let xa = |k: usize| ta[k] - if k < 2 { ca[k][a] as f64 + ca[k][b] as f64 } else { caq[a] + caq[b] } + me[k];
                    let xl = |k: usize| tl[k] - if k < 2 { cl[k][a] as f64 + cl[k][b] as f64 } else { clq[a] + clq[b] };
                    let xt = |k: usize| tt[k] - if k < 2 { ct[k][a] as f64 + ct[k][b] as f64 } else { ctq[a] + ctq[b] } + me[k];
                    let k = Kern { s, ca1: &ca[0], ca2: &ca[1], cl1: &cl[0], cl2: &cl[1], ct1: &ct[0], ct2: &ct[1],
                        r1a: row(m1, a), r1b: row(m1, b), r2a: row(m2, a), r2b: row(m2, b), ka: rowu(&sd.rank, a), kb: rowu(&sd.rank, b) };
                    // the sum over cards must leave out the hero's own two cards
                    // (a tie can also come from the hero's own hand sitting in an opponent's range)
                    let ties = tt[0] != 0.0 || tt[1] != 0.0 || me[0] != 0.0 || me[1] != 0.0;
                    let mut bb = [0f32; 5];
                    if ties {
                        bb = kern(&k, three_way);
                        for c in [a, b] { let t = k.term(c, three_way); for i in 0..5 { bb[i] -= t[i]; } }
                    } else {
                        let v = kern_nt(&k, three_way);
                        bb[0] = v[0]; bb[1] = v[1];
                        for c in [a, b] { let t = k.term_no_ties(c, three_way); bb[0] -= t[0]; bb[1] -= t[1]; }
                    }
                    let b = |i: usize| bb[i] as f64;
                    let g_aa = xa(0) * xa(1) - b(0) + xa(2);
                    let share = if three_way {
                        let g_ll = xl(0) * xl(1) - b(1) + xl(2);
                        let g_tl = xt(0) * xl(1) - b(2);
                        let g_lt = xl(0) * xt(1) - b(3);
                        let g_tt = xt(0) * xt(1) - b(4) + xt(2);
                        g_ll + 0.5 * (g_tl + g_lt) + g_tt / 3.0
                    } else {
                        let g_la = xl(0) * xa(1) - b(1) + xl(2);
                        let g_ta = xt(0) * xa(1) - b(2) + xt(2);
                        g_la + 0.5 * g_ta
                    };
                    out[pos as usize] = (pot * share - invested * g_aa) as f32;
                }
            }
            for &h in l1 { let (a, b) = cards[h as usize]; let (a, b) = (a as usize, b as usize); m1[a * W + b] = 0.0; m1[b * W + a] = 0.0; }
            for &h in l2 { let (a, b) = cards[h as usize]; let (a, b) = (a as usize, b as usize); m2[a * W + b] = 0.0; m2[b * W + a] = 0.0; }
        });
        out
    }

    fn scaled_reach(&self, hero: usize, r: [&[f32]; 3], util: f64) -> Vec<f32> {
        let (o1, o2) = ((hero + 1) % 3, (hero + 2) % 3);
        self.reach_sum(hero, r[o1], r[o2]).iter().map(|&x| (util * x) as f32).collect()
    }

    fn strategy(&self, id: usize, avg: bool) -> Vec<f32> {
        let node = &self.nodes[id]; let na = node.actions.len(); let nh = self.hands[node.player].len();
        if let Some(l) = &node.locked { return l.clone(); }
        let data = unsafe { &*node.data.get() };
        let src = if avg { &data.strat_sum } else { &data.regrets };
        let mut s = vec![0f32; na * nh];
        for h in 0..nh {
            let mut tot = 0f32;
            for a in 0..na { let v = src[a * nh + h].max(0.0); s[a * nh + h] = v; tot += v; }
            if tot > 0.0 { for a in 0..na { s[a * nh + h] /= tot; } } else { for a in 0..na { s[a * nh + h] = 1.0 / na as f32; } }
        }
        s
    }
    pub fn avg_strategy(&self, id: usize) -> Vec<f32> { self.strategy(id, true) }

    /// The three reach vectors with every hand containing `card` removed.
    pub fn without_card(&self, r: [&[f32]; 3], card: u8) -> [Vec<f32>; 3] {
        let mut out = [r[0].to_vec(), r[1].to_vec(), r[2].to_vec()];
        for &h in &self.card_hands[card as usize] { for p in 0..3 { out[p][h as usize] = 0.0; } }
        out
    }

    /// Position (in hero's hand list) of hand (a, b) with the suit of card `c` swapped for another
    /// flush-irrelevant suit, so the swapped hand no longer holds `c`. Used when a sampled card
    /// stands in for its rank: a hand holding that exact card borrows the value of its twin.
    fn swapped(&self, hero: usize, a: u8, b: u8, c: u8, board: &[u8]) -> Option<usize> {
        let (rank, s) = (c / 4, c % 4);
        for s2 in 0..4u8 {
            if s2 == s || hot_suit(board, s2) { continue; }
            let sw = |x: u8| if x % 4 == s { x - s + s2 } else if x % 4 == s2 { x - s2 + s } else { x };
            let (a2, b2) = (sw(a), sw(b));
            if a2 == b2 || a2 == c || b2 == c || board.contains(&a2) || board.contains(&b2) || board.contains(&(rank * 4 + s2)) { continue; }
            let pos = self.hpos[hero][self.deck.idx[a2 as usize][b2 as usize] as usize];
            if pos != u32::MAX { return Some(pos as usize); }
        }
        None
    }

    /// Combines the per-card results at a chance node into one value per hero hand.
    /// Cards of flush-relevant suits are all dealt and count once each. For the other suits, the
    /// dealt cards of a rank stand in for every such card the hand does not block. With every
    /// card dealt this is the exact average.
    fn combine(&self, hero: usize, board: &[u8], cards: &[u8], parts: &[Vec<f32>]) -> Vec<f32> {
        let nhh = self.hands[hero].len();
        let rem = (52 - board.len()) as f64;
        // class = (rank, hot or cold suit); n = cards of the class off the board, k = dealt
        let cls = |c: u8| (c / 4) as usize * 2 + hot_suit(board, c % 4) as usize;
        let mut n = [0usize; 26]; let mut k = [0usize; 26];
        for c in 0..52u8 { if !board.contains(&c) { n[cls(c)] += 1; } }
        for &c in cards { k[cls(c)] += 1; }
        let nc = cards.len();
        let mut out = vec![0f32; nhh];
        let mut v = vec![0f64; nc]; let mut ok = vec![false; nc];
        for i in 0..nhh {
            let (a, b) = self.deck.cards[self.hands[hero][i] as usize];
            let mut usable = [0f64; 26];
            for j in 0..nc {
                let c = cards[j]; let cl = cls(c);
                ok[j] = false;
                if a == c || b == c {
                    if k[cl] == n[cl] { continue; }
                    match self.swapped(hero, a, b, c, board) { Some(pos) => { v[j] = parts[j][pos] as f64; } None => continue }
                } else { v[j] = parts[j][i] as f64; }
                ok[j] = true; usable[cl] += 1.0;
            }
            let (mut num, mut den) = (0f64, 0f64);
            for j in 0..nc {
                if !ok[j] { continue; }
                let cl = cls(cards[j]);
                let held = (cls(a) == cl) as usize + (cls(b) == cl) as usize;
                let w = n[cl].saturating_sub(held) as f64 / usable[cl];
                num += w * v[j]; den += w;
            }
            if den > 0.0 { out[i] = (num / den * (rem - 2.0) / (rem - 6.0)) as f32; }
        }
        out
    }

    /// The board at a node: starting cards plus those dealt inside the tree.
    pub fn board_at(&self, dealt: [u8; 2]) -> Vec<u8> {
        let mut b = self.cfg.board.clone();
        for &c in &dealt { if c != 255 { b.push(c); } }
        b
    }

    /// Counterfactual values for `hero` (dense over hero's hands) below node `id`.
    pub fn walk(&self, id: usize, hero: usize, r: [&[f32]; 3], mode: Mode) -> Vec<f32> {
        let node = &self.nodes[id];
        let inv = node.invested; let folded = node.folded;
        let pot = self.cfg.pot + inv.iter().sum::<f64>();
        let nhh = self.hands[hero].len();
        match node.kind {
            Kind::FoldEnd => {
                let util = if node.player == hero { pot - inv[hero] } else { -inv[hero] };
                return self.scaled_reach(hero, r, util);
            }
            Kind::Showdown => {
                if folded[hero] { return self.scaled_reach(hero, r, -inv[hero]); }
                let (o1, o2) = ((hero + 1) % 3, (hero + 2) % 3);
                return if !folded[o1] && !folded[o2] { self.showdown(node.street, hero, o1, r[o1], r[o2], true, pot, inv[hero]) }
                    else if !folded[o1] { self.showdown(node.street, hero, o1, r[o1], r[o2], false, pot, inv[hero]) }
                    else { self.showdown(node.street, hero, o2, r[o2], r[o1], false, pot, inv[hero]) };
            }
            Kind::Chance => {
                if folded[hero] { return self.scaled_reach(hero, r, -inv[hero]); }
                let parts: Vec<Vec<f32>> = (0..node.cards.len()).into_par_iter().map(|k| {
                    let card = node.cards[k];
                    let mut nr = [self.take(0), self.take(1), self.take(2)];
                    for p in 0..3 { for &h in &self.hands[p] { nr[p][h as usize] = r[p][h as usize]; } }
                    for &h in &self.card_hands[card as usize] { for p in 0..3 { nr[p][h as usize] = 0.0; } }
                    let mut v = self.walk(node.children[k], hero, [&nr[0], &nr[1], &nr[2]], mode);
                    for &h in &self.card_hands[card as usize] { let pos = self.hpos[hero][h as usize]; if pos != u32::MAX { v[pos as usize] = 0.0; } }
                    let [n0, n1, n2] = nr; self.give(0, n0); self.give(1, n1); self.give(2, n2);
                    v
                }).collect();
                return self.combine(hero, &self.board_at(node.dealt), &node.cards, &parts);
            }
            Kind::Decision => {}
        }
        let p = node.player;
        let na = node.actions.len();
        let sigma = self.strategy(id, !matches!(mode, Mode::Train(_)));
        let nhp = self.hands[p].len();
        if p != hero {
            let mut val = vec![0f32; nhh];
            for a in 0..na {
                let mut nr = self.take(p); let mut any = false;
                for (k, &h) in self.hands[p].iter().enumerate() {
                    let v = r[p][h as usize] * sigma[a * nhp + k]; nr[h as usize] = v; if v > 0.0 { any = true; }
                }
                if any {
                    let mut rr = r; rr[p] = &nr;
                    let v = self.walk(node.children[a], hero, rr, mode);
                    for i in 0..nhh { val[i] += v[i]; }
                }
                self.give(p, nr);
            }
            return val;
        }
        // hero's decision
        let mut vals: Vec<Vec<f32>> = Vec::with_capacity(na);
        for a in 0..na {
            if node.actions[a] == Action::Fold {
                // once hero folds, the rest of the hand cannot change hero's result
                vals.push(self.scaled_reach(hero, r, -inv[hero]));
                continue;
            }
            let mut nr = self.take(p);
            for (k, &h) in self.hands[p].iter().enumerate() { nr[h as usize] = r[p][h as usize] * sigma[a * nhp + k]; }
            let mut rr = r; rr[p] = &nr;
            vals.push(self.walk(node.children[a], hero, rr, mode));
            self.give(p, nr);
        }
        let mut val = vec![0f32; nhh];
        match mode {
            Mode::BestResponse if node.locked.is_none() || self.br_ignores_locks => { for i in 0..nhh { let mut m = f32::MIN; for a in 0..na { m = m.max(vals[a][i]); } val[i] = m; } }
            _ => { for a in 0..na { for i in 0..nhh { val[i] += sigma[a * nhh + i] * vals[a][i]; } } }
        }
        if let (Mode::Train(t), true) = (mode, node.locked.is_none()) {
            let tf = t as f32;
            let (pos, neg, sd) = (tf.powf(1.5) / (tf.powf(1.5) + 1.0), 0.5f32, (tf / (tf + 1.0)).powi(2));
            // SAFETY: a node is visited by one thread at a time (river subtrees are disjoint).
            let data = unsafe { &mut *node.data.get() };
            for a in 0..na { for i in 0..nhh {
                let j = a * nhh + i;
                let rg = data.regrets[j]; let rg = rg * if rg > 0.0 { pos } else { neg };
                data.regrets[j] = rg + vals[a][i] - val[i];
                data.strat_sum[j] = data.strat_sum[j] * sd + r[p][self.hands[p][i] as usize] * sigma[j];
            } }
        }
        val
    }

    /// Unnormalised showdown share for hero, averaged over the cards still to come.
    fn equity_raw(&self, dealt: [u8; 2], hero: usize, r: [&[f32]; 3], folded: [bool; 3]) -> Vec<f32> {
        let (o1, o2) = ((hero + 1) % 3, (hero + 2) % 3);
        let blen = self.cfg.board.len() + dealt.iter().filter(|&&c| c != 255).count();
        if blen == 5 {
            let s = self.street_of[&(dealt[0], dealt[1])];
            return if !folded[o1] && !folded[o2] { self.showdown(s, hero, o1, r[o1], r[o2], true, 1.0, 0.0) }
                else if !folded[o1] { self.showdown(s, hero, o1, r[o1], r[o2], false, 1.0, 0.0) }
                else { self.showdown(s, hero, o2, r[o2], r[o1], false, 1.0, 0.0) };
        }
        let key = if blen == 3 { 254 } else { dealt[0] };
        let deal = &self.samples[&key];
        let cards: Vec<u8> = deal.iter().map(|x| x.0).collect();
        let parts: Vec<Vec<f32>> = (0..cards.len()).into_par_iter().map(|k| {
            let card = cards[k];
            let nr = self.without_card(r, card);
            let nd = if blen == 3 { [card, 255] } else { [dealt[0], card] };
            let mut v = self.equity_raw(nd, hero, [&nr[0], &nr[1], &nr[2]], folded);
            for &h in &self.card_hands[card as usize] { let pos = self.hpos[hero][h as usize]; if pos != u32::MAX { v[pos as usize] = 0.0; } }
            v
        }).collect();
        self.combine(hero, &self.board_at(dealt), &cards, &parts)
    }

    /// Per-hand chance of winning the pot at showdown (ties count as a share) against the
    /// opponents still in the hand, given everyone's current reach. Dense over hero's hands.
    /// `dealt` says which cards have come inside the tree; later cards are averaged over.
    pub fn equity(&self, dealt: [u8; 2], hero: usize, r: [&[f32]; 3], folded: [bool; 3]) -> Vec<f32> {
        let (o1, o2) = ((hero + 1) % 3, (hero + 2) % 3);
        let z = self.reach_sum(hero, r[o1], r[o2]);
        if folded[o1] && folded[o2] { return vec![1.0; z.len()]; }
        let v = self.equity_raw(dealt, hero, r, folded);
        v.iter().zip(z.iter()).map(|(&a, &b)| if b > 0.0 { (a as f64 / b).clamp(0.0, 1.0) as f32 } else { 0.0 }).collect()
    }

    pub fn iterate(&self, t: u32) {
        let rg = &self.cfg.ranges;
        for hero in 0..3 { self.walk(0, hero, [&rg[0], &rg[1], &rg[2]], Mode::Train(t)); }
    }

    /// Returns (EV, best-response EV) per player, in chips, under the average strategies.
    pub fn evaluate(&self) -> [(f64, f64); 3] {
        let rg = &self.cfg.ranges;
        let mut out = [(0.0, 0.0); 3];
        for hero in 0..3 {
            let z = self.reach_sum(hero, &rg[(hero + 1) % 3], &rg[(hero + 2) % 3]);
            let ev = self.walk(0, hero, [&rg[0], &rg[1], &rg[2]], Mode::Average);
            let br = self.walk(0, hero, [&rg[0], &rg[1], &rg[2]], Mode::BestResponse);
            let (mut zs, mut e, mut b) = (0f64, 0f64, 0f64);
            for (i, &h) in self.hands[hero].iter().enumerate() {
                let w = rg[hero][h as usize] as f64;
                zs += w * z[i]; e += w * ev[i] as f64; b += w * br[i] as f64;
            }
            out[hero] = (e / zs, b / zs);
        }
        out
    }
}

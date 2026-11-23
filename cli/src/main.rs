//! Pontiac Solver command line: solve a heads-up postflop spot from a JSON file, save the solve,
//! and query any node of a saved solve without solving again. AGPL-3.0-or-later.
#[allow(dead_code)]
#[path = "../../app/src-tauri/src/study_core.rs"]
mod study_core;
mod three;

use postflop_solver::*;
use serde_json::{json, Map, Value};
use std::time::Instant;
use study_core::{collect_locks, Leak};

const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Chips per big blind inside the engine (it works in whole chips).
pub const CHIPS: f64 = 100.0;
const RANKS: &str = "23456789TJQKA";
const SUITS: &str = "cdhs";
pub const SEATS: [&str; 9] = ["SB", "BB", "UTG", "UTG1", "UTG2", "LJ", "HJ", "CO", "BTN"];

pub fn version_string() -> String { format!("pontiac {VERSION} (two-player engine: postflop-solver nodelock fork; three-player engine: pontiac multiway; push/fold and ICM calculators)") }

fn card_name(c: u8) -> String { format!("{}{}", RANKS.as_bytes()[(c / 4) as usize] as char, SUITS.as_bytes()[(c % 4) as usize] as char) }
fn hand_name(h: (u8, u8)) -> String { let (lo, hi) = if h.0 < h.1 { (h.0, h.1) } else { (h.1, h.0) }; format!("{}{}", card_name(hi), card_name(lo)) }
fn class_name(h: (u8, u8)) -> String {
    let (a, b) = (h.0 / 4, h.1 / 4);
    let (hi, lo) = if a >= b { (a, b) } else { (b, a) };
    let r = |x: u8| RANKS.as_bytes()[x as usize] as char;
    if hi == lo { format!("{}{}", r(hi), r(lo)) } else if h.0 % 4 == h.1 % 4 { format!("{}{}s", r(hi), r(lo)) } else { format!("{}{}o", r(hi), r(lo)) }
}
fn parse_card(s: &str) -> Option<u8> {
    let b: Vec<char> = s.chars().collect();
    if b.len() != 2 { return None; }
    let r = RANKS.find(b[0].to_ascii_uppercase())?; let su = SUITS.find(b[1].to_ascii_lowercase())?;
    Some((r * 4 + su) as u8)
}
pub fn round(x: f64, d: i32) -> f64 { if !x.is_finite() { return f64::NAN; } let m = 10f64.powi(d); (x * m).round() / m }

// ------------------------------------------------------------------ spot file

struct Spot {
    names: [String; 2], // [out of position, in position]
    raw: Value,
    card: CardConfig,
    tree: TreeConfig,
    target_pct: f64,
    max_seconds: f64,
    max_iterations: u32,
    threads: usize,
}

fn sizes(v: Option<&Value>, what: &str, as_multiplier: bool) -> Result<String, String> {
    let Some(v) = v else { return Ok(String::new()) };
    let arr = v.as_array().ok_or(format!("{what} must be a list"))?;
    let mut out = vec![];
    for x in arr {
        if let Some(s) = x.as_str() { out.push(s.to_string()); continue; }
        let f = x.as_f64().ok_or(format!("{what}: sizes must be numbers or strings"))?;
        if f <= 0.0 { return Err(format!("{what}: sizes must be positive")); }
        out.push(if as_multiplier { format!("{}x", round(f, 2)) } else { format!("{}%", round(f * 100.0, 1)) });
    }
    Ok(out.join(", "))
}

fn street_sizes(tree: &Value, street: &str) -> Result<BetSizeOptions, String> {
    let st = tree.get(street).unwrap_or(&Value::Null);
    let bet = sizes(st.get("bet"), &format!("bet_tree.{street}.bet"), false)?;
    let raise = sizes(st.get("raise"), &format!("bet_tree.{street}.raise"), true)?;
    BetSizeOptions::try_from((bet.as_str(), raise.as_str())).map_err(|e| format!("bet_tree.{street}: {e}"))
}

fn parse_spot(raw: &Value) -> Result<Spot, String> {
    let players: Vec<String> = raw.get("players").and_then(|p| p.as_array()).ok_or("\"players\" must be a list of two names")?
        .iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect();
    if players.len() != 2 || players[0] == players[1] { return Err("\"players\" must be two or three different names, e.g. [\"SB\", \"BB\"]".into()); }
    // who acts first after the flop
    let oop = if let Some(o) = raw.get("oop").and_then(|x| x.as_str()) {
        players.iter().position(|p| p == o).ok_or("\"oop\" must be one of the players")?
    } else {
        let seat = |n: &str| SEATS.iter().position(|s| s.eq_ignore_ascii_case(n));
        match (seat(&players[0]), seat(&players[1])) {
            // heads-up: the small blind is the button and acts last after the flop
            (Some(a), Some(b)) if a + b == 1 => if a == 1 { 0 } else { 1 },
            (Some(a), Some(b)) => if a < b { 0 } else { 1 },
            _ => 0,
        }
    };
    let names = [players[oop].clone(), players[1 - oop].clone()];
    let board: String = raw.get("board").and_then(|b| b.as_str()).ok_or("\"board\" is required, e.g. \"Qh8c3h\"")?.chars().filter(|c| !c.is_whitespace()).collect();
    if board.len() < 6 || board.len() > 10 || board.len() % 2 != 0 { return Err("\"board\" needs 3, 4 or 5 cards".into()); }
    let flop = flop_from_str(&board[..6]).map_err(|e| format!("board: {e}"))?;
    let turn = if board.len() >= 8 { card_from_str(&board[6..8]).map_err(|e| format!("board: {e}"))? } else { NOT_DEALT };
    let river = if board.len() == 10 { card_from_str(&board[8..10]).map_err(|e| format!("board: {e}"))? } else { NOT_DEALT };
    let state = match board.len() { 6 => BoardState::Flop, 8 => BoardState::Turn, _ => BoardState::River };
    let ranges = raw.get("ranges").ok_or("\"ranges\" is required")?;
    let range = |n: &str| -> Result<Range, String> {
        ranges.get(n).and_then(|r| r.as_str()).ok_or(format!("ranges.{n} is missing"))?.parse().map_err(|e: String| format!("ranges.{n}: {e}"))
    };
    let num = |k: &str| raw.get(k).and_then(|x| x.as_f64());
    let pot = num("pot_bb").ok_or("\"pot_bb\" is required")?;
    let stack = num("effective_stack_bb").ok_or("\"effective_stack_bb\" is required (the stack behind at the start of the spot)")?;
    if pot <= 0.0 || stack <= 0.0 { return Err("pot_bb and effective_stack_bb must be positive".into()); }
    let bt = raw.get("bet_tree").cloned().unwrap_or(json!({}));
    // shared tree, or one per player: {"SB": {...}, "BB": {...}}
    let per = |i: usize| -> Value { bt.get(&names[i]).cloned().unwrap_or(bt.clone()) };
    let (t0, t1) = (per(0), per(1));
    let thr = |k: &str, d: f64| -> f64 {
        bt.get(k).and_then(|x| x.as_f64())
            .or_else(|| ["flop", "turn", "river"].iter().find_map(|s| t0.get(*s).and_then(|x| x.get(k)).and_then(|x| x.as_f64()))).unwrap_or(d)
    };
    let donk = |street: &str| -> Result<Option<DonkSizeOptions>, String> {
        match t0.get(street).and_then(|s| s.get("donk")) {
            Some(v) => { let s = sizes(Some(v), "donk", false)?; Ok(Some(DonkSizeOptions::try_from(s.as_str()).map_err(|e| format!("bet_tree.{street}.donk: {e}"))?)) }
            None => Ok(None),
        }
    };
    let (rake_rate, rake_cap) = match raw.get("rake") {
        Some(Value::Number(n)) => (n.as_f64().unwrap_or(0.0), 0.0),
        Some(Value::Object(o)) => (o.get("rate").and_then(|x| x.as_f64()).unwrap_or(0.0), o.get("cap_bb").and_then(|x| x.as_f64()).unwrap_or(0.0) * CHIPS),
        _ => (0.0, 0.0),
    };
    if rake_rate > 0.0 && rake_cap == 0.0 { return Err("rake: give {\"rate\": 0.05, \"cap_bb\": 3} (a rate needs a cap)".into()); }
    let tree = TreeConfig {
        initial_state: state,
        starting_pot: (pot * CHIPS).round() as i32,
        effective_stack: (stack * CHIPS).round() as i32,
        rake_rate, rake_cap,
        flop_bet_sizes: [street_sizes(&t0, "flop")?, street_sizes(&t1, "flop")?],
        turn_bet_sizes: [street_sizes(&t0, "turn")?, street_sizes(&t1, "turn")?],
        river_bet_sizes: [street_sizes(&t0, "river")?, street_sizes(&t1, "river")?],
        turn_donk_sizes: donk("turn")?, river_donk_sizes: donk("river")?,
        add_allin_threshold: thr("allin_threshold", 0.67),
        force_allin_threshold: thr("force_allin_threshold", 0.15),
        merging_threshold: thr("merging_threshold", 0.1),
    };
    Ok(Spot {
        card: CardConfig { range: [range(&names[0])?, range(&names[1])?], flop, turn, river },
        tree, names, raw: raw.clone(),
        target_pct: num("target_exploitability_pct").unwrap_or(0.5),
        max_seconds: num("max_seconds").unwrap_or(600.0),
        max_iterations: num("max_iterations").unwrap_or(2000.0) as u32,
        threads: num("threads").unwrap_or(0.0) as usize,
    })
}

fn build(spot: &Spot) -> Result<PostFlopGame, String> {
    let tree = ActionTree::new(spot.tree.clone())?;
    let mut game = PostFlopGame::with_config(spot.card.clone(), tree)?;
    if let Some(icm) = icm_setup(spot)? { game.set_custom_payoffs(icm.table(&game.terminal_amounts()))?; }
    Ok(game)
}

use pushfold::icm::PotIcm as Icm;

fn icm_setup(spot: &Spot) -> Result<Option<Icm>, String> {
    let Some(v) = spot.raw.get("icm") else { return Ok(None) };
    if v.is_null() { return Ok(None); }
    let help = "icm: give {\"payouts\": [50, 30, 20], \"stacks_bb\": {\"SB\": 18, \"BB\": 40}, \"others_bb\": [25, 12]} (stacks behind at the start of the spot; others are players not in the hand)";
    let list = |k: &str| -> Result<Vec<f64>, String> { match v.get(k) { None => Ok(vec![]), Some(x) => x.as_array().ok_or(help)?.iter().map(|y| y.as_f64().filter(|z| *z >= 0.0).ok_or(help.to_string())).collect() } };
    let payouts = list("payouts")?; let others: Vec<f64> = list("others_bb")?.iter().map(|x| x * CHIPS).collect();
    if payouts.is_empty() || payouts.iter().sum::<f64>() <= 0.0 { return Err(help.into()); }
    if others.len() > 7 { return Err("icm: at most 7 other players".into()); }
    let eff = spot.tree.effective_stack as f64;
    let mut behind = [eff; 2];
    if let Some(st) = v.get("stacks_bb") {
        for p in 0..2 { behind[p] = st.get(&spot.names[p]).and_then(|x| x.as_f64()).ok_or(format!("icm.stacks_bb needs both players: {} and {}", spot.names[0], spot.names[1]))? * CHIPS; }
    }
    if (behind[0].min(behind[1]) - eff).abs() > 0.5 { return Err("icm.stacks_bb: the smaller of the two stacks must equal effective_stack_bb".into()); }
    if spot.tree.rake_rate > 0.0 { return Err("icm cannot be combined with rake".into()); }
    let icm = Icm::new(behind, others, payouts, spot.tree.starting_pot as f64)?;
    Ok(Some(icm))
}

// ------------------------------------------------------------------ node paths

/// Where a path leads, plus what is needed to name actions there.
struct Cursor { history: Vec<usize>, street_start: [i32; 2] }

fn pot_now(game: &PostFlopGame) -> i32 { let t = game.total_bet_amount(); game.tree_config().starting_pot + t[0] + t[1] }

/// Canonical action names at the current node: check, fold, call, allin, bet0.33 (share of the
/// pot), raise2.5 (multiple of the bet being raised). Ties get the amount appended.
fn action_names(game: &PostFlopGame, street_start: [i32; 2]) -> Vec<String> {
    let acts = game.available_actions();
    let t = game.total_bet_amount();
    let me = game.current_player();
    let street = [t[0] - street_start[0], t[1] - street_start[1]];
    let pot = pot_now(game) as f64;
    let mut names: Vec<String> = acts.iter().map(|a| match a {
        Action::Fold => "fold".into(), Action::Check => "check".into(), Action::Call => "call".into(),
        Action::AllIn(_) => "allin".into(),
        Action::Bet(x) => format!("bet{}", round(*x as f64 / pot, 2)),
        Action::Raise(x) => format!("raise{}", round(*x as f64 / street[1 - me].max(1) as f64, 1)),
        _ => "?".into(),
    }).collect();
    for i in 0..names.len() {
        if names.iter().filter(|n| **n == names[i]).count() > 1 {
            if let Action::Bet(x) | Action::Raise(x) = acts[i] { names[i] = format!("{}={}bb", names[i].trim_end_matches(char::is_numeric).trim_end_matches('.'), round(x as f64 / CHIPS, 2)); }
        }
    }
    names
}

fn action_amount(a: &Action) -> f64 { match a { Action::Bet(x) | Action::Raise(x) | Action::AllIn(x) => *x as f64 / CHIPS, _ => 0.0 } }

/// Walks a path like "flop/BB_check/SB_bet0.33/BB_call/turn=7h/BB_check". Street words and
/// player prefixes are optional; a bet or raise matches the closest available size.
fn walk(game: &mut PostFlopGame, names: &[String; 2], path: &str) -> Result<Cursor, String> {
    game.back_to_root();
    let mut cur = Cursor { history: vec![], street_start: [0, 0] };
    for tok in path.split('/').map(|t| t.trim()).filter(|t| !t.is_empty()) {
        let low = tok.to_ascii_lowercase();
        if ["flop", "turn", "river", "root"].contains(&low.as_str()) { continue; }
        if game.is_terminal_node() { return Err(format!("path '{path}': the hand is already over before '{tok}'")); }
        if game.is_chance_node() {
            let cs = low.split(['=', ':']).last().unwrap_or("");
            let card = parse_card(cs).ok_or(format!("path '{path}': expected a card such as turn=7h, got '{tok}'"))?;
            if game.possible_cards() & (1u64 << card) == 0 { return Err(format!("path '{path}': card {cs} cannot come here")); }
            game.play(card as usize); cur.history.push(card as usize);
            cur.street_start = game.total_bet_amount();
            continue;
        }
        let me = game.current_player();
        let mut spec = tok;
        for (i, n) in names.iter().enumerate() {
            if let Some(rest) = tok.strip_prefix(&format!("{n}_")) {
                if i != me { return Err(format!("path '{path}': it is {}'s turn at '{tok}', not {n}'s", names[me])); }
                spec = rest;
            }
        }
        let spec = spec.to_ascii_lowercase();
        let acts = game.available_actions();
        let canon = action_names(game, cur.street_start);
        let idx = if let Some(i) = canon.iter().position(|c| *c == spec) { Some(i) }
            else if let Some(n) = spec.strip_prefix('#') { n.parse::<usize>().ok().filter(|&i| i < acts.len()) }
            else {
                let kind = spec.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.' || c == '=' || c == 'b' || c == 'x' || c == '%');
                let numtxt: String = spec.chars().filter(|c| c.is_ascii_digit() || *c == '.').collect();
                let want: Option<f64> = numtxt.parse().ok();
                let in_bb = spec.contains("bb");
                let t = game.total_bet_amount();
                let facing = (t[1 - me] - cur.street_start[1 - me]).max(1) as f64;
                let pot = pot_now(game) as f64;
                let mut best: Option<(usize, f64)> = None;
                for (i, a) in acts.iter().enumerate() {
                    let (ok, val) = match (kind, a) {
                        ("bet", Action::Bet(x)) => (true, if in_bb { *x as f64 / CHIPS } else { *x as f64 / pot }),
                        ("raise", Action::Raise(x)) => (true, if in_bb { *x as f64 / CHIPS } else { *x as f64 / facing }),
                        ("allin", Action::AllIn(_)) | ("jam", Action::AllIn(_)) | ("shove", Action::AllIn(_)) => (true, f64::NAN),
                        // an all-in also answers "bet 1.2" or "raise 3" when it is the closest size
                        ("bet", Action::AllIn(x)) if t[1 - me] == t[me] => (true, if in_bb { *x as f64 / CHIPS } else { *x as f64 / pot }),
                        ("raise", Action::AllIn(x)) if t[1 - me] != t[me] => (true, if in_bb { *x as f64 / CHIPS } else { *x as f64 / facing }),
                        _ => (false, 0.0),
                    };
                    if !ok { continue; }
                    if val.is_nan() { best = Some((i, 0.0)); break; }
                    // without a number, take the smallest size; with one, it must be within 25%
                    let d = match want { Some(w) => { let d = (w - val).abs(); if d > 0.25 * w.max(0.01) { continue; } d } None => i as f64 * 1e-9 + if matches!(a, Action::AllIn(_)) { 1.0 } else { 0.0 } };
                    if best.map_or(true, |b| d < b.1) { best = Some((i, d)); }
                }
                best.map(|b| b.0)
            };
        let i = idx.ok_or(format!("path '{path}': '{tok}' is not available here; choices are: {}", canon.join(", ")))?;
        game.play(i); cur.history.push(i);
    }
    Ok(cur)
}

// ------------------------------------------------------------------ reports

fn street_of(game: &PostFlopGame) -> &'static str { match game.current_board().len() { 3 => "flop", 4 => "turn", _ => "river" } }

/// Everything known about the current node. `detail`: "combo" adds every exact hand.
fn report(game: &mut PostFlopGame, names: &[String; 2], cur: &Cursor, path: &str, detail: &str, only: Option<&str>) -> Value {
    let board: Vec<String> = game.current_board().iter().map(|&c| card_name(c)).collect();
    let pot = pot_now(game) as f64 / CHIPS;
    let mut o = Map::new();
    o.insert("path".into(), json!(path)); o.insert("street".into(), json!(street_of(game)));
    o.insert("board".into(), json!(board.join(""))); o.insert("pot_bb".into(), json!(round(pot, 2)));
    if game.is_terminal_node() { o.insert("kind".into(), json!("terminal")); return Value::Object(o); }
    if game.is_chance_node() {
        o.insert("kind".into(), json!("deal"));
        let pc = game.possible_cards();
        let cards: Vec<String> = (0..52u8).filter(|c| pc & (1u64 << c) != 0).map(card_name).collect();
        let next = if board.len() == 3 { "turn" } else { "river" };
        o.insert("next_paths".into(), json!(cards.iter().map(|c| format!("{path}/{next}={c}")).collect::<Vec<_>>()));
        return Value::Object(o);
    }
    game.cache_normalized_weights();
    let me = game.current_player();
    let acts = game.available_actions();
    let canon = action_names(game, cur.street_start);
    let na = acts.len();
    let hands = game.private_cards(me).to_vec();
    let n = hands.len();
    let strat = game.strategy();
    let rawr = game.weights(me).to_vec();
    let mut w = game.normalized_weights(me).to_vec();
    // if the opponent never takes this line the weights are all zero: fall back to the player's own range
    let reached = w.iter().any(|&x| x > 0.0);
    if !reached { w = rawr.clone(); }
    let eq = game.equity(me);
    let ev = game.expected_values(me);
    let total: f64 = w.iter().map(|&x| x as f64).sum();
    o.insert("kind".into(), json!("decision")); o.insert("player".into(), json!(names[me]));
    o.insert("locked".into(), json!(game.current_locking_strategy().is_some()));
    o.insert("reached".into(), json!(reached));
    let mut freq = vec![0f64; na]; let (mut evs, mut eqs) = (0f64, 0f64);
    for h in 0..n { let x = w[h] as f64; evs += x * ev[h] as f64; eqs += x * eq[h] as f64; for a in 0..na { freq[a] += x * strat[a * n + h] as f64; } }
    let actions: Vec<Value> = (0..na).map(|a| json!({"name": canon[a], "amount_bb": round(action_amount(&acts[a]), 2),
        "frequency": round(if total > 0.0 { freq[a] / total } else { 0.0 }, 4), "path": format!("{}/{}_{}", path, names[me], canon[a])})).collect();
    o.insert("actions".into(), json!(actions));
    o.insert("range_ev_bb".into(), json!(round(if total > 0.0 { evs / total / CHIPS } else { 0.0 }, 3)));
    o.insert("range_equity".into(), json!(round(if total > 0.0 { eqs / total } else { 0.0 }, 4)));
    o.insert("range_combos".into(), json!(round(rawr.iter().map(|&x| x as f64).sum(), 2)));
    // by hand class (AKs, 77, T9o ...) and optionally by exact hand
    let mut classes: std::collections::BTreeMap<String, (f64, f64, Vec<f64>, f64, f64)> = Default::default();
    let mut combos = Map::new();
    let want = only.map(|s| s.to_string());
    for h in 0..n {
        if rawr[h] <= 1e-6 { continue; }
        let (cn, hn) = (class_name(hands[h]), hand_name(hands[h]));
        if let Some(wt) = &want {
            let alt = format!("{}{}", &hn[2..4], &hn[0..2]);
            if !wt.eq_ignore_ascii_case(&cn) && !wt.eq_ignore_ascii_case(&hn) && !wt.eq_ignore_ascii_case(&alt) { continue; }
        }
        let x = w[h] as f64;
        let e = classes.entry(cn).or_insert((0.0, 0.0, vec![0.0; na], 0.0, 0.0));
        e.0 += x; e.1 += rawr[h] as f64; e.3 += x * ev[h] as f64; e.4 += x * eq[h] as f64;
        for a in 0..na { e.2[a] += x * strat[a * n + h] as f64; }
        if detail == "combo" || want.is_some() {
            let fr: Map<String, Value> = (0..na).map(|a| (canon[a].clone(), json!(round(strat[a * n + h] as f64, 4)))).collect();
            combos.insert(hn, json!({"weight": round(rawr[h] as f64, 4), "freqs": fr, "ev_bb": round(ev[h] as f64 / CHIPS, 3), "equity": round(eq[h] as f64, 4)}));
        }
    }
    let by_class: Map<String, Value> = classes.into_iter().map(|(k, v)| {
        let d = if v.0 > 0.0 { v.0 } else { 1.0 };
        let fr: Map<String, Value> = (0..na).map(|a| (canon[a].clone(), json!(round(v.2[a] / d, 4)))).collect();
        (k, json!({"combos": round(v.1, 3), "freqs": fr, "ev_bb": round(v.3 / d / CHIPS, 3), "equity": round(v.4 / d, 4)}))
    }).collect();
    o.insert("by_class".into(), Value::Object(by_class));
    if !combos.is_empty() { o.insert("by_combo".into(), Value::Object(combos)); }
    let lf = line_frequency(game, &cur.history);
    o.insert("line_frequency".into(), json!(round(lf, 5)));
    Value::Object(o)
}

/// How often play reaches the node: the product of the range-wide frequency of each action taken.
fn line_frequency(game: &mut PostFlopGame, history: &[usize]) -> f64 {
    game.back_to_root();
    let mut p = 1.0f64;
    for &h in history {
        if !game.is_chance_node() {
            game.cache_normalized_weights();
            let me = game.current_player();
            let n = game.private_cards(me).len();
            let (st, w) = (game.strategy(), game.normalized_weights(me).to_vec());
            let (mut a, mut t) = (0f64, 0f64);
            for i in 0..n { a += w[i] as f64 * st[h * n + i] as f64; t += w[i] as f64; }
            p *= if t > 0.0 { a / t } else { 0.0 };
        }
        game.play(h);
    }
    p
}

/// Canonical paths of every decision on the starting street (stops where a card is dealt).
fn first_street_paths(game: &mut PostFlopGame, names: &[String; 2], limit: usize) -> Vec<String> {
    let start = street_of(game).to_string();
    let mut out = vec![];
    let mut stack: Vec<(Vec<usize>, String)> = vec![(vec![], start)];
    while let Some((hist, path)) = stack.pop() {
        if out.len() >= limit { break; }
        game.apply_history(&hist);
        if game.is_terminal_node() || game.is_chance_node() { continue; }
        out.push(path.clone());
        let canon = action_names(game, [0, 0]);
        let me = game.current_player();
        for i in (0..canon.len()).rev() { let mut h = hist.clone(); h.push(i); stack.push((h, format!("{}/{}_{}", path, names[me], canon[i]))); }
    }
    game.back_to_root();
    out
}

// ------------------------------------------------------------------ solving

struct Solved { iterations: u32, exploit: f32, seconds: f64, stopped: &'static str }

fn run(game: &mut PostFlopGame, spot: &Spot, budget: f64, label: &str) -> Solved {
    let t0 = Instant::now();
    let pot = spot.tree.starting_pot as f32;
    let target = pot * spot.target_pct as f32 / 100.0;
    let (mut expl, mut it, mut stopped) = (f32::MAX, 0u32, "max_iterations");
    while it < spot.max_iterations {
        solve_step(game, it); it += 1;
        if it % 10 == 0 || it == spot.max_iterations {
            expl = compute_exploitability(game);
            eprintln!("[{label}] step {it}: {:.2}% of the pot, {:.0}s", expl / pot * 100.0, t0.elapsed().as_secs_f64());
            if expl <= target { stopped = "target_reached"; break; }
        }
        if t0.elapsed().as_secs_f64() >= budget { stopped = "max_seconds"; if it % 10 != 0 { expl = compute_exploitability(game); } break; }
    }
    finalize(game);
    Solved { iterations: it, exploit: expl, seconds: t0.elapsed().as_secs_f64(), stopped }
}

fn root_evs(game: &mut PostFlopGame) -> [f64; 2] {
    game.back_to_root(); game.cache_normalized_weights();
    let mut out = [0.0; 2];
    for p in 0..2 {
        let (ev, w) = (game.expected_values(p), game.normalized_weights(p).to_vec());
        let (mut s, mut t) = (0f64, 0f64);
        for i in 0..ev.len() { s += ev[i] as f64 * w[i] as f64; t += w[i] as f64; }
        out[p] = if t > 0.0 { s / t / CHIPS } else { 0.0 };
    }
    out
}

/// One requested lock, resolved against a game positioned at its node.
/// Returns the strategy to lock (action * hands + hand); hands left at 0 everywhere stay free.
fn lock_strategy(game: &mut PostFlopGame, names: &[String; 2], lock: &Value, baseline: Option<&[f32]>) -> Result<Vec<f32>, String> {
    let me = game.current_player();
    if let Some(p) = lock.get("player").and_then(|x| x.as_str()) { if p != names[me] { return Err(format!("node_locks: at '{}' it is {}'s decision, not {p}'s", lock["node"].as_str().unwrap_or(""), names[me])); } }
    let body = lock.get("lock").unwrap_or(lock);
    let freqs = body.get("action_freqs").and_then(|x| x.as_object()).ok_or("node_locks: each lock needs \"action_freqs\"")?;
    let hands = game.private_cards(me).to_vec();
    let n = hands.len();
    let selected: Vec<bool> = match body.get("hands").and_then(|x| x.as_str()) {
        None | Some("") | Some("all") => vec![true; n],
        Some(r) => { let rg: Range = r.parse().map_err(|e: String| format!("node_locks hands: {e}"))?; hands.iter().map(|&(a, b)| rg.get_weight_by_cards(a, b) > 0.0).collect() }
    };
    let fixed = fixed_actions(game, lock, freqs)?;
    let na = fixed.len();
    let named: f64 = fixed.iter().flatten().sum();
    let free: Vec<usize> = (0..na).filter(|&a| fixed[a].is_none()).collect();
    let mut out = vec![0f32; na * n];
    for h in 0..n {
        if !selected[h] { continue; }
        // the unlisted actions share what is left, in the proportions of the normal solve when known
        let base: Vec<f64> = free.iter().map(|&a| baseline.map(|b| b[a * n + h] as f64).unwrap_or(1.0)).collect();
        let bs: f64 = base.iter().sum();
        for a in 0..na { if let Some(f) = fixed[a] { out[a * n + h] = f as f32; } }
        for (j, &a) in free.iter().enumerate() { out[a * n + h] = ((1.0 - named) * if bs > 1e-9 { base[j] / bs } else { 1.0 / free.len() as f64 }) as f32; }
        // the engine treats a hand as locked only if some action is positive
        if (0..na).all(|a| out[a * n + h] <= 0.0) { out[h] = 1e-6; }
    }
    Ok(out)
}

/// The frequency fixed for each action at the current node (None = not listed in the lock).
/// Keys are exact action names, or a kind (bet / raise / aggressive) covering several actions.
fn fixed_actions(game: &mut PostFlopGame, lock: &Value, freqs: &Map<String, Value>) -> Result<Vec<Option<f64>>, String> {
    let hist = game.history().to_vec();
    let start = game_street_start(game);
    game.apply_history(&hist);
    let acts = game.available_actions();
    let canon = action_names(game, start);
    let na = acts.len();
    let mut fixed: Vec<Option<f64>> = vec![None; na];
    for (k, v) in freqs {
        let f = v.as_f64().ok_or("node_locks: frequencies must be numbers")?;
        if !(0.0..=1.0).contains(&f) { return Err("node_locks: frequencies must be between 0 and 1".into()); }
        let key = k.to_ascii_lowercase();
        let idx: Vec<usize> = (0..na).filter(|&a| canon[a] == key || match (&acts[a], key.as_str()) {
            (Action::Bet(_), "bet") | (Action::Raise(_), "raise") | (Action::AllIn(_), "jam") | (Action::AllIn(_), "shove") => true,
            (Action::Bet(_) | Action::Raise(_) | Action::AllIn(_), "aggressive") => true,
            _ => false,
        }).collect();
        if idx.is_empty() { return Err(format!("node_locks: '{k}' is not an action at '{}'; choices are: {} (or bet, raise, aggressive)", lock["node"].as_str().unwrap_or(""), canon.join(", "))); }
        for &a in &idx { fixed[a] = Some(f / idx.len() as f64); }
    }
    let named: f64 = fixed.iter().flatten().sum();
    if named > 1.0 + 1e-6 { return Err("node_locks: the given frequencies add up to more than 1".into()); }
    if fixed.iter().all(|f| f.is_some()) && (named - 1.0).abs() > 0.01 { return Err("node_locks: when every action is listed the frequencies must add up to 1".into()); }
    Ok(fixed)
}

/// Bets already in at the start of the current street (for naming raises).
fn game_street_start(game: &mut PostFlopGame) -> [i32; 2] {
    let hist = game.history().to_vec();
    game.back_to_root();
    let mut start = [0, 0];
    for &h in &hist { let chance = game.is_chance_node(); game.play(h); if chance { start = game.total_bet_amount(); } }
    start
}

/// Two solves are needed when the locks depend on the normal solve: habits, or a lock that
/// leaves some actions unlisted (those keep the normal solve's proportions).
fn needs_baseline(spot: &Spot, game: &mut PostFlopGame) -> Result<bool, String> {
    if spot.raw.get("habits").and_then(|x| x.as_array()).map_or(false, |a| !a.is_empty()) { return Ok(true); }
    if spot.raw.get("baseline").and_then(|x| x.as_bool()).unwrap_or(false) { return Ok(true); }
    let mut two = false;
    if let Some(locks) = spot.raw.get("node_locks").and_then(|x| x.as_array()) {
        for l in locks {
            let node = l.get("node").and_then(|x| x.as_str()).ok_or("node_locks: each lock needs a \"node\" path")?;
            walk(game, &spot.names, node)?;
            if game.is_terminal_node() || game.is_chance_node() { return Err(format!("node_locks: '{node}' is not a decision")); }
            let body = l.get("lock").unwrap_or(l);
            let freqs = body.get("action_freqs").and_then(|x| x.as_object()).ok_or("node_locks: each lock needs \"action_freqs\"")?;
            if fixed_actions(game, l, freqs)?.iter().any(|f| f.is_none()) { two = true; }
        }
    }
    game.back_to_root();
    Ok(two)
}

fn summary(spot: &Spot, game: &mut PostFlopGame, s: &Solved) -> Value {
    let pot = spot.tree.starting_pot as f32;
    let ev = root_evs(game);
    let unres = compute_exploitability_unrestricted(game);
    json!({
        "iterations": s.iterations, "runtime_seconds": round(s.seconds, 2), "stopped_because": s.stopped,
        "exploitability_pct": round((s.exploit / pot * 100.0) as f64, 3),
        "exploitability_ignoring_locks_pct": round((unres / pot * 100.0) as f64, 3),
        "ev_bb": {spot.names[0].clone(): round(ev[0], 3), spot.names[1].clone(): round(ev[1], 3)},
    })
}

fn nodes_report(spot: &Spot, game: &mut PostFlopGame) -> Result<Value, String> {
    let detail = spot.raw.get("detail").and_then(|x| x.as_str()).unwrap_or("combo").to_string();
    let limit = spot.raw.get("max_report_nodes").and_then(|x| x.as_u64()).unwrap_or(60) as usize;
    let mut paths = first_street_paths(game, &spot.names, limit);
    if let Some(extra) = spot.raw.get("report_nodes").and_then(|x| x.as_array()) { for p in extra { if let Some(p) = p.as_str() { paths.push(p.to_string()); } } }
    let mut out = Map::new();
    for p in paths {
        let cur = walk(game, &spot.names, &p)?;
        let r = report(game, &spot.names, &cur, &p, &detail, None);
        out.insert(p, r);
    }
    game.back_to_root();
    Ok(Value::Object(out))
}

/// The player names when the spot has three players.
fn three_players(raw: &Value) -> Option<Vec<String>> {
    let p: Vec<String> = raw.get("players")?.as_array()?.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect();
    if p.len() == 3 && p[0] != p[1] && p[1] != p[2] && p[0] != p[2] { Some(p) } else { None }
}

fn solve(spot_path: &str, out_path: Option<&str>, save_path: Option<&str>) -> Result<Value, String> {
    let text = std::fs::read_to_string(spot_path).map_err(|e| format!("cannot read {spot_path}: {e}"))?;
    let raw: Value = serde_json::from_str(&text).map_err(|e| format!("{spot_path} is not valid JSON: {e}"))?;
    if let Some(three) = three_players(&raw) {
        let v = three::solve(&raw, &three, save_path)?;
        if let Some(p) = out_path { std::fs::write(p, serde_json::to_string_pretty(&v).unwrap()).map_err(|e| format!("cannot write {p}: {e}"))?; }
        return Ok(v);
    }
    let spot = parse_spot(&raw)?;
    let threads = if spot.threads > 0 { spot.threads } else { std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2) };
    let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().map_err(|e| e.to_string())?;
    let t_all = Instant::now();
    let locks: Vec<Value> = raw.get("node_locks").and_then(|x| x.as_array()).cloned().unwrap_or_default();
    let habits: Vec<Value> = raw.get("habits").and_then(|x| x.as_array()).cloned().unwrap_or_default();
    let icm = icm_setup(&spot)?;
    if icm.is_some() && save_path.is_some() { return Err("--save is not available with icm yet (the prize-money payoffs are not stored in the file)".into()); }
    let mut game = build(&spot)?;
    let (mem_full, mem_small) = game.memory_usage();
    // 16-bit storage halves memory for a small loss of precision; used automatically for big trees
    let compress = match raw.get("compress") { Some(Value::Bool(b)) => *b, _ => mem_full as f64 / 1048576.0 > 1500.0 };
    let mem_mb = if compress { mem_small } else { mem_full } as f64 / 1048576.0;
    if let Some(cap) = raw.get("max_memory_mb").and_then(|x| x.as_f64()) { if mem_mb > cap { return Err(format!("this spot needs about {:.0} MB, over max_memory_mb = {cap}", mem_mb)); } }
    game.allocate_memory(compress);
    let two_pass = needs_baseline(&spot, &mut game)?;
    eprintln!("{} | {} vs {} | {:.0} MB | {} threads", version_string(), spot.names[0], spot.names[1], mem_mb, threads);
    let mut result = Map::new();
    result.insert("version".into(), json!(version_string()));
    result.insert("spot".into(), raw.clone());
    result.insert("out_of_position".into(), json!(spot.names[0])); result.insert("in_position".into(), json!(spot.names[1]));
    result.insert("memory_mb".into(), json!(round(mem_mb, 1))); result.insert("compressed".into(), json!(compress)); result.insert("threads".into(), json!(threads));
    result.insert("units".into(), json!("EVs and amounts in big blinds; ev_bb counts the pot at the start of the spot, so the two players' EVs add up to pot_bb when there is no rake; frequencies and equity are 0-1"));

    if two_pass {
        // 1) normal solve, 2) derive the locks from it, 3) solve again with the locks in place
        let s = pool.install(|| run(&mut game, &spot, spot.max_seconds * 0.45, "normal"));
        let mut base_sum = summary(&spot, &mut game, &s);
        if raw.get("baseline_nodes").and_then(|x| x.as_bool()).unwrap_or(true) { base_sum["nodes"] = nodes_report(&spot, &mut game)?; }
        let mut planned: Vec<(Vec<usize>, Vec<f32>)> = vec![];
        if !habits.is_empty() {
            let leaks: Vec<Leak> = habits.iter().map(|h| {
                let name = h.get("player").and_then(|x| x.as_str()).unwrap_or("");
                let player = spot.names.iter().position(|n| n == name).ok_or(format!("habits: player '{name}' is not in this spot"))?;
                Ok(Leak { player, street: h.get("street").and_then(|x| x.as_str()).unwrap_or("all").to_string(),
                    effect: h.get("effect").and_then(|x| x.as_str()).ok_or("habits: each habit needs an \"effect\"")?.to_string(),
                    strength: h.get("strength").and_then(|x| x.as_f64()).unwrap_or(0.6) as f32 })
            }).collect::<Result<_, String>>()?;
            let freeze = raw.get("freeze_rest").and_then(|x| x.as_bool()).unwrap_or(true);
            collect_locks(&mut game, &leaks, freeze, |h, st| planned.push((h.to_vec(), st.to_vec())));
        }
        for l in &locks {
            let node = l.get("node").and_then(|x| x.as_str()).ok_or("node_locks: each lock needs a \"node\" path")?;
            let cur = walk(&mut game, &spot.names, node)?;
            if game.is_terminal_node() || game.is_chance_node() { return Err(format!("node_locks: '{node}' is not a decision")); }
            let base = game.strategy();
            let st = lock_strategy(&mut game, &spot.names, l, Some(&base))?;
            game.apply_history(&cur.history);
            // an explicit lock overrides a habit at the same node, hand by hand
            if let Some(p) = planned.iter_mut().find(|p| p.0 == cur.history) {
                let n = game.private_cards(game.current_player()).len(); let na = st.len() / n;
                for h in 0..n { if (0..na).any(|a| st[a * n + h] > 0.0) { for a in 0..na { p.1[a * n + h] = st[a * n + h]; } } }
            } else { planned.push((cur.history, st)); }
        }
        result.insert("normal".into(), base_sum);
        drop(game);
        game = build(&spot)?;
        game.allocate_memory(compress);
        for (h, st) in &planned { game.apply_history(h); game.lock_current_strategy(st); }
        game.back_to_root();
        let left = (spot.max_seconds - t_all.elapsed().as_secs_f64()).max(spot.max_seconds * 0.3);
        let s = pool.install(|| run(&mut game, &spot, left, "locked"));
        let mut sum = summary(&spot, &mut game, &s);
        sum["locked_decisions"] = json!(planned.len());
        for (k, v) in sum.as_object().unwrap() { result.insert(k.clone(), v.clone()); }
    } else {
        for l in &locks {
            let node = l.get("node").and_then(|x| x.as_str()).ok_or("node_locks: each lock needs a \"node\" path")?;
            let cur = walk(&mut game, &spot.names, node)?;
            if game.is_terminal_node() || game.is_chance_node() { return Err(format!("node_locks: '{node}' is not a decision")); }
            let st = lock_strategy(&mut game, &spot.names, l, None)?;
            game.apply_history(&cur.history);
            game.lock_current_strategy(&st);
        }
        game.back_to_root();
        let s = pool.install(|| run(&mut game, &spot, spot.max_seconds, "solve"));
        let mut sum = summary(&spot, &mut game, &s);
        sum["locked_decisions"] = json!(locks.len());
        for (k, v) in sum.as_object().unwrap() { result.insert(k.clone(), v.clone()); }
    }
    if let Some(icm) = &icm {
        // ev_bb is in chip-sized units; turn it back into prize money
        let ev = result.get("ev_bb").cloned().unwrap_or(json!({}));
        let mut solved = Map::new(); let mut split = Map::new(); let mut fold_now = Map::new();
        for p in 0..2 {
            let e = ev.get(&spot.names[p]).and_then(|x| x.as_f64()).unwrap_or(0.0) * CHIPS;
            solved.insert(spot.names[p].clone(), json!(round(icm.share_pct(p, e), 3)));
            split.insert(spot.names[p].clone(), json!(round(icm.split_pct(p), 3)));
            fold_now.insert(spot.names[p].clone(), json!(round(icm.give_up_pct(p), 3)));
        }
        result.insert("icm".into(), json!({
            "prize_pool_share_pct": solved, "if_pot_were_split_pct": split, "if_player_gave_up_the_pot_pct": fold_now,
            "note": "Payoffs are prize money (Independent Chip Model), not chips. ev_bb, EVs in the node reports and exploitability are in chip-sized units of prize money: prize pool divided by chips in play, per chip. prize_pool_share_pct is each player's expected share of the prize pool after this spot. The two players' ev_bb no longer add up to the pot."
        }));
    }
    result.insert("nodes".into(), nodes_report(&spot, &mut game)?);
    result.insert("total_seconds".into(), json!(round(t_all.elapsed().as_secs_f64(), 2)));
    if let Some(p) = save_path {
        game.back_to_root();
        let memo = json!({"version": version_string(), "names": spot.names, "spot": raw}).to_string();
        save_data_to_file(&game, &memo, p, None)?;
        result.insert("saved_to".into(), json!(p));
    }
    let v = Value::Object(result);
    if let Some(p) = out_path { std::fs::write(p, serde_json::to_string_pretty(&v).unwrap()).map_err(|e| format!("cannot write {p}: {e}"))?; }
    Ok(v)
}

fn query(save: &str, node: &str, only: Option<&str>, detail: &str) -> Result<Value, String> {
    let (mut game, memo): (PostFlopGame, String) = load_data_from_file(save, None)?;
    let m: Value = serde_json::from_str(&memo).map_err(|_| "this file was not saved by pontiac".to_string())?;
    let names: [String; 2] = [m["names"][0].as_str().unwrap_or("OOP").to_string(), m["names"][1].as_str().unwrap_or("IP").to_string()];
    let cur = walk(&mut game, &names, node)?;
    let mut r = report(&mut game, &names, &cur, node.trim_matches('/'), detail, only);
    r["version"] = m["version"].clone();
    Ok(r)
}

fn usage() -> String {
    format!("{}\n\nusage:\n  pontiac solve <spot.json> [-o result.json] [--save solved.pontiac]\n  pontiac query <solved.pontiac> --node <path> [--hand AhKd | --hand AKs] [--detail class|combo]\n  pontiac estimate <spot.json>\n  pontiac pushfold --stack 10 [--sb 0.5] [--ante 0]\n  pontiac pushfold --stacks 12,8,15 [--payouts 50,30,20] [--others 6] [--sb 0.5] [--ante 0] [--bb-ante 1]\n  pontiac icm --stacks 5000,3000,2000 --payouts 50,30,20\n  pontiac version\n\nNode paths look like: flop/BB_check/SB_bet0.33/BB_call/turn=7h/BB_check", version_string())
}

fn main() {
    // report crashes in the same JSON shape as ordinary errors
    std::panic::set_hook(Box::new(|info| {
        let msg = info.payload().downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| info.payload().downcast_ref::<String>().cloned()).unwrap_or_else(|| "unknown error".into());
        println!("{}", json!({"ok": false, "error": format!("internal error: {msg}")}));
        std::process::exit(3);
    }));
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(|s| s.as_str());
    let res: Result<Value, String> = match args.first().map(|s| s.as_str()) {
        Some("version") | Some("--version") | Some("-V") => { println!("{}", version_string()); return; }
        Some("solve") if args.len() >= 2 => solve(&args[1], flag("-o").or(flag("--out")), flag("--save")).map(|v| if flag("-o").or(flag("--out")).is_some() {
            json!({"ok": true, "written": flag("-o").or(flag("--out")), "exploitability_pct": v["exploitability_pct"], "runtime_seconds": v["total_seconds"], "saved_to": v.get("saved_to")}) } else { v }),
        Some("query") if args.len() >= 2 => query(&args[1], flag("--node").unwrap_or(""), flag("--hand").or(flag("--combo")), flag("--detail").unwrap_or("class")),
        Some("estimate") if args.len() >= 2 => std::fs::read_to_string(&args[1]).map_err(|e| e.to_string())
            .and_then(|t| serde_json::from_str::<Value>(&t).map_err(|e| e.to_string())).and_then(|raw| {
                if let Some(three) = three_players(&raw) { return three::estimate3(&raw, &three); }
                let spot = parse_spot(&raw)?; let g = build(&spot)?; let (m, mc) = g.memory_usage();
                let two = ["habits", "node_locks"].iter().any(|k| spot.raw.get(*k).and_then(|x| x.as_array()).map_or(false, |a| !a.is_empty()));
                Ok(json!({"memory_mb": round(m as f64 / 1048576.0, 1), "memory_mb_compressed": round(mc as f64 / 1048576.0, 1), "solves_at_most": if two { 2 } else { 1 }, "out_of_position": spot.names[0], "in_position": spot.names[1]})) }),
        Some("icm") => (|| -> Result<Value, String> {
            let list = |name: &str| -> Result<Vec<f64>, String> { flag(name).ok_or(format!("{name} is needed, e.g. pontiac icm --stacks 5000,3000,2000 --payouts 50,30,20"))?.split(',').map(|x| x.trim().parse::<f64>().map_err(|_| format!("{name} needs numbers separated by commas"))).collect() };
            let (stacks, payouts) = (list("--stacks")?, list("--payouts")?);
            if stacks.is_empty() || stacks.len() > 10 || stacks.iter().any(|&x| x < 0.0) || payouts.len() > 10 { return Err("give 1 to 10 stacks (zero or more chips each) and at most 10 prizes".into()); }
            let e = pushfold::icm::icm(&stacks, &payouts);
            let chips: f64 = stacks.iter().sum();
            let pool: f64 = payouts.iter().take(stacks.len()).sum();
            Ok(json!({"ok": true, "version": version_string(), "stacks": stacks, "payouts": payouts,
                "prize_value": e.iter().map(|x| round(*x, 4)).collect::<Vec<_>>(),
                "chip_share": stacks.iter().map(|x| round(x / chips, 4)).collect::<Vec<_>>(),
                "prize_share": e.iter().map(|x| round(if pool > 0.0 { x / pool } else { 0.0 }, 4)).collect::<Vec<_>>(),
                "note": "Independent Chip Model (Malmuth-Harville): expected prize for each stack if the rest of the tournament were decided by chip counts alone"}))
        })(),
        Some("preflop") => (|| -> Result<Value, String> {
            let num = |name: &str| -> Result<Option<f64>, String> { match flag(name) { Some(x) => x.parse::<f64>().map(Some).map_err(|_| format!("{name} needs a number")), None => Ok(None) } };
            // --check <result.json>: is the chart in order hand by hand? (no solving)
            if let Some(path) = flag("--check") {
                let text = std::fs::read_to_string(&path).map_err(|e| format!("could not read {path}: {e}"))?;
                let v: Value = serde_json::from_str(&text).map_err(|_| format!("{path} is not a preflop result"))?;
                let tol = num("--tolerance")?.unwrap_or(0.15);
                let min_gap = num("--min-value-gap")?.unwrap_or(0.03);
                let mut out = vec![];
                let row = |x: &Value| -> Vec<f64> { x.as_array().map(|a| a.iter().map(|y| y.as_f64().unwrap_or(0.0)).collect()).unwrap_or_default() };
                for n in v["nodes"].as_array().ok_or("no decisions in that file")? {
                    let actions: Vec<String> = n["actions"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
                    let reach = n["reach"].as_f64().unwrap_or(0.0);
                    if reach < 0.05 { continue; }
                    let strat: Vec<Vec<f64>> = (0..actions.len()).map(|a| row(&n["strategy"][a])).collect();
                    if strat.iter().any(|s| s.len() != 169) { continue; }
                    let values: Option<Vec<Vec<f64>>> = { let v: Vec<Vec<f64>> = (0..actions.len()).map(|a| row(&n["values"][a])).collect(); if v.iter().all(|x| x.len() == 169) { Some(v) } else { None } };
                    let present = row(&n["range"]);
                    let present = if present.len() == 169 { Some(present) } else { None };
                    // playing against folding
                    if let Some(fold) = actions.iter().position(|a| a == "Fold") {
                        let plays: Vec<f64> = strat[fold].iter().map(|f| 1.0 - f).collect();
                        let edge: Option<Vec<f64>> = values.as_ref().map(|v| (0..169).map(|h| (0..actions.len()).filter(|&a| a != fold).map(|a| v[a][h]).fold(f64::MIN, f64::max) - v[fold][h]).collect());
                        out.push(json!({"decision": n["situation"], "path": n["path"], "reach": reach, "compares": "playing against folding",
                            "check": pushfold::chartcheck::check_values(&plays, edge.as_deref(), present.as_deref(), tol, min_gap)}));
                    }
                    // raising against limping (or against calling or checking): only among hands that mostly do one of the two
                    let passive = actions.iter().position(|a| a == "Limp" || a == "Call" || a == "Check");
                    let raises: Vec<usize> = (0..actions.len()).filter(|&a| actions[a].starts_with("Raise") || actions[a].starts_with("Re-raise")).collect();
                    if let (Some(pa), false) = (passive, raises.is_empty()) {
                        let both: Vec<f64> = (0..169).map(|h| strat[pa][h] + raises.iter().map(|&a| strat[a][h]).sum::<f64>()).collect();
                        let prefers: Vec<f64> = (0..169).map(|h| if both[h] > 0.0 { raises.iter().map(|&a| strat[a][h]).sum::<f64>() / both[h] } else { 0.0 }).collect();
                        let edge: Option<Vec<f64>> = values.as_ref().map(|v| (0..169).map(|h| raises.iter().map(|&a| v[a][h]).fold(f64::MIN, f64::max) - v[pa][h]).collect());
                        let part: Vec<f64> = (0..169).map(|h| present.as_ref().map(|p| p[h]).unwrap_or(1.0) * if both[h] >= 0.5 { 1.0 } else { 0.0 }).collect();
                        out.push(json!({"decision": n["situation"], "path": n["path"], "reach": reach, "compares": format!("raising against {}", actions[pa].to_lowercase()),
                            "check": pushfold::chartcheck::check_values(&prefers, edge.as_deref(), Some(&part), tol, min_gap)}));
                    }
                }
                return Ok(json!({"ok": true, "version": version_string(), "file": path,
                    "note": "Pairs of hands where a clearly stronger hand of the same kind takes the stronger action (playing, or raising) less often than a weaker one, by more than the tolerance. `inversions` counts them by frequency alone, over all 169 classes; `inversions_by_value` counts only those where the action is also worth more than the value gap (default 0.03 bb) more to the weaker hand, among hands that get to the decision. Raising against limping need not run in order of strength in a sound strategy (weak hands can raise as bluffs), so read that comparison as a description, not a pass mark.",
                    "decisions": out}));
            }
            let mut spec = json!({"stack": match num("--stack")? { Some(x) => x, None => args.get(1).and_then(|x| x.parse::<f64>().ok()).ok_or("give the stack in big blinds, e.g. pontiac preflop --stack 20")? }});
            for (flag_name, key) in [("--sb", "small_blind"), ("--ante", "ante"), ("--reraise", "reraise"), ("--iterations", "iterations")] { if let Some(x) = num(flag_name)? { spec[key] = json!(x); } }
            for (flag_name, key) in [("--over-limp", "over_limp"), ("--over-open", "over_open")] { if let Some(t) = flag(flag_name) { spec[key] = json!(t.split(',').map(|x| x.trim().parse::<f64>().map_err(|_| format!("{flag_name} needs numbers separated by commas, e.g. {flag_name} 3,6.5"))).collect::<Result<Vec<f64>, String>>()?); } }
            if let Some(t) = flag("--open") { spec["open_sizes"] = json!(t.split(',').map(|x| x.trim().parse::<f64>().map_err(|_| "--open needs numbers separated by commas, e.g. --open 2,2.5".to_string())).collect::<Result<Vec<_>, _>>()?); }
            if args.iter().any(|a| a == "--no-limp") { spec["limp"] = json!(false); }
            if let Some(t) = flag("--model") { let v = t.split(',').map(|x| x.trim().parse::<f64>().map_err(|_| "--model needs four numbers: position,initiative,playability,half_spr".to_string())).collect::<Result<Vec<_>, _>>()?; if v.len() != 4 { return Err("--model needs four numbers: position,initiative,playability,half_spr".into()); } spec["model"] = json!({"position": v[0], "initiative": v[1], "playability": v[2], "half_spr": v[3]}); }
            if spec.get("iterations").is_some() { spec["iterations"] = json!(spec["iterations"].as_f64().unwrap_or(0.0) as u64); }
            let spec: pushfold::preflop::PreSpec = serde_json::from_value(spec).map_err(|e| e.to_string())?;
            let r = if args.iter().any(|a| a == "--accurate") {
                // real flop solves for every common line, fed back in and repeated
                // The flops: a built-in sample (the first --flops of it), or the ones named with --flop-list.
                // Each hand class weights them by how it connects with all flops (--plain-flops: by plain count, as the first accurate runs did).
                let flop_names: Vec<String> = match flag("--flop-list") {
                    Some(t) => t.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect(),
                    None => pushfold::flopweights::sample((num("--flops")?.unwrap_or(8.0) as usize).max(1)).iter().map(|x| x.to_string()).collect(),
                };
                let flop_cards: Vec<[u8; 3]> = flop_names.iter().map(|x| pushfold::flopweights::parse_flop(x)).collect::<Result<_, _>>()?;
                let nflops = flop_cards.len();
                if nflops == 0 { return Err("--flop-list needs at least one flop, e.g. --flop-list As7d2c,Ks8s3d".into()); }
                let plain_flops = args.iter().any(|a| a == "--plain-flops");
                // --valuing kinds: build each hand's value from kinds of flop (miss, pair, two pair, trips) at their true
                // frequencies, leaning on adjacent-kicker hands where its own flops are few (--lean N: the neighbours count
                // as N flops of evidence, default 2; 0 only fills gaps). --valuing weights (default): the fitted weights alone.
                let valuing = flag("--valuing").unwrap_or("weights");
                if valuing != "weights" && valuing != "kinds" { return Err("--valuing is weights or kinds".into()); }
                if valuing == "kinds" && plain_flops { return Err("--valuing kinds works on the fitted weights; leave out --plain-flops".into()); }
                let lean = num("--lean")?.unwrap_or(2.0).max(0.0);
                // --floor X: keep at least this much of every hand in the ranges sent to the flop solver (e.g. 0.01), so a hand
                // that has stopped taking a line still gets a real value for it and can come back. Default 0: hands under half a
                // percent are left out and take the average adjustment of the hands that are in.
                let floor = num("--floor")?.unwrap_or(0.0).clamp(0.0, 0.2);
                // --rounds fixed|average|rising: how the rounds are combined. fixed (default) moves --step (0.35) of the way to
                // each fresh solve; average keeps the plain average of every round's solve; rising weights later rounds more.
                let rule = match flag("--rounds").unwrap_or("fixed") {
                    "fixed" => pushfold::preflop::StepRule::Fixed(num("--step")?.unwrap_or(0.35)),
                    "average" => pushfold::preflop::StepRule::Average,
                    "rising" => pushfold::preflop::StepRule::Rising,
                    _ => return Err("--rounds is fixed, average or rising".into()),
                };
                // --until-settled [points]: stop once each of the button's totals has moved less than this (default 1 point) over three rounds
                let settle: Option<f64> = if args.iter().any(|a| a == "--until-settled") { Some(num("--until-settled").ok().flatten().unwrap_or(1.0)) } else { None };
                // --start-from <file.resume>: begin from the strategy another run had reached, instead of the quick estimate
                let start_from = flag("--start-from");
                let start_sigmas: Option<Vec<Vec<f64>>> = match &start_from {
                    Some(path) => {
                        let v: Value = serde_json::from_str(&std::fs::read_to_string(path).map_err(|e| format!("could not read {path}: {e}"))?).map_err(|_| format!("{path} is not a .resume file"))?;
                        Some(serde_json::from_value(v["checkpoint"]["prev"].clone()).map_err(|_| format!("{path} holds no strategy to start from"))?)
                    }
                    None => None,
                };
                let flop_fit = pushfold::flopweights::fit(&flop_cards, None);
                let flop_ways: Vec<f64> = flop_cards.iter().map(|f| pushfold::flopweights::suit_ways(*f)).collect();
                let passes = (num("--passes")?.unwrap_or(3.0) as usize).clamp(1, 40);
                let min_freq = num("--min-line")?.unwrap_or(0.02);
                let accuracy = num("--flop-accuracy")?.unwrap_or(1.0) as f32;
                let threads = num("--threads")?.map(|x| x as usize).unwrap_or_else(|| std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4));
                let log = flag("--log");
                let say = |line: String| { eprintln!("{line}"); if let Some(p) = &log { use std::io::Write; if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p) { let _ = writeln!(f, "{line}"); } } };
                let prog = study_core::StudyProgress::default();
                let flop_parallel = num("--flop-parallel")?.map(|x| x as usize).unwrap_or(1).clamp(1, 8);
                let t0 = std::time::Instant::now();
                // --flop-detail <file>: every round, the value of every hand class on each flop of each line goes to
                // <file>.round<N>.json (big blinds; null where the hand is not in the range), with the flop weights used
                let save_path = flag("--save");
                let replay: Option<Value> = match flag("--flop-values") {
                    Some(path) => Some(serde_json::from_str(&std::fs::read_to_string(&path).map_err(|e| format!("could not read {path}: {e}"))?).map_err(|_| format!("{path} is not a flop-detail file"))?),
                    None => None,
                };
                if replay.is_some() && plain_flops { return Err("--flop-values cannot be used with --plain-flops".into()); }
                let detail = flag("--flop-detail");
                let mut detail_round = 0usize;
                let mut detail_lines: Vec<Value> = vec![];
                // --line-values <file>: take each line's per-hand values from a file instead of solving flops
                // ({"lines": {"<line path>": {"button": [169 numbers or null], "big_blind": [...]}}}, big blinds out of the pot).
                // For testing other ways of turning flop results into hand values without solving again.
                let given: Option<Value> = match flag("--line-values") {
                    Some(path) => Some(serde_json::from_str(&std::fs::read_to_string(&path).map_err(|e| format!("could not read {path}: {e}"))?).map_err(|_| format!("{path} is not a line-values file"))?),
                    None => None,
                };
                let mut eval = |l: &pushfold::preflop::FlopLine, pass: usize| -> Result<pushfold::preflop::LineValues, String> {
                    if let Some(g) = &given {
                        let e = &g["lines"][&l.path];
                        if e.is_null() { return Err(format!("the line-values file has no entry for the line '{}' ({:.1}% of hands)", l.path, l.freq * 100.0)); }
                        let row = |k: &str| -> Vec<f64> { (0..169).map(|c| e[k][c].as_f64().unwrap_or(f64::NAN)).collect() };
                        say(format!("pass {pass} | {} | values taken from the file", l.path));
                        return Ok(([row("button"), row("big_blind")], nflops, 0.0));
                    }
                    let (mut num_v, mut den_v) = ([vec![0f64; 169], vec![0f64; 169]], [vec![0f64; 169], vec![0f64; 169]]);
                    let mut worst = 0f64;
                    let mut detail_flops: Vec<Value> = vec![];
                    // by_flop[player][flop][class] in big blinds, button first
                    let mut by_flop: [Vec<Vec<f64>>; 2] = [vec![], vec![]];
                    let make_spec = |board: &str, th: usize| study_core::StudySpec {
                            oop_range: if floor > 0.0 { pushfold::preflop::range_with_floor(&l.reach[1], floor) } else { l.big_blind_range.clone() },
                            ip_range: if floor > 0.0 { pushfold::preflop::range_with_floor(&l.reach[0], floor) } else { l.button_range.clone() },
                            board: board.to_string(), pot: (l.pot_bb * 10.0).round() as i32, stack: (l.behind_bb * 10.0).round() as i32,
                            oop_flop_bet: "50%".into(), ip_flop_bet: "50%".into(), turn_bet: "66%".into(), river_bet: "66%".into(), raise: "2.5x".into(),
                            allin_threshold: 0.67, accuracy_pct: accuracy, max_iter: 400, leaks: vec![], threads: th, low_memory: false, freeze_rest: true, confidence: 1.0, icm: None };
                    // --flop-parallel K: solve K flops at once, each with a share of the threads
                    type FlopOut = ([Vec<f64>; 2], f64, [Vec<f64>; 2]);
                    let mut pre: Vec<Option<(Result<FlopOut, String>, f64)>> = (0..flop_names.len()).map(|_| None).collect();
                    if flop_parallel > 1 && replay.is_none() {
                        let th = (threads / flop_parallel).max(1);
                        let next = std::sync::atomic::AtomicUsize::new(0);
                        let done = std::sync::Mutex::new(Vec::<(usize, Result<FlopOut, String>, f64)>::new());
                        let stop = save_path.as_ref().map(|p| format!("{p}.stop"));
                        std::thread::scope(|sc| { for _ in 0..flop_parallel { sc.spawn(|| loop {
                            let k = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                            if k >= flop_names.len() { break; }
                            if let Some(p) = &stop { if std::path::Path::new(p).exists() { break; } }
                            let t = std::time::Instant::now();
                            let r = study_core::class_values(&make_spec(&flop_names[k].to_string(), th), &prog);
                            let secs = t.elapsed().as_secs_f64();
                            eprintln!("pass {pass} | {} | flop {} of {} ({}) solved in {secs:.0}s", l.path, k + 1, flop_names.len(), flop_names[k]);
                            done.lock().unwrap().push((k, r, secs));
                        }); } });
                        for (k, r, secs) in done.into_inner().unwrap() { pre[k] = Some((r, secs)); }
                    }
                    for (k, board) in flop_names.iter().enumerate() {
                        let sp = make_spec(&board.to_string(), threads);
                        let t = std::time::Instant::now();
                        if let Some(p) = &save_path { if std::path::Path::new(&format!("{p}.stop")).exists() { return Err(format!("stopped because the file {p}.stop exists; remove it and run again with --resume to carry on")); } }
                        // --flop-values <detail file>: replay the per-flop values a run recorded with --flop-detail instead of solving
                        let (vals, acc, mass) = match &replay {
                            Some(d) => {
                                let line = d["lines"].as_array().and_then(|a| a.iter().find(|x| x["line"] == json!(l.path))).ok_or(format!("the flop-values file has no line '{}'", l.path))?;
                                let f = line["flops"].as_array().and_then(|a| a.iter().find(|x| x["flop"] == json!(board))).ok_or(format!("the flop-values file has no flop {board} for the line '{}'", l.path))?;
                                let row = |k: &str| -> Vec<f64> { (0..169).map(|c| f[k][c].as_f64().map(|x| x * 10.0).unwrap_or(f64::NAN)).collect() };
                                ([row("big_blind"), row("button")], f["accuracy_pct"].as_f64().unwrap_or(0.0), [vec![0.0; 169], vec![0.0; 169]])
                            }
                            None => match pre[k].take() { Some((r, _)) => r?, None => study_core::class_values(&sp, &prog)? },
                        };
                        worst = worst.max(acc);
                        for (mine, theirs) in [(0usize, 1usize), (1, 0)] { by_flop[mine].push(vals[theirs].iter().map(|x| x / 10.0).collect()); }
                        // vals[0] is the big blind (first to act after the flop), vals[1] the button; chips are tenths of a big blind
                        for (mine, theirs) in [(0usize, 1usize), (1, 0)] { for c in 0..169 { let x = vals[theirs][c]; let k = if plain_flops { flop_ways[k] * mass[theirs][c] } else { flop_fit.weight[c][k] }; if x.is_finite() && k > 0.0 { num_v[mine][c] += k * x / 10.0; den_v[mine][c] += k; } } }
                        if detail.is_some() {
                            let bb = |v: &Vec<f64>| -> Vec<Value> { v.iter().map(|x| if x.is_finite() { json!((x / 10.0 * 1e4).round() / 1e4) } else { Value::Null }).collect() };
                            detail_flops.push(json!({"flop": board, "accuracy_pct": acc, "button": bb(&vals[1]), "big_blind": bb(&vals[0])}));
                        }
                        say(format!("pass {pass} | {} | flop {} of {nflops} ({board}) | {:.0}s, within {:.2}% of the pot | {:.0} min so far", l.path, k + 1, t.elapsed().as_secs_f64(), acc, t0.elapsed().as_secs_f64() / 60.0));
                    }
                    let mut out = [0, 1].map(|p| (0..169).map(|c| if den_v[p][c] > 0.0 { num_v[p][c] / den_v[p][c] } else { f64::NAN }).collect::<Vec<f64>>());
                    let mut kinds_detail = Value::Null;
                    if valuing == "kinds" {
                        let kv = [0, 1].map(|p| pushfold::flopweights::value_by_kinds(&flop_cards, &flop_fit.weight, &by_flop[p], lean));
                        let nums = |v: &[f64]| -> Vec<Value> { v.iter().map(|x| if x.is_finite() { json!((x * 1e4).round() / 1e4) } else { Value::Null }).collect() };
                        let table = |k: &pushfold::flopweights::KindValues| json!({"own": k.own.iter().map(|r| nums(r)).collect::<Vec<_>>(), "flops": k.count.iter().map(|r| nums(r)).collect::<Vec<_>>(), "used": k.used.iter().map(|r| nums(r)).collect::<Vec<_>>()});
                        if detail.is_some() { kinds_detail = json!({"kinds": pushfold::flopweights::KIND_NAMES, "lean": lean, "weights_only_button": nums(&out[0]), "weights_only_big_blind": nums(&out[1]), "button": table(&kv[0]), "big_blind": table(&kv[1])}); }
                        out = [kv[0].value.clone(), kv[1].value.clone()];
                    }
                    if let Some(path) = &detail {
                        if pass != detail_round { detail_round = pass; detail_lines.clear(); }
                        let avg = |v: &Vec<f64>| -> Vec<Value> { v.iter().map(|x| if x.is_finite() { json!((x * 1e4).round() / 1e4) } else { Value::Null }).collect() };
                        detail_lines.push(json!({"line": l.path, "pot_bb": l.pot_bb, "behind_bb": l.behind_bb, "button_range": l.button_range, "big_blind_range": l.big_blind_range,
                            "weighted_button": avg(&out[0]), "weighted_big_blind": avg(&out[1]), "by_kind": kinds_detail, "flops": detail_flops}));
                        let doc = json!({"round": pass, "classes": (0..169).map(pushfold::class_name).collect::<Vec<_>>(), "flop_names": flop_names,
                            "weighting": if plain_flops { "plain" } else { "fitted" }, "valuing": valuing,
                            "weights": flop_fit.weight.iter().map(|w| w.iter().map(|x| (x * 1e5).round() / 1e5).collect::<Vec<f64>>()).collect::<Vec<_>>(),
                            "note": "values are in big blinds out of the line's pot, before the two sides are scaled to add up to it; weights[class][flop] add to 1 for each class (fitted weighting only)",
                            "lines": detail_lines});
                        let file = format!("{path}.round{pass}.json");
                        std::fs::write(&file, doc.to_string()).map_err(|e| format!("could not write {file}: {e}"))?;
                    }
                    Ok((out, nflops, worst))
                };
                let save = flag("--save");
                let mut report = |i: &pushfold::preflop::PassInfo, so_far: &pushfold::preflop::Preflop| {
                    // the answer so far goes to disk after every round (written beside, then renamed, so the file is never half-written)
                    if let Some(path) = &save {
                        if let Ok(mut v) = serde_json::to_value(so_far) {
                            if let Some(m) = v.as_object_mut() { m.insert("ok".into(), json!(true)); m.insert("version".into(), json!(version_string())); m.insert("rounds_done".into(), json!(i.pass)); m.insert("rounds_planned".into(), json!(passes)); }
                            let tmp = format!("{path}.part");
                            if std::fs::write(&tmp, v.to_string()).is_ok() { let _ = std::fs::rename(&tmp, path); }
                        }
                    }
                    say(format!("== after pass {}: {} =={}", i.pass, i.root_actions.iter().zip(&i.root_freqs).map(|(a, f)| format!("{a} {:.1}%", f * 100.0)).collect::<Vec<_>>().join(", "), if i.settled { " settled" } else { "" }));
                    for l in &i.lines { say(format!("   {} ({:.1}% of hands): button's share was {:.1}%, flop solves gave {:.1}%; the two sides' values came to {:.1}% of the pot", l.path, l.freq * 100.0, l.share_before * 100.0, l.share_solved * 100.0, l.values_total * 100.0)); }
                };
                // --resume: carry on from the round a stopped run last finished (kept beside the --save file)
                let want_resume = args.iter().any(|a| a == "--resume");
                if want_resume && save.is_none() { return Err("--resume needs --save <file>: the run's progress is kept beside that file".into()); }
                let cp_path = save.as_ref().map(|p| format!("{p}.resume"));
                let settings = json!({"spec": serde_json::to_value(&spec).map_err(|e| e.to_string())?, "flops": nflops, "flop_list": flop_names, "flop_weights": if plain_flops { "plain" } else { "fitted" }, "min_line": min_freq, "flop_accuracy": accuracy});
                // newer options join the settings only when used, so runs saved before they existed can still be resumed
                let mut settings = settings;
                if valuing == "kinds" { settings["valuing"] = json!({"kinds": true, "lean": lean}); }
                if rule != pushfold::preflop::StepRule::Fixed(0.35) { settings["rounds"] = json!(rule); }
                if let Some(p) = &start_from { settings["start_from"] = json!(p); }
                if floor > 0.0 { settings["floor"] = json!(floor); }
                let mut start: Option<pushfold::preflop::Checkpoint> = None;
                if want_resume {
                    if let Some(text) = cp_path.as_ref().and_then(|p| std::fs::read_to_string(p).ok()) {
                        let v: Value = serde_json::from_str(&text).map_err(|_| "the .resume file beside the --save file could not be read; delete it to start again".to_string())?;
                        if v["settings"] != settings { return Err("the saved progress was made with different settings (stack, sizes, ante, flops, min line or flop accuracy); use the same settings, or delete the .resume file to start again".into()); }
                        let c: pushfold::preflop::Checkpoint = serde_json::from_value(v["checkpoint"].clone()).map_err(|_| "the .resume file beside the --save file could not be read; delete it to start again".to_string())?;
                        say(format!("== resuming after round {} of {passes} ==", c.rounds_done));
                        start = Some(c);
                    } else { say("== no saved progress found; starting from round 1 ==".to_string()); }
                }
                let mut keep = |c: &pushfold::preflop::Checkpoint| {
                    if let Some(p) = &cp_path { let tmp = format!("{p}.part"); if std::fs::write(&tmp, json!({"settings": settings, "checkpoint": c}).to_string()).is_ok() { let _ = std::fs::rename(&tmp, p); } }
                };
                let opts = pushfold::preflop::AccurateOpts { step: rule, settled_points: settle, min_rounds: 8, start: start_sigmas };
                pushfold::preflop::solve_accurate_opts(&spec, passes, min_freq, &mut eval, &mut report, start, &mut keep, &opts)?
            } else { pushfold::preflop::solve(&spec)? };
            let mut v = serde_json::to_value(&r).map_err(|e| e.to_string())?;
            let m = v.as_object_mut().unwrap();
            m.insert("ok".into(), json!(true)); m.insert("version".into(), json!(version_string()));
            m.insert("units".into(), json!("stacks, pots, amounts and EVs in big blinds; frequencies 0-1; the 169 values in `strategy` and `range` follow `classes`; the small blind is the button and acts last after the flop"));
            Ok(v)
        })(),
        Some("pushfold") if flag("--stacks").is_some() => (|| -> Result<Value, String> {
            let list = |name: &str| -> Result<Vec<f64>, String> { match flag(name) { None => Ok(vec![]), Some(t) if t.trim().is_empty() => Ok(vec![]), Some(t) => t.split(',').map(|x| x.trim().parse::<f64>().map_err(|_| format!("{name} needs numbers separated by commas"))).collect() } };
            let num = |name: &str, d: f64| -> Result<f64, String> { match flag(name) { Some(x) => x.parse::<f64>().map_err(|_| format!("{name} needs a number")), None => Ok(d) } };
            let st = list("--stacks")?;
            if st.len() != 3 { return Err("--stacks needs three numbers in big blinds: button, small blind, big blind (use --stack for heads-up)".into()); }
            let spot = pushfold::three::Spot3 { stacks: [st[0], st[1], st[2]], small_blind: num("--sb", 0.5)?, ante: num("--ante", 0.0)?, bb_ante: num("--bb-ante", 0.0)?,
                payouts: list("--payouts")?, others: list("--others")?, iterations: num("--iterations", 200.0)? as usize };
            let r = pushfold::three::solve(&spot)?;
            let mut v = serde_json::to_value(&r).map_err(|e| e.to_string())?;
            let m = v.as_object_mut().unwrap();
            m.insert("ok".into(), json!(true)); m.insert("version".into(), json!(version_string()));
            m.insert("note".into(), json!("each player may only move all-in (or call all-in) or fold. Stacks in big blinds before antes and blinds. Per decision: pct of all hands, freq and edge by hand class (see classes); edge is what committing gains over folding each time the spot comes up."));
            Ok(v)
        })(),
        Some("pushfold") => {
            let num = |name: &str, d: f64| -> Result<f64, String> { match flag(name) { Some(x) => x.parse::<f64>().map_err(|_| format!("{name} needs a number")), None => Ok(d) } };
            (|| -> Result<Value, String> {
                let stack = match flag("--stack") { Some(_) => num("--stack", 0.0)?, None => args.get(1).and_then(|x| x.parse::<f64>().ok()).ok_or("give the stack in big blinds, e.g. pontiac pushfold --stack 10")? };
                let r = pushfold::solve(stack, num("--sb", 0.5)?, num("--ante", 0.0)?)?;
                let list = |v: &Vec<f64>| -> String { r.classes.iter().zip(v).filter(|(_, &f)| f > 0.0).map(|(c, &f)| if f >= 1.0 { c.clone() } else { format!("{c}:{f}") }).collect::<Vec<_>>().join(",") };
                let mut v = serde_json::to_value(&r).map_err(|e| e.to_string())?;
                let m = v.as_object_mut().unwrap();
                m.insert("ok".into(), json!(true)); m.insert("version".into(), json!(version_string()));
                m.insert("jam_range".into(), json!(list(&r.jam))); m.insert("call_range".into(), json!(list(&r.call)));
                m.insert("units".into(), json!("stack, blinds, antes, EVs and edges in big blinds; stack is the shorter stack before posting; frequencies 0-1; the 169 values follow `classes`"));
                m.insert("note".into(), json!("small blind may only move all-in or fold; limps and smaller raises are not part of this calculation"));
                Ok(v)
            })()
        }
        _ => { eprintln!("{}", usage()); std::process::exit(2); }
    };
    match res {
        Ok(v) => println!("{}", serde_json::to_string_pretty(&v).unwrap()),
        Err(e) => { println!("{}", json!({"ok": false, "error": e})); std::process::exit(1); }
    }
}

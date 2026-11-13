//! Pontiac Solver command line: solve a heads-up postflop spot from a JSON file, save the solve,
//! and query any node of a saved solve without solving again. AGPL-3.0-or-later.
#[allow(dead_code)]
#[path = "../../app/src-tauri/src/study_core.rs"]
mod study_core;

use postflop_solver::*;
use serde_json::{json, Map, Value};
use std::time::Instant;
use study_core::{collect_locks, Leak};

const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Chips per big blind inside the engine (it works in whole chips).
const CHIPS: f64 = 100.0;
const RANKS: &str = "23456789TJQKA";
const SUITS: &str = "cdhs";
const SEATS: [&str; 9] = ["SB", "BB", "UTG", "UTG1", "UTG2", "LJ", "HJ", "CO", "BTN"];

fn version_string() -> String { format!("pontiac {VERSION} (two-player engine: postflop-solver nodelock fork)") }

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
fn round(x: f64, d: i32) -> f64 { if !x.is_finite() { return f64::NAN; } let m = 10f64.powi(d); (x * m).round() / m }

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
    if players.len() == 3 { return Err("three-player spots are solved in the Pontiac Solver app (3-Way Pots tab); the command line handles two players for now".into()); }
    if players.len() != 2 || players[0] == players[1] { return Err("\"players\" must be two different names, e.g. [\"SB\", \"BB\"]".into()); }
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
    PostFlopGame::with_config(spot.card.clone(), tree)
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

fn solve(spot_path: &str, out_path: Option<&str>, save_path: Option<&str>) -> Result<Value, String> {
    let text = std::fs::read_to_string(spot_path).map_err(|e| format!("cannot read {spot_path}: {e}"))?;
    let raw: Value = serde_json::from_str(&text).map_err(|e| format!("{spot_path} is not valid JSON: {e}"))?;
    let spot = parse_spot(&raw)?;
    let threads = if spot.threads > 0 { spot.threads } else { std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2) };
    let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().map_err(|e| e.to_string())?;
    let t_all = Instant::now();
    let locks: Vec<Value> = raw.get("node_locks").and_then(|x| x.as_array()).cloned().unwrap_or_default();
    let habits: Vec<Value> = raw.get("habits").and_then(|x| x.as_array()).cloned().unwrap_or_default();
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
    format!("{}\n\nusage:\n  pontiac solve <spot.json> [-o result.json] [--save solved.pontiac]\n  pontiac query <solved.pontiac> --node <path> [--hand AhKd | --hand AKs] [--detail class|combo]\n  pontiac estimate <spot.json>\n  pontiac version\n\nNode paths look like: flop/BB_check/SB_bet0.33/BB_call/turn=7h/BB_check", version_string())
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
            .and_then(|t| serde_json::from_str::<Value>(&t).map_err(|e| e.to_string())).and_then(|raw| parse_spot(&raw))
            .and_then(|spot| { let g = build(&spot)?; let (m, mc) = g.memory_usage();
                let two = ["habits", "node_locks"].iter().any(|k| spot.raw.get(*k).and_then(|x| x.as_array()).map_or(false, |a| !a.is_empty()));
                Ok(json!({"memory_mb": round(m as f64 / 1048576.0, 1), "memory_mb_compressed": round(mc as f64 / 1048576.0, 1), "solves_at_most": if two { 2 } else { 1 }, "out_of_position": spot.names[0], "in_position": spot.names[1]})) }),
        _ => { eprintln!("{}", usage()); std::process::exit(2); }
    };
    match res {
        Ok(v) => println!("{}", serde_json::to_string_pretty(&v).unwrap()),
        Err(e) => { println!("{}", json!({"ok": false, "error": e})); std::process::exit(1); }
    }
}

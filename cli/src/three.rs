//! Three-player spots for the command line, on the multiway engine.
use multiway::game::{Game, Kind};
use multiway::study::*;
use serde_json::{json, Map, Value};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;

use crate::{round, version_string, CHIPS, SEATS};

const RANKS: &str = "AKQJT98765432";

fn class_of_cell(i: usize) -> String {
    let (r, c) = (i / 13, i % 13);
    let ch = |k: usize| RANKS.as_bytes()[k] as char;
    if r == c { format!("{}{}", ch(r), ch(c)) } else if c > r { format!("{}{}s", ch(r), ch(c)) } else { format!("{}{}o", ch(c), ch(r)) }
}

fn pct_list(v: Option<&Value>, what: &str) -> Result<String, String> {
    let Some(v) = v else { return Ok(String::new()) };
    let arr = v.as_array().ok_or(format!("{what} must be a list"))?;
    let mut out = vec![];
    for x in arr { let f = x.as_f64().ok_or(format!("{what}: bet sizes must be numbers (shares of the pot)"))?; out.push(format!("{}%", round(f * 100.0, 1))); }
    Ok(out.join(", "))
}

/// Turns a spot file with three players into the engine's settings. Returns it with the names in acting order.
pub fn spec_from(raw: &Value, players: &[String]) -> Result<(Spec3, [String; 3]), String> {
    let seat = |n: &str| SEATS.iter().position(|s| s.eq_ignore_ascii_case(n));
    let mut order: Vec<usize> = (0..3).collect();
    if let Some(o) = raw.get("order").and_then(|x| x.as_array()) {
        order = o.iter().map(|n| players.iter().position(|p| Some(p.as_str()) == n.as_str()).ok_or("\"order\" must list the three players".to_string())).collect::<Result<_, _>>()?;
        if order.len() != 3 { return Err("\"order\" must list the three players".into()); }
    } else if players.iter().all(|p| seat(p).is_some()) {
        order.sort_by_key(|&i| seat(&players[i]).unwrap());
    }
    let names: [String; 3] = [players[order[0]].clone(), players[order[1]].clone(), players[order[2]].clone()];
    let ranges = raw.get("ranges").ok_or("\"ranges\" is required")?;
    let range = |n: &str| -> Result<String, String> { Ok(ranges.get(n).and_then(|r| r.as_str()).ok_or(format!("ranges.{n} is missing"))?.to_string()) };
    let num = |k: &str| raw.get(k).and_then(|x| x.as_f64());
    let pot = num("pot_bb").ok_or("\"pot_bb\" is required")?;
    let stacks: [f64; 3] = if let Some(s) = raw.get("stacks_bb") {
        let one = |n: &str| s.get(n).and_then(|x| x.as_f64()).ok_or(format!("stacks_bb.{n} is missing"));
        [one(&names[0])? * CHIPS, one(&names[1])? * CHIPS, one(&names[2])? * CHIPS]
    } else {
        let e = num("effective_stack_bb").ok_or("give \"effective_stack_bb\" (same for everyone) or \"stacks_bb\": {name: stack}")?;
        [e * CHIPS; 3]
    };
    let board: String = raw.get("board").and_then(|b| b.as_str()).ok_or("\"board\" is required")?.chars().filter(|c| !c.is_whitespace()).collect();
    let cards = board.len() / 2;
    let bt = raw.get("bet_tree").cloned().unwrap_or(json!({}));
    let st = |s: &str| bt.get(s).cloned().unwrap_or(Value::Null);
    // raise sizes are shared by all streets: numbers are multiples of the bet, "60%" is a share of the pot
    let mut mults = vec![]; let mut pots = vec![];
    for s in ["flop", "turn", "river"] {
        if let Some(arr) = st(s).get("raise").and_then(|x| x.as_array()) {
            for x in arr {
                if let Some(f) = x.as_f64() { let t = format!("{}", round(f, 2)); if !mults.contains(&t) { mults.push(t); } }
                else if let Some(t) = x.as_str() { if !pots.contains(&t.to_string()) { pots.push(t.to_string()); } }
            }
        }
    }
    // prize money: {"payouts": [50, 30, 20], "others_bb": [25, 12]}; the three players' stacks are the spot's own
    let (mut icm_payouts, mut icm_others) = (vec![], vec![]);
    if let Some(v) = raw.get("icm").filter(|x| !x.is_null()) {
        let help = "icm: give {\"payouts\": [50, 30, 20], \"others_bb\": [25, 12]} (others are players not in this hand; the three players' stacks are the spot's)";
        let list = |k: &str| -> Result<Vec<f64>, String> { match v.get(k) { None => Ok(vec![]), Some(x) => x.as_array().ok_or(help)?.iter().map(|y| y.as_f64().filter(|z| *z >= 0.0).ok_or(help.to_string())).collect() } };
        icm_payouts = list("payouts")?; icm_others = list("others_bb")?.iter().map(|x| x * CHIPS).collect();
        if icm_payouts.is_empty() { return Err(help.into()); }
    }
    let (rake_rate, rake_cap) = match raw.get("rake") {
        Some(Value::Object(o)) => (o.get("rate").and_then(|x| x.as_f64()).unwrap_or(0.0), o.get("cap_bb").and_then(|x| x.as_f64()).unwrap_or(0.0) * CHIPS),
        Some(Value::Number(n)) => (n.as_f64().unwrap_or(0.0), 0.0),
        _ => (0.0, 0.0),
    };
    let (turn_cards, river_cards) = match (cards, raw.get("later_cards").and_then(|x| x.as_str()).unwrap_or(if cards == 3 { "sample" } else { "exact" })) {
        (3, "sample") => (1, 1), (3, "larger") => (2, 1), (3, "all_turns") => (0, 1), (4, "sample") => (0, 1), (_, "exact") => (0, 0),
        (_, other) => return Err(format!("later_cards '{other}': use sample, larger, all_turns or exact")),
    };
    let leaks = raw.get("habits").and_then(|x| x.as_array()).cloned().unwrap_or_default().iter().map(|h| {
        let name = h.get("player").and_then(|x| x.as_str()).unwrap_or("");
        Ok(Leak3 { player: names.iter().position(|n| n == name).ok_or(format!("habits: player '{name}' is not in this spot"))?,
            street: h.get("street").and_then(|x| x.as_str()).unwrap_or("all").to_string(),
            effect: h.get("effect").and_then(|x| x.as_str()).ok_or("habits: each habit needs an \"effect\"")?.to_string(),
            strength: h.get("strength").and_then(|x| x.as_f64()).unwrap_or(0.6) as f32 })
    }).collect::<Result<Vec<_>, String>>()?;
    let node_locks = raw.get("node_locks").and_then(|x| x.as_array()).cloned().unwrap_or_default().iter().map(|l| {
        let body = l.get("lock").unwrap_or(l);
        let freqs = body.get("action_freqs").and_then(|x| x.as_object()).ok_or("node_locks: each lock needs \"action_freqs\"")?;
        Ok(NodeLock3 { node: l.get("node").and_then(|x| x.as_str()).ok_or("node_locks: each lock needs a \"node\" path")?.to_string(),
            player: l.get("player").and_then(|x| x.as_str()).map(|s| s.to_string()),
            hands: body.get("hands").and_then(|x| x.as_str()).map(|s| s.to_string()),
            action_freqs: freqs.iter().map(|(k, v)| Ok((k.clone(), v.as_f64().ok_or("node_locks: frequencies must be numbers")?))).collect::<Result<_, String>>()? })
    }).collect::<Result<Vec<_>, String>>()?;
    let max_raises = bt.get("max_raises").and_then(|x| x.as_u64()).unwrap_or(3) as usize;
    let mut folded = [false; 3];
    for n in raw.get("folded").and_then(|x| x.as_array()).cloned().unwrap_or_default() {
        let k = names.iter().position(|p| Some(p.as_str()) == n.as_str()).ok_or("\"folded\" must name players from \"players\"")?;
        folded[k] = true;
    }
    let spec = Spec3 {
        folded,
        ranges: [range(&names[0])?, range(&names[1])?, range(&names[2])?], board, pot: pot * CHIPS, stack: stacks[0], stacks: Some(stacks),
        flop_bet_sizes: pct_list(st("flop").get("bet"), "bet_tree.flop.bet")?, turn_bet_sizes: pct_list(st("turn").get("bet"), "bet_tree.turn.bet")?,
        bet_sizes: pct_list(st("river").get("bet"), "bet_tree.river.bet")?,
        raise_sizes: pots.join(", "), raise_mults: mults.join(", "),
        flop_max_raises: max_raises, turn_max_raises: max_raises, max_raises,
        allin_spr: bt.get("allin_spr").and_then(|x| x.as_f64()).unwrap_or(1.0),
        accuracy_pct: num("target_exploitability_pct").unwrap_or(0.5), max_iter: num("max_iterations").unwrap_or([400.0, 300.0, 600.0][cards.clamp(3, 5) - 3]) as u32,
        max_seconds: num("max_seconds").unwrap_or(600.0), threads: num("threads").unwrap_or(0.0) as usize,
        turn_cards, river_cards, rake_rate, rake_cap, leaks, node_locks, names: Some(names.clone()), chips_per_bb: CHIPS,
        icm_payouts, icm_others,
    };
    // the engine needs explicit sizes on the street the spot starts on or later; an empty street is check/call only
    Ok((spec, names))
}

fn node_json(v: &NodeView3, names: &[String; 3], path: &str, detail: &str) -> Value {
    let mut o = Map::new();
    o.insert("path".into(), json!(path)); o.insert("street".into(), json!(v.street)); o.insert("board".into(), json!(v.board.join("")));
    o.insert("pot_bb".into(), json!(round(v.pot / CHIPS, 2)));
    o.insert("line_frequency".into(), json!(round(v.line_frequency, 5)));
    let per = |x: &[f64; 3], d: f64, dec: i32| -> Value { json!({names[0].clone(): round(x[0] / d, dec), names[1].clone(): round(x[1] / d, dec), names[2].clone(): round(x[2] / d, dec)}) };
    o.insert("ev_bb".into(), per(&v.ev, CHIPS, 3)); o.insert("equity".into(), per(&v.equity, 1.0, 4));
    o.insert("stacks_bb".into(), per(&v.stacks, CHIPS, 2));
    o.insert("in_hand".into(), json!((0..3).filter(|&p| !v.folded[p]).map(|p| names[p].clone()).collect::<Vec<_>>()));
    match v.kind.as_str() {
        "terminal" => { o.insert("kind".into(), json!("terminal")); o.insert("result".into(), json!(v.result)); }
        "chance" => {
            o.insert("kind".into(), json!("deal"));
            let next = if v.street == "flop" { "turn" } else { "river" };
            o.insert("next_paths".into(), json!(v.cards.iter().map(|c| format!("{path}/{next}={}", c.0)).collect::<Vec<_>>()));
            o.insert("cards_solved".into(), json!(v.cards.len())); o.insert("cards_possible".into(), json!(v.cards_possible));
        }
        _ => {
            let me = v.player;
            o.insert("kind".into(), json!("decision")); o.insert("player".into(), json!(names[me]));
            o.insert("locked".into(), json!(v.locked)); o.insert("reached".into(), json!(v.line_frequency > 0.0));
            let acts: Vec<Value> = (0..v.actions.len()).map(|a| json!({"name": v.action_names[a], "amount_bb": round(v.action_amounts[a] / CHIPS, 2),
                "frequency": round(v.action_freqs[a], 4), "path": format!("{}/{}_{}", path, names[me], v.action_names[a])})).collect();
            o.insert("actions".into(), json!(acts));
            o.insert("range_ev_bb".into(), json!(round(v.ev[me] / CHIPS, 3))); o.insert("range_equity".into(), json!(round(v.equity[me], 4)));
            o.insert("range_combos".into(), json!(round(v.range_combos[me], 2)));
            let freqs = |f: &[f64]| -> Map<String, Value> { (0..f.len()).map(|a| (v.action_names[a].clone(), json!(round(f[a], 4)))).collect() };
            let mut by_class = Map::new();
            for (i, c) in v.grid.iter().enumerate() {
                if c.weight <= 1e-6 { continue; }
                by_class.insert(class_of_cell(i), json!({"combos": round(c.weight, 3), "freqs": freqs(&c.freqs), "ev_bb": round(c.ev / CHIPS, 3), "equity": round(c.equity, 4)}));
            }
            o.insert("by_class".into(), Value::Object(by_class));
            if detail == "combo" {
                let by_combo: Map<String, Value> = v.combos.iter().map(|c| (c.hand.clone(), json!({"weight": round(c.weight, 4), "freqs": freqs(&c.freqs), "ev_bb": round(c.ev / CHIPS, 3), "equity": round(c.equity, 4)}))).collect();
                o.insert("by_combo".into(), Value::Object(by_combo));
            }
        }
    }
    Value::Object(o)
}

/// (path, history) of every decision on the starting street.
fn first_street(game: &Game, names: &[String; 3], limit: usize) -> Vec<(String, Vec<usize>)> {
    let start = match game.start_len() { 3 => "flop", 4 => "turn", _ => "river" };
    let mut out = vec![];
    let mut stack: Vec<(usize, String, Vec<usize>)> = vec![(0, start.to_string(), vec![])];
    while let Some((id, path, hist)) = stack.pop() {
        if out.len() >= limit { break; }
        let node = &game.nodes[id];
        if node.kind != Kind::Decision { continue; }
        out.push((path.clone(), hist.clone()));
        let an = action_names(game, id);
        for i in (0..an.len()).rev() { let mut h = hist.clone(); h.push(i); stack.push((node.children[i], format!("{}/{}_{}", path, names[node.player], an[i]), h)); }
    }
    out
}

fn summary_json(names: &[String; 3], iters: u32, secs: f64, ev: &[f64; 3], gain: &[f64; 3], spec: &Spec3, cancelled: bool) -> Value {
    let worst = gain.iter().cloned().fold(0.0, f64::max);
    let stopped = if worst <= spec.accuracy_pct { "target_reached" } else if iters >= spec.max_iter.max(20) { "max_iterations" } else if cancelled { "max_seconds" } else { "max_iterations" };
    json!({"iterations": iters, "runtime_seconds": round(secs, 2), "stopped_because": stopped,
        "exploitability_pct": round(worst, 3),
        "gain_pct_by_player": {names[0].clone(): round(gain[0], 3), names[1].clone(): round(gain[1], 3), names[2].clone(): round(gain[2], 3)},
        "ev_bb": {names[0].clone(): round(ev[0] / CHIPS, 3), names[1].clone(): round(ev[1] / CHIPS, 3), names[2].clone(): round(ev[2] / CHIPS, 3)}})
}

pub fn estimate3(raw: &Value, players: &[String]) -> Result<Value, String> {
    let (spec, names) = spec_from(raw, players)?;
    let e = estimate(&spec)?;
    Ok(json!({"memory_mb": round(e.memory_mb, 1), "decision_points": e.nodes, "seconds_per_iteration": round(e.secs_per_iter, 2), "acting_order": names,
        "solves_at_most": if spec.leaks.is_empty() && spec.node_locks.is_empty() { 1 } else { 2 }}))
}

pub fn solve(raw: &Value, players: &[String], save: Option<&str>) -> Result<Value, String> {
    if !matches!(raw.get("auto_sizes"), None | Some(Value::Null) | Some(Value::Bool(false))) { return Err("auto_sizes is for two-player spots; a three-player spot solves the sizes listed".into()); }
    let (spec, names) = spec_from(raw, players)?;
    let est = estimate(&spec)?;
    if let Some(cap) = raw.get("max_memory_mb").and_then(|x| x.as_f64()) { if est.memory_mb > cap { return Err(format!("this spot needs about {:.0} MB, over max_memory_mb = {cap}", est.memory_mb)); } }
    eprintln!("{} | {} / {} / {} | about {:.0} MB", version_string(), names[0], names[1], names[2], est.memory_mb);
    let prog = Arc::new(Progress3::default());
    let (p2, done) = (prog.clone(), Arc::new(std::sync::atomic::AtomicBool::new(false)));
    let d2 = done.clone();
    let t0 = Instant::now();
    let watcher = std::thread::spawn(move || {
        let mut last = (0u8, 0u32);
        while !d2.load(Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_millis(500));
            let now = (p2.phase.load(Ordering::Relaxed), p2.iter.load(Ordering::Relaxed));
            if now != last && now.1 % 10 == 0 && now.1 > 0 {
                eprintln!("[{}] step {}: {:.2}% of the pot, {:.0}s", if now.0 == 2 { "locked" } else { "normal" }, now.1, p2.gain_milli_pct.load(Ordering::Relaxed) as f64 / 1000.0, t0.elapsed().as_secs_f64());
                last = now;
            }
        }
    });
    let res = solve3(&spec, &prog);
    done.store(true, Ordering::Relaxed); let _ = watcher.join();
    let st = res?;
    let s = st.summary.clone().unwrap();
    let detail = raw.get("detail").and_then(|x| x.as_str()).unwrap_or("combo").to_string();
    let limit = raw.get("max_report_nodes").and_then(|x| x.as_u64()).unwrap_or(60) as usize;
    let nodes_of = |locked: bool| -> Result<Value, String> {
        let g = if locked { st.locked.as_ref().unwrap() } else { st.base.as_ref().unwrap() };
        let mut list = first_street(g, &names, limit);
        if let Some(extra) = raw.get("report_nodes").and_then(|x| x.as_array()) {
            for p in extra { if let Some(p) = p.as_str() { let (h, _) = resolve_path(g, &names, p, CHIPS)?; list.push((p.trim_matches('/').to_string(), h)); } }
        }
        let mut out = Map::new();
        for (path, hist) in list { let v = st.view(locked, &hist, true)?; out.insert(path.clone(), node_json(&v, &names, &path, &detail)); }
        Ok(Value::Object(out))
    };
    let mut result = Map::new();
    result.insert("version".into(), json!(version_string()));
    result.insert("spot".into(), raw.clone());
    result.insert("acting_order".into(), json!(names));
    result.insert("memory_mb".into(), json!(round(s.memory_mb, 1)));
    result.insert("units".into(), json!("EVs and amounts in big blinds; ev_bb counts the pot at the start of the spot, so the three EVs add up to pot_bb less any rake; frequencies and equity are 0-1"));
    result.insert("exploitability_note".into(), json!("with three players this is the most any one player could gain by changing only their own strategy, as a percent of the starting pot"));
    let fin = if s.locked {
        let mut normal = summary_json(&names, s.base_iters, s.base_secs, &s.base_ev, &s.base_gain_pct, &spec, false);
        if raw.get("baseline_nodes").and_then(|x| x.as_bool()).unwrap_or(true) { normal["nodes"] = nodes_of(false)?; }
        result.insert("normal".into(), normal);
        summary_json(&names, s.locked_iters, s.locked_secs, &s.locked_ev, &s.locked_gain_pct, &spec, s.cancelled)
    } else { summary_json(&names, s.base_iters, s.base_secs, &s.base_ev, &s.base_gain_pct, &spec, s.cancelled) };
    for (k, v) in fin.as_object().unwrap() { result.insert(k.clone(), v.clone()); }
    result.insert("locked_decisions".into(), json!(s.locked_nodes));
    if let Some(icm) = st.base.as_ref().and_then(|g| g.icm.as_ref()) {
        let ev = if s.locked { &s.locked_ev } else { &s.base_ev };
        let (mut share, mut split) = (Map::new(), Map::new());
        for p in 0..3 { share.insert(names[p].clone(), json!(round(icm.share_pct(p, ev[p]), 3))); split.insert(names[p].clone(), json!(round(icm.split_pct(p), 3))); }
        result.insert("icm".into(), json!({"prize_pool_share_pct": share, "if_pot_were_split_pct": split,
            "note": "Payoffs are prize money (Independent Chip Model), not chips. ev_bb, EVs in the node reports and exploitability are in chip-sized units of prize money: prize pool divided by chips in play, per chip. prize_pool_share_pct is each player's expected share of the prize pool after this spot. The three players' ev_bb add up to less than the pot when there are other players, who gain when these three clash. A player who has folded still has a stake in how the others finish, so their EV is no longer zero."}));
    }
    result.insert("nodes".into(), nodes_of(s.locked)?);
    result.insert("total_seconds".into(), json!(round(t0.elapsed().as_secs_f64(), 2)));
    if let Some(path) = save {
        let memo = json!({"version": version_string(), "names": names, "spot": raw}).to_string();
        multiway::save::save_file(&st, &memo, path)?;
        result.insert("saved_to".into(), json!(path));
    }
    Ok(Value::Object(result))
}

/// One node of a saved three-player solve. With a lock or habit the file holds two solves:
/// the locked one is reported unless `normal` is set.
pub fn query(save: &str, node: &str, only: Option<&str>, detail: &str, normal: bool) -> Result<Value, String> {
    let (st, memo) = multiway::save::load_file(save)?;
    let m: Value = serde_json::from_str(&memo).map_err(|_| "this file was not saved by pontiac".to_string())?;
    let name = |i: usize| m["names"][i].as_str().map(|s| s.to_string()).ok_or("this file was not saved by pontiac".to_string());
    let names: [String; 3] = [name(0)?, name(1)?, name(2)?];
    let locked = st.locked.is_some() && !normal;
    if normal && st.locked.is_none() { return Err("this solve has no locks or habits, so there is no separate normal solve in the file".into()); }
    let g = if locked { st.locked.as_ref().unwrap() } else { st.base.as_ref().unwrap() };
    let start = match g.start_len() { 3 => "flop", 4 => "turn", _ => "river" };
    let node = if node.trim_matches('/').is_empty() { start } else { node };
    let (hist, _) = resolve_path(g, &names, node, CHIPS)?;
    let v = st.view(locked, &hist, true)?;
    let mut r = node_json(&v, &names, node.trim_matches('/'), if only.is_some() { "combo" } else { detail });
    if let Some(want) = only {
        // narrow to one exact hand ("AhKd", either order) or one class ("AKs")
        let o = r.as_object_mut().unwrap();
        let alt = if want.len() == 4 { format!("{}{}", &want[2..4], &want[0..2]) } else { String::new() };
        let mut found = false;
        if let Some(Value::Object(c)) = o.get_mut("by_combo") {
            let class_of = |hand: &str| -> Option<String> { v.combos.iter().find(|x| x.hand == hand).map(|x| class_of_cell(x.cell)) };
            c.retain(|k, _| k.eq_ignore_ascii_case(want) || k.eq_ignore_ascii_case(&alt) || class_of(k).map_or(false, |cn| cn.eq_ignore_ascii_case(want)));
            found = !c.is_empty();
            let keep: std::collections::HashSet<String> = c.keys().filter_map(|k| class_of(k)).collect();
            if let Some(Value::Object(bc)) = o.get_mut("by_class") { bc.retain(|k, _| keep.contains(k)); }
        }
        if o.get("kind").and_then(|x| x.as_str()) == Some("decision") && !found { return Err(format!("'{want}' is not in {}'s range at this node", names[v.player])); }
    }
    r["version"] = m["version"].clone();
    r["solve"] = json!(if locked { "locked" } else if st.locked.is_some() { "normal" } else { "only" });
    Ok(r)
}

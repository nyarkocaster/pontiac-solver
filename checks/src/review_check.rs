// Checks the hand review against the solve it reads from (run: cargo run --release --bin review_check).
#[path = "../../app/src-tauri/src/study_core.rs"]
mod study_core;
use study_core::*;

fn main() {
    let spec = StudySpec {
        oop_range: "99-22,AJs-A2s,K2s+,Q2s+,J4s+,T6s+,96s+,85s+,74s+,64s+,53s+,43s,ATo-A2o,K6o+,Q8o+,J8o+,T8o+,98o,87o".into(),
        ip_range: "22+,A2s+,K2s+,Q4s+,J6s+,T6s+,96s+,85s+,75s+,64s+,54s,A2o+,K5o+,Q8o+,J8o+,T8o+,98o".into(),
        board: "Js7c2d9h".into(), pot: 40, stack: 100,
        oop_flop_bet: "33%".into(), ip_flop_bet: "33%".into(), turn_bet: "50%".into(), river_bet: "75%".into(),
        raise: "3x".into(), allin_threshold: 1.5, accuracy_pct: 0.3, max_iter: 500, leaks: vec![], threads: 2, low_memory: false, freeze_rest: true, confidence: 1.0, icm: None, ..Default::default()
    };
    let out = solve_inner(&spec, &StudyProgress::default()).unwrap();
    let mut st = StudyState::default();
    st.install(out);
    let step = |who: usize, verb: &str, to: f64| ReviewStep { kind: "action".into(), who, verb: verb.into(), to, card: String::new() };
    let card = |c: &str| ReviewStep { kind: "card".into(), who: 0, verb: String::new(), to: 0.0, card: c.into() };
    let mut fails = 0;
    let mut check = |name: &str, ok: bool, detail: String| { println!("{} {name}: {detail}", if ok { "PASS" } else { fails += 1; "FAIL" }); };

    // the in-position player holds AhJd: checked to, bets half the pot, then checks the river back
    let steps = vec![step(0, "check", 0.0), step(1, "bet", 20.0), step(0, "call", 20.0), card("3s"), step(0, "check", 0.0), step(1, "check", 0.0)];
    let r = st.review(false, 1, "AhJd", &steps).unwrap();
    for x in &r.rows { println!("  row {} hero={} acts={:?} chosen={} played_as={:?} hist={:?}", x.street, x.hero, x.actions, x.chosen, x.played_as, x.history); }
    let hero: Vec<&ReviewRow> = r.rows.iter().filter(|x| x.hero).collect();
    check("two hero decisions are priced", hero.len() == 2 && r.stopped.is_none(), format!("{} rows, stopped {:?}", hero.len(), r.stopped));
    for row in &hero {
        // what the hand is worth at the decision is the mix of its action values
        let mixed: f64 = row.freqs.iter().zip(&row.evs).map(|(f, e)| f * e).sum();
        let v = st.hand(false, &row.history, 1, "AhJd").unwrap();
        check("action values average to the hand's value", (mixed - v.ev).abs() < 0.02 * spec.pot as f64 + 1e-3 && v.acting, format!("{} street: mix {mixed:.3}, hand {:.3}", row.street, v.ev));
        check("frequencies match the hand view", row.freqs.iter().zip(&v.freqs).all(|(a, b)| (a - b).abs() < 1e-6), format!("{:?}", row.freqs));
        let top = row.evs.iter().cloned().fold(f64::MIN, f64::max);
        check("loss is measured from the best action", (row.loss - (top - row.evs[row.chosen])).abs() < 1e-6 && row.loss >= 0.0 && row.evs[row.best] == top, format!("loss {:.3}", row.loss));
    }
    check("total is the sum of the rows", (r.total_loss - hero.iter().map(|x| x.loss).sum::<f64>()).abs() < 1e-9, format!("{:.3}", r.total_loss));
    check("the bet maps to the solve's own size", hero[0].actions[hero[0].chosen].starts_with("Bet") && hero[0].played_as.is_none(), hero[0].actions[hero[0].chosen].clone());

    // a size the solve does not have is matched to the closest and said so
    let odd = st.review(false, 1, "AhJd", &[step(0, "check", 0.0), step(1, "bet", 33.0)]).unwrap();
    check("an odd size is flagged", odd.rows.last().map_or(false, |x| x.played_as.is_some()), format!("{:?}", odd.rows.last().and_then(|x| x.played_as.clone())));
    // a line the solve never takes (the first player shoving the river here) is not priced
    let never = st.review(false, 1, "AhJd", &[step(0, "check", 0.0), step(1, "bet", 20.0), step(0, "call", 20.0), card("3s"), step(0, "bet", 80.0), step(1, "call", 80.0)]).unwrap();
    println!("  (river shove line: {} hero rows, stopped {:?})", never.rows.iter().filter(|x| x.hero).count(), never.stopped);
    // a hand outside the range, and a history that disagrees with the tree, stop the review with a reason
    let out_of_range = st.review(false, 1, "7h2h", &[step(0, "check", 0.0), step(1, "check", 0.0)]).unwrap();
    check("a hand outside the range stops the review", out_of_range.stopped.is_some() && out_of_range.rows.iter().all(|x| !x.hero), format!("{:?}", out_of_range.stopped));
    let wrong_turn = st.review(false, 1, "AhJd", &[step(1, "bet", 20.0)]).unwrap();
    check("the wrong player acting stops the review", wrong_turn.stopped.is_some(), format!("{:?}", wrong_turn.stopped));
    let bad_card = st.review(false, 1, "AhJd", &[step(0, "check", 0.0), step(1, "check", 0.0), card("Ah")]).unwrap();
    check("a card already in the hand stops the review", bad_card.stopped.is_some(), format!("{:?}", bad_card.stopped));
    if fails > 0 { eprintln!("{fails} failed"); std::process::exit(1); }
    println!("all passed");
}

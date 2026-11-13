use postflop_solver::*;
use std::time::Instant;

fn node_freqs(game: &PostFlopGame, player: usize) -> Vec<(String, f32)> {
    let strat = game.strategy();
    let w = game.normalized_weights(player);
    let n = game.private_cards(player).len();
    let acts = game.available_actions();
    let tot: f32 = w.iter().sum();
    acts.iter().enumerate().map(|(a, act)| {
        let s: f32 = (0..n).map(|h| strat[a * n + h] * w[h]).sum();
        (format!("{:?}", act), 100.0 * s / tot)
    }).collect()
}

fn main() {
    let oop = "JJ-22,AQs-A2s,AQo-ATo,K9s+,KJo+,Q9s+,QJo,J9s+,T8s+,97s+,86s+,75s+,65s,54s";
    let ip  = "22+,A2s+,K8s+,Q9s+,J9s+,T8s+,97s+,86s+,75s+,65s,54s,A9o+,KTo+,QTo+,JTo";
    let card_config = CardConfig {
        range: [oop.parse().unwrap(), ip.parse().unwrap()],
        flop: flop_from_str("Ks7h2d").unwrap(),
        turn: NOT_DEALT, river: NOT_DEALT,
    };
    let oop_flop = BetSizeOptions::try_from(("33%", "100%")).unwrap();
    let ip_flop = BetSizeOptions::try_from(("33%, 75%", "100%")).unwrap();
    let later = BetSizeOptions::try_from(("75%", "100%")).unwrap();
    let tree_config = TreeConfig {
        initial_state: BoardState::Flop,
        starting_pot: 31, effective_stack: 185,
        rake_rate: 0.0, rake_cap: 0.0,
        flop_bet_sizes: [oop_flop, ip_flop],
        turn_bet_sizes: [later.clone(), later.clone()],
        river_bet_sizes: [later.clone(), later],
        turn_donk_sizes: None, river_donk_sizes: None,
        add_allin_threshold: 0.0, force_allin_threshold: 0.15, merging_threshold: 0.1,
    };
    let tree = ActionTree::new(tree_config).unwrap();
    let mut game = PostFlopGame::with_config(card_config, tree).unwrap();
    let (m, mc) = game.memory_usage();
    println!("mem {:.2}GB / compressed {:.2}GB", m as f64/1e9, mc as f64/1e9);
    game.allocate_memory(true);
    let lock = std::env::args().nth(1).unwrap_or_default();
    if lock == "station" {
        // OOP checks, IP bets 33%: lock OOP to always call (fold 0, call 1, raise 0)
        game.apply_history(&[0, 1]);
        let n = game.private_cards(0).len();
        let na = game.available_actions().len();
        println!("locking node actions {:?}", game.available_actions());
        let mut s = vec![0.0f32; na * n];
        for h in 0..n { s[n + h] = 1.0; }
        game.lock_current_strategy(&s);
        game.back_to_root();
    }
    let t = Instant::now();
    let expl = solve(&mut game, 2000, 31.0 * 0.003, false);
    println!("solved in {:.1}s, exploitability {:.3}% pot", t.elapsed().as_secs_f32(), 100.0*expl/31.0);
    game.cache_normalized_weights();
    println!("OOP root: {:?}", node_freqs(&game, 0));
    game.play(0); game.cache_normalized_weights();
    println!("IP after check: {:?}", node_freqs(&game, 1));
    // bluff check: how often does IP bet 33% with hands that have <35% equity
    let n = game.private_cards(1).len();
    let strat = game.strategy();
    let eq = game.equity(1); let w = game.normalized_weights(1);
    let (mut wb, mut wt, mut sb, mut st) = (0f32,0f32,0f32,0f32);
    for h in 0..n { let bet = strat[n+h]+strat[2*n+h];
        if eq[h] < 0.35 { wb += w[h]*bet; wt += w[h]; } else { sb += w[h]*bet; st += w[h]; } }
    println!("IP bet freq with weak hands (<35% eq): {:.1}%, with stronger hands: {:.1}%", 100.0*wb/wt, 100.0*sb/st);
    let ev = game.expected_values(1);
    println!("IP EV at this node: {:.2}", compute_average(&ev, &w));
}

//! How much do sampled turn/river cards change hand values, with realistic ranges? (check-down tree)
use multiway::cards::*;
use multiway::game::*;
fn main() {
    let deck = Deck::new();
    let rs = ["99-22,A9s-A2s,K9s-K2s,Q9s+,J9s+,T8s+,97s+,86s+,75s+,65s,54s,ATo-A8o,KTo+,QTo+,JTo", "TT-22,AJs-A2s,KTs+,QTs+,JTs,T9s,98s,87s,76s,AQo-ATo,KQo,KJo", "AA-22,AKs-A2s,K9s+,Q9s+,J9s+,T8s+,98s,87s,76s,65s,AKo-ATo,KJo+,QJo"];
    for b in ["Kh9s6d", "Qs8h7h", "Th9h4h", "7c7d2s"] {
        let board = parse_board(b).unwrap();
        let ranges = [parse_range(&deck, rs[0]).unwrap(), parse_range(&deck, rs[1]).unwrap(), parse_range(&deck, rs[2]).unwrap()];
        let mk = |tc: usize, rc: usize| Game::new(Config { board: board.clone(), pot: 60.0, stack: 100.0, ranges: ranges.clone(), flop_bet_sizes: vec![], turn_bet_sizes: vec![], bet_sizes: vec![], raise_sizes: vec![],
            flop_max_raises: 1, turn_max_raises: 1, max_raises: 1, allin_spr: 0.0, turn_cards: tc, river_cards: rc, extra: Default::default() }, true).unwrap();
        let g = mk(0, 0); let r = &g.cfg.ranges;
        let full: Vec<Vec<f32>> = (0..3).map(|h| g.walk(0, h, [&r[0], &r[1], &r[2]], Mode::Average)).collect();
        let mut line = format!("{b}:");
        for (tc, rc) in [(1usize, 1usize), (2, 1), (2, 2), (0, 1)] {
            let gx = mk(tc, rc); let (mut tot, mut dif, mut mx) = (0f64, 0f64, 0f64);
            for hero in 0..3 {
                let z = g.reach_sum(hero, &r[(hero + 1) % 3], &r[(hero + 2) % 3]);
                let v = gx.walk(0, hero, [&r[0], &r[1], &r[2]], Mode::Average);
                for k in 0..v.len() { tot += full[hero][k].abs() as f64; dif += (full[hero][k] - v[k]).abs() as f64; if z[k] > 0.0 { mx = f64::max(mx, (full[hero][k] - v[k]).abs() as f64 / z[k] / 60.0); } }
            }
            line += &format!("  {}x{}: avg {:.2}%, worst hand {:.1} equity points |", tc, rc, 100.0 * dif / tot, 100.0 * mx);
        }
        println!("{line}");
    }
}

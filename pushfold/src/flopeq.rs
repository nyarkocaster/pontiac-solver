//! All-in equity of every two-card hand on a given flop, exactly, over every turn and river.
//! Only built with the `generate` feature (it needs the hand evaluator from the multiway crate);
//! used by the tools that test and choose flop samples, never by the solver itself.
use multiway::cards::eval7;
use rayon::prelude::*;

/// Equity of every hand (indexed a * 52 + b with a < b; NaN where the flop blocks it) against an
/// opponent holding hand `o` with weight `opp[o]` (same indexing); `None` means any two cards.
pub fn flop_equities(flop: [u8; 3], opp: Option<&[f64]>) -> Vec<f64> {
    let rest: Vec<u8> = (0..52u8).filter(|c| !flop.contains(c)).collect();
    let mut runouts = vec![];
    for a in 0..rest.len() { for b in a + 1..rest.len() { runouts.push((rest[a], rest[b])); } }
    let (sum, cnt) = runouts.par_iter().map(|&(t, r)| {
        let live: Vec<u8> = rest.iter().copied().filter(|c| *c != t && *c != r).collect();
        let mut hands: Vec<(u32, u8, u8, f64)> = vec![];
        for a in 0..live.len() { for b in a + 1..live.len() {
            let w = match opp { Some(o) => o[live[a] as usize * 52 + live[b] as usize], None => 1.0 };
            hands.push((eval7(&[flop[0], flop[1], flop[2], t, r, live[a], live[b]]), live[a], live[b], w));
        } }
        hands.sort_by(|x, y| x.0.cmp(&y.0));
        let mut total = 0f64; let mut total_card = [0f64; 52];
        for h in &hands { total += h.3; total_card[h.1 as usize] += h.3; total_card[h.2 as usize] += h.3; }
        let mut sum = vec![0f64; 52 * 52]; let mut cnt = vec![0f64; 52 * 52];
        let mut below = 0f64; let mut below_card = [0f64; 52];
        let mut i = 0;
        while i < hands.len() {
            let mut j = i; while j < hands.len() && hands[j].0 == hands[i].0 { j += 1; }
            let mut g = 0f64; let mut group_card = [0f64; 52];
            for h in &hands[i..j] { g += h.3; group_card[h.1 as usize] += h.3; group_card[h.2 as usize] += h.3; }
            for h in &hands[i..j] {
                let (a, b) = (h.1 as usize, h.2 as usize);
                let den = total - total_card[a] - total_card[b] + h.3;
                if den <= 0.0 { continue; }
                let wins = below - below_card[a] - below_card[b];
                let ties = g - group_card[a] - group_card[b] + h.3;
                let k = a * 52 + b;
                sum[k] += wins + ties / 2.0; cnt[k] += den;
            }
            for h in &hands[i..j] { below_card[h.1 as usize] += h.3; below_card[h.2 as usize] += h.3; }
            below += g; i = j;
        }
        (sum, cnt)
    }).reduce(|| (vec![0f64; 52 * 52], vec![0f64; 52 * 52]), |mut a, b| { for k in 0..52 * 52 { a.0[k] += b.0[k]; a.1[k] += b.1[k]; } a });
    (0..52 * 52).map(|k| if cnt[k] > 0.0 { sum[k] / cnt[k] } else { f64::NAN }).collect()
}

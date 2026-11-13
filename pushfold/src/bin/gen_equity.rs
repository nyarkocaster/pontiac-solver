//! Builds data/preflop_equity_169.bin: all-in equity of every starting-hand class against every
//! other, from a fixed random sample of boards. Each board scores every pair of hands that can
//! coexist with it, so every class match-up gets hundreds of thousands of samples.
//!   cargo run --release --features generate --bin gen_equity -- 40000
use multiway::cards::*;
use rayon::prelude::*;

fn class_index(c: (u8, u8)) -> usize {
    let (ra, rb) = ((c.0 / 4) as usize, (c.1 / 4) as usize);
    let (hi, lo) = if ra >= rb { (ra, rb) } else { (rb, ra) };
    let (r_hi, r_lo) = (12 - hi, 12 - lo);
    if hi == lo { r_hi * 13 + r_hi } else if c.0 % 4 == c.1 % 4 { r_hi * 13 + r_lo } else { r_lo * 13 + r_hi }
}

fn main() {
    let boards: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(40000);
    let deck = Deck::new();
    let cls: Vec<usize> = deck.cards.iter().map(|&c| class_index(c)).collect();
    let mask: Vec<u64> = deck.cards.iter().map(|&(a, b)| (1u64 << a) | (1u64 << b)).collect();
    let chunks = 64usize;
    let parts: Vec<(Vec<f64>, Vec<f64>)> = (0..chunks).into_par_iter().map(|ch| {
        let mut state: u64 = 0x9E3779B97F4A7C15u64.wrapping_mul(ch as u64 + 1) ^ 0xD1B54A32D192ED03;
        let mut next = move || { state ^= state << 13; state ^= state >> 7; state ^= state << 17; state };
        let (mut win, mut cnt) = (vec![0f64; 169 * 169], vec![0f64; 169 * 169]);
        let mut st = vec![0u32; NH];
        for _ in 0..boards / chunks {
            let mut b = [0u8; 5]; let mut used = 0u64; let mut k = 0;
            while k < 5 { let c = (next() % 52) as u8; if used & (1 << c) == 0 { used |= 1 << c; b[k] = c; k += 1; } }
            let mut live: Vec<usize> = Vec::with_capacity(1081);
            for h in 0..NH {
                if mask[h] & used != 0 { continue; }
                let (x, y) = deck.cards[h];
                st[h] = eval7(&[b[0], b[1], b[2], b[3], b[4], x, y]);
                live.push(h);
            }
            for (i, &h1) in live.iter().enumerate() {
                let (m1, s1, c1) = (mask[h1], st[h1], cls[h1]);
                for &h2 in &live[i + 1..] {
                    if m1 & mask[h2] != 0 { continue; }
                    let c2 = cls[h2];
                    let w = if s1 > st[h2] { 1.0 } else if s1 == st[h2] { 0.5 } else { 0.0 };
                    win[c1 * 169 + c2] += w; cnt[c1 * 169 + c2] += 1.0;
                    win[c2 * 169 + c1] += 1.0 - w; cnt[c2 * 169 + c1] += 1.0;
                }
            }
        }
        (win, cnt)
    }).collect();
    let (mut win, mut cnt) = (vec![0f64; 169 * 169], vec![0f64; 169 * 169]);
    for (w, c) in &parts { for i in 0..169 * 169 { win[i] += w[i]; cnt[i] += c[i]; } }
    let mut bytes = Vec::with_capacity(169 * 169 * 4);
    for i in 0..169 * 169 { bytes.extend_from_slice(&((if cnt[i] > 0.0 { win[i] / cnt[i] } else { 0.5 }) as f32).to_le_bytes()); }
    std::fs::write("data/preflop_equity_169.bin", &bytes).unwrap();
    let e = |a: usize, b: usize| win[a * 169 + b] / cnt[a * 169 + b];
    // spot checks against well-known all-in equities: AA v KK ~0.82, AKs v QQ ~0.46, AKo v 22 ~0.47, 72o v AA ~0.12
    println!("boards {} | AA v KK {:.4} | AKs v QQ {:.4} | AKo v 22 {:.4} | 72o v AA {:.4} | fewest samples per match-up {:.0}",
        (boards / chunks) * chunks, e(0, 14), e(1, 28), e(13, 168), e(12 * 13 + 7, 0), cnt.iter().cloned().fold(f64::MAX, f64::min));
}

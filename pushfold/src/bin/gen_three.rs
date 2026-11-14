//! Builds data/three_way_169.bin: for every unordered triple of starting-hand classes, how many
//! ways the three hands can be dealt together and how often each finishing order happens.
//!   cargo run --release --features generate --bin gen_three -- 20000
//! Per board every live hand is ranked once; each class triple then draws one random set of three
//! live hands, weighted by how many such sets exist so that boards are not over- or under-counted.
use multiway::cards::*;
use pushfold::tables::*;
use rayon::prelude::*;

fn main() {
    let boards: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(20000);
    let deck = Deck::new();
    let cls: Vec<usize> = deck.cards.iter().map(|&(a, b)| class_of(a, b)).collect();
    let mask: Vec<u64> = deck.cards.iter().map(|&(a, b)| (1u64 << a) | (1u64 << b)).collect();
    let base = tri_base();
    // exact number of ways to deal each triple
    let mut by_class: Vec<Vec<u64>> = vec![vec![]; NC];
    for h in 0..NH { by_class[cls[h]].push(mask[h]); }
    let counts: Vec<u16> = (0..NC).into_par_iter().flat_map_iter(|i| {
        let mut out = vec![];
        for j in i..NC { for k in j..NC {
            let mut n = 0u32;
            for &a in &by_class[i] { for &b in &by_class[j] { if a & b != 0 { continue; } for &c in &by_class[k] { if (a | b) & c == 0 { n += 1; } } } }
            out.push(n as u16);
        } }
        out
    }).collect();
    assert_eq!(counts.len(), NTRI);
    let chunks = 32usize;
    let acc: Vec<f32> = (0..chunks).into_par_iter().map(|ch| {
        let mut state: u64 = 0x9E3779B97F4A7C15u64.wrapping_mul(ch as u64 + 7) ^ 0xA24BAED4963EE407;
        let mut next = move || { state ^= state << 13; state ^= state >> 7; state ^= state << 17; state };
        let mut acc = vec![0f32; NTRI * 6];
        let mut live: Vec<Vec<(u32, u64)>> = vec![Vec::with_capacity(12); NC];
        for _ in 0..boards / chunks {
            let mut b = [0u8; 5]; let mut used = 0u64; let mut n = 0;
            while n < 5 { let c = (next() % 52) as u8; if used & (1 << c) == 0 { used |= 1 << c; b[n] = c; n += 1; } }
            for l in live.iter_mut() { l.clear(); }
            for h in 0..NH {
                if mask[h] & used != 0 { continue; }
                let (x, y) = deck.cards[h];
                live[cls[h]].push((eval7(&[b[0], b[1], b[2], b[3], b[4], x, y]), mask[h]));
            }
            for i in 0..NC {
                let (li, ni) = (&live[i], live[i].len() as u64); if ni == 0 { continue; }
                for j in i..NC {
                    let (lj, nj) = (&live[j], live[j].len() as u64); if nj == 0 { continue; }
                    let slot0 = base[i * NC + j] as usize;
                    for k in j..NC {
                        let (lk, nk) = (&live[k], live[k].len() as u64); if nk == 0 { continue; }
                        let r = next();
                        let (a, bb, c) = (li[(((r & 0xFFFFF) * ni) >> 20) as usize], lj[((((r >> 20) & 0xFFFFF) * nj) >> 20) as usize], lk[((((r >> 40) & 0xFFFFF) * nk) >> 20) as usize]);
                        if a.1 & bb.1 != 0 || (a.1 | bb.1) & c.1 != 0 { continue; }
                        let s = [a.0, bb.0, c.0];
                        let w = (ni * nj * nk) as f32;
                        let at = (slot0 + k - j) * 6;
                        let mut ok = [false; 6]; let mut m = 0f32;
                        for (o, p) in ORDERS.iter().enumerate() { if s[p[0]] >= s[p[1]] && s[p[1]] >= s[p[2]] { ok[o] = true; m += 1.0; } }
                        for o in 0..6 { if ok[o] { acc[at + o] += w / m; } }
                    }
                }
            }
        }
        acc
    }).reduce(|| vec![0f32; NTRI * 6], |mut a, b| { for (x, y) in a.iter_mut().zip(&b) { *x += *y; } a });
    // file: counts (u16 LE) then six bytes per triple that add up to 255
    let mut bytes = Vec::with_capacity(NTRI * 8);
    for c in &counts { bytes.extend_from_slice(&c.to_le_bytes()); }
    let mut unseen = 0;
    for t in 0..NTRI {
        let p = &acc[t * 6..t * 6 + 6]; let tot: f32 = p.iter().sum();
        if tot <= 0.0 { if counts[t] > 0 { unseen += 1; } bytes.extend_from_slice(&[43, 43, 43, 42, 42, 42]); continue; }
        let exact: Vec<f32> = p.iter().map(|x| x / tot * 255.0).collect();
        let mut q: Vec<u8> = exact.iter().map(|x| x.floor() as u8).collect();
        let mut left = 255 - q.iter().map(|&x| x as i32).sum::<i32>();
        let mut order: Vec<usize> = (0..6).collect();
        order.sort_by(|&a, &b| (exact[b] - exact[b].floor()).partial_cmp(&(exact[a] - exact[a].floor())).unwrap());
        for &o in &order { if left <= 0 { break; } q[o] += 1; left -= 1; }
        bytes.extend_from_slice(&q);
    }
    std::fs::write("data/three_way_169.bin", &bytes).unwrap();
    // spot check: AA v KK v QQ (first-place shares roughly 0.67 / 0.17 / 0.16)
    let t = (base[0 * NC + 14] as usize + (28 - 14)) * 6;
    let tot: f32 = acc[t..t + 6].iter().sum();
    let first = |pos: usize| ORDERS.iter().enumerate().filter(|(_, p)| p[0] == pos).map(|(o, _)| acc[t + o]).sum::<f32>() / tot;
    println!("boards {} | AA/KK/QQ wins {:.3} {:.3} {:.3} | ways {} | possible triples never sampled {}", (boards / chunks) * chunks, first(0), first(1), first(2), counts[t / 6], unseen);
}

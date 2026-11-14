//! Exact all-in results for every pair and every trio of starting-hand classes, by going through
//! every possible board (one per suit pattern, weighted) and every set of hands that fits on it.
//!   gen_exact [threads] [board_limit]
//! Writes two_way_exact_169.bin, three_way_exact_169.bin and gen_exact_log.txt in the current folder.
use multiway::cards::*;
use pushfold::tables::*;
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};

const ROW: usize = NC * 2;                 // [class of strongest][ties the middle hand?]
const TABLE: usize = NC * NC * 2 * ROW;    // [weakest class][middle class][those two tie?] rows

fn canonical_boards() -> Vec<([u8; 5], u64)> {
    let perms: Vec<[u8; 4]> = { let mut v = vec![]; for a in 0..4u8 { for b in 0..4u8 { for c in 0..4u8 { for d in 0..4u8 { let p = [a, b, c, d]; let mut s = p; s.sort(); if s == [0, 1, 2, 3] { v.push(p); } } } } } v };
    let mut out = vec![];
    let mut b = [0u8; 5];
    for a in 0..52u8 { for c in a + 1..52 { for d in c + 1..52 { for e in d + 1..52 { for f in e + 1..52 {
        b = [a, c, d, e, f];
        let mut images: Vec<[u8; 5]> = perms.iter().map(|p| { let mut x = b.map(|card| (card / 4) * 4 + p[(card % 4) as usize]); x.sort(); x }).collect();
        images.sort(); images.dedup();
        if images[0] == b { out.push((b, images.len() as u64)); }
    } } } } }
    let _ = b;
    out
}

fn log(line: &str) {
    println!("{line}");
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open("gen_exact_log.txt") { let _ = writeln!(f, "{line}"); }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let auto = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    let threads: usize = args.get(1).and_then(|s| s.parse().ok()).filter(|&n| n > 0).unwrap_or(if auto > 4 { auto - 2 } else { auto });
    let limit: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(usize::MAX);
    let deck = Deck::new();
    let cls: Vec<u16> = deck.cards.iter().map(|&(a, b)| class_of(a, b) as u16).collect();
    let mask: Vec<u64> = deck.cards.iter().map(|&(a, b)| (1u64 << a) | (1u64 << b)).collect();
    let mut boards = canonical_boards();
    let total_weight: u64 = boards.iter().map(|b| b.1).sum();
    log(&format!("boards to go through: {} (standing for {} in all), threads: {}", boards.len(), total_weight, threads));
    assert_eq!(total_weight, 2_598_960);
    if limit < boards.len() { boards.truncate(limit); }
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let started = std::time::Instant::now();
    let nb = boards.len();
    let results: Vec<(Vec<u64>, Vec<u64>)> = std::thread::scope(|sc| {
        let handles: Vec<_> = (0..threads).map(|_| sc.spawn(|| {
            let mut acc = vec![0u64; TABLE];
            let mut acc2 = vec![0u64; NC * NC * 2];
            let (mut s, mut m, mut c) = (Vec::with_capacity(1081), Vec::with_capacity(1081), Vec::with_capacity(1081));
            loop {
                let bi = next.fetch_add(1, Ordering::Relaxed);
                if bi >= nb { break; }
                let (b, w) = boards[bi];
                let used: u64 = b.iter().map(|&x| 1u64 << x).sum();
                let mut live: Vec<(u32, u64, u16)> = (0..NH).filter(|&h| mask[h] & used == 0).map(|h| { let (x, y) = deck.cards[h]; (eval7(&[b[0], b[1], b[2], b[3], b[4], x, y]), mask[h], cls[h]) }).collect();
                live.sort_unstable();
                s.clear(); m.clear(); c.clear();
                for &(st, mk, cl) in &live { s.push(st); m.push(mk); c.push(cl as usize); }
                let n = s.len();
                // end[y]: first index past the hands tied with y
                let mut end = vec![n; n];
                { let mut e = n; for y in (0..n).rev() { if y + 1 < n && s[y + 1] != s[y] { e = y + 1; } end[y] = e; } }
                for x in 0..n {
                    let (mx, cx, ex) = (m[x], c[x], end[x]);
                    for y in x + 1..n {
                        if mx & m[y] != 0 { continue; }
                        let t1 = (y < ex) as usize;
                        acc2[(cx * NC + c[y]) * 2 + t1] += w;
                        let mxy = mx | m[y];
                        let base = ((cx * NC + c[y]) * 2 + t1) * ROW;
                        let row = &mut acc[base..base + ROW];
                        let ey = end[y];
                        for z in y + 1..ey { row[c[z] * 2 + 1] += w * ((mxy & m[z] == 0) as u64); }
                        for z in ey..n { row[c[z] * 2] += w * ((mxy & m[z] == 0) as u64); }
                    }
                }
                let d = done.fetch_add(1, Ordering::Relaxed) + 1;
                if d % 2000 == 0 || d == nb {
                    let el = started.elapsed().as_secs_f64();
                    log(&format!("{d} of {nb} boards, {:.0} minutes so far, about {:.0} minutes left", el / 60.0, el / d as f64 * (nb - d) as f64 / 60.0));
                }
            }
            (acc, acc2)
        })).collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let (mut acc, mut acc2) = (vec![0u64; TABLE], vec![0u64; NC * NC * 2]);
    for (a, a2) in &results { for (x, y) in acc.iter_mut().zip(a) { *x += *y; } for (x, y) in acc2.iter_mut().zip(a2) { *x += *y; } }
    drop(results);

    // two hands: [first class][second class] -> chance the first wins, chance of a tie
    let mut two = vec![0f64; NC * NC * 3]; // win, tie, total
    for cx in 0..NC { for cy in 0..NC { for t in 0..2 {
        let v = acc2[(cx * NC + cy) * 2 + t] as f64; if v == 0.0 { continue; }
        // x is the weaker hand (or tied)
        if t == 1 { two[(cx * NC + cy) * 3 + 1] += v; two[(cy * NC + cx) * 3 + 1] += v; } else { two[(cy * NC + cx) * 3] += v; }
        two[(cx * NC + cy) * 3 + 2] += v; two[(cy * NC + cx) * 3 + 2] += v;
    } } }
    let mut bytes = Vec::with_capacity(NC * NC * 8);
    for i in 0..NC * NC { let tot = two[i * 3 + 2]; let (w, t) = if tot > 0.0 { (two[i * 3] / tot, two[i * 3 + 1] / tot) } else { (0.5, 0.0) }; bytes.extend_from_slice(&(w as f32).to_le_bytes()); bytes.extend_from_slice(&(t as f32).to_le_bytes()); }
    std::fs::write("two_way_exact_169.bin", &bytes).unwrap();

    // three hands: per unordered class trio, thirteen finishing patterns by position
    let base = tri_base();
    let mut pat = vec![0u64; NTRI * 13];
    let perms: [[usize; 3]; 6] = [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]];
    for cx in 0..NC { for cy in 0..NC { for t1 in 0..2 { for cz in 0..NC { for t2 in 0..2 {
        let v = acc[((cx * NC + cy) * 2 + t1) * ROW + cz * 2 + t2]; if v == 0 { continue; }
        // ranks of the three hands: z is best
        let rz = 0u8; let ry = if t2 == 1 { 0 } else { 1 }; let rx = if t1 == 1 { ry } else { ry + 1 };
        let hands = [(cx, rx), (cy, ry), (cz, rz)];
        let mut sorted = [cx, cy, cz]; sorted.sort();
        let slot = (base[sorted[0] * NC + sorted[1]] as usize + (sorted[2] - sorted[1])) * 13;
        // every way of seating the three hands at the sorted positions that matches their classes
        for p in &perms {
            if (0..3).any(|pos| hands[p[pos]].0 != sorted[pos]) { continue; }
            pat[slot + pattern_index([hands[p[0]].1, hands[p[1]].1, hands[p[2]].1])] += v;
        }
    } } } } }
    let mut bytes = Vec::with_capacity(NTRI * 15);
    let per_deal = 1_370_754u64; // boards left once six cards are dealt: C(46, 5)
    let mut ways = vec![0u16; NTRI];
    let mut bad = 0usize;
    for t in 0..NTRI {
        let tot: u64 = pat[t * 13..t * 13 + 13].iter().sum();
        if limit == usize::MAX && tot % per_deal != 0 { bad += 1; }
        ways[t] = (tot / per_deal) as u16;
    }
    for w in &ways { bytes.extend_from_slice(&w.to_le_bytes()); }
    for t in 0..NTRI {
        let p = &pat[t * 13..t * 13 + 13]; let tot: u64 = p.iter().sum();
        if tot == 0 { bytes.extend_from_slice(&[0u8; 13]); continue; }
        let exact: Vec<f64> = p.iter().map(|&x| x as f64 / tot as f64 * 255.0).collect();
        let mut q: Vec<u8> = exact.iter().map(|x| x.floor() as u8).collect();
        let mut left = 255 - q.iter().map(|&x| x as i32).sum::<i32>();
        let mut order: Vec<usize> = (0..13).collect();
        order.sort_by(|&a, &b| (exact[b] - exact[b].floor()).partial_cmp(&(exact[a] - exact[a].floor())).unwrap());
        for &o in &order { if left <= 0 { break; } q[o] += 1; left -= 1; }
        bytes.extend_from_slice(&q);
    }
    std::fs::write("three_way_exact_169.bin", &bytes).unwrap();
    log(&format!("trios whose total is not a whole number of deals: {bad} (should be 0)"));
    // exact counts for a few trios, to be checked against a separate brute-force count
    let name = |i: usize| pushfold::class_name(i);
    for trio in [[0usize, 14, 28], [1, 28, 44], [13, 13, 13], [0, 0, 14], [168, 168, 168], [5, 77, 150]] {
        let mut sorted = trio; sorted.sort();
        let slot = (base[sorted[0] * NC + sorted[1]] as usize + (sorted[2] - sorted[1])) * 13;
        log(&format!("check {} {} {}: {:?}", name(sorted[0]), name(sorted[1]), name(sorted[2]), &pat[slot..slot + 13]));
    }
    log(&format!("check pair AA v KK win/tie/total: {} {} {}", two[14 * 3], two[14 * 3 + 1], two[14 * 3 + 2]));
    log(&format!("finished in {:.0} minutes", started.elapsed().as_secs_f64() / 60.0));
}

//! Chooses a sample of flops (with the per-class weights from `flopweights`) that stands in for all flops.
//!
//!   flopsample cache <file>                 all-in equities of every class on each of the 1,755 distinct flops (about 40 minutes on 2 cores)
//!   flopsample search <file> <n> [seconds] [seed] [start flops]   looks for n flops whose weighted equities match the exact table
//!   flopsample score <file> <flop,flop,...>  scores a given sample
//!
//! The search only looks at equity against a random hand and the connection features; equity
//! against a strong range is kept out of it and reported as a check on flops it was not tuned to.
use pushfold::flopeq::flop_equities_many;
use pushfold::flopweights::*;
use pushfold::{class_name, equity};
use rayon::prelude::*;

/// Opposing ranges the equities are worked out against: any two cards, the strongest 12% of hands,
/// the weakest half, and the strongest 40% (by all-in equity against a random hand).
const RANGE_NAMES: [&str; 4] = ["a random hand", "the strongest 12%", "the weakest half", "the strongest 40%"];
/// Which of them the search fits; the rest are reported as checks on something it was not tuned to.
const FITTED: [bool; 4] = [true, true, true, false];
const NR: usize = 4;

fn card_name(c: u8) -> String { format!("{}{}", "23456789TJQKA".as_bytes()[(c / 4) as usize] as char, "cdhs".as_bytes()[(c % 4) as usize] as char) }
fn flop_name(f: [u8; 3]) -> String { let mut v = f; v.sort_by(|a, b| b.cmp(a)); v.iter().map(|c| card_name(*c)).collect() }

struct Cache { flops: Vec<[u8; 3]>, ways: Vec<f64>, eq: Vec<Vec<Vec<f32>>> } // eq[range][flop][class]

/// Exact equity of each class against a random hand, from the bundled table.
fn exact_vs(members: Option<&[bool]>) -> Vec<f64> {
    let by_class = hands_by_class();
    (0..NC).map(|c| {
        let h = representative(c);
        let (mut num, mut den) = (0.0, 0.0);
        for (j, hands) in by_class.iter().enumerate() {
            if let Some(m) = members { if !m[j] { continue; } }
            let n = hands.iter().filter(|o| !h.contains(&o[0]) && !h.contains(&o[1])).count() as f64;
            num += n * equity(c, j); den += n;
        }
        num / den
    }).collect()
}

/// Class membership of each range.
fn ranges() -> Vec<Option<Vec<bool>>> {
    let by_class = hands_by_class();
    let eq = exact_vs(None);
    let mut order: Vec<usize> = (0..NC).collect(); order.sort_by(|a, b| eq[*b].partial_cmp(&eq[*a]).unwrap());
    let top = |share: f64| -> Vec<bool> { let mut m = vec![false; NC]; let mut n = 0.0; for &c in &order { if n >= share * 1326.0 { break; } m[c] = true; n += by_class[c].len() as f64; } m };
    let weak: Vec<bool> = top(0.5).iter().map(|x| !x).collect();
    vec![None, Some(top(0.12)), Some(weak), Some(top(0.40))]
}

fn build_cache(path: &str) {
    let all = canonical_flops();
    let by_class = hands_by_class();
    let rs = ranges();
    let weights: Vec<Option<Vec<f64>>> = rs.iter().map(|r| r.as_ref().map(|m| { let mut w = vec![0f64; 52 * 52]; for c in 0..NC { if m[c] { for h in &by_class[c] { w[h[0] as usize * 52 + h[1] as usize] = 1.0; } } } w })).collect();
    let opps: Vec<Option<&[f64]>> = weights.iter().map(|w| w.as_deref()).collect();
    let t0 = std::time::Instant::now();
    let mut bytes: Vec<u8> = vec![];
    bytes.extend((all.len() as u32).to_le_bytes());
    for (i, (f, ways)) in all.iter().enumerate() {
        let e = flop_equities_many(*f, &opps);
        bytes.extend(f); bytes.extend((*ways as f32).to_le_bytes());
        for q in 0..NR { for c in 0..NC {
            let v: Vec<f64> = by_class[c].iter().map(|h| e[q][h[0] as usize * 52 + h[1] as usize]).filter(|x| x.is_finite()).collect();
            let m = if v.is_empty() { f32::NAN } else { (v.iter().sum::<f64>() / v.len() as f64) as f32 };
            bytes.extend(m.to_le_bytes());
        } }
        if i % 50 == 49 { eprintln!("{} of {} flops, {:.0} min", i + 1, all.len(), t0.elapsed().as_secs_f64() / 60.0); std::fs::write(format!("{path}.part"), &bytes).ok(); }
    }
    std::fs::write(path, &bytes).expect("write cache");
    eprintln!("cache written: {} flops in {:.0} min", all.len(), t0.elapsed().as_secs_f64() / 60.0);
}

fn load_cache(path: &str) -> Cache {
    let b = std::fs::read(path).expect("read cache");
    let n = u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize;
    let rec = 3 + 4 + NC * 4 * NR;
    let n = n.min((b.len() - 4) / rec);
    let mut c = Cache { flops: vec![], ways: vec![], eq: vec![vec![]; NR] };
    let f32at = |o: usize| f32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
    for i in 0..n {
        let o = 4 + i * rec;
        c.flops.push([b[o], b[o + 1], b[o + 2]]); c.ways.push(f32at(o + 3) as f64);
        for q in 0..NR { c.eq[q].push((0..NC).map(|k| f32at(o + 7 + (q * NC + k) * 4)).collect()); }
    }
    c
}

struct Ctx { cache: Cache, truth: Vec<[f64; NF]>, prof: Vec<Vec<([f64; NF], f64)>>, exact: Vec<Vec<f64>>, combos: Vec<f64> }

fn context(path: &str) -> Ctx {
    let cache = load_cache(path);
    let by_class = hands_by_class();
    let truth: Vec<[f64; NF]> = (0..NC).into_par_iter().map(true_profile).collect();
    // prof[class][flop]
    let prof: Vec<Vec<([f64; NF], f64)>> = (0..NC).into_par_iter().map(|c| cache.flops.iter().map(|f| flop_profile(&by_class[c], *f)).collect()).collect();
    let exact: Vec<Vec<f64>> = ranges().iter().map(|r| exact_vs(r.as_deref())).collect();
    let combos = by_class.iter().map(|h| h.len() as f64).collect();
    Ctx { cache, truth, prof, exact, combos }
}

struct Score { cost: f64, mean: [f64; NR], worst: [f64; NR], within_half: [usize; NR], feature_gap: f64, errors: Vec<[f64; NR]>, weights: Vec<Vec<f64>>, spread: f64 }

fn score(ctx: &Ctx, sample: &[usize], iterations: usize) -> Score {
    let ways: Vec<f64> = sample.iter().map(|&f| ctx.cache.ways[f]).collect();
    let per: Vec<([f64; NR], f64, Vec<f64>)> = (0..NC).into_par_iter().map(|c| {
        let refs: Vec<&([f64; NF], f64)> = sample.iter().map(|&f| &ctx.prof[c][f]).collect();
        let (_, w) = fit_class(&ctx.truth[c], &refs, &ways, iterations);
        let mut m = [0f64; NF];
        let (mut e, mut tot) = ([0f64; NR], 0.0);
        for (k, &f) in sample.iter().enumerate() {
            if w[k] <= 0.0 || !ctx.cache.eq[0][f][c].is_finite() { continue; }
            for q in 0..NR { let x = ctx.cache.eq[q][f][c] as f64; e[q] += w[k] * if x.is_finite() { x } else { ctx.exact[q][c] }; }
            tot += w[k];
            for j in 0..NF { m[j] += w[k] * refs[k].0[j]; }
        }
        let gap: f64 = (0..NF).map(|j| (m[j] - ctx.truth[c][j]).abs()).sum::<f64>() / NF as f64;
        let mut err = [0f64; NR];
        for q in 0..NR { err[q] = (e[q] / tot.max(1e-12) - ctx.exact[q][c]) * 100.0; }
        (err, gap, w)
    }).collect();
    let total: f64 = ctx.combos.iter().sum();
    let gap = (0..NC).map(|c| ctx.combos[c] * per[c].1).sum::<f64>() / total * 100.0;
    let (mut mean, mut worst, mut within, mut cost) = ([0f64; NR], [0f64; NR], [0usize; NR], 0.5 * gap * gap);
    for q in 0..NR {
        let e: Vec<f64> = per.iter().map(|p| p.0[q]).collect();
        mean[q] = e.iter().map(|x| x.abs()).sum::<f64>() / NC as f64;
        worst[q] = e.iter().map(|x| x.abs()).fold(0.0, f64::max);
        within[q] = e.iter().filter(|x| x.abs() <= 0.5).count();
        if FITTED[q] { cost += (0..NC).map(|c| ctx.combos[c] * e[c] * e[c]).sum::<f64>() / total + 0.15 * worst[q] * worst[q]; }
    }
    // how unevenly the flops are leaned on: the largest average weight over the smallest
    let use_: Vec<f64> = (0..sample.len()).map(|k| (0..NC).map(|c| ctx.combos[c] * per[c].2[k]).sum::<f64>() / total).collect();
    let spread = use_.iter().cloned().fold(0.0, f64::max) / use_.iter().cloned().fold(1.0, f64::min).max(1e-9);
    Score { cost, mean, worst, within_half: within, feature_gap: gap, errors: per.iter().map(|p| p.0).collect(), weights: per.into_iter().map(|p| p.2).collect(), spread }
}

fn report(ctx: &Ctx, sample: &[usize], s: &Score) {
    println!("sample of {}: {}", sample.len(), sample.iter().map(|&f| flop_name(ctx.cache.flops[f])).collect::<Vec<_>>().join(","));
    for q in 0..NR {
        println!("  equity against {}{}: error in points mean {:.2}, worst {:.2}, within 0.5: {} of 169", RANGE_NAMES[q], if FITTED[q] { "" } else { " (not used in the search)" }, s.mean[q], s.worst[q], s.within_half[q]);
    }
    println!("  connection features: mean gap {:.2} points", s.feature_gap);
    for q in 0..NR {
        let mut order: Vec<usize> = (0..NC).collect(); order.sort_by(|a, b| s.errors[*b][q].abs().partial_cmp(&s.errors[*a][q].abs()).unwrap());
        println!("  largest errors against {}: {}", RANGE_NAMES[q], order.iter().take(8).map(|&c| format!("{} {:+.2}", class_name(c), s.errors[c][q])).collect::<Vec<_>>().join(", "));
    }
    // how much weight each flop gets on average: a flop nobody leans on is wasted solving time
    let use_: Vec<f64> = (0..sample.len()).map(|k| (0..NC).map(|c| ctx.combos[c] * s.weights[c][k]).sum::<f64>() / 1326.0).collect();
    println!("  average weight by flop: {}", use_.iter().map(|u| format!("{:.1}%", u * 100.0)).collect::<Vec<_>>().join(" "));
}

fn find(ctx: &Ctx, name: &str) -> usize {
    let f = parse_flop(name).expect("flop");
    let mut best = None;
    for (i, g) in ctx.cache.flops.iter().enumerate() {
        // same ranks and same suit pattern
        let mut a: Vec<u8> = f.iter().map(|c| c / 4).collect(); a.sort();
        let mut b: Vec<u8> = g.iter().map(|c| c / 4).collect(); b.sort();
        if a != b || suit_ways(f) != suit_ways(*g) { continue; }
        // suit pattern by rank must agree too (which ranks share a suit)
        let pat = |x: [u8; 3]| { let mut v: Vec<(u8, u8)> = x.iter().map(|c| (c / 4, c % 4)).collect(); v.sort(); let s: Vec<u8> = v.iter().map(|p| p.1).collect(); [s[0] == s[1], s[0] == s[2], s[1] == s[2]] };
        if pat(f) == pat(*g) { best = Some(i); break; }
    }
    best.unwrap_or_else(|| panic!("flop {name} not in cache"))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(|s| s.as_str()) {
        Some("cache") => build_cache(&args[1]),
        Some("score") => {
            let ctx = context(&args[1]);
            let sample: Vec<usize> = args[2].split(',').map(|n| find(&ctx, n.trim())).collect();
            let s = score(&ctx, &sample, 6000);
            report(&ctx, &sample, &s);
        }
        Some("search") => {
            let ctx = context(&args[1]);
            let n: usize = args[2].parse().expect("number of flops");
            let seconds: f64 = args.get(3).and_then(|x| x.parse().ok()).unwrap_or(600.0);
            let mut seed: u64 = args.get(4).and_then(|x| x.parse().ok()).unwrap_or(1);
            let mut rnd = move || { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; seed };
            let nall = ctx.cache.flops.len();
            // start: given flops, then flops drawn by how many real flops they stand for
            let mut sample: Vec<usize> = args.get(5).map(|t| t.split(',').map(|x| find(&ctx, x.trim())).collect()).unwrap_or_default();
            let total_ways: f64 = ctx.cache.ways.iter().sum();
            let draw = |rnd: &mut dyn FnMut() -> u64| { let mut x = (rnd() % 1_000_000) as f64 / 1e6 * total_ways; for i in 0..nall { x -= ctx.cache.ways[i]; if x <= 0.0 { return i; } } nall - 1 };
            while sample.len() < n { let f = draw(&mut rnd); if !sample.contains(&f) { sample.push(f); } }
            sample.truncate(n);
            let iters = 500;
            let mut cur = score(&ctx, &sample, iters);
            let (mut best, mut best_cost) = (sample.clone(), cur.cost);
            let t0 = std::time::Instant::now();
            let mut tries = 0;
            while t0.elapsed().as_secs_f64() < seconds {
                tries += 1;
                let temp = 0.05 * (1.0 - t0.elapsed().as_secs_f64() / seconds).max(0.0);
                let mut cand = sample.clone();
                let slot = (rnd() % n as u64) as usize;
                let f = draw(&mut rnd);
                if cand.contains(&f) { continue; }
                cand[slot] = f;
                let s = score(&ctx, &cand, iters);
                let accept = s.cost < cur.cost || ((rnd() % 10_000) as f64 / 1e4) < ((cur.cost - s.cost) / temp.max(1e-9)).exp();
                if accept { sample = cand; cur = s; if cur.cost < best_cost { best_cost = cur.cost; best = sample.clone(); eprintln!("{:.0}s try {tries}: cost {:.3} mean {:.2}/{:.2}/{:.2}/{:.2} worst {:.2}/{:.2}/{:.2}/{:.2}", t0.elapsed().as_secs_f64(), cur.cost, cur.mean[0], cur.mean[1], cur.mean[2], cur.mean[3], cur.worst[0], cur.worst[1], cur.worst[2], cur.worst[3]); } }
            }
            let s = score(&ctx, &best, 6000);
            report(&ctx, &best, &s);
        }
        _ => eprintln!("usage: flopsample cache <file> | search <file> <n> [seconds] [seed] [flops] | score <file> <flops>"),
    }
}

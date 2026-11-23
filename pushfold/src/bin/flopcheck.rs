//! Quick test of the flop weights, with nothing solved: for a sample of flops, the all-in equity of
//! every starting-hand class against a random hand is worked out flop by flop (exactly, over every
//! turn and river and every opposing hand), averaged with the plain count and with the fitted
//! weights, and compared with the exact figure over all boards from the bundled table.
//!
//!   cargo run --release --features generate --bin flopcheck [-- "As7d2c,Ks8s3d,..."] [--tsv out.tsv]
use pushfold::flopeq::flop_equities;
use pushfold::flopweights::*;
use pushfold::{class_name, equity};

pub const DEFAULT_16: &str = "As7d2c,Ks8s3d,QhJh6c,Ts9s8d,7s6d5c,KsKd4c,8s3d2c,Jh9h5h,AsKdTc,Qs7d2c,9c6c4d,7s7d4d,Ah8h5c,Jd7d4s,Ts6d2c,As5d5c";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let tsv = args.iter().position(|a| a == "--tsv").and_then(|i| args.get(i + 1)).cloned();
    let list = args.iter().find(|a| !a.starts_with("--") && a.contains(',')).cloned().unwrap_or(DEFAULT_16.to_string());
    let flops: Vec<[u8; 3]> = list.split(',').map(|s| parse_flop(s.trim()).expect("flop")).collect();
    let names: Vec<&str> = list.split(',').map(|s| s.trim()).collect();
    let t0 = std::time::Instant::now();
    let fw = fit(&flops, None);
    eprintln!("weights fitted for {} flops in {:.1}s", flops.len(), t0.elapsed().as_secs_f64());
    let by_class = hands_by_class();
    let eqs: Vec<Vec<f64>> = flops.iter().map(|f| flop_equities(*f, None)).collect();
    eprintln!("flop equities done at {:.1}s", t0.elapsed().as_secs_f64());
    // exact equity against a random hand, from the bundled table
    let exact: Vec<f64> = (0..NC).map(|c| {
        let h = representative(c);
        let (mut num, mut den) = (0.0, 0.0);
        for (j, hands) in by_class.iter().enumerate() {
            let n = hands.iter().filter(|o| !h.contains(&o[0]) && !h.contains(&o[1])).count() as f64;
            num += n * equity(c, j); den += n;
        }
        num / den
    }).collect();
    let class_eq = |c: usize, f: usize| -> f64 {
        let v: Vec<f64> = by_class[c].iter().map(|h| eqs[f][h[0] as usize * 52 + h[1] as usize]).filter(|x| x.is_finite()).collect();
        if v.is_empty() { 0.0 } else { v.iter().sum::<f64>() / v.len() as f64 }
    };
    let mut rows = vec![];
    for c in 0..NC {
        let plain: f64 = (0..flops.len()).map(|f| fw.plain[c][f] * class_eq(c, f)).sum();
        let fitted: f64 = (0..flops.len()).map(|f| fw.weight[c][f] * class_eq(c, f)).sum();
        rows.push((c, exact[c], plain, fitted));
    }
    let stat = |pick: &dyn Fn(&(usize, f64, f64, f64)) -> f64| {
        let e: Vec<f64> = rows.iter().map(|r| (pick(r) - r.1).abs() * 100.0).collect();
        let mean = e.iter().sum::<f64>() / e.len() as f64;
        let max = e.iter().cloned().fold(0.0, f64::max);
        let within = e.iter().filter(|x| **x <= 0.5).count();
        let within1 = e.iter().filter(|x| **x <= 1.0).count();
        (mean, max, within, within1)
    };
    let (a, b) = (stat(&|r| r.2), stat(&|r| r.3));
    println!("All-in equity against a random hand, 169 classes, {} flops: error in points against the exact table", flops.len());
    println!("  plain count:    mean {:.2}, worst {:.2}, within 0.5: {} of 169, within 1.0: {}", a.0, a.1, a.2, a.3);
    println!("  fitted weights: mean {:.2}, worst {:.2}, within 0.5: {} of 169, within 1.0: {}", b.0, b.1, b.2, b.3);
    let mut worst = rows.clone(); worst.sort_by(|x, y| (y.3 - y.1).abs().partial_cmp(&(x.3 - x.1).abs()).unwrap());
    println!("  largest errors with fitted weights (class: exact, plain, fitted):");
    for r in worst.iter().take(12) { println!("    {:4} {:.1}  {:.1}  {:.1}", class_name(r.0), r.1 * 100.0, r.2 * 100.0, r.3 * 100.0); }
    for name in ["72o", "93o", "82o", "32o", "T9s", "AKo", "77", "22"] {
        let c = (0..NC).find(|&c| class_name(c) == name).unwrap();
        let r = rows[c];
        println!("  {name}: equity exact {:.1}, plain {:.1}, fitted {:.1} | pair or better: true {:.1}%, plain {:.1}%, fitted {:.1}% | two pair or better: true {:.1}%, plain {:.1}%, fitted {:.1}%",
            r.1 * 100.0, r.2 * 100.0, r.3 * 100.0, fw.truth[c][7] * 100.0, fw.plain_profile[c][7] * 100.0, fw.fitted_profile[c][7] * 100.0, fw.truth[c][8] * 100.0, fw.plain_profile[c][8] * 100.0, fw.fitted_profile[c][8] * 100.0);
    }
    // how well each feature is matched, over all classes
    println!("  feature frequencies, mean absolute gap to the truth in points (plain, fitted):");
    for k in 0..NF {
        let g = |p: &Vec<[f64; NF]>| (0..NC).map(|c| (p[c][k] - fw.truth[c][k]).abs()).sum::<f64>() / NC as f64 * 100.0;
        println!("    {:28} {:5.2} {:5.2}", FEATURE_NAMES[k], g(&fw.plain_profile), g(&fw.fitted_profile));
    }
    if let Some(path) = tsv {
        let mut s = String::from("class\texact\tplain\tfitted");
        for n in &names { s += &format!("\tw_{n}"); }
        s += "\n";
        for r in &rows { s += &format!("{}\t{:.4}\t{:.4}\t{:.4}", class_name(r.0), r.1, r.2, r.3); for f in 0..flops.len() { s += &format!("\t{:.4}", fw.weight[r.0][f]); } s += "\n"; }
        std::fs::write(&path, s).expect("write tsv");
    }
    eprintln!("done in {:.1}s", t0.elapsed().as_secs_f64());
}

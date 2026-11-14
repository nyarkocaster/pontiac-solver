//! Independent check of gen_exact: counts the thirteen finishing patterns for a few class trios by
//! dealing every set of three hands and every one of the 1,370,754 boards directly (no suit
//! shortcuts, no sorting tricks), and prints them in the same form as gen_exact_log.txt.
use multiway::cards::*;
use pushfold::tables::*;
use rayon::prelude::*;

fn main() {
    let deck = Deck::new();
    let mut by_class: Vec<Vec<(u8, u8)>> = vec![vec![]; NC];
    for &(a, b) in &deck.cards { by_class[class_of(a, b)].push((a, b)); }
    for trio in [[0usize, 14, 28], [1, 28, 44], [5, 77, 150], [0, 0, 14]] { // about 8 minutes each on 2 cores
        let mut t = trio; t.sort();
        let mut deals = vec![];
        for &a in &by_class[t[0]] { for &b in &by_class[t[1]] { for &c in &by_class[t[2]] {
            let cards = [a.0, a.1, b.0, b.1, c.0, c.1];
            if (0..6).all(|i| (i + 1..6).all(|j| cards[i] != cards[j])) { deals.push(cards); }
        } } }
        let counts = deals.par_iter().map(|h| {
            let mut out = [0u64; 13];
            let rest: Vec<u8> = (0..52u8).filter(|c| !h.contains(c)).collect();
            let n = rest.len();
            for a in 0..n { for b in a + 1..n { for c in b + 1..n { for d in c + 1..n { for e in d + 1..n {
                let bd = [rest[a], rest[b], rest[c], rest[d], rest[e]];
                let s: Vec<u32> = (0..3).map(|p| eval7(&[bd[0], bd[1], bd[2], bd[3], bd[4], h[p * 2], h[p * 2 + 1]])).collect();
                let rank = |p: usize| (0..3).filter(|&q| s[q] > s[p]).map(|q| s[q]).collect::<std::collections::BTreeSet<u32>>().len() as u8;
                out[pattern_index([rank(0), rank(1), rank(2)])] += 1;
            } } } } }
            out
        }).reduce(|| [0u64; 13], |mut a, b| { for i in 0..13 { a[i] += b[i]; } a });
        println!("check {} {} {}: {:?}", pushfold::class_name(t[0]), pushfold::class_name(t[1]), pushfold::class_name(t[2]), counts);
    }
}

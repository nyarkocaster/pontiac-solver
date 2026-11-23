//! Does a preflop chart make sense hand by hand? A sample of flops that flatters some ranks shows
//! up as a weak hand playing more often than a clearly stronger one of the same kind. This lists
//! every such pair: same kind (pair, suited, offsuit), neither card lower and at least one higher,
//! no fewer straights that use both cards (so 94s is not held to be stronger than 54s), yet played
//! less often; and a suited hand played less often than the same two ranks offsuit.
use crate::class_name;
use serde::Serialize;

#[derive(Serialize, Clone, Debug)]
pub struct Inversion {
    pub stronger: String, pub weaker: String, pub stronger_plays: f64, pub weaker_plays: f64, pub gap: f64,
    /// how much more the action is worth to the weaker hand than to the stronger one, in big blinds
    /// (absent when the result file carries no action values)
    #[serde(skip_serializing_if = "Option::is_none")] pub value_gap: Option<f64>,
    /// counted in `inversions_by_value`
    pub counts: bool,
}

#[derive(Serialize, Clone, Debug)]
pub struct ChartCheck {
    /// how often the worst hands are played (0 to 1)
    pub trash_plays: Vec<(String, f64)>,
    /// pairs of hands compared, and how many are out of order by more than the tolerance in frequency
    pub pairs_compared: usize,
    pub inversions: usize,
    pub tolerance: f64,
    /// of those, the ones that are also out of order in value: the action is worth more than
    /// `min_value_gap` big blinds more to the weaker hand than to the stronger one, and both hands
    /// actually get to this decision. A hand that is nearly indifferent can sit at any frequency,
    /// so only these are real. Absent when the file carries no action values.
    #[serde(skip_serializing_if = "Option::is_none")] pub inversions_by_value: Option<usize>,
    pub min_value_gap: f64,
    /// the largest gap, and the average gap over the out-of-order pairs
    pub worst_gap: f64,
    pub mean_gap: f64,
    /// the worst of them (those that count first, then by frequency gap)
    pub worst: Vec<Inversion>,
}

/// (kind, high rank, low rank): kind 0 pair, 1 suited, 2 offsuit; ranks 12 = ace.
fn shape(class: usize) -> (u8, u8, u8) {
    let (r, c) = (class / 13, class % 13);
    if r == c { (0, 12 - r as u8, 12 - r as u8) } else if c > r { (1, 12 - r as u8, 12 - c as u8) } else { (2, 12 - c as u8, 12 - r as u8) }
}

/// How many straights use both cards (0 to 4), counting the ace low as well.
fn straights(hi: u8, lo: u8) -> u8 {
    let one = |d: u8| if d == 0 || d > 4 { 0 } else { 5 - d };
    let wheel = if hi == 12 && lo <= 3 { one(lo + 1) } else { 0 };
    one(hi - lo).max(wheel)
}

/// `plays[class]` is how often the hand is played at a decision (1 minus how often it folds).
pub fn check(plays: &[f64], tolerance: f64) -> ChartCheck { check_values(plays, None, None, tolerance, 0.03) }

/// As `check`, with values. `prefers[class]` is how often the hand takes the action a stronger hand
/// should take at least as often (playing instead of folding; raising instead of limping).
/// `edge[class]` is what that action is worth over the other, in big blinds. `present[class]` says
/// which hands take part (how much of the hand gets to this decision; hands under 2% are left out of
/// the count by value).
pub fn check_values(prefers: &[f64], edge: Option<&[f64]>, present: Option<&[f64]>, tolerance: f64, min_value_gap: f64) -> ChartCheck {
    let mut found = vec![];
    let mut compared = 0;
    let here = |c: usize| present.map(|p| p[c] >= 0.02).unwrap_or(true);
    for a in 0..169 { for b in 0..169 {
        if a == b { continue; }
        let (ka, ha, la) = shape(a); let (kb, hb, lb) = shape(b);
        let stronger = (ka == kb && ha >= hb && la >= lb && (ha > hb || la > lb) && straights(ha, la) >= straights(hb, lb)) || (ka == 1 && kb == 2 && ha == hb && la == lb);
        if !stronger { continue; }
        compared += 1;
        let gap = prefers[b] - prefers[a];
        if gap > tolerance {
            let value_gap = edge.map(|e| e[b] - e[a]);
            let counts = match value_gap { Some(v) => v > min_value_gap && here(a) && here(b), None => true };
            found.push(Inversion { stronger: class_name(a), weaker: class_name(b), stronger_plays: prefers[a], weaker_plays: prefers[b], gap, value_gap, counts });
        }
    } }
    found.sort_by(|x, y| y.counts.cmp(&x.counts).then(y.gap.partial_cmp(&x.gap).unwrap()));
    let r = |x: f64| (x * 1000.0).round() / 1000.0;
    let worst_gap = found.iter().map(|i| i.gap).fold(0.0, f64::max);
    ChartCheck {
        trash_plays: ["72o", "82o", "32o", "83o", "92o", "93o", "42o", "62o"].iter().map(|n| (n.to_string(), r(prefers[(0..169).find(|&c| class_name(c) == *n).unwrap()]))).collect(),
        pairs_compared: compared, inversions: found.len(), tolerance,
        inversions_by_value: edge.map(|_| found.iter().filter(|i| i.counts).count()), min_value_gap,
        worst_gap: r(worst_gap),
        mean_gap: r(if found.is_empty() { 0.0 } else { found.iter().map(|i| i.gap).sum::<f64>() / found.len() as f64 }),
        worst: found.into_iter().take(20).map(|mut i| { i.gap = r(i.gap); i.value_gap = i.value_gap.map(|v| (v * 1e4).round() / 1e4); i }).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn push_fold_charts_are_in_order() {
        // the exact push/fold answer is the reference for "a chart that makes sense"
        for stack in [8.0, 12.0, 20.0] {
            let pf = crate::solve(stack, 0.5, 0.0).unwrap();
            let c = check(&pf.jam, 0.15);
            assert!(c.inversions <= 2, "{} bb: {:?}", stack, c.worst);
        }
    }
    #[test]
    fn spots_a_trash_hand_out_of_order() {
        let mut plays = vec![0.0; 169];
        for c in 0..169 { let (k, h, l) = shape(c); plays[c] = if k == 0 || h + l >= 14 { 1.0 } else { 0.0 }; }
        assert_eq!(check(&plays, 0.15).inversions, 0);
        let c72 = (0..169).find(|&c| class_name(c) == "72o").unwrap();
        plays[c72] = 1.0;
        let c = check(&plays, 0.15);
        assert!(c.inversions > 10 && c.worst.iter().all(|i| i.weaker == "72o"), "{:?}", c.worst);
        // with values: a 0.01 bb difference is indifference and does not count; a 0.1 bb one does
        let mut edge = vec![0.0; 169];
        edge[c72] = 0.01;
        let c = check_values(&plays, Some(&edge), None, 0.15, 0.03);
        assert!(c.inversions > 10 && c.inversions_by_value == Some(0));
        edge[c72] = 0.1;
        let c = check_values(&plays, Some(&edge), None, 0.15, 0.03);
        assert_eq!(c.inversions_by_value, Some(c.inversions));
        // 93o is one of the hands 72o should not be played ahead of
        let c93 = (0..169).find(|&c| class_name(c) == "93o").unwrap();
        let (k9, h9, l9) = shape(c93); let (k7, h7, l7) = shape(c72);
        assert!(k9 == k7 && h9 >= h7 && l9 >= l7 && straights(h9, l9) >= straights(h7, l7));
    }
}

//! Prize-money value of tournament stacks (the Independent Chip Model, Malmuth-Harville):
//! a player's chance of finishing first is their share of the chips, and the same rule is applied
//! again among the remaining players for each lower place.

/// Expected prize for every stack. `payouts[0]` is first prize. Players with no chips share the
/// places just below the players who still have chips.
pub fn icm(stacks: &[f64], payouts: &[f64]) -> Vec<f64> {
    let n = stacks.len();
    let mut out = vec![0.0; n];
    let alive: Vec<usize> = (0..n).filter(|&i| stacks[i] > 1e-9).collect();
    let busted: Vec<usize> = (0..n).filter(|&i| stacks[i] <= 1e-9).collect();
    if !busted.is_empty() {
        let share: f64 = (alive.len()..alive.len() + busted.len()).map(|p| payouts.get(p).copied().unwrap_or(0.0)).sum::<f64>() / busted.len() as f64;
        for &b in &busted { out[b] = share; }
    }
    let total: f64 = alive.iter().map(|&i| stacks[i]).sum();
    let places = payouts.len().min(alive.len());
    fn rec(stacks: &[f64], alive: &[usize], used: u32, left: f64, place: usize, places: usize, prob: f64, payouts: &[f64], out: &mut [f64]) {
        if place == places || left <= 1e-12 { return; }
        for (n, &i) in alive.iter().enumerate() {
            if used & (1 << n) != 0 { continue; }
            let p = prob * stacks[i] / left;
            out[i] += p * payouts[place];
            if place + 1 < places { rec(stacks, alive, used | (1 << n), left - stacks[i], place + 1, places, p, payouts, out); }
        }
    }
    rec(stacks, &alive, 0, total, 0, places, 1.0, payouts, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::icm;
    #[test]
    fn known_values() {
        // equal stacks share the prizes equally
        let e = icm(&[1000.0; 4], &[50.0, 30.0, 20.0]);
        for x in &e { assert!((x - 25.0).abs() < 1e-9); }
        // the classic 3-handed example: 5000 / 3000 / 2000 chips, 50 / 30 / 20
        let e = icm(&[5000.0, 3000.0, 2000.0], &[50.0, 30.0, 20.0]);
        assert!((e[0] - 38.393).abs() < 0.01 && (e[1] - 32.750).abs() < 0.01 && (e[2] - 28.857).abs() < 0.01, "{:?}", e);
        assert!((e.iter().sum::<f64>() - 100.0).abs() < 1e-9);
        // winner take all is just the share of chips
        let e = icm(&[3.0, 1.0], &[10.0]);
        assert!((e[0] - 7.5).abs() < 1e-12);
        // a busted player takes the next place down
        let e = icm(&[6000.0, 4000.0, 0.0], &[50.0, 30.0, 20.0]);
        assert!((e[2] - 20.0).abs() < 1e-12 && (e[0] - 42.0).abs() < 1e-9);
    }
}

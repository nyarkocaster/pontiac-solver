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

/// Prize-money pressure for a two-player pot (Independent Chip Model): what each result of the
/// hand is worth in prize money to the two players in the pot, with the other players at the
/// table holding their stacks. All chip amounts are in the caller's units.
#[derive(Clone, Debug)]
pub struct PotIcm {
    /// stacks behind at the start of the spot: [out of position, in position]
    pub behind: [f64; 2],
    pub others: Vec<f64>,
    pub payouts: Vec<f64>,
    pub pot: f64,
    /// prize money per chip, so payoffs keep the scale of chips
    pub per_chip: f64,
    /// the prizes that can still be won by the players left
    pub prize: f64,
    /// each player's prize-money value if the pot at the start of the spot were split
    pub split_value: [f64; 2],
}

impl PotIcm {
    pub fn new(behind: [f64; 2], others: Vec<f64>, payouts: Vec<f64>, pot: f64) -> Result<PotIcm, String> {
        if payouts.is_empty() || payouts.iter().any(|x| !x.is_finite() || *x < 0.0) || payouts.iter().sum::<f64>() <= 0.0 { return Err("Prize money: give at least one prize, none negative.".into()); }
        if others.iter().any(|x| !x.is_finite() || *x < 0.0) || behind.iter().any(|x| !x.is_finite() || *x <= 0.0) { return Err("Prize money: stacks must be positive numbers.".into()); }
        if others.len() > 7 { return Err("Prize money: at most 7 other players.".into()); }
        let total: f64 = behind[0] + behind[1] + pot + others.iter().sum::<f64>();
        let prize: f64 = payouts.iter().take(2 + others.len()).sum();
        let mut s = PotIcm { behind, others, payouts, pot, per_chip: prize / total, prize, split_value: [0.0; 2] };
        s.split_value = s.value([behind[0] + pot / 2.0, behind[1] + pot / 2.0]);
        Ok(s)
    }
    /// Prize-money value for both players given their final stacks.
    pub fn value(&self, stacks: [f64; 2]) -> [f64; 2] {
        let mut all = vec![stacks[0].max(0.0), stacks[1].max(0.0)];
        all.extend(self.others.iter().cloned());
        let e = icm(&all, &self.payouts);
        [e[0], e[1]]
    }
    /// For each amount a player can have bet when the hand ends: [win, tie, lose] per player, in
    /// chip-sized units of prize money, where zero means getting half the starting pot back.
    pub fn table(&self, amounts: &[i32]) -> Vec<(i32, [[f64; 3]; 2])> {
        amounts.iter().map(|&a| {
            let a_f = a as f64;
            let mut row = [[0.0; 3]; 2];
            for p in 0..2 {
                let q = 1 - p;
                let mut win = [0.0; 2]; win[p] = self.behind[p] + self.pot + a_f; win[q] = self.behind[q] - a_f;
                let mut lose = [0.0; 2]; lose[p] = self.behind[p] - a_f; lose[q] = self.behind[q] + self.pot + a_f;
                row[p] = [(self.value(win)[p] - self.split_value[p]) / self.per_chip, 0.0, (self.value(lose)[p] - self.split_value[p]) / self.per_chip];
            }
            (a, row)
        }).collect()
    }
    /// Share of the prize pool (percent) for a player whose EV in chip-sized units, counted the
    /// solver's way (pot included, so a split is pot / 2), is `ev`.
    pub fn share_pct(&self, player: usize, ev: f64) -> f64 { (self.split_value[player] + (ev - self.pot / 2.0) * self.per_chip) / self.prize * 100.0 }
    pub fn split_pct(&self, player: usize) -> f64 { self.split_value[player] / self.prize * 100.0 }
    /// Share of the prize pool (percent) if this player gave the pot up now.
    pub fn give_up_pct(&self, player: usize) -> f64 { let mut st = self.behind; st[1 - player] += self.pot; self.value(st)[player] / self.prize * 100.0 }
}

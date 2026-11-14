//! Shared layout of the bundled tables.
pub const NC: usize = 169;
/// Number of unordered class triples i <= j <= k.
pub const NTRI: usize = NC * (NC + 1) * (NC + 2) / 6;
/// The six finishing orders of three hands: ORDERS[o] = [best, middle, worst] as positions 0..3.
pub const ORDERS: [[usize; 3]; 6] = [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]];

/// base[i * NC + j] + (k - j) is the slot of the triple i <= j <= k.
pub fn tri_base() -> Vec<u32> {
    let mut base = vec![0u32; NC * NC];
    let mut run = 0u32;
    for i in 0..NC { for j in i..NC { base[i * NC + j] = run; run += (NC - j) as u32; } }
    debug_assert_eq!(run as usize, NTRI);
    base
}

/// Chart cell of a two-card hand (cards are rank * 4 + suit, rank 0 = deuce).
pub fn class_of(x: u8, y: u8) -> usize {
    let (ra, rb) = ((x / 4) as usize, (y / 4) as usize);
    let (hi, lo) = if ra >= rb { (ra, rb) } else { (rb, ra) };
    let (r_hi, r_lo) = (12 - hi, 12 - lo);
    if hi == lo { r_hi * 13 + r_hi } else if x % 4 == y % 4 { r_hi * 13 + r_lo } else { r_lo * 13 + r_hi }
}

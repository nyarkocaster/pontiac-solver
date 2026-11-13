//! Cards, hand indexing, hand evaluation and range parsing.
pub const NH: usize = 1326;
pub const RANKS: &str = "23456789TJQKA";
pub const SUITS: &str = "cdhs";

pub struct Deck {
    pub idx: [[u16; 52]; 52],
    pub cards: Vec<(u8, u8)>,
}
impl Deck {
    pub fn new() -> Self {
        let mut idx = [[u16::MAX; 52]; 52];
        let mut cards = Vec::with_capacity(NH);
        for a in 0..52u8 {
            for b in (a + 1)..52u8 {
                idx[a as usize][b as usize] = cards.len() as u16;
                idx[b as usize][a as usize] = cards.len() as u16;
                cards.push((a, b));
            }
        }
        Deck { idx, cards }
    }
}

pub fn parse_card(s: &str) -> Result<u8, String> {
    let b: Vec<char> = s.chars().collect();
    if b.len() != 2 { return Err(format!("bad card '{s}'")); }
    let r = RANKS.find(b[0].to_ascii_uppercase()).ok_or(format!("bad rank in '{s}'"))?;
    let su = SUITS.find(b[1].to_ascii_lowercase()).ok_or(format!("bad suit in '{s}'"))?;
    Ok((r * 4 + su) as u8)
}
pub fn card_str(c: u8) -> String {
    format!("{}{}", RANKS.as_bytes()[(c / 4) as usize] as char, SUITS.as_bytes()[(c % 4) as usize] as char)
}
pub fn parse_board(s: &str) -> Result<Vec<u8>, String> {
    let t: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    let mut v = vec![];
    let ch: Vec<char> = t.chars().collect();
    for k in ch.chunks(2) { v.push(parse_card(&k.iter().collect::<String>())?); }
    Ok(v)
}

/// Strength of a 5-card hand: bigger is better.
pub fn eval5(c: &[u8; 5]) -> u32 {
    let mut cnt = [0u8; 13];
    let mut flush = true;
    for i in 0..5 {
        cnt[(c[i] / 4) as usize] += 1;
        if c[i] % 4 != c[0] % 4 { flush = false; }
    }
    // ranks ordered by (count, rank) descending
    let mut rs: Vec<(u8, u8)> = (0..13u8).filter(|&r| cnt[r as usize] > 0).map(|r| (cnt[r as usize], r)).collect();
    rs.sort_by(|a, b| b.cmp(a));
    let mut straight_high: Option<u8> = None;
    if rs.len() == 5 {
        let hi = rs[0].1; let lo = rs[4].1;
        if hi - lo == 4 { straight_high = Some(hi); }
        else if hi == 12 && rs[1].1 == 3 { straight_high = Some(3); } // wheel
    }
    let mut kick: u32 = 0;
    for &(_, r) in &rs { kick = kick * 13 + r as u32; }
    for _ in rs.len()..5 { kick *= 13; }
    let cat: u32 = if let (Some(h), true) = (straight_high, flush) { kick = h as u32; 8 }
        else if rs[0].0 == 4 { 7 }
        else if rs[0].0 == 3 && rs[1].0 == 2 { 6 }
        else if flush { 5 }
        else if let Some(h) = straight_high { kick = h as u32; 4 }
        else if rs[0].0 == 3 { 3 }
        else if rs[0].0 == 2 && rs[1].0 == 2 { 2 }
        else if rs[0].0 == 2 { 1 }
        else { 0 };
    cat * 400_000 + kick + 1
}

pub fn eval7(c: &[u8; 7]) -> u32 {
    let mut best = 0;
    for i in 0..7 { for j in (i + 1)..7 {
        let mut h = [0u8; 5]; let mut k = 0;
        for m in 0..7 { if m != i && m != j { h[k] = c[m]; k += 1; } }
        let v = eval5(&h); if v > best { best = v; }
    } }
    best
}

fn rank_of(ch: char) -> Result<u8, String> {
    RANKS.find(ch.to_ascii_uppercase()).map(|x| x as u8).ok_or(format!("bad rank '{ch}'"))
}

/// Parses ranges like "TT+,88-55,AKs,AQo,A5s+,KJ:0.5". Returns 1326 weights.
pub fn parse_range(deck: &Deck, s: &str) -> Result<Vec<f32>, String> {
    let mut w = vec![0f32; NH];
    for tok in s.split(',') {
        let tok = tok.trim(); if tok.is_empty() { continue; }
        let (body, wt) = match tok.split_once(':') { Some((b, x)) => (b, x.parse::<f32>().map_err(|e| e.to_string())?), None => (tok, 1.0) };
        let mut add = |hi: u8, lo: u8, kind: char| {
            for s1 in 0..4u8 { for s2 in 0..4u8 {
                if hi == lo { if s1 >= s2 { continue; } }
                else if kind == 's' && s1 != s2 { continue; }
                else if kind == 'o' && s1 == s2 { continue; }
                let (a, b) = (hi * 4 + s1, lo * 4 + s2);
                w[deck.idx[a as usize][b as usize] as usize] = wt;
            } }
        };
        let parse_one = |t: &str| -> Result<(u8, u8, char), String> {
            let ch: Vec<char> = t.chars().collect();
            if ch.len() < 2 || ch.len() > 3 { return Err(format!("bad range token '{t}'")); }
            let (a, b) = (rank_of(ch[0])?, rank_of(ch[1])?);
            let kind = if ch.len() == 3 { ch[2].to_ascii_lowercase() } else { 'a' };
            Ok((a.max(b), a.min(b), kind))
        };
        if let Some((l, r)) = body.split_once('-') {
            let (h1, l1, k) = parse_one(l)?; let (h2, l2, _) = parse_one(r)?;
            if h1 == l1 { for p in l2.min(l1)..=l1.max(l2) { add(p, p, 'a'); } let _ = h2; }
            else { for lo in l1.min(l2)..=l1.max(l2) { add(h1, lo, k); } }
        } else if let Some(b) = body.strip_suffix('+') {
            let (h, l, k) = parse_one(b)?;
            if h == l { for p in h..13 { add(p, p, 'a'); } } else { for lo in l..h { add(h, lo, k); } }
        } else {
            let (h, l, k) = parse_one(body)?; add(h, l, k);
        }
    }
    Ok(w)
}

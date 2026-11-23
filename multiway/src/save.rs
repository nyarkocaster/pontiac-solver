//! Keeping a finished three-player solve in a file and opening it again without solving.
//!
//! The file holds what was asked for, the summary, and for each solved tree its settings, the
//! average strategy at every decision (the same 16-bit numbers the solver holds, so a reopened
//! solve reports exactly the same figures) and any fixed strategies. The tree itself is rebuilt
//! from the settings. The working numbers the solver learns from are not kept, so a reopened
//! solve can be browsed and continued onto exact turn cards, but not solved further.
use crate::game::{Config, Game, Kind};
use crate::study::{Spec3, State3, Sub, Summary3};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

const MAGIC: &[u8; 8] = b"PONTIAC3";
const FORMAT: u32 = 1;

#[derive(Serialize, Deserialize)]
struct Head {
    format: u32,
    /// program version that wrote the file, and anything else the caller wants kept (names, the spot as given)
    memo: String,
    spec: Spec3,
    summary: Summary3,
    /// the trees that follow, in order: "base", "locked", "sub_base", "sub_locked"
    games: Vec<String>,
    sub: Option<SubHead>,
}

#[derive(Serialize, Deserialize)]
struct SubHead { for_locked: bool, prefix: Vec<usize>, summary: Summary3 }

fn put(w: &mut impl Write, bytes: &[u8]) -> Result<(), String> { w.write_all(bytes).map_err(|e| format!("could not write the file: {e}")) }
fn put_u32(w: &mut impl Write, x: u32) -> Result<(), String> { put(w, &x.to_le_bytes()) }
fn get<const N: usize>(r: &mut impl Read) -> Result<[u8; N], String> { let mut b = [0u8; N]; r.read_exact(&mut b).map_err(|_| "the file is cut short or damaged".to_string())?; Ok(b) }
fn get_u32(r: &mut impl Read) -> Result<u32, String> { Ok(u32::from_le_bytes(get::<4>(r)?)) }

fn write_game(w: &mut impl Write, g: &Game) -> Result<(), String> {
    let cfg = serde_json::to_vec(&g.cfg).map_err(|e| e.to_string())?;
    put_u32(w, cfg.len() as u32)?; put(w, &cfg)?;
    put_u32(w, g.nodes.len() as u32)?;
    for node in &g.nodes {
        if node.kind != Kind::Decision { continue; }
        // SAFETY: nothing is solving while a finished solve is written out
        let data = unsafe { &*node.data.get() };
        put_u32(w, data.strat_sum.len() as u32)?;
        put(w, &data.sum_scale.to_le_bytes())?;
        let mut raw = Vec::with_capacity(data.strat_sum.len() * 2);
        for &x in &data.strat_sum { raw.extend_from_slice(&x.to_le_bytes()); }
        put(w, &raw)?;
        match &node.locked {
            None => put_u32(w, 0)?,
            Some(l) => { put_u32(w, l.len() as u32)?; let mut raw = Vec::with_capacity(l.len() * 4); for &x in l { raw.extend_from_slice(&x.to_le_bytes()); } put(w, &raw)?; }
        }
    }
    Ok(())
}

fn read_game(r: &mut impl Read) -> Result<Game, String> {
    let stale = "this file does not match this version's tree for the same settings; solve the spot again";
    let n = get_u32(r)? as usize;
    if n > 64 << 20 { return Err("the file is damaged".into()); }
    let mut cfg = vec![0u8; n];
    r.read_exact(&mut cfg).map_err(|_| "the file is cut short or damaged".to_string())?;
    let cfg: Config = serde_json::from_slice(&cfg).map_err(|_| "the file's settings could not be read (saved by a different version?)".to_string())?;
    // the tree without the solver's working memory: only the average strategy is filled in
    let mut g = Game::new(cfg, false)?;
    if get_u32(r)? as usize != g.nodes.len() { return Err(stale.into()); }
    for id in 0..g.nodes.len() {
        if g.nodes[id].kind != Kind::Decision { continue; }
        let want = g.nodes[id].actions.len() * g.hands[g.nodes[id].player].len();
        let len = get_u32(r)? as usize;
        if len != want { return Err(stale.into()); }
        let scale = f32::from_le_bytes(get::<4>(r)?);
        let mut raw = vec![0u8; len * 2];
        r.read_exact(&mut raw).map_err(|_| "the file is cut short or damaged".to_string())?;
        let sum: Vec<i16> = raw.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect();
        let locked = get_u32(r)? as usize;
        if locked != 0 && locked != want { return Err(stale.into()); }
        let data = g.nodes[id].data.get_mut();
        data.strat_sum = sum; data.sum_scale = scale;
        if locked != 0 {
            let mut raw = vec![0u8; locked * 4];
            r.read_exact(&mut raw).map_err(|_| "the file is cut short or damaged".to_string())?;
            g.set_lock(id, raw.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect());
        }
    }
    Ok(g)
}

/// Writes a finished solve. `memo` is free text kept with it (the command line stores the program
/// version, the players' names and the spot as given).
pub fn save_state(st: &State3, memo: &str, out: impl Write) -> Result<(), String> {
    let (Some(base), Some(spec), Some(summary)) = (&st.base, &st.spec, &st.summary) else { return Err("there is no finished solve to save".into()) };
    let mut out = out;
    put(&mut out, MAGIC)?;
    let mut w = zstd::stream::write::Encoder::new(out, 3).map_err(|e| e.to_string())?;
    let mut games = vec!["base".to_string()];
    if st.locked.is_some() { games.push("locked".into()); }
    if let Some(sub) = &st.sub { games.push("sub_base".into()); if sub.locked.is_some() { games.push("sub_locked".into()); } }
    let head = Head { format: FORMAT, memo: memo.to_string(), spec: spec.clone(), summary: summary.clone(), games,
        sub: st.sub.as_ref().map(|s| SubHead { for_locked: s.for_locked, prefix: s.prefix.clone(), summary: s.summary.clone() }) };
    let head = serde_json::to_vec(&head).map_err(|e| e.to_string())?;
    put_u32(&mut w, head.len() as u32)?; put(&mut w, &head)?;
    write_game(&mut w, base)?;
    if let Some(g) = &st.locked { write_game(&mut w, g)?; }
    if let Some(sub) = &st.sub { write_game(&mut w, &sub.base)?; if let Some(g) = &sub.locked { write_game(&mut w, g)?; } }
    w.finish().map_err(|e| format!("could not write the file: {e}"))?.flush().map_err(|e| format!("could not write the file: {e}"))
}

/// True when the bytes start like a three-player save (lets a caller tell it from a two-player file).
pub fn is_three_player(first_bytes: &[u8]) -> bool { first_bytes.len() >= 8 && &first_bytes[..8] == MAGIC }

/// Opens a saved solve. Returns it with the memo it was saved with.
pub fn load_state(input: impl Read) -> Result<(State3, String), String> {
    let mut input = input;
    if &get::<8>(&mut input)? != MAGIC { return Err("this is not a three-player Pontiac solve".into()); }
    let mut r = zstd::stream::read::Decoder::new(input).map_err(|e| e.to_string())?;
    let n = get_u32(&mut r)? as usize;
    if n > 64 << 20 { return Err("the file is damaged".into()); }
    let mut head = vec![0u8; n];
    r.read_exact(&mut head).map_err(|_| "the file is cut short or damaged".to_string())?;
    let head: Head = serde_json::from_slice(&head).map_err(|_| "the file could not be read (saved by a different version?)".to_string())?;
    if head.format != FORMAT { return Err("this file was saved by a newer version of Pontiac".into()); }
    let mut st = State3 { spec: Some(head.spec), summary: Some(head.summary), ..Default::default() };
    let (mut sub_base, mut sub_locked) = (None, None);
    for name in &head.games {
        let g = read_game(&mut r)?;
        match name.as_str() { "base" => st.base = Some(g), "locked" => st.locked = Some(g), "sub_base" => sub_base = Some(g), "sub_locked" => sub_locked = Some(g), _ => return Err("the file is damaged".into()) }
    }
    if st.base.is_none() { return Err("the file is damaged".into()); }
    if let (Some(h), Some(base)) = (head.sub, sub_base) { st.sub = Some(Sub { for_locked: h.for_locked, prefix: h.prefix, base, locked: sub_locked, summary: h.summary }); }
    Ok((st, head.memo))
}

pub fn save_file(st: &State3, memo: &str, path: &str) -> Result<(), String> {
    let f = std::fs::File::create(path).map_err(|e| format!("could not create {path}: {e}"))?;
    save_state(st, memo, std::io::BufWriter::new(f))
}

pub fn load_file(path: &str) -> Result<(State3, String), String> {
    let f = std::fs::File::open(path).map_err(|e| format!("could not open {path}: {e}"))?;
    load_state(std::io::BufReader::new(f))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::study::*;

    /// A solve with a lock (so two trees), saved and reopened: every decision's report is identical.
    #[test]
    fn a_reopened_solve_reports_the_same_numbers() {
        let spec = Spec3 {
            ranges: ["99-22,A9s-A2s,KTs+,QTs+,ATo+,KQo".into(), "TT-22,AJs-A2s,KTs+,QJs,AQo+".into(), "AA-22,AKs-A2s,K9s+,QTs+,AKo-ATo,KJo+".into()],
            board: "Kh9s6d4c2s".into(), pot: 30.0, stack: 60.0, stacks: Some([40.0, 60.0, 55.0]),
            bet_sizes: "66%".into(), raise_sizes: "60%".into(), max_raises: 1, allin_spr: 1.0, threads: 1, accuracy_pct: 0.5, max_iter: 60,
            names: Some(["A".into(), "B".into(), "C".into()]),
            node_locks: vec![NodeLock3 { node: "river".into(), player: Some("A".into()), hands: None, action_freqs: [("check".to_string(), 0.5)].into_iter().collect() }],
            icm_payouts: vec![50.0, 30.0, 20.0], icm_others: vec![25.0],
            ..Default::default()
        };
        let st = solve3(&spec, &Progress3::default()).unwrap();
        assert!(st.locked.is_some());
        let mut bytes = vec![];
        save_state(&st, "memo text", &mut bytes).unwrap();
        assert!(is_three_player(&bytes));
        let (back, memo) = load_state(&bytes[..]).unwrap();
        assert_eq!(memo, "memo text");
        assert_eq!(back.summary.as_ref().unwrap().base_ev, st.summary.as_ref().unwrap().base_ev);
        let mut seen = 0;
        for locked in [false, true] {
            let g = if locked { st.locked.as_ref().unwrap() } else { st.base.as_ref().unwrap() };
            let mut stack: Vec<(usize, Vec<usize>)> = vec![(0, vec![])];
            while let Some((id, hist)) = stack.pop() {
                let node = &g.nodes[id];
                if node.kind != Kind::Decision { continue; }
                let (a, b) = (st.view(locked, &hist, true).unwrap(), back.view(locked, &hist, true).unwrap());
                assert_eq!(a.action_freqs, b.action_freqs); assert_eq!(a.ev, b.ev); assert_eq!(a.equity, b.equity); assert_eq!(a.locked, b.locked);
                seen += 1;
                for (i, &c) in node.children.iter().enumerate() { let mut h = hist.clone(); h.push(i); stack.push((c, h)); }
            }
        }
        assert!(seen > 10, "{seen}");
        // a damaged file is refused, not half-read
        assert!(load_state(&bytes[..bytes.len() / 2]).is_err());
        assert!(load_state(&b"not a pontiac file at all"[..]).is_err());
    }
}

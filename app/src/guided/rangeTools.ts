// Tools for widening and tightening ranges written as text ("TT+,AJs+,KQo:0.5").
// STRENGTH lists the 169 starting hands from most to least playable. The order blends all-in equity
// against any hand, against the best half of hands and against the best fifth, then nudges suited
// and connected hands up and big cards with weak offsuit kickers down, the way opening ranges are built.
export const STRENGTH: string[] = ["AA", "KK", "QQ", "JJ", "AKs", "TT", "AQs", "99", "AKo", "AJs", "88", "AQo", "ATs", "KQs", "77", "AJo", "KJs", "66", "A9s", "ATo", "QJs", "A8s", "55", "KQo", "KTs", "QTs", "A9o", "A7s", "JTs", "44", "A6s", "A5s", "KJo", "A4s", "A8o", "A3s", "K9s", "33", "A2s", "Q9s", "J9s", "KTo", "T9s", "QJo", "K8s", "22", "K7s", "A7o", "QTo", "K6s", "98s", "Q8s", "T8s", "J8s", "JTo", "K5s", "K9o", "87s", "A6o", "K4s", "A5o", "Q7s", "97s", "K3s", "Q6s", "76s", "Q9o", "Q5s", "K8o", "K2s", "T7s", "A4o", "J7s", "65s", "86s", "Q4s", "A3o", "J9o", "Q3s", "T9o", "J6s", "75s", "96s", "Q8o", "Q2s", "J5s", "A2o", "T6s", "J4s", "K7o", "J8o", "J3s", "98o", "K6o", "T5s", "T8o", "J2s", "85s", "95s", "54s", "T4s", "K5o", "64s", "T3s", "Q7o", "74s", "T7o", "87o", "T2s", "97o", "53s", "84s", "Q6o", "K4o", "94s", "93s", "63s", "43s", "J7o", "76o", "K3o", "73s", "Q5o", "T6o", "92s", "86o", "96o", "83s", "52s", "82s", "65o", "K2o", "Q4o", "62s", "75o", "J6o", "42s", "T5o", "72s", "32s", "85o", "Q3o", "54o", "95o", "J5o", "T4o", "64o", "T3o", "Q2o", "74o", "J4o", "53o", "T2o", "84o", "94o", "J3o", "93o", "63o", "43o", "73o", "92o", "83o", "J2o", "52o", "82o", "62o", "42o", "72o", "32o"];

const RANKS = "AKQJT98765432";
const rank = new Map(STRENGTH.map((h, i) => [h, i]));
export const comboCount = (h: string) => (h.length === 2 ? 6 : h[2] === "s" ? 4 : 12);
const TOTAL = 1326;

/** A range split into hand classes with weights, plus any exact-card entries ("KhJh") kept as written. */
export interface ParsedRange { weights: Map<string, number>; extras: string[] }

function classesOf(tok: string): string[] | null {
  const pair = (r: number) => RANKS[r] + RANKS[r];
  const two = (a: number, b: number, suf: string) => (suf ? [RANKS[a] + RANKS[b] + suf] : [RANKS[a] + RANKS[b] + "s", RANKS[a] + RANKS[b] + "o"]);
  const m = /^([AKQJT2-9])([AKQJT2-9])([so]?)(\+)?$/.exec(tok);
  const d = /^([AKQJT2-9])([AKQJT2-9])([so]?)-([AKQJT2-9])([AKQJT2-9])([so]?)$/.exec(tok);
  if (m) {
    let a = RANKS.indexOf(m[1]), b = RANKS.indexOf(m[2]);
    if (a === b) { if (m[3]) return null; const out = []; for (let r = m[4] ? 0 : a; r <= a; r++) out.push(pair(r)); return out; }
    if (a > b) [a, b] = [b, a];
    const out: string[] = [];
    for (let k = m[4] ? a + 1 : b; k <= b; k++) out.push(...two(a, k, m[3]));
    return out;
  }
  if (d) {
    let a = RANKS.indexOf(d[1]), b = RANKS.indexOf(d[2]), c = RANKS.indexOf(d[4]), e = RANKS.indexOf(d[5]);
    if (d[3] !== d[6]) return null;
    if (a === b && c === e) { const out = []; for (let r = Math.min(a, c); r <= Math.max(a, c); r++) out.push(pair(r)); return out; }
    if (a > b) [a, b] = [b, a]; if (c > e) [c, e] = [e, c];
    if (a !== c || a === b || c === e) return null;
    const out: string[] = [];
    for (let k = Math.min(b, e); k <= Math.max(b, e); k++) out.push(...two(a, k, d[3]));
    return out;
  }
  return null;
}

export function parseRange(text: string): ParsedRange {
  const weights = new Map<string, number>(); const extras: string[] = [];
  for (const raw of text.split(",")) {
    const t = raw.trim(); if (!t) continue;
    const [body, w] = t.split(":");
    const cls = classesOf(body.trim());
    if (!cls) { extras.push(t); continue; }
    const weight = w === undefined ? 1 : Math.max(0, Math.min(1, parseFloat(w) || 0));
    for (const c of cls) weights.set(c, weight);
  }
  return { weights, extras };
}

/** Share of all starting hands in the range, 0 to 100 (exact-card entries count one hand each). */
export function rangePercent(p: ParsedRange): number {
  let n = p.extras.length;
  p.weights.forEach((w, h) => { n += w * comboCount(h); });
  return (n / TOTAL) * 100;
}

/** Writes hand classes back as compact text: "TT+", "99-66", "A5s+", "K9s-K6s". */
export function formatRange(p: ParsedRange): string {
  const out: string[] = [];
  const full = (h: string) => (p.weights.get(h) ?? 0) >= 1;
  // pairs
  for (let r = 0; r < 13; ) {
    if (!full(RANKS[r] + RANKS[r])) { r++; continue; }
    let e = r; while (e + 1 < 13 && full(RANKS[e + 1] + RANKS[e + 1])) e++;
    const lo = RANKS[e] + RANKS[e], hi = RANKS[r] + RANKS[r];
    out.push(r === 0 ? (e === 0 ? "AA" : lo + "+") : e === r ? hi : `${hi}-${lo}`);
    r = e + 1;
  }
  for (const suf of ["s", "o"]) for (let a = 0; a < 12; a++) {
    for (let k = a + 1; k < 13; ) {
      if (!full(RANKS[a] + RANKS[k] + suf)) { k++; continue; }
      let e = k; while (e + 1 < 13 && full(RANKS[a] + RANKS[e + 1] + suf)) e++;
      const hi = RANKS[a] + RANKS[k] + suf, lo = RANKS[a] + RANKS[e] + suf;
      out.push(k === a + 1 ? (e === k ? hi : lo + "+") : e === k ? hi : `${hi}-${lo}`);
      k = e + 1;
    }
  }
  for (const h of STRENGTH) { const w = p.weights.get(h) ?? 0; if (w > 0 && w < 1) out.push(`${h}:${Math.round(w * 100) / 100}`); }
  return [...out, ...p.extras].join(",");
}

/**
 * Moves a range to about `target` percent of all hands. Going up adds the strongest hands that are
 * not yet fully in; going down removes the weakest hands that are in. Hands the base range already
 * has stay as they are (including part weights) unless they are the ones removed.
 */
export function resize(base: ParsedRange, target: number): ParsedRange {
  const weights = new Map(base.weights);
  let pct = rangePercent({ weights, extras: base.extras });
  const step = (h: string, from: number, to: number) => { weights.set(h, to); pct += ((to - from) * comboCount(h) / TOTAL) * 100; };
  if (target > pct) {
    for (const h of STRENGTH) {
      const w = weights.get(h) ?? 0; if (w >= 1) continue;
      const gain = ((1 - w) * comboCount(h) / TOTAL) * 100;
      if (pct + gain / 2 > target) break;
      step(h, w, 1);
    }
  } else {
    for (let i = STRENGTH.length - 1; i >= 0; i--) {
      const h = STRENGTH[i]; const w = weights.get(h) ?? 0; if (w <= 0) continue;
      const loss = (w * comboCount(h) / TOTAL) * 100;
      if (pct - loss / 2 < target) break;
      step(h, w, 0);
    }
  }
  for (const [h, w] of [...weights]) if (w <= 0) weights.delete(h);
  return { weights, extras: base.extras };
}

/** The strongest `target` percent of hands, as text. */
export const topPercent = (target: number) => formatRange(resize({ weights: new Map(), extras: [] }, target));
export const strengthRank = (h: string) => rank.get(h) ?? 999;

/** `base` with every hand of `remove` taken out, as text. */
export function without(base: string, remove: string): string {
  const b = parseRange(base); const r = parseRange(remove);
  r.weights.forEach((_w, h) => b.weights.delete(h));
  return formatRange(b);
}

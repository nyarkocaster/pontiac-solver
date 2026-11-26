// Counts how each player in a session played: how often they put money in before the flop, raised,
// re-raised, bet the flop after raising, folded to such bets, and reached showdown. Works from the
// actions alone, so it reads every layout handFormats.ts reads. Winnings are not counted: several
// layouts do not carry them through.
import { splitHands, toStarsLayout } from "./handFormats";

/** made out of chances */
export type Count = [number, number];
export interface PlayerStats {
  name: string; hero: boolean; hands: number;
  /** put money in before the flop by choice (VPIP) */
  vpip: Count;
  /** raised before the flop (PFR) */
  pfr: Count;
  /** re-raised when facing one raise (3-bet) */
  threeBet: Count;
  /** folded after raising and being re-raised */
  foldTo3: Count;
  sawFlop: Count;
  /** bet the flop as the last raiser before it, when nobody had bet yet (c-bet) */
  cbet: Count;
  /** folded when first facing such a bet */
  foldToCbet: Count;
  /** of hands that saw the flop, reached showdown */
  showdown: Count;
  /** bets and raises after the flop, and calls after the flop */
  aggr: number; calls: number;
}
export interface SessionStats { players: PlayerStats[]; hands: number; unread: number }

const blank = (name: string): PlayerStats => ({ name, hero: false, hands: 0, vpip: [0, 0], pfr: [0, 0], threeBet: [0, 0], foldTo3: [0, 0], sawFlop: [0, 0], cbet: [0, 0], foldToCbet: [0, 0], showdown: [0, 0], aggr: 0, calls: 0 });
const ACT = /^(.+?):?\s+(folds|checks|calls|raises|bets|is all-?in|all-?in)\b/i;

export function sessionStats(text: string): SessionStats {
  const by = new Map<string, PlayerStats>(); let hands = 0, unread = 0;
  const heroCount = new Map<string, number>();
  for (const part of splitHands(text)) {
    let lines: string[];
    try { lines = toStarsLayout(part).text.split("\n").map((l) => l.trim()).filter(Boolean); } catch { unread++; continue; }
    const holeAt = lines.findIndex((l) => /^\*+\s*HOLE CARDS/i.test(l) || /^Dealt to /i.test(l));
    if (holeAt < 0) { unread++; continue; }
    const names: string[] = [];
    for (const l of lines.slice(0, holeAt)) {
      const m = l.match(/^Seat\s+(\d+):\s+(.+?)\s+\(\s*[$€£]?([\d,.]+)(?:\s+in chips)?[^)]*\)(.*)$/i);
      if (m && !/sitting out|out of hand/i.test(m[4])) names.push(m[2]);
    }
    if (names.length < 2) { unread++; continue; }
    const nameOf = (raw: string) => names.find((n) => n === raw) ?? names.find((n) => raw.startsWith(n));
    hands++;
    const st = new Map(names.map((n) => { let s = by.get(n); if (!s) { s = blank(n); by.set(n, s); } s.hands++; return [n, s] as const; }));
    const folded = new Set<string>(), saw = new Set<string>();
    let street = 0; // 0 before the flop, 1 flop, 2 turn, 3 river
    let raises = 0, firstRaiser = "", lastRaiser = "";
    const vpip = new Set<string>(), pfr = new Set<string>(), faced1 = new Set<string>(), faced3 = new Set<string>();
    let flopBet = "", flopActed = new Set<string>(), flopRaised = false; const facedCbet = new Set<string>();
    for (const l of lines.slice(holeAt)) {
      if (/^\*+\s*(SHOW ?DOWN|SUMMARY)/i.test(l)) break;
      const d = l.match(/^Dealt to (.+?)\s*\[/i);
      if (d) { const h = nameOf(d[1]); if (h) heroCount.set(h, (heroCount.get(h) ?? 0) + 1); continue; }
      const sm = l.match(/^\*+\s*(FLOP|TURN|RIVER)/i);
      if (sm) {
        street = { FLOP: 1, TURN: 2, RIVER: 3 }[sm[1].toUpperCase() as "FLOP" | "TURN" | "RIVER"];
        if (street === 1) for (const n of names) if (!folded.has(n)) { st.get(n)!.sawFlop[0]++; saw.add(n); }
        continue;
      }
      const m = l.match(ACT); if (!m) continue;
      const who = nameOf(m[1]); if (!who) continue;
      const s = st.get(who)!; let verb = m[2].toLowerCase();
      if (/all-?in/.test(verb)) verb = "raises";
      if (street === 0) {
        if (raises === 1 && who !== firstRaiser && !faced1.has(who)) { faced1.add(who); s.threeBet[1]++; if (verb === "raises") s.threeBet[0]++; }
        if (raises === 2 && who === firstRaiser && lastRaiser !== who && !faced3.has(who)) { faced3.add(who); s.foldTo3[1]++; if (verb === "folds") s.foldTo3[0]++; }
        if (verb === "calls" || verb === "raises" || verb === "bets") vpip.add(who);
        if (verb === "raises" || verb === "bets") { pfr.add(who); raises++; if (raises === 1) firstRaiser = who; lastRaiser = who; }
      } else {
        if (verb === "bets" || verb === "raises") s.aggr++; else if (verb === "calls") s.calls++;
        if (street === 1) {
          if (flopBet && !flopRaised && who !== flopBet && flopBet === lastRaiser && !facedCbet.has(who)) { facedCbet.add(who); s.foldToCbet[1]++; if (verb === "folds") s.foldToCbet[0]++; }
          if (who === lastRaiser && !flopActed.has(who) && !flopBet) { s.cbet[1]++; if (verb === "bets") s.cbet[0]++; }
          if (verb === "bets" && !flopBet) flopBet = who; else if (verb === "raises") flopRaised = true;
          flopActed.add(who);
        }
      }
      if (verb === "folds") folded.add(who);
    }
    const river = lines.some((l) => /^\*+\s*RIVER/i.test(l));
    const left = names.filter((n) => !folded.has(n));
    for (const n of names) {
      const s = st.get(n)!;
      s.vpip[1]++; s.pfr[1]++; s.sawFlop[1]++;
      if (vpip.has(n)) s.vpip[0]++; if (pfr.has(n)) s.pfr[0]++;
      if (saw.has(n)) { s.showdown[1]++; if (river && left.length >= 2 && !folded.has(n)) s.showdown[0]++; }
    }
  }
  const hero = [...heroCount].sort((a, b) => b[1] - a[1])[0]?.[0];
  const players = [...by.values()].sort((a, b) => b.hands - a.hands || a.name.localeCompare(b.name));
  for (const p of players) p.hero = p.name === hero;
  players.sort((a, b) => Number(b.hero) - Number(a.hero));
  return { players, hands, unread };
}

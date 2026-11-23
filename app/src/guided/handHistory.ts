// Reads a pasted hand history and turns it into the spot builder's inputs: table, each seat's
// preflop action, the board, and the hero's cards. handFormats.ts first rewrites whatever layout
// was pasted into the PokerStars text layout, which is what this file reads.
import { splitHands, toStarsLayout } from "./handFormats";
import { seatsFor, type Act, type SeatInput, type TableSetup } from "./preflop";

export interface ParsedHand {
  table: TableSetup; seats: SeatInput[]; hero: string; heroHand: string; board: number[];
  /** the layout the paste was recognised as, e.g. "PokerStars", "888poker", "Ignition" */
  site: string;
  /** things the reader should know: guesses made and anything the setup cannot express */
  notes: string[];
  /** what happened from the flop on, in order: bets (the player's total on that street, in the setup's units) and the cards dealt */
  later: LaterStep[];
}
export type LaterStep = { kind: "action"; seat: string; verb: "fold" | "check" | "call" | "bet" | "raise"; to: number } | { kind: "card"; card: string };
const RANKS = "23456789TJQKA", SUITS = "cdhs";
const num = (s: string) => Number(s.replace(/[$€£,]/g, ""));
const cardId = (c: string) => RANKS.indexOf(c[0].toUpperCase()) * 4 + SUITS.indexOf(c[1].toLowerCase());
const SIZES = [2, 3, 6, 8, 9];

export function parseHandHistory(pasted: string): ParsedHand {
  const norm = toStarsLayout(pasted);
  const lines = norm.text.split("\n").map((l) => l.trim()).filter(Boolean);
  if (!lines.length) throw new Error("Paste a hand history first.");
  const notes: string[] = [...norm.notes];
  const holeAt = lines.findIndex((l) => /^\*+\s*HOLE CARDS/i.test(l) || /^Dealt to /i.test(l));
  if (holeAt < 0) throw new Error("This does not look like a hand history: the layout was not recognised. Paste the whole hand as the site saved it (PokerStars, GGPoker, Winamax, 888poker, partypoker, Americas Cardroom, Ignition or Bovada, iPoker, or a PokerNow log).");
  const head = lines.slice(0, holeAt), rest = lines.slice(holeAt);
  const tourney = /tournament|sit\s*&?\s*go|level\s+[ivxl\d]+/i.test(head.slice(0, 2).join(" "));

  // who is sitting where
  const seated: { no: number; name: string; stack: number }[] = [];
  for (const l of head) {
    const m = l.match(/^Seat\s+(\d+):\s+(.+?)\s+\(\s*[$€£]?([\d,.]+)(?:\s+in chips)?[^)]*\)(.*)$/i);
    if (m && !/sitting out|out of hand/i.test(m[4])) seated.push({ no: Number(m[1]), name: m[2], stack: num(m[3]) });
  }
  if (seated.length < 2) throw new Error("Could not find the players (lines like \"Seat 1: Name ($200 in chips)\").");
  if (seated.length > 9) throw new Error("This hand has more than nine players, which the setup does not cover.");
  const nameOf = (raw: string) => seated.find((s) => s.name === raw) ?? seated.find((s) => raw.startsWith(s.name));

  // blinds and antes
  let sb = 0, bb = 0; let sbName = "", bbName = ""; const antes = new Set<string>();
  for (const l of head) {
    let m = l.match(/^(.+?):?\s+posts (?:the )?small blind\s+[$€£]?([\d,.]+)/i); if (m) { sb = num(m[2]); sbName = nameOf(m[1])?.name ?? ""; continue; }
    m = l.match(/^(.+?):?\s+posts (?:the )?big blind\s+[$€£]?([\d,.]+)/i); if (m && !bb) { bb = num(m[2]); bbName = nameOf(m[1])?.name ?? ""; continue; }
    m = l.match(/^(.+?):?\s+posts (?:the )?ante\s+[$€£]?([\d,.]+)/i); if (m) antes.add(nameOf(m[1])?.name ?? m[1]);
  }
  if (!bb) { const m = head.join(" ").match(/[$€£]?([\d,.]+)\s*\/\s*[$€£]?([\d,.]+)/); if (m) { sb = num(m[1]); bb = num(m[2]); } }
  if (!bb) throw new Error("Could not find the blinds.");
  if (!sb) sb = bb / 2;

  // order of action before the flop: first to act ... button, small blind, big blind
  const btn = Number((head.join("\n").match(/Seat\s+#?(\d+) is the button/i) ?? [])[1] ?? NaN);
  const ring = [...seated].sort((a, b) => a.no - b.no);
  let bbAt = ring.findIndex((s) => s.name === bbName);
  if (bbAt < 0) {
    const b = ring.findIndex((s) => s.no === btn);
    if (b < 0) throw new Error("Could not tell who the blinds are.");
    bbAt = (b + (ring.length === 2 ? 1 : 2)) % ring.length;
  }
  const order = ring.map((_s, i) => ring[(bbAt + 1 + i) % ring.length]); // ends with the big blind
  const size = SIZES.find((n) => n >= order.length)!;
  const names = seatsFor(size).slice(size - order.length);
  if (order.length === 2) { names[0] = "BTN"; names[1] = "BB"; }
  if (size !== order.length) notes.push(`${order.length} players were dealt in, so this uses the ${size}-handed table with the earliest seat${size - order.length > 1 ? "s" : ""} folded.`);
  const seatOf = new Map(order.map((p, i) => [p.name, names[i]]));
  if (sbName && seatOf.get(sbName) !== "SB" && order.length > 2) notes.push("The blinds were posted out of the usual order; seats are placed from the big blind backwards.");

  // preflop action
  const k = tourney ? bb : 1; // tournaments are set up in big blinds
  const r1 = (x: number) => Math.round((x / k) * 100) / 100;
  const inputs = new Map<string, SeatInput>(order.map((p) => [p.name, { seat: seatOf.get(p.name)!, act: "fold" as Act, raiseTo: 0, reply: "call" as const, stack: r1(p.stack), type: "solid" as const }]));
  const flopAt = rest.findIndex((l) => /^\*+\s*FLOP/i.test(l));
  const pre = flopAt < 0 ? rest : rest.slice(0, flopAt);
  let raises = 0; const acted = new Set<string>(); let heroName = "", heroHand = "";
  for (const l of pre) {
    const d = l.match(/^Dealt to (.+?)\s*\[\s*([2-9TJQKA][cdhs])\s*([2-9TJQKA][cdhs])\s*\]/i);
    if (d) { heroName = nameOf(d[1])?.name ?? ""; heroHand = d[2][0].toUpperCase() + d[2][1].toLowerCase() + d[3][0].toUpperCase() + d[3][1].toLowerCase(); continue; }
    const m = l.match(/^(.+?):?\s+(folds|checks|calls|raises|bets|is all-?in|all-?in)\b(.*)$/i);
    if (!m) continue;
    const who = nameOf(m[1]); if (!who) continue;
    const inp = inputs.get(who.name)!; const verb = m[2].toLowerCase(); const first = !acted.has(who.name); acted.add(who.name);
    const amounts = [...m[3].matchAll(/[$€£]?([\d,]*\.?\d+)/g)].map((x) => num(x[1]));
    if (verb === "folds") { if (first) inp.act = "fold"; else inp.reply = "fold"; }
    else if (verb === "checks") { if (first) inp.act = "limp"; }
    else if (verb === "calls") { if (first) inp.act = raises ? "call" : "limp"; else inp.reply = "call"; }
    else {
      raises++;
      const to = /raises/.test(verb) && amounts.length >= 2 ? amounts[amounts.length - 1] : amounts[amounts.length - 1] ?? 0;
      if (!first && inp.act === "raise") notes.push(`${inp.seat} raised more than once before the flop; the last raise size is used.`);
      inp.act = "raise"; inp.raiseTo = r1(to); inp.reply = "call";
      if (/all-?in/i.test(l)) notes.push(`${inp.seat} was all-in before the flop. With nothing left to bet, use Push / Fold for that decision instead.`);
    }
  }
  if (!heroName) { notes.push("The hand does not say which cards you held (no \"Dealt to\" line), so the big blind is treated as you. Change it in step 2."); heroName = order[order.length - 1].name; }

  // board
  const board: number[] = [];
  for (const l of rest) {
    if (!/^\*+\s*(FLOP|TURN|RIVER)/i.test(l)) continue;
    const groups = [...l.matchAll(/\[([^\]]+)\]/g)].map((g) => g[1].trim().split(/\s+/));
    for (const c of groups[groups.length - 1] ?? []) { if (/^[2-9TJQKA][cdhs]$/i.test(c) && !board.includes(cardId(c))) board.push(cardId(c)); }
  }
  if (board.length < 3) throw new Error("This hand ended before the flop, so there is no flop to study. For all-in decisions before the flop, use Push / Fold.");

  // what happened from the flop on
  const later: LaterStep[] = [];
  if (flopAt >= 0) {
    const put = new Map<string, number>(); // each player's total on the current street
    for (const l of rest.slice(flopAt + 1)) {
      if (/^\*+\s*(SHOW ?DOWN|SUMMARY)/i.test(l)) break;
      const st = l.match(/^\*+\s*(TURN|RIVER)/i);
      if (st) {
        const groups = [...l.matchAll(/\[([^\]]+)\]/g)].map((g) => g[1].trim().split(/\s+/));
        const c = (groups[groups.length - 1] ?? []).find((x) => /^[2-9TJQKA][cdhs]$/i.test(x));
        if (c) later.push({ kind: "card", card: c[0].toUpperCase() + c[1].toLowerCase() });
        put.clear();
        continue;
      }
      const m = l.match(/^(.+?):?\s+(folds|checks|calls|raises|bets)\b(.*)$/i);
      if (!m) continue;
      const who = nameOf(m[1]); const seat = who && seatOf.get(who.name); if (!who || !seat) continue;
      const verb = m[2].toLowerCase(); const amounts = [...m[3].matchAll(/[$€£]?([\d,]*\.?\d+)/g)].map((x) => num(x[1]));
      const last = amounts[amounts.length - 1] ?? 0;
      if (verb === "folds") later.push({ kind: "action", seat, verb: "fold", to: 0 });
      else if (verb === "checks") later.push({ kind: "action", seat, verb: "check", to: 0 });
      else if (verb === "calls") { put.set(who.name, (put.get(who.name) ?? 0) + last); later.push({ kind: "action", seat, verb: "call", to: r1(put.get(who.name)!) }); }
      else if (verb === "bets") { put.set(who.name, last); later.push({ kind: "action", seat, verb: "bet", to: r1(last) }); }
      else { put.set(who.name, last); later.push({ kind: "action", seat, verb: "raise", to: r1(last) }); } // "raises 4 to 6": the total is the last number
    }
  }

  const stacks = order.map((p) => r1(p.stack)).sort((a, b) => a - b);
  const anteKind = !antes.size ? "none" : antes.size === 1 ? "bb" : "each";
  if (!tourney && antes.size) notes.push("This cash hand had antes; they are not added to the pot here.");
  const table: TableSetup = { game: tourney ? "tourney" : "cash", size, sb: tourney ? Math.round((sb / bb) * 100) / 100 : sb, bb: tourney ? 1 : bb, ante: tourney ? anteKind : "none", startStack: stacks[Math.floor(stacks.length / 2)] };
  // seats the table has but the hand did not use
  const all = seatsFor(size).map((s) => [...inputs.values()].find((x) => x.seat === s) ?? { seat: s, act: "fold" as Act, raiseTo: 0, reply: "call" as const, stack: table.startStack, type: "solid" as const });
  if (board.length > 3) {
    const shown = board.slice(3).map((c) => RANKS[c >> 2] + "♣♦♥♠"[c & 3]).join(" then ");
    notes.push(`The study starts on the flop. The hand went on to ${shown}: with two players, the result page reviews each of your decisions; otherwise click the actions that were taken and deal ${board.length > 4 ? "those cards" : "that card"} to follow it.`);
  }
  return { table, seats: all, hero: seatOf.get(heroName)!, heroHand, board: board.slice(0, 3), site: norm.site, notes, later };
}

/** One hand of a session, summarised for the list of hands worth studying. */
export interface SessionHand {
  /** position in the file, from 1 */
  no: number;
  /** the hand's own text, to load it into the setup */
  text: string;
  site: string; hero: string; heroHand: string;
  /** the board as far as it was dealt, e.g. "Ks 7d 2c 9h" */
  board: string;
  reached: "flop" | "turn" | "river";
  /** players who acted after the flop */
  players: number;
  /** what the hero put in after the flop, in big blinds */
  heroPut: number;
  /** "review": two players, so the decision-by-decision review applies. "study": three players, the spot can be solved but not reviewed. */
  kind: "review" | "study";
}
export interface Session { hands: SessionHand[]; total: number; left: { why: string; count: number }[] }

/** Reads a whole session (many hands in one paste or file) and lists the hands that can be studied, biggest first. */
export function readSession(text: string): Session {
  const parts = splitHands(text);
  const hands: SessionHand[] = []; const left = new Map<string, number>();
  const skip = (why: string) => left.set(why, (left.get(why) ?? 0) + 1);
  parts.forEach((part, i) => {
    let p: ParsedHand;
    try { p = parseHandHistory(part); } catch (e) {
      const m = String((e as Error).message ?? e);
      skip(/ended before the flop/.test(m) ? "ended before the flop" : /not no-limit hold'em/.test(m) ? "not no-limit hold'em" : /more than nine/.test(m) ? "more than nine players" : "could not be read");
      return;
    }
    const acts = p.later.filter((s): s is Extract<LaterStep, { kind: "action" }> => s.kind === "action");
    const seats = new Set(acts.map((a) => a.seat));
    if (!p.heroHand) return skip("your cards are not shown");
    if (!seats.size) return skip("everyone was all-in before the flop");
    if (!seats.has(p.hero)) return skip("you folded before the flop");
    if (seats.size > 3) return skip("more than three players saw the flop");
    // what the hero put in: on each street, the last total they reached
    let put = 0, street = 0;
    for (const s of p.later) { if (s.kind === "card") { put += street; street = 0; } else if (s.seat === p.hero && s.verb !== "fold" && s.verb !== "check") street = s.to; }
    put += street;
    const cards = [...p.board.map((c) => RANKS[c >> 2] + "cdhs"[c & 3]), ...p.later.filter((s): s is Extract<LaterStep, { kind: "card" }> => s.kind === "card").map((s) => s.card)];
    hands.push({
      no: i + 1, text: part, site: p.site, hero: p.hero, heroHand: p.heroHand, board: cards.join(" "),
      reached: cards.length >= 5 ? "river" : cards.length === 4 ? "turn" : "flop", players: seats.size,
      heroPut: Math.round((put / (p.table.game === "cash" ? p.table.bb : 1)) * 10) / 10, kind: seats.size === 2 ? "review" : "study",
    });
  });
  hands.sort((a, b) => (a.kind === b.kind ? b.heroPut - a.heroPut || a.no - b.no : a.kind === "review" ? -1 : 1));
  return { hands, total: parts.length, left: [...left].map(([why, count]) => ({ why, count })).sort((a, b) => b.count - a.count) };
}

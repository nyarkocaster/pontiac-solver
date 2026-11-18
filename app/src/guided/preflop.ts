// Describes what happened before the flop, seat by seat, and turns it into a spot the solvers can
// take: who is left, the pot, the stacks behind, a starting range for each player, and which solver
// fits. Amounts are in dollars for cash games and in big blinds for tournaments.
import { formatRange, parseRange, rangePercent, resize, topPercent, without, type RangeKind } from "./rangeTools";

export type Game = "cash" | "tourney";
export type AnteKind = "none" | "bb" | "each";
export interface TableSetup { game: Game; size: number; sb: number; bb: number; ante: AnteKind; startStack: number }

export const TABLE_SIZES = [
  { n: 9, label: "9-handed" }, { n: 8, label: "8-handed" }, { n: 6, label: "6-max" }, { n: 3, label: "3-handed" }, { n: 2, label: "Heads-up" },
];
const SEAT_ORDER: Record<number, string[]> = {
  9: ["UTG", "UTG+1", "MP", "LJ", "HJ", "CO", "BTN", "SB", "BB"],
  8: ["UTG", "UTG+1", "LJ", "HJ", "CO", "BTN", "SB", "BB"],
  6: ["LJ", "HJ", "CO", "BTN", "SB", "BB"],
  3: ["BTN", "SB", "BB"],
  2: ["BTN", "BB"],
};
export const SEAT_NAME: Record<string, string> = {
  UTG: "Under the gun", "UTG+1": "Under the gun +1", MP: "Middle position", LJ: "Lojack", HJ: "Hijack", CO: "Cutoff", BTN: "Button", SB: "Small blind", BB: "Big blind",
};
export const seatsFor = (size: number) => SEAT_ORDER[size] ?? SEAT_ORDER[9];

export type TypeId = "solid" | "rec" | "station" | "nit" | "maniac";
export const PLAYER_TYPES: { id: TypeId; label: string; help: string; open: number; call: number; reraise: number; profile: string | null }[] = [
  { id: "solid", label: "Solid regular", help: "Disciplined ranges. Use for players you respect, and for yourself.", open: 1, call: 1, reraise: 1, profile: null },
  { id: "rec", label: "Loose recreational", help: "Plays too many hands, calls more than raises.", open: 1.15, call: 1.6, reraise: 0.6, profile: "austin" },
  { id: "station", label: "Calling station", help: "Calls with almost anything, rarely raises.", open: 1.1, call: 2.0, reraise: 0.5, profile: "station" },
  { id: "nit", label: "Tight player (nit)", help: "Few hands, and only strong ones.", open: 0.7, call: 0.7, reraise: 0.6, profile: "nit" },
  { id: "maniac", label: "Very aggressive", help: "Raises and re-raises far more than normal.", open: 1.6, call: 1.3, reraise: 2.0, profile: "maniac" },
];
const typeOf = (id: TypeId) => PLAYER_TYPES.find((t) => t.id === id) ?? PLAYER_TYPES[0];

export type Act = "fold" | "limp" | "call" | "raise";
export interface SeatInput { seat: string; act: Act; raiseTo: number; reply: "call" | "fold"; stack: number; type: TypeId }

export interface FlopPlayer {
  seat: string; name: string; hero: boolean; type: TypeId; role: string; rangeKind: RangeKind;
  range: string; rangePct: number; put: number; behind: number; raisedLast: boolean; score: number; why: string;
}
export interface Preflop {
  /** what each seat did, in words, for seats that put money in voluntarily or folded after doing so */
  story: string;
  players: FlopPlayer[];   // in the order they act after the flop
  pot: number; dead: number; raises: number; lastBet: number;
  /** seats that folded before the flop, with the stack each has left (for prize-money solves) */
  out: { seat: string; name: string; behind: number }[];
  /** per seat: what the seat may do now, and whether it has to answer a later raise */
  options: Record<string, { acts: Act[]; needsReply: boolean; minRaise: number; facing: number }>;
  errors: string[];
}

const BASE_OPEN: Record<string, number> = { UTG: 12, "UTG+1": 13, MP: 15, LJ: 18, HJ: 22, CO: 28, BTN: 43, SB: 38, BB: 30 };
const BASE_CALL: Record<string, number> = { UTG: 4, "UTG+1": 4, MP: 4.5, LJ: 4.5, HJ: 5.5, CO: 7, BTN: 9.5, SB: 6 };
const BB_CALL_VS: Record<string, number> = { UTG: 23, "UTG+1": 23, MP: 25, LJ: 27, HJ: 30, CO: 34, BTN: 42, SB: 50 };
const calling = (pct: number) => formatRange(resize({ weights: new Map(), extras: [] }, Math.max(1, Math.min(96, pct)), "call"));
const raising = (pct: number) => topPercent(Math.max(0.9, Math.min(100, pct)));
const r2 = (x: number) => Math.round(x * 100) / 100;

/** Works out the spot from the table and each seat's action. `hero` is a seat name. */
export function buildPreflop(table: TableSetup, inputs: SeatInput[], hero: string): Preflop {
  const order = seatsFor(table.size);
  const bb = table.game === "tourney" ? 1 : table.bb;
  const sb = table.game === "tourney" ? 0.5 : table.sb;
  const headsUp = table.size === 2;
  const errors: string[] = [];
  const seats = order.map((s) => inputs.find((i) => i.seat === s) ?? { seat: s, act: "fold" as Act, raiseTo: 0, reply: "call" as const, stack: table.startStack, type: "solid" as TypeId });

  const put: Record<string, number> = {}; const inHand: Record<string, boolean> = {}; const role: Record<string, string> = {};
  const kind: Record<string, string> = {}; // open | iso | reraise | coldcall | bbcall | limp | complete | check | callraise | call3
  const options: Preflop["options"] = {};
  order.forEach((s) => { put[s] = 0; inHand[s] = false; });
  // blinds: heads-up the button posts the small blind
  const sbSeat = headsUp ? "BTN" : "SB";
  put[sbSeat] = sb; put.BB = bb;
  const perAnte = table.ante === "each" ? 0.125 * bb : 0;
  const dead = table.ante === "bb" ? bb : perAnte * table.size;

  let bet = bb, raises = 0, lastRaiser = -1, limpers = 0, callers = 0, firstRaiser = "";
  const story: string[] = [];
  seats.forEach((st, i) => {
    const s = st.seat;
    const acts: Act[] = bet > bb ? ["fold", "call", "raise"] : ["fold", "limp", "raise"];
    options[s] = { acts, needsReply: false, minRaise: r2(bet > bb ? bet * 2 : bb * 2), facing: bet };
    let act = st.act;
    if (act === "call" && bet <= bb) act = "limp";
    if (act === "limp" && bet > bb) act = "call";
    const name = SEAT_NAME[s] ?? s;
    if (act === "fold") return;
    inHand[s] = true;
    if (act === "limp") {
      put[s] = bb; limpers++;
      kind[s] = s === "BB" ? "check" : s === sbSeat ? "complete" : "limp";
      role[s] = s === "BB" ? "checks" : s === sbSeat && !headsUp ? "completes" : "limps";
      story.push(`${name} ${role[s]}`);
    } else if (act === "call") {
      put[s] = bet; callers++;
      kind[s] = raises >= 2 ? "call3" : s === "BB" ? "bbcall" : "coldcall";
      role[s] = raises >= 2 ? `calls the ${raises === 2 ? "3-bet" : "4-bet"}` : "calls";
      story.push(`${name} calls`);
    } else {
      const to = st.raiseTo > 0 ? st.raiseTo : options[s].minRaise;
      if (to <= bet) errors.push(`${name}'s raise has to be bigger than the bet before it (${fmtAmt(table, bet)}).`);
      raises++; lastRaiser = i; if (!firstRaiser) firstRaiser = s;
      put[s] = to; bet = Math.max(bet, to);
      kind[s] = raises === 1 ? (limpers > 0 ? "iso" : "open") : "reraise";
      role[s] = raises === 1 ? (limpers > 0 ? "raises the limpers" : "opens") : raises === 2 ? "3-bets" : raises === 3 ? "4-bets" : "5-bets";
      story.push(`${name} ${raises === 1 ? "raises" : raises === 2 ? "3-bets" : "re-raises"} to ${fmtAmt(table, to)}`);
    }
  });
  // players who acted before the last raise have to answer it (call or fold; no further re-raise here)
  if (lastRaiser >= 0) {
    seats.forEach((st, i) => {
      const s = st.seat;
      if (i >= lastRaiser || !inHand[s] || put[s] >= bet) return;
      options[s].needsReply = true;
      const name = SEAT_NAME[s] ?? s;
      if (st.reply === "fold") { inHand[s] = false; story.push(`${name} folds to the raise`); return; }
      const was = kind[s];
      put[s] = bet;
      kind[s] = was === "open" || was === "iso" || was === "reraise" ? "callraise" : raises >= 2 ? "call3" : "calliso";
      role[s] = `${role[s]}, then calls the ${raises >= 3 ? "4-bet" : raises === 2 ? "3-bet" : "raise"}`;
      story.push(`${name} calls`);
    });
  }

  const pot = r2(order.reduce((a, s) => a + put[s], 0) + dead);
  const short = table.game === "tourney" && table.startStack < 23;
  const openOf = (s: string, t: TypeId) => (headsUp && s === "BTN" ? 80 : BASE_OPEN[s] ?? 20) * typeOf(t).open * (short ? 0.85 : 1);
  const lastRaiserSeat = lastRaiser >= 0 ? seats[lastRaiser].seat : "";
  const others = order.filter((s) => inHand[s]).length;

  const rangeFor = (st: SeatInput): { text: string; rk: RangeKind } => {
    const s = st.seat; const t = typeOf(st.type); const k = kind[s];
    const callM = t.call * (short ? 0.8 : 1);
    switch (k) {
      case "open": return { text: raising(openOf(s, st.type)), rk: "open" };
      case "iso": return { text: raising(openOf(s, st.type) * 0.6), rk: "open" };
      case "reraise": {
        const vs = firstRaiser; const late = ["CO", "BTN", "SB"].includes(vs);
        const base = raises >= 3 ? 2.4 : s === "BB" || s === "SB" ? (late ? 9 : 5) : late ? 7 : 4.5;
        return { text: raising(base * t.reraise), rk: "open" };
      }
      case "callraise": {
        // an earlier raiser who calls a re-raise: the better part of their range, without the hands that would raise again
        const was = openOf(s, st.type);
        return { text: without(raising(Math.min(was * 0.45, 14) * Math.max(1, t.call * 0.8)), "AA,KK"), rk: "call" };
      }
      case "call3": return { text: calling(2.6 * callM), rk: "call" };
      case "calliso": return { text: calling(13 * callM), rk: "call" };
      case "coldcall": return { text: calling((headsUp ? 55 : BASE_CALL[s] ?? 5) * callM), rk: "call" };
      case "bbcall": return { text: calling(Math.min(88, ((headsUp ? 62 : BB_CALL_VS[firstRaiser] ?? 28) + (others > 2 ? 5 : 0)) * Math.max(0.8, callM * 0.85))), rk: "call" };
      case "limp": return { text: calling(Math.min(70, (headsUp ? 60 : 20) * callM)), rk: "call" };
      case "complete": return { text: calling(Math.min(80, 45 * Math.max(0.8, callM * 0.8))), rk: "call" };
      case "check": return { text: calling(88), rk: "call" };
      default: return { text: "", rk: "open" };
    }
  };

  // order of play after the flop: blinds first, then around to the button
  const post = headsUp ? ["BB", "BTN"] : [...order.filter((s) => s === "SB" || s === "BB"), ...order.filter((s) => s !== "SB" && s !== "BB")];
  const players: FlopPlayer[] = post.filter((s) => inHand[s]).map((s) => {
    const st = seats.find((x) => x.seat === s)!;
    const anteCost = perAnte + (s === "BB" && table.ante === "bb" ? bb : 0);
    const behind = r2(st.stack - put[s] - anteCost);
    const name = SEAT_NAME[s] ?? s;
    if (behind <= 0) errors.push(`${name} would be all-in before the flop. For all-in spots use the Push / Fold tab.`);
    const { text, rk } = rangeFor(st);
    return {
      seat: s, name: s === hero ? `You (${name})` : name, hero: s === hero, type: st.type, role: role[s] ?? "", rangeKind: rk,
      range: text, rangePct: rangePercent(parseRange(text)), put: put[s], behind, raisedLast: s === lastRaiserSeat, score: 0, why: "",
    };
  });
  // how much each opponent matters to the hand, for choosing who to leave out when four or more see the flop
  players.forEach((p, i) => {
    const bits: string[] = []; let sc = 0;
    if (p.raisedLast) { sc += 3; bits.push("made the last raise, so drives the betting"); }
    const depth = Math.min(3, p.behind / Math.max(pot, 1e-9)); sc += depth;
    if (depth < 0.75) bits.push("has a short stack next to the pot"); else if (depth >= 2) bits.push("has a deep stack behind");
    const heroAt = players.findIndex((x) => x.hero);
    if (heroAt >= 0 && i > heroAt) { sc += 1; bits.push("acts after you"); }
    if (p.rangePct > 45) { sc -= 1; bits.push("holds a very wide, weak range"); }
    else if (p.rangePct < 9) { sc += 0.5; bits.push("holds a narrow, strong range"); }
    p.score = r2(sc); p.why = bits.join("; ");
  });
  const out = order.filter((s) => !inHand[s]).map((s) => {
    const st = seats.find((x) => x.seat === s)!;
    return { seat: s, name: SEAT_NAME[s] ?? s, behind: Math.max(0, r2(st.stack - put[s] - perAnte - (s === "BB" && table.ante === "bb" ? bb : 0))) };
  });
  return { story: story.length ? story.join(". ") + "." : "Nobody has entered the pot yet.", players, pot, dead, raises, lastBet: bet, options, errors, out };
}

export const fmtAmt = (t: { game: Game }, x: number) => (t.game === "tourney" ? `${r2(x)} bb` : `$${r2(x)}`);

export type RouteKind = "none" | "two" | "three" | "many";
export interface Route {
  kind: RouteKind; title: string; text: string[];
  /** when four or more see the flop: the opponents suggested to keep, by seat */
  keep: string[];
}

/** Which solver fits, and what to do when the spot is bigger than the solvers go. */
export function routeFor(p: Preflop): Route {
  const n = p.players.length; const hero = p.players.find((x) => x.hero);
  if (n < 2) return { kind: "none", title: "Nobody to play against yet", text: ["Give at least two players an action that takes them to the flop."], keep: [] };
  if (!hero) return { kind: "none", title: "You are not in the hand", text: ["Mark your own seat with \"This is me\" and give it an action that sees the flop."], keep: [] };
  if (n === 2) return { kind: "two", title: "Two players: exact solve", text: ["Solved with every turn and river card. Usually well under a minute to a few minutes."], keep: [] };
  if (n === 3) return { kind: "three", title: "Three players: three-way solve", text: ["Solved with the three-player engine. Flops take minutes and deal a balanced sample of later cards; any turn card can be solved exactly afterwards."], keep: [] };
  const opp = p.players.filter((x) => !x.hero).sort((a, b) => b.score - a.score);
  return {
    kind: "many", title: `${n} players see the flop: more than any solver here can handle`,
    text: [
      "Each extra player multiplies the work: three players already take minutes and gigabytes on a flop, and four would take days. No widely available solver handles four-way flops exactly.",
      "The usual way to study it is to solve the hand three-way with the two opponents who matter most, and leave the others' chips in the pot as dead money.",
      "Then adjust for the missing players at the table: bluff less, value-bet a little thinner hands less often, and give more credit to bets and raises, because every extra player makes it likelier someone has a strong hand.",
    ],
    keep: opp.slice(0, 2).map((x) => x.seat),
  };
}

export interface SolveSpot {
  players: FlopPlayer[]; pot: number; description: string; left: FlopPlayer[];
}
/** The spot to solve: the hero plus the chosen opponents. Everyone else's chips stay in the pot. */
export function spotFor(p: Preflop, keep: string[]): SolveSpot {
  const players = p.players.filter((x) => x.hero || keep.includes(x.seat) || p.players.length <= 3);
  const left = p.players.filter((x) => !players.includes(x));
  return { players, pot: p.pot, description: p.story, left };
}

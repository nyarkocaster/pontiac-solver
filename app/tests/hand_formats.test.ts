// Checks that every hand-history layout reads to the same spot as the PokerStars layout.
// Run: node scripts/test_hand_formats.mjs
import { parseHandHistory, type ParsedHand } from "../src/guided/handHistory";
import * as H from "./hands";

let failed = 0, passed = 0;
function check(name: string, ok: boolean, detail = "") { if (ok) passed++; else { failed++; console.log(`FAIL ${name}${detail ? "\n     " + detail : ""}`); } }
const core = (p: ParsedHand, withHero = true) => JSON.stringify({ table: p.table, seats: p.seats, board: p.board, later: p.later, ...(withHero ? { hero: p.hero, heroHand: p.heroHand } : {}) });
function same(name: string, text: string, ref: ParsedHand, site: string, withHero = true) {
  let p: ParsedHand;
  try { p = parseHandHistory(text); } catch (e) { check(name, false, String(e)); return undefined; }
  check(`${name}: same spot as the PokerStars layout`, core(p, withHero) === core(ref, withHero), `got  ${core(p, withHero)}\n     want ${core(ref, withHero)}`);
  check(`${name}: recognised as ${site}`, p.site === site, `got ${p.site}`);
  check(`${name}: no warning about amounts`, !p.notes.some((n) => /add up to more/.test(n)), p.notes.join(" | "));
  return p;
}

const ref = parseHandHistory(H.STARS);
// the reference itself, checked by hand
check("reference: table", ref.table.game === "cash" && ref.table.size === 6 && ref.table.sb === 1 && ref.table.bb === 2, JSON.stringify(ref.table));
check("reference: hero", ref.hero === "SB" && ref.heroHand === "AhKd" && ref.site === "PokerStars");
const seat = (p: ParsedHand, s: string) => p.seats.find((x) => x.seat === s)!;
check("reference: button raised to 6 and called the re-raise", seat(ref, "BTN").act === "raise" && seat(ref, "BTN").raiseTo === 6 && seat(ref, "BTN").reply === "call" && seat(ref, "BTN").stack === 200);
check("reference: small blind re-raised to 20", seat(ref, "SB").act === "raise" && seat(ref, "SB").raiseTo === 20 && seat(ref, "SB").stack === 210.5);
check("reference: big blind and cutoff folded", seat(ref, "BB").act === "fold" && seat(ref, "CO").act === "fold");
check("reference: flop Ks 7d 2c", ref.board.join() === [11 * 4 + 3, 5 * 4 + 1, 0 * 4 + 0].join(), ref.board.join());
check("reference: what happened after the flop", JSON.stringify(ref.later) === JSON.stringify([
  { kind: "action", seat: "SB", verb: "bet", to: 15 }, { kind: "action", seat: "BTN", verb: "raise", to: 45 }, { kind: "action", seat: "SB", verb: "call", to: 45 },
  { kind: "card", card: "9h" }, { kind: "action", seat: "SB", verb: "check", to: 0 }, { kind: "action", seat: "BTN", verb: "bet", to: 60 }, { kind: "action", seat: "SB", verb: "call", to: 60 },
  { kind: "card", card: "3s" }, { kind: "action", seat: "SB", verb: "check", to: 0 }, { kind: "action", seat: "BTN", verb: "bet", to: 75 }, { kind: "action", seat: "SB", verb: "fold", to: 0 },
]), JSON.stringify(ref.later));

same("GGPoker", H.GG, ref, "GGPoker");
same("Winamax", H.WINAMAX, ref, "Winamax");
same("888poker", H.P888, ref, "888poker");
same("partypoker", H.PARTY, ref, "partypoker");
const acr = same("Americas Cardroom", H.ACR, ref, "Americas Cardroom");
check("Americas Cardroom: says the game type was a guess", !!acr?.notes.some((n) => /cash game or a tournament/.test(n)));
same("Ignition", H.IGNITION, ref, "Ignition");
same("888poker skin", H.P888.replace("888poker Hand History", "Cassava Hand History"), ref, "888poker");
same("Bovada", H.IGNITION.replace("Ignition Hand", "Bovada Hand"), ref, "Bovada");
same("iPoker", H.IPOKER, ref, "iPoker");
const now = same("PokerNow file", H.POKERNOW, ref, "PokerNow", false);
check("PokerNow file: says your seat is not known and does not track the hand", !!now && now.heroHand === "" && now.notes.some((n) => /not which player you are/.test(n)), now?.notes.join(" | "));
same("PokerNow copied lines", H.POKERNOW_PLAIN, ref, "PokerNow", false);
const shown = parseHandHistory(H.POKERNOW_SHOWN);
check("PokerNow at showdown: finds your seat from the cards shown", shown.hero === "SB" && shown.heroHand === "AhKd", `${shown.hero} ${shown.heroHand}`);
// two hands in one paste: the first is read and a note says so
const two = same("two hands pasted together", H.P888 + "\n\n" + H.P888.replace(/Game 1 /, "Game 2 ").replace("Ks, 7d, 2c", "Qs, 7d, 2c"), ref, "888poker");
check("two hands pasted together: note", !!two?.notes.some((n) => /only the first/.test(n)));
const log2 = H.POKERNOW_PLAIN.replace(/#7/g, "#6").replace(/^Flop[\s\S]*?(?=-- ending)/m, "") + "\n" + H.POKERNOW_PLAIN;
const picked = same("PokerNow log with an earlier hand that ended before the flop", log2, ref, "PokerNow", false);
check("PokerNow log: says which hand was picked", !!picked?.notes.some((n) => /hand #7/.test(n)), picked?.notes.join(" | "));

const tref = parseHandHistory(H.STARS_TOURNEY);
check("tournament reference", tref.table.game === "tourney" && tref.table.size === 2 && tref.table.bb === 1 && tref.table.sb === 0.5 && tref.table.ante === "each" && tref.hero === "BTN" && tref.heroHand === "QsJh" && seat(tref, "BTN").raiseTo === 2 && seat(tref, "BTN").stack === 20 && seat(tref, "BB").stack === 30 && seat(tref, "BB").act === "call", JSON.stringify(tref));
check("tournament reference: river bet of 4.5 big blinds", JSON.stringify(tref.later.slice(-2)) === JSON.stringify([{ kind: "action", seat: "BB", verb: "bet", to: 4.5 }, { kind: "action", seat: "BTN", verb: "call", to: 4.5 }]), JSON.stringify(tref.later));
same("Ignition tournament", H.IGNITION_TOURNEY, tref, "Ignition");

const twice = parseHandHistory(H.GG_TWICE);
check("GGPoker dealt twice: first board only", twice.later.filter((s) => s.kind === "card").map((s) => (s as { card: string }).card).join() === "9h,3s" && twice.board.length === 3, JSON.stringify(twice.later));

for (const [name, text, want] of [
  ["nothing pasted", "   ", /Paste a hand history first/],
  ["not a hand", "hello there\nthis is not poker", /layout was not recognised/],
  ["hand that ended before the flop", H.STARS.replace(/\*\*\* FLOP[\s\S]*?(?=Uncalled)/, ""), /ended before the flop/],
  ["Omaha hand", H.ACR.replace("(Hold'em)", "(Omaha)"), /not no-limit hold'em/],
  ["888 hand that ended before the flop", H.P888.replace(/\*\* Dealing flop[\s\S]*?(?=\*\* Summary)/, ""), /ended before the flop/],
] as [string, string, RegExp][]) {
  let msg = "";
  try { parseHandHistory(text); } catch (e) { msg = String(e); }
  check(`${name}: explains itself`, want.test(msg), msg || "no error");
}

console.log(`${passed} passed, ${failed} failed`);
if (failed) process.exit(1);

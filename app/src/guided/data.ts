// Guided Study data: range library, scenarios, sizing presets, leak presets.
// Hero ranges = Chris's Austin Stage 1 charts (retooled for the $15 open).
// Rec ranges = CLP "for-fun rec" profile + Claude estimates (slowplay weights).

export const RANGES: Record<string, { label: string; range: string; source: string }> = {
  hero_open_UTG: { label: "My UTG open (5.6%)", range: "66+,AKs,AKo,AQs", source: "Stage 1" },
  hero_open_UTG1: { label: "My UTG+1 open (7.5%)", range: "55+,AKs,AKo,AQs,AQo,AJs,KQs", source: "Stage 1" },
  hero_open_UTG2: { label: "My UTG+2 open (9.7%)", range: "55+,AKs,AKo,AQs,AQo,AJs,AJo,ATs,KQs,KQo", source: "Stage 1" },
  hero_open_LJ: { label: "My LJ open (12.5%)", range: "44+,ATs+,ATo+,A9s,KQs,KQo,KJs,KTs,QJs,JTs", source: "Stage 1" },
  hero_open_HJ: { label: "My HJ open (14.5%)", range: "33+,A8s+,A5s,A4s,ATo+,KTs+,KQo,QTs+,JTs,T9s", source: "Stage 1" },
  hero_open_CO: { label: "My CO open (18.6%)", range: "22+,A4s+,ATo+,K9s+,KJo+,QTs+,QJo,J9s+,T9s,98s,87s", source: "Stage 1" },
  hero_open_BTN: { label: "My BTN open (21.6%)", range: "22+,A2s+,A9o+,K8s+,KJo+,Q9s+,QJo,J9s+,T8s+,97s+,87s,76s", source: "Stage 1" },
  hero_open_SB: { label: "My SB open (13.0%)", range: "55+,A9s+,A5s,A4s,ATo+,KTs+,KQo,QTs+,JTs", source: "Stage 1" },
  hero_btnflat_EP: { label: "My BTN flat vs EP", range: "JJ-77,AQs-AJs", source: "Stage 1" },
  hero_btnflat_MP: { label: "My BTN flat vs MP", range: "TT-66,AJs-ATs,KQs,QJs", source: "Stage 1" },
  hero_btnflat_CO: { label: "My BTN flat vs CO", range: "99-66,AJs-ATs,KQs-KJs,QJs,JTs,T9s", source: "Stage 1" },
  hero_bbflat_EP: { label: "My BB flat vs EP", range: "JJ-88,AQs", source: "Stage 1" },
  hero_bbflat_MP: { label: "My BB flat vs MP", range: "TT-66,AJs,ATs,KQs", source: "Stage 1" },
  hero_bbflat_CO: { label: "My BB flat vs CO", range: "99-55,AJs-ATs,KQs-KJs,QJs", source: "Stage 1" },
  hero_bbflat_BTN: { label: "My BB flat vs BTN", range: "99-55,ATs-A5s,KJs-KTs,QJs-QTs,JTs,AJo,ATo,KQo,KJo", source: "Stage 1" },
  hero_bbflat_SB: { label: "My BB flat vs SB", range: "99-22,ATs-A2s,KJs-K9s,QJs-QTs,JTs,T9s,AJo-A9o,KQo-KTo", source: "Stage 1 (widened)" },
  rec_open: { label: "Rec's open", range: "TT+,ATs+,ATo+,KJs+,KJo+,QJs,QJo,JTs,JTo", source: "CLP for-fun rec" },
  rec_coldcall: {
    label: "Rec's cold-call",
    range: "AA:0.25,KK:0.5,QQ-22,AKs:0.5,AQs-A2s,AKo:0.5,AQo-ATo,KQs-K9s,KQo-KTo,QJs-Q9s,QJo-QTo,JTs-J9s,JTo,T9s-T8s,98s-97s,87s-86s,76s-75s,65s-64s,54s",
    source: "Claude estimate",
  },
  rec_bbdefend: {
    label: "Rec's BB defend",
    range: "AA:0.25,KK:0.5,QQ-22,AKs:0.5,AQs-A2s,AKo:0.5,AQo-A7o,KQs-K5s,KQo-K9o,QJs-Q7s,QJo-Q9o,JTs-J7s,JTo-J9o,T9s-T7s,T9o,98s-96s,98o,87s-85s,76s-74s,65s-63s,54s-53s,43s",
    source: "Claude estimate",
  },
  hu_btn_raise: {
    label: "HU button min-raise (approx.)",
    range: "22+,A2s+,K2s+,Q4s+,J6s+,T6s+,96s+,85s+,75s+,64s+,54s,A2o+,K5o+,Q8o+,J8o+,T8o+,98o",
    source: "Approximate starter range — replace with your own",
  },
  hu_bb_call: {
    label: "HU big blind call vs min-raise (approx.)",
    range: "99-22,AJs-A2s,K2s+,Q2s+,J4s+,T6s+,96s+,85s+,74s+,64s+,53s+,43s,ATo-A2o,K6o+,Q8o+,J8o+,T8o+,98o,87o",
    source: "Approximate starter range — replace with your own",
  },
};

export type Seat = "UTG" | "UTG1" | "UTG2" | "LJ" | "HJ" | "CO" | "BTN" | "SB";
export const SEAT_LABEL: Record<string, string> = {
  UTG: "UTG", UTG1: "UTG+1", UTG2: "UTG+2", LJ: "Lojack", HJ: "Hijack", CO: "Cutoff", BTN: "Button", SB: "Small blind",
};
const group = (s: string) => (["UTG", "UTG1", "UTG2"].includes(s) ? "EP" : ["LJ", "HJ"].includes(s) ? "MP" : s);

export interface Opponent { id: string; label: string; help: string }
export const OPPONENTS: Opponent[] = [
  { id: "rec", label: "Austin rec", help: "Plays the loose, wide ranges CLP describes for a for-fun recreational player." },
  { id: "solid", label: "Solid player", help: "Plays the same disciplined ranges you do. Good for learning baseline strategy." },
];

export interface ScenarioResult {
  heroIsOop: boolean;
  oopRange: string;
  ipRange: string;
  pot: number; // in chip units
  stack: number;
  pfrIsOop: boolean;
  description: string;
  oopName: string;
  ipName: string;
}

export interface Scenario {
  id: string;
  label: string;
  help: string;
  format: "live" | "hu";
  seats: string[]; // seat choices; meaning depends on scenario
  seatQuestion: string;
  build: (seat: string, opp: string, open: number, stack: number) => ScenarioResult;
}

export const SCENARIOS: Scenario[] = [
  {
    id: "open_bb",
    label: "I open, the big blind calls",
    help: "The most common pot you'll play. You raise, everyone folds to the big blind, and they call.",
    format: "live",
    seats: ["UTG", "UTG1", "UTG2", "LJ", "HJ", "CO", "BTN", "SB"],
    seatQuestion: "Where did you open from?",
    build: (seat, opp, open, stack) => {
      const hero = RANGES[`hero_open_${seat}`].range;
      const vill = opp === "rec" ? RANGES.rec_bbdefend.range : RANGES[`hero_bbflat_${group(seat)}`].range;
      const heroIsOop = seat === "SB";
      const pot = seat === "SB" ? 2 * open : 2 * open + 1;
      return {
        heroIsOop, pfrIsOop: heroIsOop,
        oopRange: heroIsOop ? hero : vill, ipRange: heroIsOop ? vill : hero,
        pot, stack: stack - open,
        oopName: heroIsOop ? "You (SB)" : "Big blind", ipName: heroIsOop ? "Big blind" : `You (${SEAT_LABEL[seat]})`,
        description: `You open to $${open} from the ${SEAT_LABEL[seat]}, the big blind calls.`,
      };
    },
  },
  {
    id: "open_ip_caller",
    label: "I open, one player calls behind me",
    help: "You raise, a player behind you calls, and the blinds fold. You play the hand out of position.",
    format: "live",
    seats: ["UTG", "UTG1", "UTG2", "LJ", "HJ", "CO"],
    seatQuestion: "Where did you open from?",
    build: (seat, opp, open, stack) => {
      const hero = RANGES[`hero_open_${seat}`].range;
      const vill = opp === "rec" ? RANGES.rec_coldcall.range : RANGES[`hero_btnflat_${group(seat)}`].range;
      return {
        heroIsOop: true, pfrIsOop: true, oopRange: hero, ipRange: vill,
        pot: 2 * open + 3, stack: stack - open,
        oopName: `You (${SEAT_LABEL[seat]})`, ipName: "Caller",
        description: `You open to $${open} from the ${SEAT_LABEL[seat]}, one player calls behind you, blinds fold.`,
      };
    },
  },
  {
    id: "rec_open_btn",
    label: "Someone opens, I call on the button",
    help: "A player raises before you and you flat on the button. Your only flatting seat besides the big blind.",
    format: "live",
    seats: ["UTG", "LJ", "CO"],
    seatQuestion: "Where did they open from?",
    build: (seat, opp, open, stack) => {
      const g = group(seat);
      const hero = RANGES[`hero_btnflat_${g}`].range;
      const vill = opp === "rec" ? RANGES.rec_open.range : RANGES[`hero_open_${seat}`].range;
      return {
        heroIsOop: false, pfrIsOop: true, oopRange: vill, ipRange: hero,
        pot: 2 * open + 3, stack: stack - open,
        oopName: `Opener (${SEAT_LABEL[seat]})`, ipName: "You (Button)",
        description: `A player opens to $${open} from the ${SEAT_LABEL[seat]}, you call on the button, blinds fold.`,
      };
    },
  },
  {
    id: "rec_open_bb",
    label: "Someone opens, I defend my big blind",
    help: "A player raises and you call from the big blind. You play out of position.",
    format: "live",
    seats: ["UTG", "LJ", "CO", "BTN", "SB"],
    seatQuestion: "Where did they open from?",
    build: (seat, opp, open, stack) => {
      const g = group(seat);
      const hero = RANGES[`hero_bbflat_${g}`].range;
      const vill = opp === "rec" ? RANGES.rec_open.range : RANGES[`hero_open_${seat}`].range;
      const pot = seat === "SB" ? 2 * open : 2 * open + 1;
      return {
        heroIsOop: seat !== "SB", pfrIsOop: seat === "SB",
        oopRange: seat === "SB" ? vill : hero, ipRange: seat === "SB" ? hero : vill,
        pot, stack: stack - open,
        oopName: seat === "SB" ? "Opener (SB)" : "You (Big blind)",
        ipName: seat === "SB" ? "You (Big blind)" : `Opener (${SEAT_LABEL[seat]})`,
        description: `A player opens to $${open} from the ${SEAT_LABEL[seat]}, you call in the big blind.`,
      };
    },
  },
  {
    id: "hu_sng",
    label: "Heads-up SNG: button min-raises, big blind calls",
    help: "Heads-up sit & go. The button (small blind) raises to 2 big blinds and the big blind calls. Chips are in big blinds.",
    format: "hu",
    seats: ["BTN", "BB"],
    seatQuestion: "Which seat are you?",
    build: (seat, _opp, _open, stackBB) => ({
      heroIsOop: seat === "BB", pfrIsOop: false,
      oopRange: RANGES.hu_bb_call.range, ipRange: RANGES.hu_btn_raise.range,
      pot: 40, stack: stackBB * 10 - 20, // tenths of a big blind
      oopName: seat === "BB" ? "You (Big blind)" : "Big blind", ipName: seat === "BTN" ? "You (Button)" : "Button",
      description: `Heads-up, ${stackBB} big blinds deep. The button raises to 2bb and the big blind calls.`,
    }),
  },
];

export interface SizingPreset { id: string; label: string; help: string; pfrFlop: string; callerFlop: string; turn: string; river: string; raise: string }
export const SIZINGS: Record<"live" | "hu", SizingPreset[]> = {
  live: [
    { id: "clp", label: "Simple — CLP default", help: "Flop 33% or 66%, turn 75%, river 75%. Small/big sizing like your CLP notes.", pfrFlop: "33%, 66%", callerFlop: "33%", turn: "75%", river: "75%", raise: "3x" },
    { id: "pot", label: "Pot-size test", help: "Flop 33% or a full pot bet. Tests CLP's 'bet pot in heads-up pots' advice.", pfrFlop: "33%, 100%", callerFlop: "33%", turn: "75%", river: "75%", raise: "3x" },
    { id: "one", label: "One size per street (fastest)", help: "Flop 33%, turn 66%, river 75%. Fastest solves, simplest to learn from.", pfrFlop: "33%", callerFlop: "33%", turn: "66%", river: "75%", raise: "3x" },
  ],
  hu: [
    { id: "hu", label: "Heads-up default", help: "Raiser bets 33% or 75% on the flop; turn 50%, river 75%; all-in available.", pfrFlop: "33%, 75%", callerFlop: "33%", turn: "50%", river: "75%", raise: "3x" },
    { id: "hu1", label: "Heads-up, one size (lighter)", help: "Flop 33%, turn 50%, river 75%. Uses less memory, but exploits can look extreme because a big value bet isn't available.", pfrFlop: "33%", callerFlop: "33%", turn: "50%", river: "75%", raise: "3x" },
  ],
};

export const ACCURACY = [
  { id: "quick", label: "Quick", help: "About a minute. Good for exploring.", pct: 1.0, iters: 400 },
  { id: "study", label: "Study", help: "Standard accuracy. Recommended.", pct: 0.5, iters: 1000 },
  { id: "precise", label: "Precise", help: "Slower, for close decisions.", pct: 0.25, iters: 2000 },
];

export interface LeakDef { id: string; label: string; help: string; street: string; effect: string }
export const LEAKS: LeakDef[] = [
  { id: "calls_too_much", label: "Calls too much — hates folding", help: "Folds far less than they should when you bet, on every street.", street: "all", effect: "fold_less" },
  { id: "calls_river", label: "Pays you off on the river", help: "Calls river bets far too often ('I had to see it').", street: "river", effect: "fold_less" },
  { id: "folds_too_much", label: "Folds too easily", help: "Gives up to bets too often — fit-or-fold.", street: "all", effect: "fold_more" },
  { id: "never_raises", label: "Rarely raises — a raise means the nuts", help: "Just calls with almost everything instead of raising.", street: "all", effect: "raise_less" },
  { id: "no_bluffs", label: "Bets mean strength — doesn't bluff", help: "When they bet, they almost always have it. Weak hands check.", street: "all", effect: "bluff_less" },
  { id: "bets_too_much", label: "Bets whenever checked to", help: "Fires too often when given the chance.", street: "all", effect: "bet_more" },
  { id: "passive", label: "Passive — rarely bets", help: "Checks most hands, even good ones.", street: "all", effect: "bet_less" },
];

export const STRENGTHS = [
  { label: "A little", value: 0.3 },
  { label: "A lot", value: 0.6 },
  { label: "Extreme", value: 0.9 },
];

export const PROFILES: { id: string; label: string; help: string; leaks: Record<string, number> }[] = [
  { id: "austin", label: "Typical Austin rec", help: "Calls too much, pays off rivers, rarely raises, bets mean strength.", leaks: { calls_too_much: 0.3, calls_river: 0.6, never_raises: 0.6, no_bluffs: 0.3 } },
  { id: "station", label: "Calling station", help: "Almost never folds, almost never raises.", leaks: { calls_too_much: 0.6, never_raises: 0.6 } },
  { id: "nit", label: "Nit", help: "Folds a lot and only bets with the goods.", leaks: { folds_too_much: 0.6, no_bluffs: 0.6 } },
  { id: "maniac", label: "Maniac", help: "Bets every time you check.", leaks: { bets_too_much: 0.6 } },
];

// ---------- cards / boards ----------
export const RANK_CHARS = "23456789TJQKA";
export const SUIT_CHARS = "cdhs";
export const SUIT_SYMBOL: Record<string, string> = { c: "♣", d: "♦", h: "♥", s: "♠" };
export const SUIT_COLOR: Record<string, string> = { c: "text-[#1E7A3C]", d: "text-[#1F5FBF]", h: "text-[#C8102E]", s: "text-ink" };
export const cardStr = (id: number) => RANK_CHARS[id >> 2] + SUIT_CHARS[id & 3];

export const TEXTURES: { id: string; label: string; test: (c: number[]) => boolean }[] = [
  { id: "any", label: "Any random flop", test: () => true },
  { id: "dry", label: "Dry, high card (like K72 rainbow)", test: (c) => rainbow(c) && !paired(c) && maxRank(c) >= 10 && spread(c) >= 6 },
  { id: "wet", label: "Wet & connected (like T98 two-tone)", test: (c) => !paired(c) && suits(c) === 2 && spread(c) <= 4 && maxRank(c) <= 10 },
  { id: "ace", label: "Ace-high", test: (c) => maxRank(c) === 12 && !paired(c) },
  { id: "paired", label: "Paired board", test: (c) => paired(c) },
  { id: "mono", label: "Monotone (one suit)", test: (c) => suits(c) === 1 },
  { id: "low", label: "Low board (like 652)", test: (c) => maxRank(c) <= 5 && !paired(c) },
];
const ranks = (c: number[]) => c.map((x) => x >> 2);
const maxRank = (c: number[]) => Math.max(...ranks(c));
const spread = (c: number[]) => Math.max(...ranks(c)) - Math.min(...ranks(c));
const paired = (c: number[]) => new Set(ranks(c)).size < 3;
const suits = (c: number[]) => new Set(c.map((x) => x & 3)).size;
const rainbow = (c: number[]) => suits(c) === 3;

export const randomFlop = (textureId: string): number[] => {
  const t = TEXTURES.find((x) => x.id === textureId) ?? TEXTURES[0];
  for (let i = 0; i < 5000; i++) {
    const deck = [...Array(52).keys()];
    const c: number[] = [];
    while (c.length < 3) c.push(deck.splice(Math.floor(Math.random() * deck.length), 1)[0]);
    if (t.test(c)) return c.sort((a, b) => b - a);
  }
  return [47, 21, 2];
};

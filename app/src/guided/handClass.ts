// Names what a two-card hand has made on a board ("Top pair", "Flush draw"), for grouping a range
// the way players talk about it. Cards are written like "Ah" or "Td".
const RANKS = "23456789TJQKA";
export const MADE = ["Straight flush", "Four of a kind", "Full house", "Flush", "Straight", "Three of a kind", "Two pair", "Overpair", "Top pair", "Middle pair", "Low pair", "Ace high", "No pair"] as const;
export const DRAWS = ["Flush draw", "Open-ended straight draw", "Gutshot"] as const;
export type Made = (typeof MADE)[number];
export type Draw = (typeof DRAWS)[number];

const straightTop = (mask: number): number => {
  for (let hi = 12; hi >= 4; hi--) if (((mask >> (hi - 4)) & 31) === 31) return hi;
  return (mask & 0b1111) === 0b1111 && mask & (1 << 12) ? 3 : -1; // the ace can play low
};
const maskOf = (ranks: number[]) => ranks.reduce((a, r) => a | (1 << r), 0);
/** how many ranks would complete a straight */
const straightOuts = (mask: number) => { let n = 0; for (let r = 0; r < 13; r++) if (!(mask & (1 << r)) && straightTop(mask | (1 << r)) >= 0) n++; return n; };

export function classify(hand: string, board: string[]): { made: Made; draws: Draw[] } {
  const hole = [hand.slice(0, 2), hand.slice(2, 4)];
  const all = [...hole, ...board];
  const rk = (c: string) => RANKS.indexOf(c[0]); const su = (c: string) => c[1];
  const hr = hole.map(rk); const br = board.map(rk).sort((a, b) => b - a);
  const count = new Map<number, number>(); all.forEach((c) => count.set(rk(c), (count.get(rk(c)) ?? 0) + 1));
  const suits = new Map<string, string[]>(); all.forEach((c) => suits.set(su(c), [...(suits.get(su(c)) ?? []), c]));
  const flushSuit = [...suits.entries()].find(([, v]) => v.length >= 5)?.[0];
  const allMask = maskOf(all.map(rk)); const boardMask = maskOf(br);
  const counts = [...count.values()].sort((a, b) => b - a);
  const boardCount = new Map<number, number>(); br.forEach((r) => boardCount.set(r, (boardCount.get(r) ?? 0) + 1));
  const usesHole = (r: number) => hr.includes(r);
  const pocket = hr[0] === hr[1];
  const boardDistinct = [...new Set(br)];
  let made: Made;
  if (flushSuit && straightTop(maskOf(suits.get(flushSuit)!.map(rk))) >= 0) made = "Straight flush";
  else if (counts[0] === 4 && [...count.entries()].some(([r, n]) => n === 4 && usesHole(r))) made = "Four of a kind";
  else if (counts[0] === 3 && counts[1] >= 2 && [...count.entries()].some(([r, n]) => n >= 2 && usesHole(r))) made = "Full house";
  else if (flushSuit && hole.some((c) => su(c) === flushSuit)) made = "Flush";
  else if (straightTop(allMask) >= 0 && straightTop(allMask) !== straightTop(boardMask)) made = "Straight";
  else if ([...count.entries()].some(([r, n]) => n === 3 && usesHole(r))) made = "Three of a kind";
  else {
    // pairs made with the hole cards
    const paired = hr.filter((r) => (boardCount.get(r) ?? 0) >= 1);
    const mine = new Set(paired).size + (pocket && !paired.length ? 1 : 0);
    if (mine >= 2) made = "Two pair";
    else if (pocket && !paired.length) made = hr[0] > boardDistinct[0] ? "Overpair" : hr[0] > (boardDistinct[1] ?? -1) ? "Middle pair" : "Low pair";
    else if (mine === 1) { const r = paired[0]; made = r === boardDistinct[0] ? "Top pair" : r === boardDistinct[1] ? "Middle pair" : "Low pair"; }
    else made = Math.max(...hr) === 12 ? "Ace high" : "No pair";
  }
  const draws: Draw[] = [];
  const weak = MADE.indexOf(made) >= MADE.indexOf("Three of a kind");
  if (board.length < 5 && weak) {
    if ([...suits.entries()].some(([s, v]) => v.length === 4 && hole.some((c) => su(c) === s) && board.filter((c) => su(c) === s).length <= 3 && v.length > board.filter((c) => su(c) === s).length)) draws.push("Flush draw");
    const outs = straightOuts(allMask), boardOuts = straightOuts(boardMask);
    if (outs > boardOuts) draws.push(outs >= 2 ? "Open-ended straight draw" : "Gutshot");
  }
  return { made, draws };
}

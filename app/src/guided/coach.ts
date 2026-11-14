// Plain-language "coach notes" generated from node views.
import type { NodeView } from "./api";

export const pct = (x: number) => `${Math.round(x * 100)}%`;

export const actionColors = (actions: string[]): string[] => {
  const reds = ["#fca5a5", "#f87171", "#ef4444", "#b91c1c", "#7f1d1d"];
  let k = 0;
  return actions.map((a) => {
    if (a.startsWith("Fold")) return "#93c5fd";
    if (a.startsWith("Check") || a.startsWith("Call")) return "#6ee7b7";
    return reds[Math.min(k++, reds.length - 1)];
  });
};

const verb = (a: string) =>
  a.startsWith("Fold") ? "folds" : a.startsWith("Check") ? "checks" : a.startsWith("Call") ? "calls"
    : a.startsWith("Raise") ? "raises" : a.startsWith("All-in") ? "goes all-in" : "bets";

const aggressive = (a: string) => /^(Bet|Raise|All-in)/.test(a);

export function sizeWord(amount: number, pot: number) {
  if (!pot) return "";
  const r = amount / pot;
  if (r < 0.4) return "small";
  if (r < 0.8) return "medium";
  if (r < 1.2) return "pot-sized";
  return "overbet";
}

export function describeNode(v: NodeView, name: string, fmt: (n: number) => string): string[] {
  if (v.kind !== "player") return [];
  const notes: string[] = [];
  const aggr = v.actions.map((a, i) => (aggressive(a) ? v.action_freqs[i] : 0)).reduce((s, x) => s + x, 0);
  const passive = 1 - aggr;
  const order = v.action_freqs.map((f, i) => [f, i]).sort((a, b) => b[0] - a[0]);
  const [topF, topI] = order[0];
  notes.push(`${name} ${verb(v.actions[topI])} most often here (${pct(topF)}).`);
  if (aggr > 0.05 && v.actions.filter(aggressive).length > 1) {
    const sizes = v.actions
      .map((a, i) => ({ a, i }))
      .filter((x) => aggressive(x.a))
      .sort((x, y) => v.action_freqs[y.i] - v.action_freqs[x.i]);
    const s = sizes[0];
    notes.push(
      `When ${name.toLowerCase().startsWith("you") ? "you bet" : "they bet"}, the ${sizeWord(v.action_amounts[s.i], v.pot)} size (${fmt(v.action_amounts[s.i])}) is the favourite.`
    );
  }
  const isChecking = v.actions[0] === "Check";
  if (aggr < 0.03 && isChecking) {
    notes.push(`${name} checks essentially everything here — betting gains nothing with this range on this board.`);
    return notes;
  }
  if (aggr > 0.8) notes.push(`That's an almost-always-bet spot: ${pct(aggr)} aggressive overall.`);
  else if (passive > 0.8 && v.actions.some(aggressive)) notes.push(`Mostly a passive spot: only ${pct(aggr)} aggressive.`);

  const [strong, medium, weak] = v.buckets;
  const bAggr = (b: typeof strong) => b.freqs.map((f, i) => (aggressive(v.actions[i]) ? f : 0)).reduce((s, x) => s + x, 0);
  if (strong && weak && strong.share > 0.02 && weak.share > 0.02 && v.actions.some(aggressive)) {
    const hasMedium = !!medium && medium.share > 0.02;
    notes.push(
      hasMedium
        ? `Strong hands are aggressive ${pct(bAggr(strong))} of the time, medium hands ${pct(bAggr(medium))}, weak hands ${pct(bAggr(weak))}.`
        : `Strong hands are aggressive ${pct(bAggr(strong))} of the time, weak hands ${pct(bAggr(weak))}. This range has almost no medium-strength hands on this board, which is why it has little reason to check.`
    );
    if (hasMedium && bAggr(weak) > bAggr(medium) + 0.1)
      notes.push("Notice the weak hands bet more than the medium ones: those are bluffs, while medium hands prefer to check and get to showdown.");
  }
  if (v.actions[0] === "Fold") {
    const foldI = 0;
    notes.push(`Facing this bet, ${name.toLowerCase()} folds ${pct(v.action_freqs[foldI])} of the range.`);
  }
  return notes;
}

export function compareNodes(base: NodeView, locked: NodeView, name: string, isHero: boolean, fmt: (n: number) => string): string[] {
  if (base.kind !== "player" || locked.kind !== "player") return [];
  const notes: string[] = [];
  const who = isHero ? "you" : "they";
  base.actions.forEach((a, i) => {
    const d = (locked.action_freqs[i] ?? 0) - base.action_freqs[i];
    if (Math.abs(d) >= 0.05)
      notes.push(`${isHero ? "You" : name} ${verb(a)}${aggressive(a) ? ` ${fmt(base.action_amounts[i])}` : ""} ${d > 0 ? "more" : "less"}: ${pct(base.action_freqs[i])} → ${pct(locked.action_freqs[i])}.`);
  });
  const aggrOf = (v: NodeView, b: number) =>
    v.buckets[b].freqs.map((f, i) => (aggressive(v.actions[i]) ? f : 0)).reduce((s, x) => s + x, 0);
  const names = ["strong hands", "medium hands", "weak hands (bluffs)"];
  if (base.actions.some(aggressive)) {
    [0, 1, 2].forEach((b) => {
      const d = aggrOf(locked, b) - aggrOf(base, b);
      if (Math.abs(d) >= 0.08 && base.buckets[b].share > 0.03)
        notes.push(`With ${names[b]}, ${who} ${d > 0 ? "get more aggressive" : "slow down"}: ${pct(aggrOf(base, b))} → ${pct(aggrOf(locked, b))} aggressive.`);
    });
  }
  if (!notes.length) notes.push("Almost nothing changes at this decision — the leak matters elsewhere in the hand.");
  return notes;
}

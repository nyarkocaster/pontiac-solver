// Hand histories come in several layouts. This file recognises the common ones and rewrites each
// into the PokerStars text layout, which handHistory.ts reads. Layouts covered: PokerStars and the
// sites that follow it (GGPoker, Winamax), 888poker and partypoker, Americas Cardroom and the rest
// of the Winning network, Ignition / Bovada / Bodog, iPoker (XML), and PokerNow logs.

export interface Normalised {
  /** the hand in PokerStars layout */
  text: string;
  /** which layout the paste was recognised as */
  site: string;
  notes: string[];
}

type Kind = "fold" | "check" | "call" | "bet" | "raise" | "allin";
/** `add`: the amount is what this action put in. `to`: it is the player's total on this street. */
interface Act { name: string; kind: Kind; amt: number; mode: "add" | "to" }
/** `amt` leaves the stack; `live` of it counts as a bet in front of the player */
interface Post { name: string; kind: "sb" | "bb" | "ante" | "straddle" | "dead"; amt: number; live: number }
interface Hand {
  site: string; tourney: boolean; players: { name: string; stack: number }[]; button?: string; posts: Post[];
  hero?: string; heroCards?: string[]; streets: Act[][]; board: string[]; notes: string[];
}

const amount = (s: string) => Number((s.match(/\d[\d,]*\.?\d*|\.\d+/) ?? ["0"])[0].replace(/,/g, ""));
const amounts = (s: string) => [...s.matchAll(/\d[\d,]*\.?\d*|\.\d+/g)].map((m) => Number(m[0].replace(/,/g, "")));
const show = (x: number) => String(Math.round(x * 100) / 100);

/** one card in any of the spellings sites use: "Ah", "10d", "K♦", or iPoker's suit-first "H10" */
export function readCard(raw: string): string | null {
  const s = raw.trim().replace(/10/g, "T").replace(/♠/g, "s").replace(/♥/g, "h").replace(/♦/g, "d").replace(/♣/g, "c");
  let m = s.match(/^([2-9TJQKA])([cdhs])$/i);
  if (m) return m[1].toUpperCase() + m[2].toLowerCase();
  m = s.match(/^([CDHS])([2-9TJQKA])$/);
  if (m) return m[2] + m[1].toLowerCase();
  return null;
}
const readCards = (s: string) => s.split(/[\s,]+/).map(readCard).filter((c): c is string => !!c);

function who(h: Hand, raw: string): string | undefined {
  const t = raw.trim();
  const exact = h.players.find((p) => p.name === t);
  if (exact) return exact.name;
  return h.players.filter((p) => t.startsWith(p.name)).sort((a, b) => b.name.length - a.name.length)[0]?.name;
}

/** Writes the hand out in PokerStars layout, working out raise totals and all-ins from the stacks. */
function emit(h: Hand): Normalised {
  const notes = [...h.notes];
  const out: string[] = [];
  const sb = h.posts.find((p) => p.kind === "sb")?.amt ?? 0, bb = h.posts.find((p) => p.kind === "bb")?.amt ?? 0;
  out.push(`${h.site} Hand #1: ${h.tourney ? "Tournament #1, " : ""}Hold'em No Limit (${show(sb)}/${show(bb)})`);
  const b = h.players.findIndex((p) => p.name === h.button);
  out.push(`Table 'Pasted'${b >= 0 ? ` Seat #${b + 1} is the button` : ""}`);
  h.players.forEach((p, i) => out.push(`Seat ${i + 1}: ${p.name} (${show(p.stack)} in chips)`));
  const left = new Map(h.players.map((p) => [p.name, p.stack])), put = new Map<string, number>();
  const pay = (name: string, amt: number, live: number) => { left.set(name, (left.get(name) ?? 0) - amt); put.set(name, (put.get(name) ?? 0) + live); };
  const straddles: Post[] = [];
  for (const p of h.posts) {
    if (p.kind === "straddle") { straddles.push(p); continue; }
    pay(p.name, p.amt, p.live);
    if (p.kind === "ante") out.push(`${p.name}: posts the ante ${show(p.amt)}`);
    else if (p.kind === "sb") out.push(`${p.name}: posts small blind ${show(p.amt)}`);
    else if (p.kind === "bb") out.push(`${p.name}: posts big blind ${show(p.amt)}`);
  }
  out.push("*** HOLE CARDS ***");
  if (h.hero && h.heroCards?.length === 2) out.push(`Dealt to ${h.hero} [${h.heroCards.join(" ")}]`);
  const allIn = (name: string) => ((left.get(name) ?? 0) <= 0.005 ? " and is all-in" : "");
  for (const p of straddles) {
    const max = Math.max(0, ...put.values());
    pay(p.name, p.amt, p.live);
    out.push(`${p.name}: raises ${show(p.amt - max)} to ${show(p.amt)}`);
    notes.push("This hand had a straddle, which the setup does not have; it is read as a raise to that amount.");
  }
  const b3 = h.board;
  for (let si = 0; si < 4; si++) {
    if (si === 1) { if (b3.length < 3) break; out.push(`*** FLOP *** [${b3.slice(0, 3).join(" ")}]`); }
    if (si === 2) { if (b3.length < 4) break; out.push(`*** TURN *** [${b3.slice(0, 3).join(" ")}] [${b3[3]}]`); }
    if (si === 3) { if (b3.length < 5) break; out.push(`*** RIVER *** [${b3.slice(0, 4).join(" ")}] [${b3[4]}]`); }
    if (si > 0) put.clear();
    for (const a of h.streets[si] ?? []) {
      const cur = put.get(a.name) ?? 0, max = Math.max(0, ...put.values());
      if (a.kind === "fold") { out.push(`${a.name}: folds`); continue; }
      if (a.kind === "check") { out.push(`${a.name}: checks`); continue; }
      const to = a.mode === "to" ? a.amt : cur + a.amt;
      let kind = a.kind;
      if (kind === "allin") kind = to <= max + 1e-9 ? "call" : "raise";
      if (kind === "call") {
        const add = Math.max(0, to - cur);
        pay(a.name, add, add);
        out.push(`${a.name}: calls ${show(add)}${allIn(a.name)}`);
        continue;
      }
      pay(a.name, to - cur, to - cur);
      if (max <= 1e-9) out.push(`${a.name}: bets ${show(to)}${allIn(a.name)}`);
      else out.push(`${a.name}: raises ${show(to - max)} to ${show(to)}${allIn(a.name)}`);
    }
  }
  out.push("*** SUMMARY ***");
  if ([...left.values()].some((x) => x < -0.011)) notes.push("The amounts in this hand add up to more than a player had, so a bet size may have been read wrongly. Check the sizes below against the hand.");
  return { text: out.join("\n"), site: h.site, notes };
}

const blank = (site: string): Hand => ({ site, tourney: false, players: [], posts: [], streets: [[], [], [], []], board: [], notes: [] });
const VERB: Record<string, Kind> = { fold: "fold", folds: "fold", check: "check", checks: "check", call: "call", calls: "call", bet: "bet", bets: "bet", raise: "raise", raises: "raise" };

/** 888poker (and its skins) and partypoker: amounts in square brackets, each one what the action added */
function partyLike(lines: string[]): Hand {
  const title = lines.find((l) => /Hand History for Game/i.test(l)) ?? "";
  const brand = (title.match(/\*+\s*(.*?)\s*Hand History/i) ?? [])[1] ?? "";
  // 888 files start with a "#Game No" line and name the skin in the title; partypoker's title has no name
  const h = blank(/888|pacific|lucky|cassava/i.test(brand) || lines.some((l) => /^#Game No/i.test(l)) ? "888poker" : "partypoker");
  h.tourney = /Trny|Tournament|Buy-?in|Sit\s*&?\s*Go/i.test(lines.slice(0, 5).join(" "));
  const seats: { no: number; name: string; stack: number }[] = [];
  let street = -1, bb = 0;
  for (const l of lines) {
    let m = l.match(/^Seat\s+(\d+):\s+(.+?)\s+\(\s*([^)]*?)\s*\)\s*$/);
    if (m && street < 0) { seats.push({ no: Number(m[1]), name: m[2], stack: amount(m[3]) }); h.players = [...seats].sort((a, b) => a.no - b.no); continue; }
    m = l.match(/^Seat\s+(\d+) is the button/i);
    if (m) { h.button = `#${m[1]}`; continue; }
    if (/^\*\*\s*Dealing down cards/i.test(l)) { street = 0; continue; }
    m = l.match(/^\*\*\s*Dealing (flop|turn|river)\s*\*\*\s*(?::\s*)?\[([^\]]*)\]/i);
    if (m) { street = { flop: 1, turn: 2, river: 3 }[m[1].toLowerCase() as "flop"]; h.board.push(...readCards(m[2])); continue; }
    if (/^\*\*\s*Summary/i.test(l)) break;
    m = l.match(/^Dealt to (.+?)\s*\[([^\]]*)\]/i);
    if (m) { const c = readCards(m[2]); if (c.length === 2) { h.hero = who(h, m[1]); h.heroCards = c; } continue; }
    m = l.match(/^(.+?) posts (small blind|big blind \+ dead|big blind|dead blind|ante)\s*\[([^\]]*)\]/i);
    if (m) {
      const name = who(h, m[1]); if (!name) continue;
      const kind = m[2].toLowerCase(), a = amounts(m[3]), total = a.reduce((x, y) => x + y, 0);
      if (kind === "small blind" && !h.posts.some((p) => p.kind === "sb")) h.posts.push({ name, kind: "sb", amt: total, live: total });
      else if (kind === "big blind" && !bb) { bb = total; h.posts.push({ name, kind: "bb", amt: total, live: total }); }
      else if (kind === "ante") h.posts.push({ name, kind: "ante", amt: total, live: 0 });
      else h.posts.push({ name, kind: "dead", amt: total, live: Math.min(total, bb || total) });
      continue;
    }
    if (street < 0) continue;
    m = l.match(/^(.+?) (folds|checks|calls|bets|raises|is all-?in)\b\s*(?:\[([^\]]*)\])?/i);
    if (!m) continue;
    const name = who(h, m[1]); if (!name) continue;
    const v = m[2].toLowerCase();
    h.streets[street].push({ name, kind: VERB[v] ?? "allin", amt: amount(m[3] ?? ""), mode: "add" });
  }
  if (h.button) { const no = Number(h.button.slice(1)); h.button = seats.find((s) => s.no === no)?.name; }
  return h;
}

/** Americas Cardroom, Black Chip Poker and the rest of the Winning network */
function winning(lines: string[]): Hand {
  const h = blank("Americas Cardroom");
  const head = lines.find((l) => /^Game ID:/i.test(l)) ?? "";
  const seats: { no: number; name: string; stack: number }[] = [];
  const got = new Map<string, string[]>();
  let street = 0, btn = 0, anyAnte = false;
  for (const l of lines) {
    if (/^-+\s*Summary/i.test(l)) break;
    let m = l.match(/^Seat\s+(\d+):\s+(.+?)\s+\(([\d.,]+)\)/);
    if (m) { seats.push({ no: Number(m[1]), name: m[2], stack: amount(m[3]) }); h.players = [...seats].sort((a, b) => a.no - b.no); continue; }
    m = l.match(/^Seat\s+(\d+) is the button/i);
    if (m) { btn = Number(m[1]); continue; }
    m = l.match(/^\*+\s*(FLOP|TURN|RIVER)\s*\*+:?\s*(.*)$/i);
    if (m) {
      street = { flop: 1, turn: 2, river: 3 }[m[1].toLowerCase() as "flop"];
      const groups = [...m[2].matchAll(/\[([^\]]*)\]/g)];
      h.board.push(...readCards(groups[groups.length - 1]?.[1] ?? ""));
      continue;
    }
    m = l.match(/^Player (.+?) received card: \[([^\]]*)\]/i);
    if (m) { const name = who(h, m[1]); if (name) got.set(name, [...(got.get(name) ?? []), ...readCards(m[2])]); continue; }
    m = l.match(/^Player (.+?) (has small blind|has big blind|straddle|posts ante|ante|posts) \(([\d.,]+)\)/i);
    if (m) {
      const name = who(h, m[1]); if (!name) continue;
      const k = m[2].toLowerCase(), a = amount(m[3]);
      if (k === "has small blind") h.posts.push({ name, kind: "sb", amt: a, live: a });
      else if (k === "has big blind") h.posts.push({ name, kind: "bb", amt: a, live: a });
      else if (k === "straddle") h.posts.push({ name, kind: "straddle", amt: a, live: a });
      else if (/ante/.test(k)) { anyAnte = true; h.posts.push({ name, kind: "ante", amt: a, live: 0 }); }
      else h.posts.push({ name, kind: "dead", amt: a, live: a });
      continue;
    }
    m = l.match(/^Player (.+?) (folds|checks|calls|bets|raises|allin|all-in)\b\s*(?:\(([\d.,]+)\))?/i);
    if (!m) continue;
    const name = who(h, m[1]); if (!name) continue;
    h.streets[street].push({ name, kind: VERB[m[2].toLowerCase()] ?? "allin", amt: amount(m[3] ?? ""), mode: "add" });
  }
  h.button = seats.find((s) => s.no === btn)?.name;
  for (const [name, c] of got) if (c.length === 2) { h.hero = name; h.heroCards = c; }
  h.tourney = anyAnte || /tournament|tourney|GTD|freeroll|satellite|sit\s*&?\s*go|SNG|\bTable\s+\d+/i.test(head);
  h.notes.push(`This layout does not say whether the hand is from a cash game or a tournament; it is read as a ${h.tourney ? "tournament" : "cash game"}. Change the game in step 1 if that is wrong.`);
  return h;
}

/** Ignition, Bovada and Bodog: players are named by their seat ("UTG+1", "Dealer"), you are marked [ME] */
function ignition(lines: string[]): Hand {
  const h = blank((lines[0].match(/^(Ignition|Bovada|Bodog)/i) ?? ["Ignition"])[0]);
  h.tourney = /tournament/i.test(lines[0]);
  const rank = (n: string) => { const m = n.match(/^UTG(?:\+(\d+))?$/i); return m ? Number(m[1] ?? 0) : /^dealer$/i.test(n) ? 100 : /^small blind$/i.test(n) ? 101 : /^big blind$/i.test(n) ? 102 : 50; };
  let street = 0, bb = 0;
  for (const l of lines.slice(1)) {
    if (/^\*+\s*SUMMARY/i.test(l)) break;
    let m = l.match(/^Seat\s+\d+:\s+(.+?)(\s+\[ME\])?\s+\(\$?([\d,.]+) in chips\)/i);
    if (m) { h.players.push({ name: m[1].trim(), stack: amount(m[3]) }); if (m[2]) h.hero = m[1].trim(); continue; }
    m = l.match(/^\*+\s*(FLOP|TURN|RIVER)\s*\*+\s*(.*)$/i);
    if (m) {
      street = { flop: 1, turn: 2, river: 3 }[m[1].toLowerCase() as "flop"];
      const groups = [...m[2].matchAll(/\[([^\]]*)\]/g)];
      h.board.push(...readCards(groups[groups.length - 1]?.[1] ?? ""));
      continue;
    }
    m = l.match(/^(.+?)\s*(\[ME\])?\s*:\s*(.+)$/);
    if (!m) continue;
    const name = who(h, m[1]); if (!name) continue;
    const body = m[3].trim(), a = amounts(body.replace(/\[[^\]]*\]/g, "")), last = a[a.length - 1] ?? 0;
    if (/^Card dealt to a spot/i.test(body)) { if (m[2] || name === h.hero) { const c = readCards((body.match(/\[([^\]]*)\]/) ?? [])[1] ?? ""); if (c.length === 2) { h.hero = name; h.heroCards = c; } } }
    else if (/^Small blind/i.test(body)) { if (!h.posts.some((p) => p.kind === "sb")) h.posts.push({ name, kind: "sb", amt: last, live: last }); }
    else if (/^Big blind/i.test(body)) { if (!bb) { bb = last; h.posts.push({ name, kind: "bb", amt: last, live: last }); } else h.posts.push({ name, kind: "dead", amt: last, live: last }); }
    else if (/^Ante/i.test(body)) h.posts.push({ name, kind: "ante", amt: last, live: 0 });
    else if (/^Posts?\b.*chip/i.test(body)) h.posts.push({ name, kind: "dead", amt: last, live: Math.min(last, bb || last) });
    else if (/^Folds?\b/i.test(body)) h.streets[street].push({ name, kind: "fold", amt: 0, mode: "add" });
    else if (/^Checks?\b/i.test(body)) h.streets[street].push({ name, kind: "check", amt: 0, mode: "add" });
    else if (/^Calls?\b/i.test(body)) h.streets[street].push({ name, kind: "call", amt: last, mode: "add" });
    else if (/^Bets?\b/i.test(body)) h.streets[street].push({ name, kind: "bet", amt: last, mode: "to" });
    else if (/^Raises?\b/i.test(body)) h.streets[street].push({ name, kind: "raise", amt: last, mode: "to" });
    else if (/^All-?in\s*\(\s*raise\s*\)/i.test(body)) h.streets[street].push({ name, kind: "raise", amt: last, mode: "to" });
    else if (/^All-?in\b/i.test(body)) h.streets[street].push({ name, kind: "allin", amt: last, mode: "add" });
  }
  h.players.sort((x, y) => rank(x.name) - rank(y.name));
  h.button = h.players.find((p) => /^dealer$/i.test(p.name))?.name;
  return h;
}

/** iPoker network sites (bet365, Betfair, William Hill and others) save hands as XML */
function ipoker(text: string): Hand {
  const h = blank("iPoker");
  const game = (text.match(/<game\b[\s\S]*?<\/game>/i) ?? [])[0];
  if (!game) throw new Error("This looks like an iPoker file, but there is no complete hand in it.");
  const attr = (tag: string, k: string) => (tag.match(new RegExp(`\\b${k}="([^"]*)"`)) ?? [])[1] ?? "";
  h.tourney = /<tournament(name|code)>\s*[^<\s]/i.test(text) || /tournament|tourney/i.test((text.match(/<gametype>([^<]*)/i) ?? [])[1] ?? "");
  const seats = [...game.matchAll(/<player\b([^>]*)\/?>/gi)].map((m) => ({ no: Number(attr(m[1], "seat")), name: attr(m[1], "name"), stack: amount(attr(m[1], "chips")), dealer: attr(m[1], "dealer") === "1" }));
  seats.sort((a, b) => a.no - b.no);
  h.players = seats.map((s) => ({ name: s.name, stack: s.stack }));
  h.button = seats.find((s) => s.dealer)?.name;
  const nick = ((text.match(/<nickname>([^<]*)/i) ?? [])[1] ?? "").trim();
  const pockets = new Map<string, string[]>();
  let bb = 0;
  for (const r of game.matchAll(/<round\b[^>]*\bno="(\d+)"[^>]*>([\s\S]*?)<\/round>/gi)) {
    const no = Number(r[1]);
    for (const c of r[2].matchAll(/<cards\b([^>]*)>([^<]*)<\/cards>/gi)) {
      const type = attr(c[1], "type").toLowerCase(), cs = readCards(c[2]);
      if (type === "pocket") { if (cs.length === 2) pockets.set(attr(c[1], "player"), cs); }
      else if (type === "flop" || type === "turn" || type === "river") h.board.push(...cs);
    }
    const acts = [...r[2].matchAll(/<action\b([^>]*)\/?>/gi)].map((m) => ({ no: Number(attr(m[1], "no")), name: attr(m[1], "player"), type: Number(attr(m[1], "type")), sum: amount(attr(m[1], "sum")) }));
    acts.sort((a, b) => a.no - b.no);
    for (const a of acts) {
      if (!h.players.some((p) => p.name === a.name)) continue;
      if (no === 0) {
        if (a.type === 1 && !h.posts.some((p) => p.kind === "sb")) h.posts.push({ name: a.name, kind: "sb", amt: a.sum, live: a.sum });
        else if (a.type === 2 && !bb) { bb = a.sum; h.posts.push({ name: a.name, kind: "bb", amt: a.sum, live: a.sum }); }
        else if (a.type === 15) h.posts.push({ name: a.name, kind: "ante", amt: a.sum, live: 0 });
        else h.posts.push({ name: a.name, kind: "dead", amt: a.sum, live: Math.min(a.sum, bb || a.sum) });
        continue;
      }
      const s = h.streets[Math.min(no, 4) - 1];
      if (a.type === 0) s.push({ name: a.name, kind: "fold", amt: 0, mode: "add" });
      else if (a.type === 4) s.push({ name: a.name, kind: "check", amt: 0, mode: "add" });
      else if (a.type === 3) s.push({ name: a.name, kind: "call", amt: a.sum, mode: "add" });
      else if (a.type === 5) s.push({ name: a.name, kind: "bet", amt: a.sum, mode: "to" });
      else if (a.type === 23 || a.type === 6) s.push({ name: a.name, kind: "raise", amt: a.sum, mode: "to" });
      else if (a.type === 7) s.push({ name: a.name, kind: "allin", amt: a.sum, mode: "add" });
    }
  }
  if (nick && pockets.has(nick)) { h.hero = nick; h.heroCards = pockets.get(nick); }
  else if (pockets.size === 1) { const [n, c] = [...pockets][0]; h.hero = n; h.heroCards = c; }
  return h;
}

/** PokerNow game logs: the downloaded CSV (newest line first) or the same lines copied from the log window */
function pokerNowRows(text: string): string[] {
  let rows = text.replace(/\r/g, "").split("\n").map((l) => l.trim()).filter(Boolean).map((l) => {
    const m = l.match(/^"((?:[^"]|"")*)"\s*,[^"]*$/); // a line of the downloaded file: "entry",time,order
    return (m ? m[1].replace(/""/g, '"') : l).trim();
  });
  // the downloaded file runs newest first: there the stacks line sits just above its "starting hand" line
  const firstStart = rows.findIndex((r) => /starting hand #/.test(r));
  const n = (rows[firstStart]?.match(/#(\d+)/) ?? [])[1] ?? "";
  const endAt = rows.findIndex((r) => new RegExp(`ending hand #${n}\\b`).test(r));
  if ((endAt >= 0 && endAt < firstStart) || (firstStart > 0 && /^Player stacks:/i.test(rows[firstStart - 1]))) rows = rows.reverse();
  return rows;
}
function pokerNow(text: string): Hand {
  const rows = pokerNowRows(text);
  const hands: string[][] = [];
  for (const r of rows) { if (/starting hand #/.test(r)) hands.push([]); if (hands.length) hands[hands.length - 1].push(r); }
  const flop = (hd: string[]) => hd.some((r) => /^flop\b/i.test(r));
  const pick = hands.find((hd) => flop(hd) && hd.some((r) => /^Your hand is/i.test(r))) ?? hands.find(flop) ?? hands[0];
  if (!pick) throw new Error("This PokerNow log has no hand in it (no \"starting hand\" line).");
  const h = blank("PokerNow");
  if (hands.length > 1) h.notes.push(`The log has ${hands.length} hands; this is hand #${(pick[0].match(/#(\d+)/) ?? [])[1] ?? "?"}, the first that reached a flop${pick.some((r) => /^Your hand is/i.test(r)) ? " with your cards shown" : ""}. To study another, paste only that hand's lines.`);
  const nm = (raw: string) => raw.replace(/^"|"$/g, "").replace(/\s*@\s*\S+$/, "").trim();
  h.button = nm((pick[0].match(/dealer:\s*"([^"]*)"/) ?? [])[1] ?? "");
  let street = 0, mine: string[] = [], bb = 0, told = "";
  for (const r of pick) {
    let m = r.match(/^Player stacks:\s*(.*)$/i);
    if (m) {
      const list = [...m[1].matchAll(/(?:#(\d+)\s*)?"([^"]*)"\s*\(([\d.,]+)\)/g)].map((x, i) => ({ no: x[1] ? Number(x[1]) : i, name: nm(x[2]), stack: amount(x[3]) }));
      list.sort((a, b) => a.no - b.no);
      h.players = list.map((x) => ({ name: x.name, stack: x.stack }));
      continue;
    }
    m = r.match(/^Your hand is (.*)$/i);
    if (m) { mine = readCards(m[1].replace(/\.$/, "")); continue; }
    m = r.match(/^You are "([^"]*)"$/); // added by splitHands when another hand of the log gave the player away
    if (m) { told = nm(m[1]); continue; }
    if (/second run|\(second\)/i.test(r)) continue;
    m = r.match(/^(flop|turn|river)\b[^:]*:\s*(.*)$/i);
    if (m) {
      street = { flop: 1, turn: 2, river: 3 }[m[1].toLowerCase() as "flop"];
      const inside = (m[2].match(/\[([^\]]*)\]/) ?? [])[1] ?? "";
      const cs = readCards(inside);
      if (h.board.length < (street === 1 ? 3 : street + 2)) h.board.push(...cs);
      continue;
    }
    m = r.match(/^"([^"]*)"\s+(.*)$/);
    if (!m) continue;
    const name = who(h, nm(m[1])); if (!name) continue;
    const body = m[2], a = amount(body);
    if (/^posts a(?: missing| missed)? small blind/i.test(body)) { if (!h.posts.some((p) => p.kind === "sb")) h.posts.push({ name, kind: "sb", amt: a, live: a }); else h.posts.push({ name, kind: "dead", amt: a, live: 0 }); }
    else if (/^posts a(?: missing| missed)? big blind/i.test(body)) { if (!bb) { bb = a; h.posts.push({ name, kind: "bb", amt: a, live: a }); } else h.posts.push({ name, kind: "dead", amt: a, live: a }); }
    else if (/^posts an ante/i.test(body)) h.posts.push({ name, kind: "ante", amt: a, live: 0 });
    else if (/^posts a straddle/i.test(body)) h.posts.push({ name, kind: "straddle", amt: a, live: a });
    else if (/^folds/i.test(body)) h.streets[street].push({ name, kind: "fold", amt: 0, mode: "to" });
    else if (/^checks/i.test(body)) h.streets[street].push({ name, kind: "check", amt: 0, mode: "to" });
    else if (/^calls/i.test(body)) h.streets[street].push({ name, kind: "call", amt: a, mode: "to" });
    else if (/^bets/i.test(body)) h.streets[street].push({ name, kind: "bet", amt: a, mode: "to" });
    else if (/^raises/i.test(body)) h.streets[street].push({ name, kind: "raise", amt: a, mode: "to" });
    else if (mine.length === 2 && /^shows a /i.test(body) && readCards(body.replace(/^shows a /i, "").replace(/\.$/, "")).sort().join() === [...mine].sort().join()) { h.hero = name; h.heroCards = mine; }
  }
  if (mine.length === 2 && !h.hero && told && who(h, told)) { h.hero = who(h, told); h.heroCards = mine; }
  if (mine.length === 2 && !h.hero) h.notes.push(`The log shows your cards (${mine.join(" ")}) but not which player you are, so the big blind is treated as you and your hand is not tracked. Pick your seat in step 2.`);
  return h;
}

/** PokerStars and the sites that follow its layout; only small differences need smoothing over */
function starsLike(lines: string[]): Normalised {
  const first = lines[0] ?? "";
  const site = /winamax/i.test(first) ? "Winamax" : /^Poker Hand #|GGPoker|GG ?Poker/i.test(first) ? "GGPoker" : /pokerstars/i.test(first) ? "PokerStars" : "PokerStars layout";
  const out: string[] = [];
  let skip = false;
  for (const l of lines) {
    if (/^\*+\s*(SECOND|THIRD)\s+(FLOP|TURN|RIVER)/i.test(l)) { skip = true; continue; }
    if (/^\*+\s*(SHOW ?DOWN|SUMMARY|FIRST)/i.test(l)) skip = false;
    if (skip) continue;
    out.push(l.replace(/^\*+\s*PRE-?FLOP\s*\*+.*$/i, "*** HOLE CARDS ***").replace(/^(\*+\s*)FIRST\s+(FLOP|TURN|RIVER)/i, "$1$2"));
  }
  return { text: out.join("\n"), site, notes: [] };
}

/** keep only the first hand when several were pasted together */
function firstHand(lines: string[], isStart: (l: string) => boolean): { lines: string[]; more: boolean } {
  const a = lines.findIndex(isStart);
  const b = lines.findIndex((l, i) => i > a && isStart(l));
  return a >= 0 && b > a ? { lines: lines.slice(a, b), more: true } : { lines: a > 0 ? lines.slice(a) : lines, more: false };
}

const START = {
  ignition: (l: string) => /^(Ignition|Bovada|Bodog) Hand #/i.test(l),
  party: (l: string) => /Hand History for Game/i.test(l),
  winning: (l: string) => /^Game started at:/i.test(l),
  stars: (l: string) => /^(PokerStars|Poker|Winamax|GGPoker|Full Tilt)\b.*(Hand|HandId:?|Game)\s*#/i.test(l),
};

/** Cuts a paste or a file holding many hands into one text per hand, each readable by `toStarsLayout`. */
export function splitHands(raw: string): string[] {
  const text = raw.replace(/^\uFEFF/, "").replace(/\r/g, "");
  if (/<game\b/i.test(text) && /<round\b/i.test(text)) {
    const head = text.slice(0, text.search(/<game\b/i));
    return (text.match(/<game\b[\s\S]*?<\/game>/gi) ?? []).map((g) => `${head}${g}\n</session>`);
  }
  let lines = text.split("\n").map((l) => l.trim()).filter(Boolean);
  let isStart: (l: string) => boolean;
  if (lines.some(START.ignition)) isStart = START.ignition;
  else if (lines.some(START.party)) {
    // 888 puts a "#Game No" line above each title; keep it with its hand
    const titled = lines.some((l) => /^#Game No/i.test(l));
    isStart = titled ? (l) => /^#Game No/i.test(l) : START.party;
  } else if (lines.some(START.winning) && lines.some((l) => /^Game ID:/i.test(l))) isStart = START.winning;
  else if (lines.some((l) => /starting hand #\d+/i.test(l))) { lines = pokerNowRows(text); isStart = (l) => /starting hand #\d+/i.test(l); }
  else isStart = START.stars;
  const out: string[][] = [];
  for (const l of lines) { if (isStart(l)) out.push([]); if (out.length) out[out.length - 1].push(l); }
  if (out.length > 1 && out[0].some((l) => /starting hand #\d+/i.test(l))) {
    // A PokerNow log never names "you", but a hand where you showed your cards gives it away for the whole log.
    let you = "";
    for (const h of out) {
      const mine = readCards((h.find((l) => /^Your hand is /i.test(l)) ?? "").replace(/^Your hand is /i, "").replace(/\.$/, "")).sort().join();
      if (!mine) continue;
      for (const l of h) { const m = l.match(/^"([^"]*)"\s+shows a (.*?)\.?$/i); if (m && readCards(m[2]).sort().join() === mine) you = m[1]; }
      if (you) break;
    }
    if (you) for (const h of out) h.splice(1, 0, `You are "${you}"`);
  }
  return out.length ? out.map((h) => h.join("\n")) : [text];
}

export function toStarsLayout(raw: string): Normalised {
  const text = raw.replace(/^﻿/, "").replace(/\r/g, "");
  const lines = text.split("\n").map((l) => l.trim()).filter(Boolean);
  if (!lines.length) throw new Error("Paste a hand history first.");
  const top = text.slice(0, 700);
  if (/omaha|\bPLO\d*\b|stud\b|razz|draw\b|short ?deck|6\+ hold/i.test(top) && !/<gametype>\s*Holdem/i.test(top) && !/Hold'?em No Limit|No Limit Hold'?em|NL Texas Hold'?em|Holdem no limit|\(Hold'em\)|HOLDEM/i.test(top))
    throw new Error("This hand is not no-limit hold'em. Pontiac studies no-limit hold'em only.");
  let res: Normalised, more = false;
  if (/<game\b/i.test(text) && /<round\b/i.test(text)) {
    more = (text.match(/<game\b/gi) ?? []).length > 1;
    res = emit(ipoker(text));
  } else if (lines.some((l) => /^(Ignition|Bovada|Bodog) Hand #/i.test(l))) {
    const f = firstHand(lines, (l) => /^(Ignition|Bovada|Bodog) Hand #/i.test(l)); more = f.more;
    res = emit(ignition(f.lines));
  } else if (lines.some((l) => /Hand History for Game/i.test(l))) {
    const f = firstHand(lines, (l) => /Hand History for Game/i.test(l)); more = f.more;
    res = emit(partyLike(f.lines));
  } else if (lines.some((l) => /^Game started at:/i.test(l)) && lines.some((l) => /^Game ID:/i.test(l))) {
    const f = firstHand(lines, (l) => /^Game started at:/i.test(l)); more = f.more;
    res = emit(winning(f.lines));
  } else if (lines.some((l) => /starting hand #\d+/i.test(l))) {
    res = emit(pokerNow(text));
  } else {
    const f = firstHand(lines, (l) => /^(PokerStars|Poker|Winamax|GGPoker|Full Tilt)\b.*(Hand|HandId:?|Game)\s*#/i.test(l)); more = f.more;
    res = starsLike(f.lines);
  }
  if (more) res.notes.push("Several hands were pasted; only the first is read.");
  return res;
}

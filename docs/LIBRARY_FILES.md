# Library files (`.plib`)

A whole flop solve is too big to share: about 1 GB for a heads-up 20 bb flop with two bet sizes. A library file keeps the part that cannot be worked out again quickly, the flop, and leaves the rest to the reader's machine:

- **Stored:** both players' flop strategy at every flop decision, what each action is worth to each hand there, and a line per turn card saying what the whole solve made of that card.
- **Solved again when asked for:** the turn and river. Picking a turn card solves that turn (rivers included) from the ranges the stored flop strategy leaves each player.

The code is the `library/` crate (`plib`). The command line, the Python package and the desktop app all read and write files through it.

## What it costs and what you get

Measured on `examples/hu_sng_flop_20bb.json` (Qh8c3h, 18 bb behind, 4 bb pot, two sizes per street), solved to 0.27% of the pot on 2 cores:

| | Whole solve (`--save`) | Library file (`--library`) |
|---|---|---|
| Size on disk | 1,048 MB | 44 KB |
| Open and read a flop decision | 4.7 s | 0.15 s |
| Read a turn decision | 4.7 s | the turn is solved first: see below |

Flop answers are the whole solve's own, rounded for storage: over all 16 flop decisions, range frequencies agree within 0.0005, each hand's value within 0.0002 bb on average.

A turn solved again is not the whole solve's turn copied out; it is a fresh solve of the same turn from the same ranges. Two solves of one spot never agree exactly, so the fair question is whether the gap is larger than the gap between two honest solves. On this spot it is not: see the project notes for 2026-11-23, and the summary in `docs/RELEASE_NOTES.md`.

## Limits

- Two players, a spot that starts on the flop. Turn and river spots are small enough to keep whole.
- A normal solve only: no node locks or habits (a lock on the turn or river would be lost).
- Not with prize money (ICM) yet, and not with separate turn lead ("donk") sizes: a turn solved on its own would not offer them.
- A line the flop solve never takes has no range left to solve a turn with; asking for a turn there gives an error that says so.
- Rarely taken lines are kept, with their ranges, but a whole solve spends little effort there and a re-solved turn can differ from it a lot. This is true of any solver's rare lines.

## Layout

All numbers little-endian.

```
8 bytes   "PNTCLIB" then the byte 01
u32       length of the header
header    JSON, UTF-8, not compressed
u32       length of the body before compression
body      zstd
```

### Header

Readable without unpacking the body (`pontiac info file.plib`), so a folder of files can be indexed quickly.

| Field | Meaning |
|---|---|
| `format` | 1. A reader refuses a larger number. |
| `board` | the flop |
| `tree` | the bet tree in full: `starting_pot`, `effective_stack` (chips), `rake_rate`, `rake_cap`, `flop` / `turn` / `river` (per player: `bet` and `raise` sizes), `river_donk`, and the three thresholds. Sizes are `{"pot": 0.33}`, `{"prev": 2.5}`, `{"add": [chips, cap]}`, `{"geo": [streets, max or null]}` or `"allin"`. This is what a turn is rebuilt from. |
| `strategy_bits` | 8 or 16: how finely action frequencies are stored |
| `flop_nodes`, `hands` | sizes, for information |
| `made_by`, `created` | who wrote it and when (Unix seconds) |
| `names`, `chips_per_bb`, `spot`, `solve` | headless files: player names [first to act, last to act], chips per big blind (100), the spot file as solved, and how the solve went |
| `name`, `app`, `summary` | files saved from the desktop app: the entry's name, the screen's own description of the spot, and the solve summary |

### Body

```
u8        strategy bits (8 or 16)
3 bytes   flop cards (rank * 4 + suit; suits c d h s)
u16 u16   number of hands of each player (first to act, then last to act)
          each player's hands, two card bytes per hand
          each player's starting weights, one f32 per hand
node      the first flop decision, followed by its children, depth first
```

A node:

```
u8        0 decision, 1 a card is dealt, 2 the hand is over (a fold, or an all-in and a call)
i32 i32   chips each player has put in since the start of the spot
```

then, for a decision:

```
u8        who decides (0 first to act, 1 last to act)
u8        number of actions
          per action: u8 kind (1 fold, 2 check, 3 call, 4 bet, 5 raise, 6 all-in), i32 amount
          [first decision only] u8 player, then VALUES for that player: the hands of the player who is not deciding
          per action: f32 scale, then one step per hand in range (u8, or u16 as two planes: high bytes, then low bytes)
          per action that is not a fold: VALUES, what the action is worth to each hand in range
          one child node per action
```

for a card being dealt:

```
u8        player, then VALUES for that player: the one who did not act last
u8        number of cards that can come
          per card: u8 card, f32 x 5: value of each player's range, each player's chance to win, how often the first player bets
```

`VALUES` is `f32 lowest, f32 highest`, then one u16 per hand in range (high bytes, then low bytes), each a step between lowest and highest.

Three rules make the rest work:

1. **Hands in range.** Per-hand lists only cover hands that still have weight at that node. Weight is the starting weight times the stored frequency of each action the player took on the way. The reader and the writer both work it out from the stored (rounded) frequencies, so they always agree on which hands are listed.
2. **Frequencies.** A stored step means `scale * step / 255` (or `/ 65535`); `scale` is the action's largest frequency over the hands in range. After reading, each hand's frequencies are scaled to add up to 1. Measuring each action against its own largest value keeps the shape of rarely taken actions, so the range that reaches a rare line stays usable.
3. **Values.** A value is what the hand collects from the final pot on average, in chips, the same as in a whole solve. A player's value at a node is read from the parent decision when that player made it (the stored worth of the action taken), and from the node's own `VALUES` otherwise. Chance to win is not stored: it is worked out from the ranges when asked for.

## Solving a turn again

At a node where a card is dealt, for the chosen card:

- ranges: each player's hands in range with their weights, minus hands holding the card;
- pot: the starting pot plus what both players put in on the flop; stack: the starting stack minus one player's flop chips;
- bet tree: the header's `turn` and `river` sizes, rake and thresholds.

That is an ordinary two-player turn spot. It is solved to 0.1% of its pot by default (60 seconds at most). The result is compared with the stored line for that card (`turn_solve` in a query answer; the note under the action list in the app).

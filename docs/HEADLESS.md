# Pontiac Solver: headless interface

For running the solver with nobody at the keyboard: a JSON spot file goes in, a JSON result comes out. Written for the HU SNG coaching loop, section by section against its request.

## 1. Access

- **Environment:** Linux x86-64 container with Python 3.9+. No GPU, no downloads beyond pip.
- **Install and smoke-test:**

  ```
  pip install -e ".[test]" && pytest
  ```

  Fourteen tests, about 15 seconds. They solve a river spot, query the saved solve, exercise full locks, partial locks, habits and error messages, solve three-player spots, check push/fold ranges, and compare the two engines on the same spots.
- **What gets installed:** a small Python package, `pontiac`, plus a prebuilt solver program for Linux x86-64 at `pontiac/bin/pontiac-linux-x86_64` (3 MB, no dependencies beyond glibc). If that program will not run on a machine, the package falls back to building it with `cargo build --release` in `cli/` (needs Rust; crates come from crates.io). `PONTIAC_BIN` overrides the choice.
- **Where the code lives:** `github.com/nyarkocaster/pontiac-solver` (public). Saved solves (`*.pontiac`) are large and are git-ignored.

## 2. Headless interface

### Python

```python
import pontiac

result = pontiac.solve("examples/hu_sng_flop_20bb.json", save="q8h3.pontiac")   # dict
node   = pontiac.query("q8h3.pontiac", "flop/BB_check/SB_bet0.33", hand="AhKd")  # no re-solve
size   = pontiac.estimate(spot_dict)   # memory needed, without solving
pontiac.version()
```

`solve` and `estimate` take a dict or a file path. Problems raise `pontiac.SolverError` with a message that says what to change.

### Command line

```
pontiac solve spot.json [-o result.json] [--save solved.pontiac]
pontiac query solved.pontiac --node <path> [--hand AhKd | --hand AKs] [--detail class|combo]
pontiac estimate spot.json
pontiac pushfold --stack 10 [--sb 0.5] [--ante 0]
pontiac version
```

(`python -m pontiac ...` is the same thing.) Results go to stdout as JSON; progress goes to stderr. On an error, stdout is `{"ok": false, "error": "..."}` and the exit code is non-zero.

### Spot file

```json
{
  "players": ["SB", "BB"],
  "effective_stack_bb": 18,
  "pot_bb": 4,
  "board": "Qh8c3h",
  "ranges": {"SB": "22+,A2s+,K5s+,QJo:0.5", "BB": "..."},
  "bet_tree": {
    "flop":  {"bet": [0.33, 0.75], "raise": [2.5]},
    "turn":  {"bet": [0.5, 1.0],  "raise": [2.5]},
    "river": {"bet": [0.5, 1.0],  "raise": [2.5]},
    "allin_threshold": 0.67
  },
  "rake": 0,
  "target_exploitability_pct": 0.5,
  "max_seconds": 600,
  "node_locks": [],
  "habits": []
}
```

| Field | Meaning |
|---|---|
| `players` | Two names. The names are used everywhere else in the file and in the result. |
| `oop` | Optional: who acts first after the flop. Default: worked out from the seat names (heads-up, `BB` acts first and `SB` last; otherwise earlier seat first). With non-seat names, the first listed. |
| `effective_stack_bb` | **Stack behind at the start of the spot**, not the stack at the start of the hand. 20 bb deep after a min-raise and call is `pot_bb: 4`, `effective_stack_bb: 18`. |
| `pot_bb` | Pot at the start of the spot. |
| `board` | 3 cards (flop, turn and river are solved), 4 (turn and river) or 5 (river only). |
| `ranges` | Per player. `AA`, `TT+`, `99-66`, `AKs`, `A5s+`, `KJo`, exact hands `AhKd`, and weights `QQ:0.5`. |
| `bet_tree` | Per street: `bet` as shares of the pot, `raise` as multiples of the bet being raised (strings such as `"60%"` or `"a"` for all-in pass straight to the engine). Optional `donk` on turn and river. `allin_threshold`: all-in is added when a bet would leave less than this share of the pot behind (default 0.67). For different trees per player, nest them: `{"SB": {"flop": ...}, "BB": {"flop": ...}}`. A street with no sizes is check/call only. |
| `rake` | `0`, or `{"rate": 0.05, "cap_bb": 3}`. |
| `target_exploitability_pct` | Stop when the solution is this close, as a percent of the starting pot (default 0.5). |
| `max_seconds` | Time limit (default 600). The result reached so far is kept and `stopped_because` says `max_seconds`. With locks that need two solves, the limit covers both. |
| `max_iterations`, `threads`, `max_memory_mb`, `compress` | Optional limits. `compress` forces 16-bit storage on or off; by default it switches on above 1.5 GB and halves memory. |
| `report_nodes` | Extra node paths to include in the result, e.g. a specific turn and river. |
| `detail` | `"combo"` (default) or `"class"` to leave exact hands out of the result. |
| `max_report_nodes` | Cap on first-street nodes in the result (default 60). |

### Result

Top level:

| Field | Meaning |
|---|---|
| `version` | Solver build string. Also stored in saved solves and returned by `query`. |
| `exploitability_pct` | Accuracy actually reached, percent of the starting pot, **within any locks**. |
| `exploitability_ignoring_locks_pct` | How exploitable the result is if the locked player were free. Equal to the above with no locks; large by design when a lock models a mistake. |
| `iterations`, `runtime_seconds`, `total_seconds`, `stopped_because` | `target_reached`, `max_seconds` or `max_iterations`. |
| `ev_bb` | Per player, at the start of the spot. EVs count the starting pot, so with no rake they add up to `pot_bb`. |
| `memory_mb`, `compressed`, `threads` | What the solve used. |
| `locked_decisions` | How many decisions were fixed. |
| `normal` | Present when a normal (unlocked) solve ran first: same summary fields plus its own `nodes`, for before/after comparison. |
| `nodes` | Map from node path to node report: every decision on the starting street, plus `report_nodes`. |
| `spot` | The input, echoed. |

Node report:

```json
{
  "path": "flop/BB_check", "kind": "decision", "street": "flop", "board": "3h8cQh",
  "player": "SB", "pot_bb": 4.0, "locked": false, "reached": true, "line_frequency": 0.9043,
  "actions": [{"name": "check", "amount_bb": 0.0, "frequency": 0.476, "path": "flop/BB_check/SB_check"},
              {"name": "bet0.33", "amount_bb": 1.32, "frequency": 0.4336, "path": "flop/BB_check/SB_bet0.33"}],
  "range_ev_bb": 2.41, "range_equity": 0.55, "range_combos": 912.0,
  "by_class": {"AKs": {"combos": 3.0, "freqs": {"check": 0.2, "bet0.33": 0.8}, "ev_bb": 3.1, "equity": 0.63}},
  "by_combo": {"AhKh": {"weight": 1.0, "freqs": {"check": 0.1, "bet0.33": 0.9}, "ev_bb": 3.4, "equity": 0.66}}
}
```

- `line_frequency`: how often play reaches this node (product of the range-wide frequency of each action on the way).
- `reached: false`: the opponent never takes this line in this solve. Frequencies are then averaged over the player's own range, and should be read as rough.
- `weight` (by combo) and `combos` (by class): how much of the hand is still in range here.
- `kind: "deal"` nodes list `next_paths`, one per card that can come. `kind: "terminal"` means the hand is over.

### Node paths

```
flop/BB_check/SB_bet0.33/BB_call/turn=7h/BB_check/SB_bet0.5/BB_call/river=2c
```

- Segments are separated by `/`. The street words (`flop`, `turn`, `river`) are optional markers.
- An action is `<player>_<action>`; the player prefix is optional but is checked when given.
- Actions: `check`, `call`, `fold`, `allin` (also `jam`), `bet<share of pot>`, `raise<multiple of the bet being raised>`. A size in big blinds also works: `bet1.3bb`.
- A bet or raise matches the closest available size and must be within 25% of it; otherwise the error lists the choices. An all-in counts as a size, so `bet1` finds the all-in when a pot-size bet was merged into it.
- A dealt card is `turn=7h` or `river=2c`.
- Every node report gives the exact path of each action, so paths can be followed without guessing sizes.

### Query without re-solving

`pontiac.query(saved, node, hand=None, detail="class")` loads a saved solve and reports one node. `hand` narrows it to one exact hand (`"AhKd"`, either card order) or one class (`"AKs"`). A query takes a few seconds, nearly all of it loading the file. Saved solves are big: the 20 bb flop example is about 1 GB on disk.

## 3. Node locking

```json
"node_locks": [
  {"node": "flop/BB_check/SB_bet0.33", "player": "BB",
   "lock": {"hands": "A2s-A5s,76s,65s", "action_freqs": {"raise": 0.0}}}
]
```

- `node`: path of the decision. `player` is optional and checked.
- `hands`: range text for the hands the lock covers. Leave it out to lock the whole range. **Hands outside it stay free and are re-solved.**
- `action_freqs`: frequencies by action name. Besides exact names (`bet0.33`), `bet`, `raise`, `jam` and `aggressive` cover every action of that kind, sharing the frequency equally.
- **Every action listed** (adding up to 1): a plain lock, one solve.
- **Some actions left out**: the listed ones are fixed, and each hand's leftover frequency is split between the unlisted actions in the proportions of the normal solve. This needs a normal solve first, so the run does two solves and the result includes `normal`. Example: `{"raise": 0.0}` means "never raises here, otherwise plays as it normally would".

Limit worth knowing: a hand is either locked or free. The engine cannot fix one action's frequency for a hand and re-optimise that same hand's other actions; the proportional split above is the stand-in.

Examples from the request:

- *Never bluffs with air on the turn:* lock the turn betting node with `hands` set to the air hands and `{"check": 1.0}`, or use the habit below, which finds the weak hands itself.
- *3-bet jams 40% at 20 bb:* that is a preflop frequency, and this solver starts on the flop. Model it in the ranges you pass in.

### Habits (player profiles)

Tendencies applied across the whole tree, as in the app:

```json
"habits": [{"player": "BB", "effect": "fold_less", "strength": 0.6, "street": "all"}]
```

| Effect | Meaning |
|---|---|
| `fold_less` | Calls too much: that share of folds become calls. |
| `fold_more` | Folds too much: that share of calls and raises become folds. |
| `raise_less` | Too passive facing bets: raises become calls. |
| `bet_less` | Too passive when checked to: bets become checks. |
| `bet_more` | Bets too often when checked to. |
| `bluff_less` | Weak hands (under 45% equity) stop betting. |

`street` is `flop`, `turn`, `river` or `all`. `strength` is 0 to 1. Habits run a normal solve first, then fix the habit player to "normal play plus the habit" everywhere and re-solve the other player. Set `"freeze_rest": false` to fix only the decisions the habit touches. Habits and `node_locks` can be combined; an explicit lock wins where both apply.

## 4. Scope

- **Preflop: push/fold only.** The solver starts on the flop, turn or river, so supply preflop ranges. Heads-up push/fold ranges for any stack depth come from `pontiac.pushfold` (section 6). Limps and small raises preflop are not solved.
- **Depth: yes.** A full flop-through-river tree at 15 to 30 bb is supported heads-up, with every turn and river card solved exactly.
- **Performance**, measured on a 2-core cloud container (8 GB RAM), the example spot `examples/hu_sng_flop_20bb.json`: 18 bb behind, 4 bb pot, wide ranges (SB opens about 80% of hands), raises at 2.5x, all-in available.

  | Bet sizes | Memory | Time to under 0.5% of the pot |
  |---|---|---|
  | Two per street | 2.1 GB (16-bit storage; 4.1 GB without) | 6.1 minutes, 130 iterations |
  | One per street | 0.8 GB (16-bit storage; 1.6 GB without) | 1.5 minutes, 80 iterations |

  Three sizes per street was not measured; expect more than double the two-size figures. A lock that needs a normal solve first doubles the time. Turn and river spots take seconds. Use `pontiac.estimate` to check memory before solving.
- **Three players: yes**, with the same `solve` call (section 5). No `save`/`query` for three players yet: the result itself lists the first street's nodes.

## 5. Three players

Give three names in `players` and the same `solve` / `estimate` calls use the three-player engine. Example: `examples/three_way_river.json`.

```json
{
  "players": ["BB", "CO", "BTN"],
  "stacks_bb": {"BB": 40, "CO": 100, "BTN": 65},
  "pot_bb": 15,
  "board": "Kh9s6d4c2s",
  "ranges": {"BB": "...", "CO": "...", "BTN": "..."},
  "bet_tree": {"river": {"bet": [0.33, 0.75], "raise": [2.5]}, "max_raises": 3, "allin_spr": 1.0},
  "rake": {"rate": 0.05, "cap_bb": 1.5},
  "target_exploitability_pct": 0.2,
  "max_seconds": 120
}
```

What differs from two players:

- **Acting order** follows the seat names (SB, BB, UTG ... CO, BTN). For other names, add `"order": ["first", "second", "third"]`. The result repeats it as `acting_order`.
- **Stacks:** `stacks_bb` per player, or one `effective_stack_bb` for everyone. Unequal stacks create side pots: a short stack that is all-in plays for the main pot only.
- **Rake:** `{"rate": 0.05, "cap_bb": 1.5}`; taken from pots that see a call. Leave it out for tournaments.
- **Bet tree:** per street `bet` (shares of the pot) and `raise` (a number is a multiple of the bet being raised; a text like `"60%"` is a share of the pot). `max_raises` (default 3) caps raises per street; a bet becomes all-in when what is left behind is at most `allin_spr` times the pot (default 1.0).
- **Later cards:** flop and turn spots can deal a balanced sample of later cards to save time and memory: `"later_cards": "sample"` (default on the flop), `"larger"`, `"all_turns"`, or `"exact"` (default on the turn; every card).
- **Accuracy:** `exploitability_pct` is the most any one player could gain by changing only their own strategy, as a percent of the starting pot; `gain_pct_by_player` lists all three. With three players there can be more than one sound solution, so treat exact frequencies as a guide.
- **A player who already folded:** `"folded": ["BTN"]` starts the spot with that player out and their chips in the pot. Their range still matters as dead cards; give every hand if you do not know what they folded.
- **Node locks and habits** work as in section 3; paths name the player at every step (`river/BB_check/CO_bet0.33/BTN_fold`).
- **Result:** `ev_bb` (three values, adding up to the pot less rake), and `nodes` for the first street, each with `actions`, `by_class`, `by_combo`, `stacks_bb` and who is still `in_hand`.

Measured on the same 2-core container: the river example solves in about 1 second; the same spot started on the turn with one bet size and every river card takes about 35 seconds and 60 MB. Flop spots take minutes to hours depending on bet sizes and the card sample; check `pontiac.estimate` first (`seconds_per_iteration` times roughly 100 to 250 iterations).

**Cross-check between the two engines.** A three-way pot with one player already folded is a heads-up pot, and the three-player and two-player engines share no code. The tests solve the same turn and river spots (30 bb behind, bet 50%, raise 2.5x, all-in) with both: the decision points match and the players' EVs agree within 0.002 bb of a 12 bb pot.

## 6. Heads-up push/fold

```python
r = pontiac.pushfold(10)                  # 10 bb effective, blinds 0.5/1, no ante
r = pontiac.pushfold(8.5, small_blind=0.5, ante=0.125)
r["jam_pct"], r["call_pct"]               # 0.581, 0.375 at 10 bb
r["jam_range"], r["call_range"]           # range text, mixed hands as "Q8o:0.036"
dict(zip(r["classes"], r["jam"]))["K5o"]  # frequency for one hand class
```

The small blind moves all-in or folds; the big blind calls or folds. `stack_bb` is the shorter stack before the blinds are posted. The answer is the equilibrium (checked against published charts: 58% jam / 37% call at 10 bb), found in well under a second. `jam_edge_bb` and `call_edge_bb` say how much each hand gains over folding, so values near zero are close decisions. `ev_small_blind_bb` is the small blind's average result per hand. Heads-up sit-and-gos pay the winner only, so chip values need no prize adjustment.

## 7. Extras

- **Samples:** `examples/river_quick.json` (solves in milliseconds) with its expected output in `examples/expected/`, `examples/hu_sng_flop_20bb.json`, and `examples/three_way_river.json`.
- **Version string:** in every result, every saved solve and every query answer.
- **Reproducibility:** the solver uses no random numbers. The same spot file, program version and thread count give the same result; the smoke tests check this. Different thread counts can differ in the last decimals.

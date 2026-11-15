# Pontiac Solver

A free, open-source no-limit hold'em postflop solver with node locking, built for study.

- **Desktop app (Windows):** guided heads-up study with opponent habits, three-way pots from the flop, turn or river (different stacks, side pots and rake), push/fold calculators (heads-up, and three-handed for chips or prize money), tournament setup in big blinds with antes, range sliders, and the original advanced solver screens.
- **Headless (Linux, Python or command line):** JSON spot in, JSON result out, for two or three players, plus push/fold ranges. See [docs/HEADLESS.md](docs/HEADLESS.md).

## Quick start, headless

```
pip install -e ".[test]" && pytest
```

```python
import pontiac
result = pontiac.solve("examples/river_quick.json", save="spot.pontiac")
print(result["exploitability_pct"], result["ev_bb"])
print(pontiac.query("spot.pontiac", "river/BB_check/SB_bet0.5", hand="Qc9c"))
```

## What is in this repository

| Folder | What it is |
|---|---|
| `pontiac/` | Python package; bundles the Linux solver program in `pontiac/bin/` |
| `cli/` | Source of that program (Rust): `solve`, `query`, `estimate`, `pushfold` |
| `engine/` | Two-player solver: b-inary's postflop-solver with tonyyet's node-locking changes, patched |
| `multiway/` | Three-player solver, original to this project. `cargo run --release --bin mwcheck` runs its checks |
| `pushfold/` | Heads-up push/fold calculator and its preflop all-in equity table |
| `app/` | Desktop app (Tauri + Vue) |
| `checks/` | Test tools and the browser dev server for the app |
| `examples/`, `tests/`, `docs/` | Sample spots with expected output, smoke tests, interface reference |
| `scripts/build_cli.sh` | Rebuilds the solver program and refreshes the bundled copy |

`BUILDING.txt` covers building the desktop app.

## What the desktop app shows

- **Study a spot**: describe the table and the preflop action seat by seat; the app picks the two-player or three-player solver and solves on that page.
- **Result**: the board and the line so far stay pinned at the top. Each decision has the hand chart, a by-suit breakdown, play by type of hand (top pair, flush draw and so on), an equity graph of every player's range with a plain-language reading of it, and coach notes.
- **Which cards matter**: at each turn or river card choice, one click compares every card: how often the next player bets on it and who it helps.
- **Flop report**: solves the same two-player preflop spot on 8, 16 or 24 representative flops and lines the answers up, with averages by kind of flop.
- **Push / Fold**: heads-up and three-handed with prize money (ICM).
- **Library**: save a solve from the Result page and open it again later. Two-player solves are stored whole (compressed); three-player spots are solved again when opened.
- **Node locks in the app**: at any decision, fix what a player does (whole range or one chart square) and solve again; the normal solve is reused.
- **Chart views**: actions, value (EV) or chance to win on the hand chart. **Blockers**: which cards in your hand make the other player fold more or less after a bet.
- **Paste a hand history** (PokerStars text layout) to fill in the table, preflop action, stacks, flop and your cards.
- Dark and light looks (switch in the top bar).

## Limits

- Preflop is limited to push/fold (heads-up and three-handed); limps and small raises are not solved.
- Prize-money (ICM) payoffs apply to push/fold only; postflop solves play for chips.
- Three-player solves cannot be saved and queried later yet; the result lists the first street.
- Three-player flop spots deal a sample of turn and river cards by default. The app can then solve any single turn card exactly.

## Licence and credits

AGPL-3.0-or-later (see `LICENSE`). Built on [postflop-solver](https://github.com/b-inary/postflop-solver) and [desktop-postflop](https://github.com/b-inary/desktop-postflop) by Wataru Inariba (b-inary), and the node-locking fork by tonyyet. If you share the app or run it as a service, the licence requires you to make this source available.

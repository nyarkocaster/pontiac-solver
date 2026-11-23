# Pontiac Solver

A free, open-source no-limit hold'em postflop solver with node locking, built for study.

- **Desktop app (Windows, macOS, Linux):** guided heads-up study with opponent habits, three-way pots from the flop, turn or river (different stacks, side pots and rake), push/fold calculators (heads-up, and three-handed for chips or prize money), tournament setup in big blinds with antes, range sliders, and the original advanced solver screens.
- **Headless (Linux, Python or command line):** JSON spot in, JSON result out, for two or three players, plus push/fold ranges. See [docs/HEADLESS.md](docs/HEADLESS.md).

## Download

Ready-made builds for Windows, macOS and Linux are on the [Releases page](https://github.com/nyarkocaster/pontiac-solver/releases): the desktop app, and the command-line program on its own.

- **Windows:** run the installer (`.msi` or `-setup.exe`). Windows may warn that the publisher is unknown: choose "More info", then "Run anyway".
- **macOS:** open the `.dmg` and drag the app to Applications. The app is not signed with an Apple developer certificate, so the first time, right-click the app and choose Open, or allow it under System Settings, Privacy & Security.
- **Linux:** the `.AppImage` runs as is (`chmod +x` first); there is also a `.deb`.

## Quick start, headless

```
pip install "git+https://github.com/nyarkocaster/pontiac-solver"
python -m pontiac selftest
```

```python
import pontiac
spot = pontiac.example("river_quick")
result = pontiac.solve(spot, save="spot.pontiac")
print(result["exploitability_pct"], result["ev_bb"])
print(pontiac.query("spot.pontiac", "river/BB_check/SB_bet0.5", hand="Qc9c"))
```

The bundled solver program is for Linux x86-64. On Windows or a Mac, download the command-line program from the Releases page and set `PONTIAC_BIN` to its path.

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
- **Preflop**: heads-up push or fold; heads-up with limps, raises and re-raises (all-in pots exact, pots that see a flop estimated, with each line's ranges handed to the flop solver); three-handed push or fold with prize money (ICM).
- **Hand review**: paste a hand history in Study a spot. For a two-player pot the result page prices each of your decisions after the flop for the exact hand you held and shows what the action you took gave up.
- **Library**: save a solve from the Result page and open it again later. Solves are stored whole (compressed) and open without solving again.
- **Node locks in the app**: at any decision, fix what a player does (whole range or one chart square) and solve again; the normal solve is reused.
- **Chart views**: actions, value (EV) or chance to win on the hand chart. **Blockers**: which cards in your hand make the other player fold more or less after a bet.
- **Pick the bet sizes for me**: a rough first solve tries five bet sizes and keeps the two each player uses most, so you do not have to guess which sizes matter on a board.
- **Paste a hand history** to fill in the table, preflop action, stacks, flop and your cards. Hands are read as saved by PokerStars, GGPoker, Winamax, 888poker, partypoker, Americas Cardroom (Winning network), Ignition / Bovada / Bodog, iPoker sites and PokerNow.
- Dark and light looks (switch in the top bar).

## Limits

- Preflop: all-in or fold is solved exactly (heads-up and three-handed). Limps and small raises are an estimate, not a solution.
- Prize-money (ICM) payoffs: push/fold, two-player postflop spots (app and headless) and three-player postflop spots (app and headless). No rake with prize money.
- Three-player flop spots deal a sample of turn and river cards by default. The app can then solve any single turn card exactly.
- Checked against GTO Wizard on a heads-up flop spot (chips, a node lock, prize money) and on three-handed push/fold. Three players after the flop has internal checks only.

## Licence and credits

AGPL-3.0-or-later (see `LICENSE`). Built on [postflop-solver](https://github.com/b-inary/postflop-solver) and [desktop-postflop](https://github.com/b-inary/desktop-postflop) by Wataru Inariba (b-inary), and the node-locking fork by tonyyet. If you share the app or run it as a service, the licence requires you to make this source available.

# Pontiac Solver

A free, open-source no-limit hold'em postflop solver with node locking, built for study.

- **Desktop app (Windows):** guided heads-up study with opponent habits, three-way pots from the flop, turn or river, and the original advanced solver screens.
- **Headless (Linux, Python or command line):** JSON spot in, JSON result out, for automated analysis. See [docs/HEADLESS.md](docs/HEADLESS.md).

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
| `cli/` | Source of that program (Rust): `solve`, `query`, `estimate` |
| `engine/` | Two-player solver: b-inary's postflop-solver with tonyyet's node-locking changes, patched |
| `multiway/` | Three-player solver, original to this project. `cargo run --release --bin mwcheck` runs its checks |
| `app/` | Desktop app (Tauri + Vue) |
| `checks/` | Test tools and the browser dev server for the app |
| `examples/`, `tests/`, `docs/` | Sample spots with expected output, smoke tests, interface reference |
| `scripts/build_cli.sh` | Rebuilds the solver program and refreshes the bundled copy |

`BUILDING.txt` covers building the desktop app.

## Limits

- Postflop only: no preflop solving.
- The command line handles two players; three-player spots are in the desktop app.
- Three-player spots give all players the same stack (no side pots) and have no rake.

## Licence and credits

AGPL-3.0-or-later (see `LICENSE`). Built on [postflop-solver](https://github.com/b-inary/postflop-solver) and [desktop-postflop](https://github.com/b-inary/desktop-postflop) by Wataru Inariba (b-inary), and the node-locking fork by tonyyet. If you share the app or run it as a service, the licence requires you to make this source available.

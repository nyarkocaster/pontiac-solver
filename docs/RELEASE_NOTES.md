# Pontiac Solver 0.13.3

**New since 0.13.0 (speed):**

- Three-player spots played for tournament prize money solve about four times faster, with the same answers. A prize-money solve now costs under twice a chips solve instead of seven times.
- Three-player solves get closer to the answer in the same number of steps (the average strategy now leans harder on recent steps), and the accuracy check, which costs several steps' worth of time, is run only when it is likely to matter.
- A time limit or Cancel now stops a three-player solve within a moment, even in the middle of a long step; everything learned up to then is kept.

The first public release with ready-made downloads. Free and open source (AGPL-3.0-or-later).

## Which file do I want?

| You have | Desktop app | Command-line program only |
|---|---|---|
| Windows 10 or 11 | the `.msi` or the `-setup.exe` installer | `pontiac-windows-x64.exe` |
| Mac (Apple or Intel chip) | the `.dmg` | `pontiac-macos-universal` |
| Linux | the `.AppImage` (or the `.deb` on Debian and Ubuntu) | `pontiac-linux-x64` |

- **Windows** may say the publisher is unknown: choose "More info", then "Run anyway".
- **macOS**: the app is not signed with an Apple developer certificate. The first time, right-click the app and choose Open, or allow it under System Settings, Privacy & Security.
- **Linux**: `chmod +x` the AppImage, then run it.

`SHA256SUMS.txt` lists a checksum for every file.

## What it does

- Heads-up postflop solving from the flop, turn or river, with node locks and opponent habits.
- Three-player postflop solving with different stacks, side pots and rake.
- Tournament prize money (ICM): all-in-or-fold ranges heads-up and three-handed, and postflop spots.
- Study tools: hand charts, equity graphs, flop reports, blockers, a library of saved solves, hand-history import.
- A headless interface for scripts: JSON in, JSON out, from the command line or Python (`docs/HEADLESS.md`).

## How it has been checked

- Against GTO Wizard on a heads-up flop spot: range EVs agree to the third decimal, responses to a bet within one or two points, with and without a node lock, and with prize money. Three-handed all-in-or-fold with prize money agrees within about one point on all six decisions.
- Three players after the flop has internal checks only: a full count of showdowns, and agreement between the project's two engines when one player has folded.
- Every command-line build in this release passed the built-in self-test on its own operating system before it was published. **The macOS and Linux desktop apps were built automatically and have not yet been opened by a person**: please report what you find.

## Known limits

- Preflop: all-in or fold is solved exactly. Limps and small raises are an estimate, not a solution.
- Three-player solves cannot be saved and reopened; three-player flop spots use a sample of later cards.
- Three-player solves on Apple-chip Macs run without the fast showdown code written for Intel and AMD chips, so they are slower there.

Problems and suggestions: https://github.com/nyarkocaster/pontiac-solver/issues

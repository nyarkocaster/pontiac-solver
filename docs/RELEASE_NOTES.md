# Pontiac Solver 0.13.17

**New in 0.13.17, session statistics:** load a whole session in Study a spot ("Paste a hand history") and open "How each player played in this session". For you and every other player it counts, out of the chances each had: played the hand (VPIP), raised before the flop (PFR), re-raised a raise (3-bet), folded to a re-raise, saw the flop, bet the flop after raising (c-bet), folded to that bet, went to showdown, and bets and raises per call after the flop. It counts every hand in the file, including those that ended before the flop, and reads every site layout the hand loader reads. Winnings are not counted.

**New in 0.13.16:**

- Ranges from other programs: in Study a spot, each player's range has "Bring in a range from another program". Paste the text or choose a text file. Read: lists such as "TT+, AJs+, KQo"; weights as "AKs:0.5" or "AKs:50"; weights as blocks, "[50]AQo,KJs[/50]"; and card-by-card lists such as "AhKh:0.25, AsKs:1, …", which are averaged into hand classes (the app says which hands had different weights by suit). Anything not understood is listed and left out.
- Lock sets side by side: after each "Solve again" with node locks, a table under the locks lists every set solved so far, with your value, the change from the normal solve, and the play at the decision that was on screen. "Use again" brings a set's locks back. Only the newest solve can be browsed; up to eight sets are kept until the next new solve.

**New in 0.13.15:**

- Types of hand: click several types to show them together, and Ctrl-click (or Shift-click) a type to hide it. "Flush draw" shown with "Top pair" hidden leaves the flush draws that are not also top pair.
- Flop report: "Show only" buttons for paired, connected, one suit, two-tone, rainbow and four heights of flop. Picks in the same family widen (two-tone or rainbow), picks across families narrow (ace high and two-tone). The average of the shown flops sits above the average of all.

**New in 0.13.14, highlight a type of hand:** on the Result page, click a row in "play by type of hand" (top pair, flush draw, no pair and so on) and the chart fades every square that holds none of that type, while "Every hand" lists only those hands. Click the row again, or "Show all hands", to go back. The choice stays as you move through the hand, so you can follow one type of hand across streets. Two-player results only for now.

**Fixed in 0.13.13:** two-player tournament spots counted chips in tenths of a big blind, so a pot that is not a whole tenth was rounded before solving (with a 0.125 bb ante, 2.25 bb was solved as 2.3 bb, 2% too much). They now count in hundredths. Spots saved to the library before this version open as they were solved.

**New since 0.13.0, the Classic solver tab is gone:**

- Study a spot now has everything the Classic solver tab had and it did not, under "More control over the tree" (two-player spots):
  - **Lead sizes**: what the first player may bet on the turn or river after calling on the street before.
  - **Change the tree line by line**: walk to any decision, take an action out there or add a bet or raise of an exact amount, and undo any change. The count of decision points shows what a change does to the size of the tree.
  - **Cards the folded players took**: type what up to four folded players fold and the solve counts it. Much slower: a spot that took 20 seconds took 2 minutes with one or two folded players; four was stopped unfinished after 8 minutes.
  - **Rake** for two-player cash games (percent and cap).
- The tab itself is replaced by About (version, licence, credits).
- A solve with hand-made tree changes or folded players' cards cannot be saved as a small library file; "Save whole" works.
- Checked in the browser development build; not yet tried in the Windows app.

**More on the Result page (0.13.10):**

- (0.13.12) Hover over a chart square for a card with that hand's share still in range, how often it takes each action, its value and its chance to win.
- (0.13.12) "What the other player holds here", under the chart: the range of the player who is not deciding, as a second chart that follows the Actions / Value / Chance to win buttons.
- A table drawing above the charts: both seats with what each has behind and has put in, the board, the pot, whose turn it is, and each range's chance to win and value. Your cards are shown when a hand history was loaded. "Hide the table" puts it away and is remembered.
- "Every hand ... can hold", under the charts: each exact hand still in the deciding player's range with how much of it is left, how often it takes each action, what it is worth, its chance to win, and "share kept" (value against chance to win times the pot). Click a column to sort, type to filter (AK, 7h, s), click a row to select its square in the chart.
- Checked in the browser development build; not yet tried in the Windows app.

**Flop solves small enough to share (0.13.9):**

- A whole flop solve is about 1 GB. "Save small, to share" on the Result page keeps the flop only: both players' flop strategy and what every action is worth to every hand. The same 20 bb flop is 44 KB. From the command line and Python: `solve --library spot.plib`, or `pack` on a whole solve saved earlier.
- Opening a small file shows the flop at once. When you pick a turn card, that turn and its rivers are solved on your computer first, from the ranges the flop strategy leaves each player: about 1 second on average and 7 at most on a 2-core machine for the 20 bb example. "Which cards matter" works without solving anything, because the file keeps a line per turn card.
- In the Library, "Get the file" hands a small entry over as one `.plib` file and "Add a library file" takes one in, from another person or from the headless program.
- How close a turn solved this way is to the whole solve's own turn, on the 20 bb example (33 turn spots, weighted by how often they come up): how often each action is taken differed by 1.2 points on average and 7 at worst; the value of each player's range by 0.04% of the pot on average and 0.4% at worst. Two solves of the same turn differ from each other by about as much. On lines the flop solve takes under 1% of the time the gap can be several times larger.
- Not in a small file: node locks and habits (solve the spot yourself to lock it), prize money, and spots with separate turn lead sizes. Checked in the browser development build and from the command line; not yet tried in the Windows app.
- Fixed: on a long line, the step tiles above the result were squeezed and their text ran over "Back one step". They now keep their width and the strip scrolls to the newest step.

**Load a whole session:**

- Paste a whole session of hands into the hand-history box, or use "Open a file", and you get a list of the hands that can be studied: your cards, seat, board, how many players saw the flop, and how much you put in after the flop, biggest first. Click "Study" on a row to load that hand. Hands that ended before the flop, that you folded, or that the solvers do not cover are counted and left out, with the reason.
- Works for every layout the box reads. In a PokerNow log, one hand where you showed your cards is enough to find your seat in all the others.
- Each hand is still solved when you study it; the list is for finding the hands worth the time. A 2,000-hand session is listed in well under a second.

**The solver can pick the bet sizes:**

- Tick "Pick the bet sizes for me" in Study a spot (two-player pots). A rough first solve tries 33, 50, 75, 100 and 150% of the pot on the street the spot starts on and keeps the two sizes each player uses most; the real solve then uses those. The result page says which sizes were kept and which were left out. Sizes on later streets stay as typed.
- From the command line and Python: add `"auto_sizes": true` to a spot (or `{"candidates": [...], "keep": 2}`). The result reports what was offered, how often each size was used, and what was kept.
- Checked on a 20 bb flop on the laptop: with the picked pair, neither player gave up anything measurable against having all five sizes, and the solve was faster and smaller.
- Measured on eight heads-up turn spots: with five sizes offered, the two kept sizes gave up at most 0.3% of the pot against having all five, and under 0.05% on five of the eight. The rough first solve is small, because it uses one size on later streets: on a 20 bb flop it needed half the memory of the normal two-size solve.

**Hand histories from more sites:**

- The hand-history paste box now reads hands as these sites save them, not only the PokerStars layout: GGPoker, Winamax, 888poker, partypoker, Americas Cardroom and the other Winning network sites, Ignition, Bovada and Bodog, iPoker sites (bet365, Betfair and others, whose files are XML), and PokerNow game logs. Paste the hand as it is; the app says which layout it recognised. Everything that worked for PokerStars hands, including the hand review below, works for all of them.
- A PokerNow log does not say which player you are. If the hand reached a showdown your seat is found from the cards you showed; otherwise pick your seat yourself.
- Checked against real sample hands for 888poker, partypoker, Winamax, the Winning network, iPoker, Ignition and PokerStars. The GGPoker and PokerNow layouts were written from their published descriptions and have not yet been tried on a file saved by those sites; if one does not load, please send it.

**Hand review:**

- Paste a hand history for a two-player pot and the result page now reviews it: each of your decisions after the flop, what the solve does with your exact hand, what each action is worth, and how much the action you took gave up. "Show" opens the result at that decision. Bets in sizes the solve does not have are matched to the closest size and marked.

**Three-player spots:**

- Three-player solves can be saved and opened again without solving: in the app's Library, and from the command line and Python (`solve --save`, then `query`).
- Prize money (ICM) for three-player tournament pots is in the app: tick "Play for prize money, not chips" in Study a spot.
- Node locks by hand on the three-player result: fix what a player does at a decision, for every hand or one chart square, and solve again to see how the other two adjust. It reuses the solve on screen, including one opened from the Library.

**Speed:**

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

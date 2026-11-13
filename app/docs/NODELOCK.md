# Node Locking — app notes (Desktop Postflop fork)

Branch: `nodelock` of the local `desktop-postflop` fork (upstream `b-inary/desktop-postflop`,
fork point = `0c15f97` v0.2.7). The Rust engine is the local `nodelock` clone of
`postflop-solver`, referenced **by path** from `src-tauri/Cargo.toml` (revert to the git
dependency before merging upstream).

Written 2026-10-03. Companion document (engine side): `postflop-solver/docs/NODELOCK.md`.

---

## 1. What it does

Solve a spot as usual (the *baseline*), navigate to the villain node you want to fix, edit that
node's frequencies, and re-solve. The new solve is the optimal **restricted response** to the
locked strategy; the UI shows both the convergence metric inside the locks and the *real*
exploitability of the profile you are looking at.

Workflow:

1. Configure a spot and solve it normally. The metrics of that run are cached as the baseline.
2. Walk the result tree to the node you want to lock (the displayed node is tracked by
   `ResultNav.vue` → `lockStore.setCurrentHistory(history)`).
3. In the **Node Lock** panel (`NodeLockEditor.vue`):
   * tick **Enable node locking**,
   * edit the frequency grid per hand (column *Lock*), or use the quick actions
     *lock to current strategy*, *lock to pure strategy*, *remove an action* (0 %),
   * optionally set a **node frequency target (%)** per action — the app distributes it over the
     hands proportionally to the current (locked/draft) strategy, so you can type
     "check 60 % / bet 40 %" directly,
   * optionally make it a **soft lock** (weight slider, see §5).
4. Press **Apply locks & re-solve** — the app re-runs the whole pipeline
   (`init → allocate_memory → apply every pending lock → solve → finalize`), because the engine
   only accepts locks after `game_allocate_memory` and before the first solve step. The tree is
   rebuilt from the saved config, so the pending locks are replayed on the fresh instance.
5. Compare **Locked vs baseline** (restricted & real exploitability, root EV) and, if wanted,
   run a **sensitivity sweep** over one parameter.

---

## 2. Tauri commands added

All of them return `Option<String>`, where `Some(message)` is a human-readable failure and
`None` means success. Every command first calls `locking_allowed(&game)`, which converts the
engine's `panic!`s ("memory not allocated" / "already solved") into messages.

| Command | Signature (Rust) | Notes |
|---|---|---|
| `game_lock_strategy` | `(history: Vec<usize>, strategy: Vec<f32>) -> Option<String>` | hard lock; `strategy.len()` must equal `#actions * #hands` |
| `game_lock_strategy_with_weight` | `(history, strategy, weight: f32) -> Option<String>` | soft lock / RRN; `weight` clamped to `[0, 1]`, validated before use |
| `game_unlock_strategy` | `(history) -> Option<String>` | removes the lock at that node |
| `game_current_locking_strategy` | `(history) -> Option<Vec<f32>>` | echo of the stored targets |
| `game_locking_weight` | `(history) -> Option<f32>` | `None` = not locked |
| `game_clear_locks` | `() -> Option<String>` | clear all node locks |
| `game_exploitability_unrestricted` | `() -> f32` | `async` command; real (locks-ignored) exploitability, `O(tree)` |

Terminal and chance nodes are rejected with a message instead of panicking. The commands save and
restore the current history, so calling them does not move the user's position in the tree.

---

## 3. Frontend architecture

* `src/invokes.ts` — thin wrappers: `gameLockStrategy`, `gameLockStrategyWithWeight`,
  `gameUnlockStrategy`, `gameCurrentLockingStrategy`, `gameLockingWeight`, `gameClearLocks`,
  `gameExploitabilityUnrestricted`.
* `src/store.ts` — **`useLockStore`** (Pinia):
  * `locks: NodeLock[]` keyed by `history.join(".")` (`lockKey`), `upsert` / `remove` / `clear`
    / `hasLocks`,
  * `softLockEnabled`, `softLockWeight` (default 0.5),
  * `currentHistory` (set by `ResultNav.vue`),
  * `baseline` / `lastLocked`: `RunMetrics` of the last unlocked / locked run
    (`recordRun()` routes them by `hasLocks`),
  * sweep state: `sweepRequest: SweepStep[]`, `sweepRunning`, `sweepCancelled`, `sweepProgress`,
    `sweepResults: SweepPoint[]`.
  * a `nodelockResolveRequested` flag in the config store asks `RunSolver.vue` for a full re-solve.
  * `NodeLock` = `{ history, strategy (flat, `action * numHands + hand`), numActions, numHands,
    label, isSoft, weight, targetFreqs | null }`; `SweepPoint` adds `exploitability`,
    `unrestricted`, `ev: [oop, ip]`, `elapsedMs`.
* `src/components/RunSolver.vue`
  * `consumeResolveRequest()` — full rebuild (`buildTree()` + `runSolver()`), then applies every
    pending lock; a lock with `weight >= 1` uses `gameLockStrategy`, otherwise
    `gameLockStrategyWithWeight`; failures surface as `Error: could not lock node <label>: …`.
  * `consumeSweepRequest()` — one complete re-solve per `SweepStep`; after each step it reads the
    restricted exploitability, the unrestricted one, the results (EV, normalizers) and pushes a
    `SweepPoint`; cancellable between steps.
  * metric labels: `Exploitability (within locks): …` when locks are active, plus
    `Exploitability (unrestricted, real): …` computed from the raw value (percent of the starting
    pot), EV averaged over hands with the reach weights (`weightedEv`).
* `src/components/NodeLockEditor.vue` — the editor, the *Locked vs baseline* panel and the sweep
  panel (mode `weight` = soft-lock weight `p`, or `freq` = one action's node frequency; 2…21
  points; *Run sweep*, progress %, *Cancel*; result table = parameter, exploitability, real,
  EV OOP, EV IP).
* `src/components/ResultViewer.vue` mounts `NodeLockEditor` for the selected spot;
  `ResultNav.vue` publishes the current history.

---

## 4. Stopping criterion

* The solve loop stops on the **restricted** exploitability (`game_exploitability`), i.e. the
  best response constrained at locked nodes. It converges to ~0 even with locks present, so the
  original stopping behaviour is preserved.
* The **unrestricted** value is displayed separately and never used for stopping: by
  construction it does not have to approach 0 (the engine document explains why, with a measured
  example: restricted ≈ 7.8e-5 vs unrestricted ≈ 1.52 chips for an equilibrium-frequency lock).

---

## 5. Soft lock / RRN

* `weight = 1` — hard lock (the node plays exactly the target).
* `0 < weight < 1` — soft lock: the node plays `weight * target + (1 - weight) * solved` on every
  iteration, so the residual `1 - weight` is optimized by the solver. This is the relaxed
  (RRN-style) variant: the *displayed* strategy at the node is the blend, and so are the EV /
  exploitability numbers.
* `weight = 0` — equivalent to unlocked; values outside `[0, 1]` are clamped by the engine.
* The weight is **not serialized** with the game: it must be restored after unserializing.

---

## 6. Sensitivity sweep

One full re-solve per point, so runtime = points × solve time (big trees take minutes per point).

* mode `weight`: sweep `p ∈ [from, to]` (default `0 → 1`) of the soft lock applied to the
  currently selected node; at least one hand must already be locked.
* mode `freq`: sweep one action's **node frequency** in percent (default `0 → 100`) — for each
  point the app rebuilds the lock of the selected node so that the chosen action has that exact
  node frequency, keeping every other node's lock untouched.
* Output per point: parameter value, restricted exploitability, real exploitability,
  EV OOP / EV IP (reach-weighted), elapsed ms.

---

## 7. Build, run, performance

Nightly is required (the engine uses `#![feature(allocator_api)]` for `custom-alloc`):

```bash
cd <repo>/src-tauri && cargo --version            # rustup toolchain nightly must be installed

# development (frontend hot reload), release-optimized backend:
set RUSTUP_TOOLCHAIN=nightly&& npx tauri dev --release

# production build: vue-tsc + vite build + cargo (release), then bundling
npm run build            # vue-tsc && vite build            -> dist/
npx tauri build          # -> src-tauri/target/release/*.exe (+ installers in .../bundle/)
```

Performance — why solves were ~5–10x too slow before, and what was done:

| Build | ms / step (same bench, same machine) |
|---|---|
| upstream engine, release | 1.674 |
| this branch, release | 1.535 |
| this branch, `tauri dev` (dev profile, `opt-level = 0`) | **41.6** |
| this branch, dev profile + `[profile.dev] opt-level = 3` | 1.558 |

* The solver hot path is generic code that gets **monomorphized into the engine crate**, so
  `[profile.dev.package."*"]` alone does not help; the whole dev profile has to be optimized.
* Fix applied: `[profile.dev] opt-level = 3` in `src-tauri/Cargo.toml` (measured 1.558 ms/step,
  i.e. release speed ≈ 1.5 ms/step, ~25x faster than the default dev build).
* Alternatively (or additionally) run `npx tauri dev --release`.
* The production build (`npm run tauri build`) uses the release profile and is unaffected.

---

## 8. Verification

Automated

```bash
npm run build                                   # vue-tsc: 0 errors, vite build OK
cargo +nightly test --offline                   # engine suite (70 tests)
cargo +nightly test --offline --test nodelock_verify   # 16 node-locking tests
```

Manual checklist on the real machine (Windows, WebView2):

1. Launch, configure an empty spot, solve → baseline is recorded (no locks, exploitability
   converges as before).
2. Navigate to a villain node, tick *Enable node locking*, set a pure strategy, *Apply locks &
   re-solve* → the hero's response at the parent node changes; the metric shows
   `Exploitability (within locks)` and a separate `(unrestricted, real)` value.
3. `Locked vs baseline` shows the baseline and the locked run side by side.
4. Soft lock with `p < 1` → displayed frequencies are the blend, exploitability drops relative to
   the equivalent hard lock.
5. `Run sweep` on `weight` and on a node frequency → N points, progress and cancel work.

Sanity signals used while developing: `lock_current_strategy` bookkeeping (`game_locking_weight`
round-trip), and window rendering checked by screenshot (thousands of distinct colours) plus
`ocr_utils.ocr_image` on the window.

---

## 9. Known limitations

* A single global `PostFlopGame` instance: every lock change means a full re-solve (seconds to
  minutes); no incremental update and no background solve.
* `src-tauri/Cargo.toml` points at the local engine clone by path — revert to the git dependency
  before merging upstream.
* Weights are not serialized (see §5); a tree rebuild drops every lock.
* The sweep is sequential (one re-solve per point) and only cancellable between points.

---

## 10. Shipped artifact

* Executable: `src-tauri/target/release/Nodelock Solver.exe` (4.35 MB) —
  production build (v0.2.7 + node locking), **standalone**: the frontend is embedded through the
  `custom-protocol` feature (verified byte-wise: the exe contains the built asset names
  `index-e9543a35.js` / `index-a5aa5ce4.css`), so it runs without Vite or `npm run dev`.
  Requires the WebView2 runtime (preinstalled on Windows 10/11).
* Desktop shortcut: `Nodelock Solver.lnk` → that executable (working dir = the release dir,
  icon = `src-tauri/icons/icon.ico`).
* How it was built (2026-10-03, from a clean `dist/`):

  ```bash
  npm run build                                   # vue-tsc && vite build  -> dist/
  cd src-tauri && set RUSTUP_TOOLCHAIN=nightly&& cargo build --release
  ```

  Plain `cargo build --release` is enough because `[features] default = ["custom-protocol"]`;
  `cargo` emits `desktop-postflop.exe`, which is copied to `Nodelock Solver.exe`.
  (`npx tauri build` does the same and additionally produces the msi/nsis installers —
  it needs to download the WiX/NSIS toolsets, so it is not part of the default flow here.)
* Labelling: `tauri.conf.json` → `package.productName = "Nodelock Solver"` and
  `tauri.windows[0].title = "Nodelock Solver"` (commit `c747c0d`); revert both to
  "Desktop Postflop" before merging upstream.
* Verification after shipping: launched the exe from an idle machine (no Vite/Node process) —
  window title `Nodelock Solver`, 1365x848, and OCR of the window returned the full start screen
  (`Desktop Postflop v0.2.7`, `Solver` / `Results`, OOP/IP range grids, `Tree Configuration`),
  i.e. the embedded UI really renders.

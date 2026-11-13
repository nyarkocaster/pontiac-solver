# Desktop Postflop — **node-locking fork**

> [!IMPORTANT]
> **This repository is a fork** of [b-inary/desktop-postflop] — upstream `v0.2.7` (`0c15f97`), whose development was suspended in October 2023 — maintained by [@tonyet] on the `nodelock` branch.
> It adds **the one feature upstream never shipped: node locking** (see [What's new in this fork](#whats-new-in-this-fork-node-locking) and [docs/NODELOCK.md](docs/NODELOCK.md)). The matching engine changes live in the fork [tonyet/postflop-solver@nodelock].
>
> There are **no prebuilt binaries** in this fork (the upstream *Download* link below installs upstream, i.e. **without** node locking) — build from source with the [Build](#build) section and note the fork-specific dependency step there.
> This fork keeps the upstream license (AGPL-3.0, see [LICENSE](LICENSE)).
>
> Everything below the *What's new* section is the upstream README, kept unchanged so the diff stays readable.

[b-inary/desktop-postflop]: https://github.com/b-inary/desktop-postflop
[@tonyet]: https://github.com/tonyet
[tonyet/postflop-solver@nodelock]: https://github.com/tonyet/postflop-solver/tree/nodelock

---

## What's new in this fork: node locking

Upstream listed *"Node-locking feature"* as a roadmap item and never implemented it. This fork does, engine + UI:

- **Hard lock** — fix a node's strategy to an exact per-hand frequency distribution and re-solve. The result is the optimal *restricted response* to the locked strategy.
- **Soft lock / RRN** — a weight in `0…1` blends the node's strategy with the solved one on every iteration (`weight × target + (1 − weight) × solved`), so the residual stays optimizable instead of being forced. `weight = 1` is a hard lock, `0` is unlocked.
- **Node frequency targets** — type a target frequency in percent per action (e.g. *check 60 % / bet 40 %*); the app distributes it over the node's hands proportionally to the current strategy.
- **Restricted vs. real metrics** — the convergence metric *inside* the locks (which still converges to ~0, so upstream's stopping criterion is preserved) is displayed next to the **real exploitability** of the profile you are looking at. The latter is ≥ 0 by construction: locking equilibrium frequencies changes the equilibrium (measured example: restricted `≈ 7.8e-5` vs unrestricted `≈ 1.52` chips).
- **Sensitivity sweep** — re-solves one parameter over 2–21 points in one click: soft-lock **weight** (`0 → 1` by default) of the selected node, or one action's **node frequency** (`0 → 100 %`). Each point reports restricted/real exploitability, EV OOP/IP (reach-weighted) and elapsed time, with break-even interpolation between points. Cancellable; runtime = points × solve time.
- Terminal and chance nodes are rejected with a readable message, and all lock commands save/restore your position in the tree.

### How to use it

1. **Solve a spot normally.** That run becomes the **baseline** (its metrics are cached).
2. Go to **Results** and walk the tree to the (villain) node you want to fix.
3. In the **Node Lock** panel at the bottom of the Results view:
   - tick **Enable node locking**,
   - edit the per-hand frequency grid (the *Lock* column), or use the quick actions **lock to current strategy**, **lock to pure strategy** and **remove an action** (0 %),
   - optionally set a **node frequency target (%)** per action,
   - optionally drag the **weight** slider to turn the lock into a soft lock.
4. Press **Apply locks & re-solve**. The whole pipeline is replayed (`init → allocate_memory → apply pending locks → solve → finalize`) because the engine only accepts locks after memory allocation and before the first solve step; the tree is rebuilt from the saved config and the pending locks are replayed on the fresh instance.
5. Compare **Locked vs. baseline** (restricted & real exploitability, root EV), and run a **sensitivity sweep** if you want to see how the solution reacts to one parameter.

Notes: the panel only renders once a solve result exists; locks are not serialized with the game (re-apply them after loading a saved game).

Full details — the added Tauri commands, frontend architecture, stopping criterion, verification checklist and known limitations — are in [docs/NODELOCK.md](docs/NODELOCK.md) (engine-side notes: [postflop-solver/docs/NODELOCK.md](https://github.com/tonyet/postflop-solver/blob/nodelock/docs/NODELOCK.md)).

---

# Desktop Postflop

> [!IMPORTANT]
> **As of October 2023, I have started developing a poker solver as a business and have decided to suspend development of this open-source project. See [this issue] for more information.**

[this issue]: https://github.com/b-inary/postflop-solver/issues/46

---

**Desktop Postflop** is a free, open-source GTO solver for Texas hold'em poker.

This is a port of [WASM Postflop] to a native desktop application using the [Tauri] framework.
Since WASM Postflop works on web browsers, it is more suitable for casual use.
However, if you want to use the solver for more serious purposes, please consider trying Desktop Postflop.

[WASM Postflop]: https://github.com/b-inary/wasm-postflop
[Tauri]: https://tauri.app/

**Related repositories**
- Solver engine: https://github.com/b-inary/postflop-solver

## Comparison to WASM Postflop

- **Shared features**:
  - Free to use and open-source
  - Same solver engine and user interface
- **Advantages**:
  - Faster computation
  - Able to use more than 4GB of memory
- **Disadvantages**:
  - Needs to download the program and trust the execution
  - macOS builds are not distributed (you need to build the app yourself)

See the [WASM Postflop repository] for more detailed comparisons, including some commercial solvers.

[WASM Postflop repository]: https://github.com/b-inary/wasm-postflop#comparison

## Supported environments

- OS
  - Windows: 10/11
  - macOS: 11.7 and later
    - We do not distribute macOS builds because we are not enrolled in the Apple Developer Program and cannot sign the app (please see the "Build" section below and build it yourself).
  - Linux: glibc 2.31 and later (e.g., Ubuntu 20.04 and later)
- CPU
  - x86-64: must support AVX2 instructions
    - Intel: Haswell (2013) and later
    - AMD: Zen (1st gen; 2017) and later
    - If you have a CPU without AVX2 support, you can modify `src-tauri/.cargo/config.toml` and build it yourself.
  - Apple silicon: M1 and later

## Download

> **Fork:** no binaries are published from this fork. The [GitHub releases page] below is the upstream one — it does **not** contain node locking. Build from source instead (see [Build](#build)).

You can download the app from the [GitHub releases page].

[GitHub releases page]: https://github.com/b-inary/desktop-postflop/releases

- Windows
  - The installer version (.msi) automatically installs dependent runtimes.
  - The portable version (.exe) requires the [WebView2 runtime] to be installed.
    - In most cases, the runtime should already be installed on Windows 10/11.
- Linux
  - AppImage should work on most Linux distributions.
  - Alternatively, the .deb package is available for Debian-based distributions, including Ubuntu.

[WebView2 runtime]: https://developer.microsoft.com/en-us/microsoft-edge/webview2/#download-section

## Uninstall

On Windows, please remove the following folder after the regular uninstallation to completely uninstall Desktop Postflop:

```
C:\Users\<username>\AppData\Local\b-inary.desktop-postflop
```

## Build

[Rust] and [Node.js] need to be installed to build.

**Fork note (build):** `src-tauri/Cargo.toml` in this branch points at the engine fork **by local path** (`postflop-solver = { path = ... }`). On a fresh clone, replace that line with the engine fork, which is where node locking actually lives:

```toml
postflop-solver = { git = "https://github.com/tonyet/postflop-solver", branch = "nodelock", features = ["custom-alloc"] }
```

Nightly Rust is required for `custom-alloc` (the engine uses `#![feature(allocator_api)]`), and both forks must be on their `nodelock` branch for the Node Lock panel to exist.
On Linux, you will also need to install some dependencies; please see the [Tauri documentation] for details.
For better performance, we also recommend installing the Rust nightly channel:

```sh
$ rustup install nightly
$ rustup default nightly
```

Then clone this reporitory and run the following commands:

```sh
$ npm install
$ npm run tauri build
```

If the build was successful, you should be able to find the application in the `src-tauri/target/release/bundle/` directory.

If you want to use stable Rust instead of nightly Rust, please modify the following line in `src-tauri/Cargo.toml` (performance will be sacrificed):

```diff
[dependencies]
...
- postflop-solver = { git = "https://github.com/b-inary/postflop-solver", features = ["custom-alloc"] }
+ postflop-solver = { git = "https://github.com/b-inary/postflop-solver" }
```

[Rust]: https://www.rust-lang.org/learn/get-started
[Node.js]: https://nodejs.org/en/
[Tauri documentation]: https://tauri.app/v1/guides/getting-started/prerequisites/#setting-up-linux

## ~~Roadmap (in order of priority)~~

- Results saving/loading feature ([#8](https://github.com/b-inary/desktop-postflop/issues/8))
- Hand filter feature for the result viewer ([#6](https://github.com/b-inary/desktop-postflop/issues/6))
- ~~Node-locking feature~~ — **implemented in this fork** (see *What's new in this fork* above)
- Short deck support
- Aggregated reporting feature for multiple flops
- GTO training mode ([#9](https://github.com/b-inary/desktop-postflop/issues/9))

## License

Copyright (C) 2022 Wataru Inariba

This program is free software: you can redistribute it and/or modify it under the terms of the GNU Affero General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.

This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU Affero General Public License for more details.

You should have received a copy of the GNU Affero General Public License along with this program.  If not, see <https://www.gnu.org/licenses/>.

"""Pontiac Solver, headless.

    import pontiac
    result = pontiac.solve("examples/river_quick.json", save="spot.pontiac")
    node = pontiac.query("spot.pontiac", "river/BB_check/SB_bet0.5", hand="Qc9c")

`solve` takes a spot (a dict or the path of a JSON file) and returns the result as a dict.
`query` reads any node of a saved solve without solving again. See docs/HEADLESS.md.
"""
import json
import os
import subprocess
import tempfile
import time
from pathlib import Path

from ._binary import binary

__version__ = "0.13.8"
__all__ = ["solve", "query", "estimate", "pushfold", "pushfold3", "preflop", "icm", "version", "SolverError", "binary",
           "example", "examples", "guide", "selftest"]


class SolverError(RuntimeError):
    """The solver rejected the request; the message says what to change."""


def _run(args, timeout=None, progress=False):
    proc = subprocess.run(
        [binary(), *args],
        stdout=subprocess.PIPE,
        stderr=None if progress else subprocess.DEVNULL,
        text=True,
        timeout=timeout,
    )
    try:
        data = json.loads(proc.stdout)
    except json.JSONDecodeError:
        raise SolverError(f"solver exited with code {proc.returncode} and no readable output") from None
    if isinstance(data, dict) and data.get("ok") is False:
        raise SolverError(data.get("error", "unknown error"))
    return data


def _spot_file(spot):
    """Returns (path, temporary?) for a spot given as a dict or a path."""
    if isinstance(spot, (str, os.PathLike)):
        return str(spot), False
    f = tempfile.NamedTemporaryFile("w", suffix=".json", delete=False)
    json.dump(spot, f)
    f.close()
    return f.name, True


def solve(spot, save=None, timeout=None, progress=False):
    """Solves a spot. `save`: file to keep the full solve in, for `query`.
    `timeout`: hard limit in seconds for the whole call (the spot's own `max_seconds`
    is the normal way to bound a solve; it keeps the result reached so far).
    `progress`: show the solver's progress lines."""
    path, temp = _spot_file(spot)
    try:
        args = ["solve", path]
        if save:
            args += ["--save", str(save)]
        return _run(args, timeout=timeout, progress=progress)
    finally:
        if temp:
            os.unlink(path)


def query(saved, node, hand=None, detail="class", normal=False):
    """One node of a saved solve. `node` is a path such as
    "flop/BB_check/SB_bet0.33/BB_call/turn=7h". `hand` narrows the answer to one
    exact hand ("AhKd") or one hand class ("AKs"). `detail="combo"` lists every exact hand.
    `normal=True`: for a three-player solve saved with a lock or habit, read the
    normal solve instead of the locked one."""
    args = ["query", str(saved), "--node", node, "--detail", detail]
    if hand:
        args += ["--hand", hand]
    if normal:
        args += ["--normal"]
    return _run(args)


def estimate(spot):
    """Memory needed for a spot, without solving it."""
    path, temp = _spot_file(spot)
    try:
        return _run(["estimate", path])
    finally:
        if temp:
            os.unlink(path)


def pushfold(stack_bb, small_blind=0.5, ante=0.0):
    """Heads-up push/fold equilibrium: the small blind moves all-in or folds, the big blind
    calls or folds. `stack_bb` is the shorter stack before the blinds are posted. Returns the
    two ranges as text (`jam_range`, `call_range`) and per hand class (`jam`, `call`, `classes`)."""
    return _run(["pushfold", "--stack", str(stack_bb), "--sb", str(small_blind), "--ante", str(ante)])


def preflop(stack_bb, open_sizes=(2.0,), limp=True, reraise=3.0, small_blind=0.5, ante=0.0, iterations=0, model=None):
    """Heads-up preflop with limps, raises, re-raises and all-ins. `stack_bb` is the shorter
    stack before blinds; `open_sizes` are the raise sizes the button may open to (big blinds).
    Returns `nodes` (every decision: `path`, `actions`, `freqs`, `strategy` per hand class),
    `lines` (each way to reach a flop, with both ranges for the postflop solver), EVs and
    `max_gain_bb`. All-in pots are exact; pots that see a flop are estimated (see `model_note`).
    `model` overrides the estimate: (position, initiative, playability, half_spr)."""
    args = ["preflop", "--stack", str(stack_bb), "--open", ",".join(str(x) for x in open_sizes),
            "--reraise", str(reraise), "--sb", str(small_blind), "--ante", str(ante)]
    if not limp:
        args.append("--no-limp")
    if iterations:
        args += ["--iterations", str(int(iterations))]
    if model:
        args += ["--model", ",".join(str(x) for x in model)]
    return _run(args)


def pushfold3(stacks_bb, payouts=None, others_bb=None, small_blind=0.5, ante=0.0, bb_ante=0.0, iterations=200):
    """Three-handed push/fold. `stacks_bb` is (button, small blind, big blind) before antes and
    blinds. With `payouts` (first prize first) players play for prize money (ICM) instead of chips;
    `others_bb` lists stacks of players still in the tournament but not in the hand. Returns six
    decisions (`btn_jam`, `sb_call`, `sb_jam`, `bb_call_btn`, `bb_call_sb`, `bb_call_both`), each
    with `pct`, `range`, and `freq` / `edge` per hand class."""
    join = lambda xs: ",".join(str(x) for x in (xs or []))
    args = ["pushfold", "--stacks", join(stacks_bb), "--sb", str(small_blind), "--ante", str(ante),
            "--bb-ante", str(bb_ante), "--iterations", str(iterations)]
    if payouts:
        args += ["--payouts", join(payouts)]
    if others_bb:
        args += ["--others", join(others_bb)]
    return _run(args)


def icm(stacks, payouts):
    """Prize-money value of tournament stacks (Independent Chip Model). Returns `prize_value`,
    `chip_share` and `prize_share`, one entry per stack."""
    return _run(["icm", "--stacks", ",".join(str(x) for x in stacks), "--payouts", ",".join(str(x) for x in payouts)])


def version():
    """Version string of the solver program; the same string is in every result."""
    return subprocess.run([binary(), "version"], capture_output=True, text=True).stdout.strip()


# ---- shipped with the package: sample spots, the interface guide, a self-test ----

_DATA = Path(__file__).resolve().parent


def examples():
    """Names of the sample spots shipped with the package."""
    return sorted(p.stem for p in (_DATA / "examples").glob("*.json") if not p.stem.endswith(".expected"))


def example(name):
    """A sample spot as a dict, ready to edit and pass to `solve`. See `examples()` for the names."""
    path = _DATA / "examples" / f"{name}.json"
    if not path.exists():
        raise SolverError(f"no example called {name!r}; choose from {', '.join(examples())}")
    return json.loads(path.read_text())


def guide():
    """The interface reference (docs/HEADLESS.md) as text."""
    return (_DATA / "HEADLESS.md").read_text()


def selftest(full=False):
    """Checks that the solver runs correctly on this machine. Takes a few seconds; `full=True`
    adds a three-player solve and prize-money solves (under a minute on two cores).
    Returns {"ok": bool, "version": str, "checks": [{"name", "ok", "seconds", "detail"}]}."""
    checks = []

    def run(name, fn):
        t = time.time()
        try:
            detail = fn()
            checks.append({"name": name, "ok": True, "seconds": round(time.time() - t, 2), "detail": detail})
        except Exception as e:  # report every failure, never raise
            checks.append({"name": name, "ok": False, "seconds": round(time.time() - t, 2), "detail": f"{type(e).__name__}: {e}"})

    def near(a, b, tol, what):
        if abs(a - b) > tol:
            raise AssertionError(f"{what}: got {a}, expected {b}")

    def river():
        r = solve(example("river_quick"))
        want = json.loads((_DATA / "examples" / "river_quick.expected.json").read_text())
        for name, ev in want["ev_bb"].items():
            near(r["ev_bb"][name], ev, 0.02, f"EV of {name}")
        return f"EVs {r['ev_bb']}, {r['exploitability_pct']}% of the pot"

    def save_and_query():
        with tempfile.TemporaryDirectory() as d:
            path = os.path.join(d, "spot.pontiac")
            r = solve(example("river_quick"), save=path)
            first = next(iter(r["nodes"]))
            q = query(path, first)
            near(q["range_ev_bb"], r["nodes"][first]["range_ev_bb"], 0.001, "queried EV")
        return f"node {first!r} read back from the saved solve"

    def push_fold():
        r = pushfold(10)
        near(r["jam_pct"], 0.583, 0.01, "10 bb heads-up all-in share")
        return "10 bb heads-up: 58% all-in"

    def prizes():
        v = icm([5000, 3000, 2000], [50, 30, 20])
        near(float(v["prize_value"][0]), 38.39, 0.02, "ICM value of the big stack")
        r = pushfold3([10, 10, 10], payouts=[50, 30, 20])
        near(r["decisions"][0]["pct"], 0.271, 0.01, "three-handed button all-in share")
        return "ICM 38.39 / 32.75 / 28.86; three-handed button all-in 27%"

    def three_players():
        spot = example("three_way_river")
        with tempfile.TemporaryDirectory() as d:
            path = os.path.join(d, "three.pontiac")
            r = solve(spot, save=path)
            if r["exploitability_pct"] > spot["target_exploitability_pct"]:
                raise AssertionError(f"reached only {r['exploitability_pct']}%")
            first = next(iter(r["nodes"]))
            q = query(path, first)
            near(q["range_ev_bb"], r["nodes"][first]["range_ev_bb"], 0.001, "queried EV")
        return f"EVs {r['ev_bb']}; saved and read back"

    def prize_money_postflop():
        spot = example("icm_bubble_river")
        r = solve(spot)
        if not sum(r["ev_bb"].values()) < spot["pot_bb"]:
            raise AssertionError("prize-money EVs should add up to less than the pot")
        return f"prize pool shares {r['icm']['prize_pool_share_pct']}"

    try:
        ver = version()
    except Exception as e:
        return {"ok": False, "version": None, "checks": [{"name": "program runs", "ok": False, "seconds": 0, "detail": str(e)}]}
    run("two-player river solve matches the stored answer", river)
    run("save and query", save_and_query)
    run("heads-up push/fold", push_fold)
    run("prize-money (ICM) values and three-handed push/fold", prizes)
    if full:
        run("three-player river solve", three_players)
        run("prize-money postflop solve", prize_money_postflop)
    return {"ok": all(c["ok"] for c in checks), "version": ver, "checks": checks}

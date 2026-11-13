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

from ._binary import binary

__version__ = "0.5.0"
__all__ = ["solve", "query", "estimate", "pushfold", "version", "SolverError", "binary"]


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


def query(saved, node, hand=None, detail="class"):
    """One node of a saved solve. `node` is a path such as
    "flop/BB_check/SB_bet0.33/BB_call/turn=7h". `hand` narrows the answer to one
    exact hand ("AhKd") or one hand class ("AKs"). `detail="combo"` lists every exact hand."""
    args = ["query", str(saved), "--node", node, "--detail", detail]
    if hand:
        args += ["--hand", hand]
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


def version():
    """Version string of the solver program; the same string is in every result."""
    return subprocess.run([binary(), "version"], capture_output=True, text=True).stdout.strip()

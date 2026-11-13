"""Smoke tests: a river spot solves in well under a second, so these run anywhere."""
import json
from pathlib import Path

import pytest

import pontiac

SPOT = json.loads((Path(__file__).parent.parent / "examples" / "river_quick.json").read_text())
NODE = "river/BB_check/SB_bet0.5"


def freqs(node):
    return {a["name"]: a["frequency"] for a in node["actions"]}


def test_version():
    assert pontiac.version().startswith("pontiac ")


def test_solve_reaches_target_and_evs_add_up(tmp_path):
    save = tmp_path / "spot.pontiac"
    r = pontiac.solve(SPOT, save=save)
    assert r["version"] == pontiac.version()
    assert r["out_of_position"] == "BB" and r["in_position"] == "SB"
    assert r["exploitability_pct"] <= SPOT["target_exploitability_pct"]
    assert r["stopped_because"] == "target_reached"
    assert abs(sum(r["ev_bb"].values()) - SPOT["pot_bb"]) < 0.05
    root = r["nodes"]["river"]
    assert root["player"] == "BB" and abs(sum(freqs(root).values()) - 1) < 1e-3
    assert "by_class" in root and "by_combo" in root
    # the saved solve answers the same question without solving again
    q = pontiac.query(save, NODE, hand="Q9s")
    assert q["player"] == "BB" and set(q["by_combo"]) == {"Qc9c", "Qd9d", "Qs9s"}
    assert q["by_class"]["Q9s"]["freqs"] == r["nodes"][NODE]["by_class"]["Q9s"]["freqs"]
    one = pontiac.query(save, NODE, hand="9cQc")
    assert list(one["by_combo"]) == ["Qc9c"]


def test_same_spot_gives_same_answer():
    a, b = pontiac.solve(SPOT), pontiac.solve(SPOT)
    assert a["ev_bb"] == b["ev_bb"] and a["iterations"] == b["iterations"]


def test_lock_every_action_for_some_hands():
    lock = {"node": NODE, "player": "BB", "lock": {"hands": "Q9s,Q8s,Q7s", "action_freqs": {"fold": 0, "call": 1, "allin": 0}}}
    r = pontiac.solve({**SPOT, "node_locks": [lock]})
    n = r["nodes"][NODE]
    assert "normal" not in r and n["locked"] and r["locked_decisions"] == 1
    assert n["by_class"]["Q9s"]["freqs"]["call"] == 1.0
    assert n["by_class"]["TT"]["freqs"]["call"] < 1.0  # hands outside the lock stay free


def test_partial_lock_keeps_normal_proportions():
    lock = {"node": NODE, "player": "BB", "lock": {"action_freqs": {"allin": 0.0}}}
    r = pontiac.solve({**SPOT, "node_locks": [lock]})
    assert "normal" in r  # a normal solve ran first
    assert freqs(r["nodes"][NODE])["allin"] == 0.0
    assert r["ev_bb"]["BB"] <= r["normal"]["ev_bb"]["BB"] + 0.02  # giving up an option cannot help


def test_habit():
    r = pontiac.solve({**SPOT, "habits": [{"player": "BB", "effect": "fold_less", "strength": 0.6}]})
    assert r["locked_decisions"] > 0
    assert r["ev_bb"]["SB"] > r["normal"]["ev_bb"]["SB"]  # the caller pays for it


def test_errors_are_readable():
    with pytest.raises(pontiac.SolverError, match="not an action"):
        pontiac.solve({**SPOT, "node_locks": [{"node": NODE, "lock": {"action_freqs": {"limp": 1}}}]})
    with pytest.raises(pontiac.SolverError, match="BB's turn"):
        pontiac.solve({**SPOT, "report_nodes": ["river/SB_check"]})
    with pytest.raises(pontiac.SolverError, match="ranges.BB"):
        pontiac.solve({**SPOT, "ranges": {"SB": "AA"}})

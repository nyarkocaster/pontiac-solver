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


# ---- three players ----

THREE = json.loads((Path(__file__).parent.parent / "examples" / "three_way_river.json").read_text())


def test_three_players_unequal_stacks_and_rake():
    r = pontiac.solve(THREE)
    assert r["acting_order"] == ["BB", "CO", "BTN"]
    assert r["exploitability_pct"] <= THREE["target_exploitability_pct"]
    total = sum(r["ev_bb"].values())
    # rake comes out of the pot: 5% capped at 1.5 bb
    assert THREE["pot_bb"] - 1.5 - 0.05 <= total < THREE["pot_bb"]
    root = r["nodes"]["river"]
    assert root["player"] == "BB" and abs(sum(freqs(root).values()) - 1) < 1e-3
    assert root["stacks_bb"] == {"BB": 40.0, "CO": 100.0, "BTN": 65.0}
    # the shortest stack can never put in more than it has
    assert all(a["amount_bb"] <= 40.0 + 1e-6 for a in root["actions"])


def test_three_players_node_lock_is_obeyed():
    spot = dict(THREE, node_locks=[{"node": "river", "player": "BB", "action_freqs": {"check": 1.0}}])
    r = pontiac.solve(spot)
    assert r["locked_decisions"] >= 1
    assert freqs(r["nodes"]["river"])["check"] == pytest.approx(1.0, abs=1e-4)


def test_three_players_habit_reports_normal_play_too():
    spot = dict(THREE, habits=[{"player": "BTN", "street": "river", "effect": "fold_more", "strength": 0.5}])
    r = pontiac.solve(spot)
    assert "normal" in r and r["gain_pct_by_player"]["BTN"] == 0.0
    # the others profit from the mistake
    assert r["ev_bb"]["BTN"] < r["normal"]["ev_bb"]["BTN"]


# ---- push/fold ----

def test_pushfold_matches_published_ranges():
    r = pontiac.pushfold(10)
    assert r["jam_pct"] == pytest.approx(0.58, abs=0.03)
    assert r["call_pct"] == pytest.approx(0.37, abs=0.03)
    jam = dict(zip(r["classes"], r["jam"]))
    call = dict(zip(r["classes"], r["call"]))
    assert jam["AA"] == 1 and jam["22"] == 1 and jam["72o"] == 0
    assert call["A2o"] == 1 and call["32o"] == 0
    assert r["ev_small_blind_bb"] == pytest.approx(-r["ev_big_blind_bb"])
    # shorter stacks jam wider
    assert pontiac.pushfold(5)["jam_pct"] > r["jam_pct"] > pontiac.pushfold(20)["jam_pct"]


def test_pushfold_rejects_nonsense():
    with pytest.raises(pontiac.SolverError):
        pontiac.pushfold(0.5)


# ---- the two engines against each other ----

ALL_HANDS = ("22+,A2s+,K2s+,Q2s+,J2s+,T2s+,92s+,82s+,72s+,62s+,52s+,42s+,32s,"
             "A2o+,K2o+,Q2o+,J2o+,T2o+,92o+,82o+,72o+,62o+,52o+,42o+,32o")


@pytest.mark.parametrize("board,street", [("Qh8c3h5d2s", "river"), ("Qh8c3h5d", "turn")])
def test_three_player_engine_agrees_with_two_player_engine(board, street):
    """A three-way pot where one player has already folded is a heads-up pot. The three-player
    engine (written for this project) and the two-player engine (postflop-solver) share no code,
    so the same answer from both is an independent check on each."""
    sb = "QQ,88,33,AQs-A9s,KQs,QJs,QTs,AQo,KQo,QJo,A8s,98s,JTs,T9s,76s,65s,55,44,KTs,K9s"
    bb = "Q9s,Q8s,Q7s,Q9o,KQo:0.5,A8s,K8s,98s,87s,A3s,JJ:0.25,TT,99,77,66,A5s,A4s,64s,54s,KJo,J9s"
    streets = {s: {"bet": [0.5], "raise": [2.5]} for s in ("turn", "river")}
    two = pontiac.solve({
        "players": ["SB", "BB"], "effective_stack_bb": 30, "pot_bb": 12, "board": board,
        "ranges": {"SB": sb, "BB": bb}, "bet_tree": dict(streets, allin_threshold=0),
        "target_exploitability_pct": 0.05, "max_seconds": 120})
    three = pontiac.solve({
        "players": ["BB", "CO", "BTN"], "folded": ["BTN"], "effective_stack_bb": 30, "pot_bb": 12, "board": board,
        # the folded player's cards are unknown, so they hold every hand equally
        "ranges": {"BB": bb, "CO": sb, "BTN": ALL_HANDS},
        "bet_tree": dict(streets, max_raises=3, allin_spr=0.0), "later_cards": "exact",
        "target_exploitability_pct": 0.05, "max_seconds": 300})
    assert sorted(k.replace("SB_", "CO_") for k in two["nodes"]) == sorted(three["nodes"])
    assert three["ev_bb"]["BTN"] == 0
    assert three["ev_bb"]["BB"] == pytest.approx(two["ev_bb"]["BB"], abs=0.02)
    assert three["ev_bb"]["CO"] == pytest.approx(two["ev_bb"]["SB"], abs=0.02)

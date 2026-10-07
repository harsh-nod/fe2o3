#!/usr/bin/env python3
"""Recover exact old diagnostic inputs; reconstruction is not proof execution."""
import hashlib
import json
from pathlib import Path

BASE = Path(__file__).resolve().parent
DELTA_SHA = "6842b7ca3234bb23ab0f859eb25d607fbc1a65994c9bd963a626508543a16a95"
V = Path("crates/fe2o3-runtime-model/verus")
NEW_INPUTS = {
    V / "context_live_validation_definitions_v1.rs",
    Path("crates/fe2o3-runtime/src/context/versions/live_validation_bodies.rs"),
    Path("crates/fe2o3-runtime/src/context/versions/live_validation_declarations.rs"),
}
LIVE_FIXTURE_EOF = {
    V / "producer_input_runtime_declarations_v1.rs": (
        "1c2dd60498287eedb995425e5b0b7eb1233aa8e7a0ae1c52909618e73a320810",
        "a51dd73602950f3e49bdca860208eb371e735c84bf239042ec03c97950b9b71c"),
    V / "producer_input_journal_comparison_declarations_v1.rs": (
        "2855aafb7e2763d739c4d9c9a80cb24aadb67bf76a40f1eea38eb3c2d5f7b964",
        "5b1ddfe8dea991d6e3a17d98da8774746913e5fc6e65153e4901d6a2d1cee3eb"),
    V / "producer_input_composition_logic_v1.rs": (
        "9b352a9b980994c08d059fdedb554f2cd9824d1a064bd6aff21ff053582d0bc0",
        "8d2b020e5fd562ace00abd9cd4bb9459eb12410fde6f8e6b51cc2cac19c9214f"),
}


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(text):
    return hashlib.sha256(text.encode()).hexdigest()


def source(path, text):
    raw = (BASE / "producer-live-predecessor-v1.json").read_bytes()
    need(hashlib.sha256(raw).hexdigest() == DELTA_SHA, "exact reviewed reversible historical delta")
    delta = json.loads(raw)["files"].get(str(path))
    if delta is None:
        return text
    need(sha(text) == delta["current_sha256"], "current source matches exact extraction input")
    previous = len(text)
    for change in reversed(delta["edits"]):
        start, end = change["start"], change["end"]
        need(type(start) is int and type(end) is int and 0 <= start <= end <= previous,
             "ordered disjoint exact historical source ranges")
        text = text[:start] + change["replacement"] + text[end:]
        previous = start
    need(sha(text) == delta["predecessor_sha256"], "exact historical source bytes recovered")
    return text


def parts(values):
    return {name: source(V / name, text) for name, text in values.items()}


def live_fixture_source(path, text):
    """Restore only reviewed EOF bytes of later live diagnostic captures."""
    pair = LIVE_FIXTURE_EOF.get(path)
    if pair is None:
        return text
    need(sha(text) == pair[0], "exact current EOF-only fixture source")
    result = text + "\n"
    need(sha(result) == pair[1], "exact captured EOF-only fixture source")
    return result


def historical_cases(builder, rows, sources, leaf):
    prior = {path: source(path, text) for path, text in sources.items() if path not in NEW_INPUTS}
    historical = {}
    for key, row in rows.items():
        old = prior[row["path"]]
        before = row["before"].replace("        #[verifier::spinoff_prover]\n", "")
        after = row["after"].replace("        #[verifier::spinoff_prover]\n", "")
        text = builder.replacement(old, before, after)
        historical[key] = {**row, "text": text, "original_sha256": sha(old),
            "before": before, "after": after,
            "closure": tuple(path for path in row["closure"] if path not in NEW_INPUTS),
            "implementation_span": builder.span(old, row["implementation_scope"], row["implementation_anchor"], leaf)}
    need(len(historical) == 89, "exact historical 89-case roster, never current live controls")
    return historical, prior

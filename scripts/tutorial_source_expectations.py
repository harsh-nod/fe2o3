"""Declared source-driver outcomes, never observed execution or authority."""

from __future__ import annotations

from collections.abc import Callable
from typing import Any


DIAGNOSTIC_KIND = "diagnostic-kir-export-v1"
DIAGNOSTIC_ORDERS = ("blocked", "striped")


def validate_source_expectation(
    value: Any, label: str, *,
    require_object: Callable[[Any, str], dict[str, Any]],
    require_string: Callable[[Any, str], str],
    require_exact_keys: Callable[[dict[str, Any], set[str], str], None],
    fail: Callable[[str], None],
) -> None:
    """Validate an expectation without upgrading its source or execution status.

    Diagnostic V18 is raw canonical KIR, not a simulation bundle. Its explicit
    distribution describes an observation, not cross-distribution equivalence.
    Source authentication, proof, native, artifact, load, launch, hardware and
    performance authority are not granted by any accepted diagnostic contract.
    """
    expectation = require_object(value, f"{label}.expectation")
    kind = require_string(expectation.get("kind"), f"{label}.expectation.kind")
    if kind == DIAGNOSTIC_KIND:
        require_exact_keys(expectation, {
            "kind", "canonicalKirVersion", "diagnosticTileOrders", "authority",
        }, f"{label}.expectation")
        version = expectation["canonicalKirVersion"]
        if type(version) is not int or version != 18:
            fail(f"{label} has an unsupported diagnostic canonical KIR version")
        orders = expectation["diagnosticTileOrders"]
        if (not isinstance(orders, list) or not 1 <= len(orders) <= len(DIAGNOSTIC_ORDERS)
                or any(type(order) is not str or order not in DIAGNOSTIC_ORDERS for order in orders)
                or orders != [order for order in DIAGNOSTIC_ORDERS if order in orders]):
            fail(f"{label} requires unique diagnostic tile orders in canonical order")
        if expectation["authority"] != "observation_only":
            fail(f"{label} requires observation-only diagnostic authority")
        return
    if kind not in {"verified-bundle-export", "rejected"}:
        fail(f"{label} has an unsupported expectation")
    keys = {"kind", "bundleVersion"} | (
        {"diagnosticContains", "outputArtifact"} if kind == "rejected" else set())
    require_exact_keys(expectation, keys, f"{label}.expectation")
    version = expectation["bundleVersion"]
    if type(version) is not int or not 1 <= version <= 6:
        fail(f"{label} has an unsupported bundle version")
    if kind == "rejected":
        diagnostic = require_string(expectation["diagnosticContains"], f"{label}.diagnosticContains")
        if len(diagnostic) > 512 or expectation["outputArtifact"] != "absent":
            fail(f"{label} requires an exact refusal and absent artifact")

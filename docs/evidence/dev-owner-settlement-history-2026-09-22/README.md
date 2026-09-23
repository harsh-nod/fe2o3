# Actual-Owner Settlement Historical Correspondence

This developer checkpoint connects the shared public settlement methods to the
independent historical journal executor. It extends the
[raw settlement checkpoint](../dev-owner-settlement-2026-09-22/README.md).

## Scope

- Both executors run independently. The bridge requires represented initial
  contents and projected arguments, not successful admission or valid custody.
- Rejection preserves the complete actual and historical owner values.
- Success preserves the stable-read and producer-reservation owner fields.
  Initial producer invariants and issued custody are preserved conditionally.
- Previously valid retained producer references remain lookup-valid. Pending
  reads for the settled writer resolve to Success or NoEffect; other valid
  statuses are unchanged. This is not error preservation for arbitrary invalid
  requests.
- Synthetic-start witnesses execute paired batch enrollment, registration,
  Begin, producer acquisition, stable acquisition, settlement and subsequent
  producer/stable queries. They cover both outcomes, evidence mismatch,
  insufficient return capacity, and replay. A separate raw witness deliberately
  starts outside valid producer custody.

The historical projection contains 37 declarations copied exactly from three
frozen roots. The runner authenticates their full text and the new include
envelope, pins inherited inputs to `4700057ca993062dc58cdfbb15008c00f25cc65b`,
and brackets input hashes and the runtime/Cargo source-discovery roster.

## Qualification

Source: `8ff8efee5d24bd6bbf7c5ef03ed3e542c214387b` (also recorded in `SOURCE`).
The complete campaign authenticated 359 inputs and recorded 26 terminal commands.

- Historical whole root: **916 verified, zero errors**, before and after controls.
- All **16 negative controls** produced scoped logical verification failures.
- Unchanged standalone raw root: **306 verified, zero errors**.
- CPU: **956 unit tests and 27 doctests passed; 18 tests ignored**.
- Formatting, warnings-denied Clippy and release test build passed.
- Pinned Verus closure matched before/after: 190 files, 129,019,839 bytes.
- Input hashes and the runtime/Cargo discovery roster remained unchanged.

The campaign runs whole-root historical positives before and after 16 scoped
negative controls, plus an unchanged raw-root regression. Controls retain the
11 raw-wrapper checks and add skipped actual/historical execution, mismatched
historical outcome/capacity, and an incorrect resolved-status law. Each negative
must fail logically, not by syntax error, timeout, or solver resource exhaustion.
CPU checks cover unit tests, doctests, formatting, warnings-denied Clippy and a
release test build.

Run `verus/check-owner-settlement-historical.py` from the `SOURCE` checkout;
its required arguments select the repository, pinned Verus binary, new output
directory and owned Cargo target directory. The packet auditor supports
`--repo <git-repository> --selftest` and works from Git objects without a source
checkout. It checks record integrity and exact commands/results, not execution
attestation or commit signatures, and does not rerun Verus.
Its 15 rehashed-corruption selftests include substituted proof roots, omitted
historical inputs, altered result types/counts, changed scope and receipt order.

## Limits

This is normal-state content correspondence. Initial actual construction is
synthetic; observed capacities are parameters, not physical Vec refinement.
Fallible allocation, unwinding, complete wrapper/trait coverage and native
pending-consumer admission remain open. No GPU, XGMI, HIP/HSA timing or parity
claim follows from these CPU/proof records. The CPU environment is not a
hermetic build attestation. Negative diagnostics are recorded observations,
not precommitted diagnostic replay.

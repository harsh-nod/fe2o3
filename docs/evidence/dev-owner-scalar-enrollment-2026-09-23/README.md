# Public Scalar Enrollment Correspondence

This developer checkpoint connects scalar allocation enrollment across the
journal, stable-read owner and producer-read owner to an independent logical
executor. It extends the [settlement checkpoint](../dev-owner-settlement-history-2026-09-22/README.md).

## Scope

- The ordinary Rust and actual-owner Verus methods instantiate the same bodies.
  Frozen Rust baselines come from `376a60343c906313fee87d1c78744bda662cb248`,
  with only authenticated name/visibility changes and prescribed formatting.
- Scalar admission keeps its own error order: allocation context, allocation ID,
  device context, device ID, extent, replay scan, free-stack exhaustion and
  selected-slot vacancy. It is not replaced by singleton batch admission.
- Raw correspondence requires only represented contents and mapped inputs.
  It accepts the existing malformed-state domain, including logical-capacity
  mismatch and aliased or invalid unused free prefixes.
- Both executors run independently. The logical scalar executor is new, not a
  frozen historical projection. Only successful scalar updates are specialized
  to existing singleton enrollment preservation lemmas.
- Rejection preserves the complete owner. Success changes one allocation slot
  and removes the free-stack tail, preserving all other owner fields.
  Initial producer invariants and issued custody are preserved conditionally.
- Previously valid producer statuses are preserved; arbitrary invalid-request
  errors need not remain unchanged when a new allocation becomes available.
- Synthetic-start executable witnesses cover live stable/producer reads,
  enrollment, subsequent lookups, replay and exhausted storage, plus success
  outside initial custody with malformed reader metadata and an aliased prefix.

The ten added CPU differential tests execute frozen and candidate methods on
the same owner allocation, with touched-only restoration, complete state
comparisons and storage-identity checks. They include all four producer statuses,
retirement reuse, empty physical arenas, malformed private state and large
descriptors. Indexed-access counts remain: header rejection 0; first replay at
slot j, j + 1; empty free stack A + 1; invalid selected slot A + 2; success A + 4,
where A is the actual allocation arena length. These are work counts, not timing
or HIP/HSA comparisons.

## Qualification

Source: `de34cb456cbf0bfd21f006b04ce77f0219b24177` (also recorded in `SOURCE`).
The source-bound campaign completed with 373 authenticated input files:

- Paired whole-root proofs: 929 verified, zero errors, before and after controls.
- All 18 scoped negative controls failed logically as intended, without frontend errors.
- Raw-owner regression: 310 verified, zero errors.
- Historical-settlement regression: 916 verified, zero errors.
- CPU suite: 966 unit tests and 27 doctests passed; 18 unit tests intentionally ignored.
- Formatting, warnings-denied all-target Clippy and release test build passed.
- Recorder tests passed, including 29 malformed or conflicting operation rejections.
- Both pinned Verus distribution checks matched: 190 files, 129019839 bytes.

The thirty serial commands have terminal receipts and controller acceptance.
Bounded resume retained completed records; a final complete-campaign replay left
the entire record tree byte-identical. Raw stdout/stderr are packaged unchanged.
The packet contains 94 record files and 98 files overall; `SHA256SUMS` covers the
other 97 files. No exploratory or interrupted command is counted as qualification.

## Replay

Run `crates/fe2o3-runtime-model/verus/check-owner-scalar-enrollment.py` from the
`SOURCE` checkout with `--repo`, `--verus`, `--output` and `--target`; use a fresh
output directory and owned Cargo target directory. The recorder authenticates
the frozen methods, exact runtime adapters, include envelopes and closed source
discovery roster, and brackets source hashes and the pinned Verus distribution.

Long campaigns may use `--stop-after <case>` and then `--resume` with the same
arguments and source checkout. An incomplete checkpoint is not qualification.
The recorder holds an exclusive controller lock, checks the entire saved prefix
before launching anything, and reuses only exact-source, controller-accepted
records. `accepted.json` binds each completed command's three raw files after
normal process cleanup and semantic validation. Interrupted or unaccepted
records require inspection; they are not automatically retried or discarded.
This marker records the controller's acceptance, not independent execution
attestation. All thirty commands must complete before final summaries exist.

The packet auditor accepts `--repo <git-repository> --selftest` and reads source
from Git objects without needing a source checkout. It checks integrity, source
identity, exact command scopes/results and terminal receipts. It does not rerun
Verus, independently attest execution, or verify commit signatures. Its 22
rehashed-corruption selftests include substituted roots, omitted model input,
changed result types/counts, incorrect scopes, receipt order, controller acceptance,
verifier metadata and CPU transcripts. These mutations rehash both the packet
and controller records where applicable, so semantic checks remain exercised.

## Limits

This is normal-state content correspondence, not physical Vec refinement.
Initial actual construction in witnesses is synthetic. Fallible construction,
allocation/unwind behavior, complete trait/wrapper coverage, retirement/disposal
and native pending-consumer admission remain open. No GPU, XGMI, HIP/HSA timing
or parity claim follows from these records. The CPU environment is not a
hermetic build attestation. Negative diagnostics are recorded observations,
not precommitted diagnostic replay.

# Unknown Disposal Correspondence

This developer checkpoint connects Unknown-writer disposal across the journal,
stable-read owner and producer-read owner to an independent logical executor.
It extends the [allocation retirement checkpoint](../dev-owner-retirement-2026-09-23/README.md).

## Scope

- Ordinary Rust and actual-owner Verus methods instantiate the same eight bodies:
  journal planning, scratch checking, staging, commit, validation and execution,
  plus both outer wrappers. Frozen CPU baselines come from
  `c0f0766d3fb3c5a588dcfb1a6c1355f9efb73cb8`; only method names, visibility,
  redirected baseline calls and prescribed formatting differ.
- Journal planning preserves Unknown-only admission, roster length before the
  complete chain check, and the complete chain check before descriptor checks.
  All three checked return-count additions precede lazy logical/physical
  headroom checks. Scratch vacancy is checked last and only over the prefix.
- Journal execution checks the retained writer before exact evidence identity,
  including writer kind. Outer wrappers retain unread-before-child precedence
  and repeated validation. Empty chains still require writer-return headroom.
- Actual and logical methods execute independently. The logical executor does
  not invoke shared Rust bodies or production helpers. Raw journal contracts
  are total over represented normal contents; outer execution requires only
  reached-prefix count/index/arithmetic safety, not global custody.
- Rejection preserves complete owner contents. Success removes the exact chain's
  allocations and members, appends their slots in chain order, then removes and
  returns the writer. Execution restores the scratch prefix and leaves any dirty
  suffix unchanged. All unrelated fields retain their values.
- Producer invariants and issuance history are preserved conditionally on their
  initial validity. Under the initial producer invariant, stable-lease and
  producer-reservation lookups are preserved for all reference arguments.
  Producer status preservation covers requests held by pre-existing live
  reservations, not arbitrary unretained requests to disposed allocations.
- Synthetic-start executable witnesses cover live reads, busy and evidence
  rejection, empty disposal, and raw success with malformed unrelated metadata.
  A paired lifecycle settles Success/NoEffect, reuses the old writer slot for a
  different allocation, marks that writer Unknown and disposes it while retaining
  the original reservation's status and lookup results.

Sixteen added CPU tests comprise fifteen frozen-versus-candidate comparisons and
one lazy-capacity observation probe. Differential tests run on the same physical
owner, restoring only touched contents after baseline success; complete contents
and storage identity are checked. Coverage includes both writer kinds,
all four writer-reference coordinates, all descriptor coordinates at every
position, simultaneous faults, nonmonotone physical slots, logical and physical
headroom, lazy capacity observations, scratch boundaries, retained producer
statuses and malformed unreached reader storage. Roster sizes include 0 and 4097.

For a healthy Unknown chain of k members, instrumented validation/disposal
counts are 1+3k / 4+4k for the journal, 1+5k / 5+9k for the stable owner, and
1+7k / 6+16k for the producer owner. These match the frozen baseline. They count
selected indexed accesses, not elapsed time or total work, and imply no HIP/HSA
performance comparison.

## Qualification

Source: `2cfff18d91c1e0b182adc974bceee67b45c1bd05` (also recorded in `SOURCE`).
The source-bound campaign completed with 405 authenticated input files:

- Paired whole-root proofs: 1,085 verified, zero errors, before and after controls.
- All 37 scoped mutations failed logically as intended, without frontend errors,
  timeouts or solver resource exhaustion.
- Raw-owner regression: 356 verified, zero errors.
- Allocation-retirement regression: 999 verified, zero errors.
- CPU suite: 996 unit tests and 27 doctests passed; 18 unit tests were ignored.
- Formatting, warnings-denied all-target Clippy and the optimized test build passed.
- Recorder tests rejected 29 malformed/conflicting operations, eight malformed
  retirement diagnostics, and 19 malformed disposal diagnostic/independence/source
  cases. All 37 mutation anchors were authenticated.
- Both pinned Verus distribution checks matched: 190 files, 129019839 bytes.

All 49 serial commands have terminal receipts and controller acceptance. Complete
campaign replay left all 151 record files byte-identical. Raw stdout/stderr are
packaged unchanged. The packet contains 151 record files and
155 files overall; `SHA256SUMS` covers the other 154. No exploratory or interrupted
command is counted as qualification.

## Replay

Run `crates/fe2o3-runtime-model/verus/check-owner-disposal.py` from the `SOURCE`
checkout with `--repo`, `--verus`, `--output` and `--target`, using fresh output
and owned Cargo target directories. The recorder authenticates frozen methods,
exact runtime adapters, the explicitly pinned roadmap, include envelopes,
independent logical execution and the closed source discovery roster. It brackets
source hashes and the pinned Verus distribution.

Use `--stop-after <case>` and `--resume` with the same source and arguments for
bounded checkpoints. An incomplete prefix is not qualification. Resume accepts
only exact-source, controller-accepted records under an exclusive controller
lock. Interrupted or unaccepted commands are not silently retried or discarded.
Controller acceptance records normal terminal cleanup and semantic validation;
it is not independent execution attestation.

The packet auditor reads source from Git objects with `--repo <git-repository>`;
`--selftest` additionally exercises rehashed-corruption controls. It checks record
integrity and semantics, but does not rerun Verus or verify commit signatures.
Its 32 rehashed packet-corruption controls cover source identity/roster, proof
scope and results, terminal receipts, controller acceptance, diagnostic policy,
CPU inventory and artifact closure. Two roster controls preserve cardinality
and require the exact roster rejection, rather than merely failing a count check.
A separate source-document mutation must fail the explicit document pin.
The auditor and all corruption selftests passed against both the working
repository and a bare Git repository without a source checkout.
Historical execution, staged mutation bytes, environment and process-group
disappearance remain recorder observations, not independent auditor attestations.

## Limits

This is normal-return content correspondence, not physical Vec refinement.
Physical capacity remains an explicit proof observation. Fallible construction,
allocator behavior, panic/unwind cleanup, full trait/wrapper correspondence,
native disposal authority and native pending-consumer admission remain open.
Witnesses start from synthetic represented storage; subsequent paired lifecycle
execution does not establish actual-constructor reachability. The CPU environment
is not a hermetic build attestation. No GPU, XGMI, HIP/HSA timing or parity claim
follows from these records, and Gate 1 remains open.

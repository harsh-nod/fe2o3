# Queued Read Resolution Checkpoint

Conditional shared-production-body proof and CPU developer evidence. Not a
full-owner refinement, native execution qualification or HIP/HSA parity claim.

Signed source: `1b74652c56463b878c0f2685932de90dce89e0ab`.
Source tree: `555e4d831d83ad4f863fc151888f8b9e3c79f744`.
Initial extraction: `9b4d7eaa9b65d250007223040d88930e7e5b5127`.
Baseline before this work: `f0cb2cb34d0fa3e13efadd3e42e91e233deadd63`.
The proof campaign verified the source SSH signature for `harmenon@amd.com`.

## Implemented

The actual queued-reader resolution loop is now shared by Rust and Verus through
`context_queued_writers/read_resolution_body.rs`. Rust supplies empty proof hooks.
The extraction replaces the bounded for-loop with an equivalent bounded while-loop;
it adds no new admission or execution behavior.

An independent logical update selects all Pending reservations by exact producer
identity, rather than following the executable linked-list traversal. The proof
shows exact status/version updates and detachment while retaining reservation
identity, consumer/range bindings, incarnation, occupied slots and reader accounting.
Only the selected root's attachment head/count clear. Live phase and fail-stop
state are preserved, including the captured Queued/live Unknown caller shape.

Explicit premises include complete producer membership, valid attached storage,
selected-storage representation, authentic outcomes and settled lookup
correspondence. Reachable-list validation alone does not prove that no orphaned
same-producer record exists elsewhere. The projection omits other outer arenas;
it does not establish their construction invariant or complete-owner refinement.
See the [precise proof boundary](../../runtime-queued-read-resolution-proof-v1.md).

## Formal Qualification

`campaign-2/results.json` records all 24 campaign phases passing:

- Clean signed source and signed blob binding; authenticated inherited classifier.
- Pinned Verus release closure before and after: 190 files, 129,019,839 bytes.
- Original, relocated and closing positive proofs: 25 verified, 0 errors each.
- All 16 production-body-only mutations produce accepted logical failures.
- Closing source and relocated/mutated-input continuity checks pass.

The negative controls cover omitted/truncated traversal, lost cursor, wrong status,
omitted/fabricated successful versions, invented failure versions, retained links,
wrong root, uncleared head/count, changed incarnation, premature free-slot refund
and cleared terminal state. They execute the shared body directly in a concrete
chain witness, not through an unchanged resolver contract. Full positives separately
prove the unbounded resolver and prefix lemmas.

Executable witnesses include empty, singleton and noncontiguous rosters, different
consumers/versions, unrelated Pending and already-resolved same-producer entries,
all terminal outcomes, and live/captured phase disagreement. They demonstrate
nonvacuity of the selected projection, not constructor-origin reachability of the
whole owner.

The first campaign is retained as failed evidence: its generic omitted-traversal
negative hit a solver resource limit, correctly rejected by the classifier. The
replacement directly instantiates the shared body in the concrete witness without
changing the general theorem or accepting resource failures. Earlier development
trigger, identity-contract, invariant and solver failures are also retained.
Historical proof pins/checkers were not changed.

## CPU Qualification

The frozen-baseline differential test compares full model state, storage
addresses/capacities and indexed lookup work across all three terminal statuses
and 0/1/3/8 selected reads, including interleaved unrelated readers and the live
Unknown phase change. The focused original queued suite passed 45 tests; the new
differential test passed separately. This is not a latency or throughput benchmark.

Final signed-source command, with no failed target skipped and no concurrent
compilation or proof campaign during Worker deadline tests:

```sh
CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2 cargo test \
  -p fe2o3-runtime -p fe2o3-runtime-model --all-features --no-fail-fast
```

- Model unit tests: 1,080 passed, 0 failed, 19 ignored.
- Runtime unit tests: 1,716 passed, 3 failed, 28 ignored.
- Runtime integration tests: 11 passed, 3 hardware tests ignored.
- Runtime doctests: 52 passed. Model doctests: 29 passed.
- Final strict all-feature/all-target runtime/model Clippy and formatting passed.
- Final runtime/model no-default-feature check passed.

The full test command exits 101. These unchanged tests still fail at
`authorized_execution.rs:1317` with `InspectSocket(PermissionDenied)`:

- `cooperative_debug_telemetry_emits_only_bounded_logical_records`
- `failed_session_end_is_explicit_and_terminal`
- `pre_native_telemetry_failure_is_returned_and_poisoned`

They remain failed, not waived. No permissions or escalation were requested.

## Remaining Gates

This does not close #182, A1/A2 or the overall parity objective. Still required:
outer-owner invariant preservation and representation, admission/activation,
attachment validation, queued release/refunds/disposal, and the active-then-queued
Context release sequence with its committed failure prefixes. Native terminal and
unwind integration tests remain open; this proof does not cover unwinding.

The separately authorized three-phase GPU profile and matched HIP/HSA measurements
remain open. Broader partial/ReadWrite outputs, native profiles and the wider
Worker/device-language/multi-device/memory/atomic/collective/profiling qualification
are not closed by this packet. No GPU or matched performance run occurred here.

MI300X SSH and both source pushes failed DNS resolution. No remote files or jobs
were created. Unrelated inspection evidence remains untouched. Publication of the
evidence commit is attempted separately after freezing this archive.

`receipts.tar.xz` retains source identity/patch/digests, commands, statuses,
timestamps, development and final logs, and both proof campaigns including signed
inputs, tool closure, relocated/mutated sources and diagnostics. Its companion
SHA-256 identifies the archive.

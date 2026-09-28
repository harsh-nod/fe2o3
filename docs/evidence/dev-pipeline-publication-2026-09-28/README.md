# Shared Pipeline Publication Metadata

Development shared-source verification and CPU qualification. Native receipt
authentication, full runtime refinement and HIP/HSA parity remain open.

## Source And Architecture

Signed source: `c012c6fcd955b9d970ecb49ea4ca140ab653c131`.
Tree: `8c6f03aff50efd1d14cbdc30b5c515e1ba1e9660`.

`RuntimeComputePipelineV1` now delegates to a private module borrowing its actual
slot table and scalar heads. Runtime and Verus compile the same complete vacancy,
lookup, frontier and staged-state scans and stage/confirm/withdraw writes.
Stage combines duplicate-owner, epoch and first-vacancy inspection in one pass
after frontier validation, retaining the first eligible slot while checking the
entire suffix before mutation. No copied metadata roster or caller-provided
validity flag replaces the real scans. Public APIs, capacities and native
authority boundaries are unchanged. Reduced table traversal is not a measured
latency or throughput improvement.

## Qualification

Commands, logs, timestamps, statuses, signed source snapshots, exact relocated
inputs and mutation diagnostics are in `receipts.tar.xz`. Its checksum is in
`receipts.tar.xz.sha256`.

| Check | Result |
| --- | --- |
| Original, relocated and closing Verus runs | Each 26 verified, 0 errors |
| Isolated production-body solver mutations | 18 expected logical failures |
| Pinned verifier closure before/after | Passed |
| Synthetic campaign/classifier calibration | Passed |
| Focused pipeline CPU tests | 14 passed |
| Full runtime library suite | 1800 passed, 3 failed, 28 ignored |
| Strict runtime all-feature/all-target Clippy | Passed |
| Runtime no-default-feature compilation | Passed |
| Workspace formatting and source continuity | Passed |

Aggregate qualification exits 1. Three existing telemetry failures remain unwaived,
all at `authorized_execution.rs:1317`, reporting `InspectSocket(PermissionDenied)`:

- `cooperative_debug_telemetry_emits_only_bounded_logical_records`
- `failed_session_end_is_explicit_and_terminal`
- `pre_native_telemetry_failure_is_returned_and_poisoned`

No failing test was skipped or relaxed. Focused/full counts overlap. The runtime
suite took 197.60 seconds in the debug CPU harness; this is not a GPU benchmark. The KFD
library suite, other model suites, doctests and native qualification were not
rerun in this packet. Final commands ran serially with
`CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2`, without source edits while live.
Preflight failures and their corrections are preserved in the archive.

## Proven Contracts

The shared bodies operate over borrowed slices with exact machine-width
metadata and an unconstrained opaque Active payload `T`, without `Copy`/`Clone`.
Lookup/frontier/staged-state contracts cover arbitrary representable metadata;
capacity requires nonempty storage, and stage requires 1 to 65,536 slots inclusive. The actual
runtime constructors still admit only 64 or accounted 1024 slots.

- Vacancy selects the first eligible slot and never wraps its generation.
- Exact lookup authenticates the stored identity and submission. Mutable lookup
  frames neighboring slots and generation while relating the selected entry to
  the final returned reference, not freezing fields the caller may change.
- Frontier/staged-state scans implement their exact existing predicates.
- Refused transitions preserve every slot/head and return the original incoming
  owner where applicable. Successful transitions change only specified fields.
- Withdrawal returns the original selected owner and preserves its burned
  generation and the next logical epoch. Confirmation alone consumes that epoch.
- Separate theorems preserve unique roster identities and occupancy, and prove
  that every accepted stage satisfies the settlement predicate.
- A generic success witness stages, withdraws, retries and confirms at `u64::MAX`,
  preserving its original payload and exhausting the epoch without wrapping.

The 18 mutations cover generation reuse, capacity/owner/identity checks, occupied
counts, duplicate frontiers, successor selection, newer siblings, incorrect
rejection, early scan termination, last-vacancy selection, duplicate admission,
owner substitution, missing occupancy, epoch/frontier advancement and withdrawal
generation reset. Only executable shared bodies are mutated. Every accepted
negative has a genuine postcondition or loop-invariant failure at an allowed
source path; compile errors, timeouts and solver/resource errors are rejected.
These are solver controls, not executed Rust behavioral mutants.

Six new CPU groups compare against frozen pre-extraction predicates over snapshots
and exercise 64/1024 capacities, high slots, generation/epoch exhaustion, owner
payload addresses, exact neighbor frames, accounting and zero-allocation metadata
paths. The fixtures populate Arc recipes and vectors but use scripted execution;
they do not contain native GPU receipts. Existing eight pipeline groups and the
full runtime suite also exercise the wired adapters.

## Remaining Boundaries

`unique_roster` is not the full pipeline invariant. It does not establish a
contiguous confirmed epoch interval or correct relationships among all heads.
For example, a valid epoch 5/frontier 5 with next 100 can pass the current raw
guards and acquire epoch 100, leaving no immediate successor 6. Exact acceptance
tests intentionally preserve such existing limits; they do not endorse corrupt
states as healthy. Zero-valued matching lookup metadata and malformed older
staged siblings likewise retain their existing behavior.

Next establish the complete confirmed interval and share/prove promotion
(`take_commit_frontier`) and quarantine. Promotion is movement into the separately
retained Active slot, not physical retirement or logical completion. Keep the
quarantined mode and transient Published/Attempt::Published bridge explicit.

The standalone proof uses mirrored metadata types and does not mechanically
verify the `HostMetadataTableV1`/runtime adapters, native outcome authentication,
selected lane/recipe/storage binding, lower no-side-effect retry authority,
Pending handoff, resource retains, logical commit, profiling or AUX restoration.
No GPU execution, native cleanup, compiler/hardware refinement, protected Worker
or matched HIP/HSA performance is newly qualified. A0-A7 and accepted Native
R125, Admission R118B and Resources R116/V3 checkpoints are unchanged.

MI300X SSH failed DNS resolution before connecting, so no remote files/jobs were
created. Both source pushes failed to resolve `github.com`; the issue API read
also failed. No remote synchronization or fresh issue-state observation is claimed.
Only task-owned local incremental/example build caches were removed for space.

The archive is frozen after source qualification and source-push attempts.
Documentation-commit publication is recorded separately. Verify this directory
with `sha256sum --check receipts.tar.xz.sha256`.

## Reproduction

This standalone development campaign is separate from the global
`verify-verus.sh` release gate. From a signed, clean source checkout, set `VERUS`
to the canonical executable in the pinned release and use a new absolute output
directory outside the repository:

```sh
python3 -I -B crates/fe2o3-runtime-model/verus/check-compute-pipeline-publication.py \
  --verus "$VERUS" --output "$OUTPUT"
```

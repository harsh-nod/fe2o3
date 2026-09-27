# R57 V2 Qualification Identity

CPU developer evidence only. No V2 hardware qualification or parity claim.

Source: `05b92526e25fc09fbcd50fcd5d9839f7c3562932`.
Parent: `ef4ef1fa79c5c4100e07f40b5c6416bc7e5d40ce`.
The source commit has a verified SSH signature for `harmenon@amd.com`.

## Repaired Contract

The [queued Context checkpoint](../dev-queued-context-2026-09-27/README.md)
exposed an earlier qualification mismatch: the R57 example used a mixed-memory
negative while reporting the historical uninitialized-C policy's V1 identity.
DeviceLocal allocations now begin zero-initialized, so restoring that source
assertion alone would misrepresent the current qualification.

V2 pins the actual A/B/HostVisible-upload rejection, exact InvalidLaunch reason,
zero final-authority consultations and ordering before C/D H2D. It has separate
policy, typed-argument and authority identities. The two-phase implementation
still verifies exact initial content and permits only A/B/C then C/B/D.
No generic backend admission behavior was weakened or special-cased.

Historical V1 policy/source/object bytes and identities remain unchanged. Both
public authority entry points remain available; cross-profile requests reject.
The current example reports V2 plus its policy digest, performs two successful
dispatch waits and four exact readbacks, and reports PASS only after cleanup.
Old V1 GPU receipts are not evidence for V2.

## Validation

All nine focused R57 unit tests and both source-shape gates passed. Four new
tests cover cross-profile isolation, malformed first-phase requests, second-phase
identity/content and one-shot completion, and the real Context mixed-memory
rejection. The latter uses scripted host allocation/initialization plus three
scripted DeviceLocal owners; rejection preserves ownership, capacity, native
steps and authority count, and explicit release consumes all owners without
unexpected drops. This is CPU custody evidence, not native execution evidence.

Strict all-feature/all-target runtime/model Clippy, the no-default-feature
runtime check, formatting and whitespace checks passed. The source guard
requires the exact negative, upload/launch/wait
ordering, initial-content checks, V2 identity, four readbacks and cleanup.
It is a source-shape regression guard, not a formal or machine-code proof.

The final serial all-feature runtime run on this source reports 1,703 unit tests
passed, three failed and 28 ignored; all 11 integration tests passed with three
hardware tests ignored; all 52 doctests passed. The three unchanged failures are
`cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`failed_session_end_is_explicit_and_terminal`, and
`pre_native_telemetry_failure_is_returned_and_poisoned`, each failing at
`authorized_execution.rs:1317` with `InspectSocket(PermissionDenied)`.
They remain failures, not waived or skipped gates. The previously failing R57
source gate now passes under the distinct V2 contract.

## Limits

No Verus or source-bound proof campaign, GPU qualification or matched HIP/HSA
benchmark ran. MI300X DNS failed and no remote jobs/files were created. The
queued Context composition, queued-output reads, broader write semantics and
the full runtime parity goal still require implementation/qualification.

`receipts.tar.xz` retains commands/exits, development failures, source hashes,
signature verification, review notes and final test output. The companion
SHA-256 file identifies the archive. Publication is attempted separately after
freezing this evidence.

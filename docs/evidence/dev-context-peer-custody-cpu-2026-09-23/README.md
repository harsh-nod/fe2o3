# Context Scalar Peer-Copy Custody: CPU Checkpoint

Development CPU evidence for [Context scalar dependency custody](../../runtime-context-peer-custody-v1.md).
This is not pending-source admission, native GPU qualification, executable/native
proof refinement, a matched performance comparison or A1/A2 acceptance.

## Implementation

Ordinary scalar peer copies now retain their exact operation provenance and
complete event-to-producer roster independently of public events. Context roots
and balanced producer retains precede backend entry; public release and cleanup
both refuse a retained producer. Consumer quiescence discharges the holds once,
before callbacks. Completed provenance remains until submission release. Terminal
errors, detected malformed custody and backend panics preserve unresolved roots,
including provisional attempts without returned handles and nonjournaled Contexts.

Context owns the existing producer-aware journal, without exposing mutable access
to its inner stable journal. Existing stable-read behavior remains; pending source
reads still reject. The new root does not grant success-gated execution or native
lease authority. Segmented/generated operations and Worker protocols are not
upgraded. Peer dependency duplicate checking now uses bounded stack sorting,
preserving original backend order and reducing the admission bound to O(D log D).
Launch preflight is unchanged. Validation is local to the touched roster; it is
not global retain-count recomputation or arbitrary-corruption authentication.

## Qualification

Pinned nightly `2026-04-03`, all features, test optimization level 1, debug
assertions enabled, incremental compilation disabled and two build jobs:

- GNU runtime library: 1,265 passed, zero failed, 20 hardware-only ignores.
- Musl runtime library: 1,265 passed, zero failed, 20 hardware-only ignores.
- Runtime doctests: 46 passed in the two reported groups (4 and 42).
- Formatting and all-target warnings-denied Clippy pass.
- All seven command receipts report status zero and owned process-group absence.
- All 3,807 captured source/configuration/fixture hashes and the runner hash match
  before and after execution. The current source also matches those hashes.

Fourteen new CPU test functions cover fanout, complete event rosters and aliases,
early event release, initial and completion-time failure classes, first nonjournal
panic, zero/duplicate returned handles, cancellation, event observation, stream
destruction, partial cleanup and retry, repeated drain/callback panic, missing
roots and insufficient alias counts, held-producer stream compatibility, overflow,
late duplicates and continued pending-source rejection. These are function counts,
not the larger number of loop cases. The generated-issue regressions also run in
both complete runtime suites.

Two independent read-only reviewers audited the production change. Their findings
led to removing an unintended held-producer-stream restriction and making the
validation scope explicit. Final review found no remaining production blocker.
The fixtures are scripted CPU backends, not native fault or performance evidence.

The archive has 23 raw records: seven command triples and two input brackets.
GNU/musl stdout retains Cargo's final blank lines and the musl harness's
interleaved trailing whitespace; those raw bytes are not normalized.
`runner.py` retains its original absolute workspace and private-target convention;
its argument selects the output directory. This is development reproducibility
evidence, not a portable/hermetic source or toolchain attestation.

## Retained History

Private scratch root:
`/home/harsh/.codex-tmp/fe2o3-peer-custody-20260923-wKbQLlhb`.

- `focused-1` failed compilation because the initially narrowed test accessor
  overlooked four mutable generated-issue test callers. Its unchanged input
  brackets and status-101/group-absent receipt remain retained. The correction
  exposes the producer owner, never a mutable escape to the inner stable journal.
- `focused-2` passes the fourteen new tests, but retains its preliminary source
  cohort. A misplaced duplicate-check optimization was then restored in launch
  preflight and applied solely to peer-copy preflight.
- The complete `cpu-1` campaign supplies every result in Qualification above.
  No preliminary or failed result is substituted for its final-source runs.

After qualification, `cargo clean` removed the private target's 1,339 files
(660.2 MiB). The raw records and runner remain retained; no shared cache was removed.

## Remaining Gates

Next are an explicit scalar directed-route success-gating contract, exact producer
read reservations, producer-first reconciliation and bounded async integration,
followed by native chain/canary/fault/cleanup qualification. Pending sources remain
rejected. No SSH, GPU workload or HIP/HSA benchmark ran for this packet.

`context.rs` has changed from the historical constructor-origin lifecycle proof's
captured inputs. That packet stays evidence for its captured source, not proof of
this integration; its checker, pins and records were not rewritten. Context event
binding, physical/unwind correspondence and native refinement remain open.

Native R125, Admission R118B and Resources R116/V3 remain the broader accepted
checkpoints. #182, A1/A2 and HIP/HSA parity remain open.

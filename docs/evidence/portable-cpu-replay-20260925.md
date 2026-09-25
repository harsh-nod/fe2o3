# Shared CPU Replay And Protected F Status

Compiler candidate: `ca75075ed1612ff7d1c6a82a778ff3db5345395d`.
This follows the [conditional source-to-F integration](conditional-final-source-join-20260925.md).
The shared CPU replay implementation and its local regressions pass. The first
protected F matrix failed in its diagnostic transport; the corrected candidate
has not yet repeated that matrix. No issue #272 milestone, native authority,
safe-launch coverage or tutorial-kernel completion is claimed.

## One CPU Implementation

`fe2o3-verifier::portable_reference_v1` now owns the existing inert reference
records, logical-signature projection, canonical hash, acyclic resolver, helper
substitution, debit rules and scoped replay. Live rustc extraction uses the same
resolver through its original source-work adapter. Authenticated binding replay
lends a `ReferenceReplayInputV1` to that shared implementation. No second
interpreter, ledger, executable graph or dependency edge was added.

Rustc extraction, source/helper authentication, retained-input rederivation and
the loop engine remain backend-owned. The public portable input is inert: it
does not authenticate CPU source or construct an authenticated binding. The
legacy effect digest is unchanged and is not a complete CPU/source commitment.
The existing conditional acyclic domain is unchanged; this move does not add
conditional loop support.

Replay independently reconstructs outputs, assignment values and bounds, checks
both retained output lists, and uses the caller's original account. Its callback
cannot retain borrowed replay storage. Cleanup releases only replay-owned
scratch after dropping it; incoming floors, work, peaks and first denials remain.
Foreign-account replacement and damaged floors reject without unauthorized
refunds, including during unwind.

Independent static review found no production regression. Its test-coverage
finding was addressed with a funded foreign account, separate floor-corruption
cases, success/error/unwind denial-history assertions and adapter parity checks
that include both denial fields. Static comparisons support this code move;
parity tests that share an implementation are not independent semantic proofs.

## Local Validation

All five final guards used unchanged 8,201-file source inventory SHA256
`5143dddb7c8e9376e1465880e139b4fffb72a6ab7f60f5b54ccc69652f7ece30`.
This documentation was added afterward. Guards used pinned nightly `2026-04-03`,
locked offline dependencies, one Cargo job/test thread, disabled HIP, hidden
GPUs, a 12-GiB virtual-memory ceiling and a 1,200-second deadline.

| Guard | Result | Log SHA256 |
| --- | --- | --- |
| `portable-cpu-verifier-final-tests-r1` | 11 passed | `de561b3d51c84ea0bcabc72331813e2dd2b652fba6354624c9e9845126aa80f9` |
| `portable-cpu-verifier-final-docs-r1` | 1 compile-fail passed | `75a920d93e3585f39c9695afcfb07566924e01cb61527b86595e3ae105a8eb0a` |
| `portable-cpu-backend-reference-tests-r2` | 106 passed | `6c01d2243455c0e474f30608a6cacee963ffa79a2ceaf923ef99eb9c312af123` |
| `portable-cpu-backend-pipeline-tests-r1` | 86 passed, 2 ignored | `d71ec7b812132b292662bfbd4ff755c54ac2925ae284b1822568102596b9483a` |
| `portable-cpu-normal-check-r1` | library check passed | `9add231c3dc7e39bb579d2bc5d0c6a2a6a069a47bb056e55c293fd57392994ca` |

Coverage includes ordinary loops/helpers/signatures, source and CPU-read joins,
exact/one-short resource limits, failed-account cleanup, ordinary/conditional
Policy7 encoding, policy6, source launch, final-graph descriptor and simulator
checks, owned packet accounting, and the corrected F result protocol. The two
ignored children require genuine protected execution and are not passes.

The first backend build exposed a missing shared-trait import in the raw-source
adapter. That import and two unused moved imports were corrected before the
successful rerun. Failed log SHA256 is
`847738fe3891a333bc5d3320f2f57816c2c1507dd8aa63961bc89b8048cd472d`.
Compiler warnings remain; this is not a warning-free or complete workspace test
claim. Formatting, dependency policy, hygiene and DCO checks pass.

## Protected Run And Fix

The frozen r11 runner executed once on MI350 using published candidate
`bfa616c996fa529da67f2f6c32e7829213914e3e`, before the portable replay move.
Runtime audit and all three real protected-runtime preflights passed, including
false-proof rejection. The original policy6 actual-source parent also passed.

The main F matrix failed on its first gfx942/F child. The child reached the
expected `ConditionalFinalizerRequired` refusal and passed its account/floor
assertions, then rejected its own report: the schema expects the CPU label
`gfx942`, while the test wrote the full target ID `gfx942:xnack-`. The fix uses
`Profile::cpu()` only for this transport. The production target assertion still
compares the complete target ID. Regression tests distinguish both forms across
both profiles and every main/late mode; the schema and runner were not weakened.

The archived report says `tests_passed: false`. Result archive SHA256 is
`2d1f31fee7e94e72c7e785112a29384b1ec7bed63752eab2d50edcfc8ba08f09`;
inside-report SHA256 is
`fa9af49603aec8a75802c89b3c97f95c6757e5eeeda67d850063c597479a7aed`.
The run drained; its service, slice, root run, bootstrap and incoming upload
directory were cleaned after archival. Shared runtime installations were not
changed. No complete main or late F matrix, GPU run or safe launch passed here.

## Remaining Work

First repeat the corrected protected main F matrix, then its late-failure matrix
with fresh, fully joined source and artifact inputs. Next implement the paired
bounded conditional native packet producer/independent consumer: canonical CPU
input decoding, complete CPU/source commitment, versioned signed statement,
fresh CPU/source/formula replay and retained final-F custody. This shared engine
is a prerequisite, not that packet path itself.

Actual-F conditional formal/descriptor joins, native finalization and durable
restart, sealed host/machine admission, generated safe launch and the complete
target-matched tutorial matrix remain. `ConditionalFinalizerRequired` still
blocks finalization. All M0-M7 milestones remain open.

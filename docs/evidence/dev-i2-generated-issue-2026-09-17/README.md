# Private I2 Publication And Physical Settlement Evidence

Development integration above `0967725d2e44e6d79df4a9d7cf21cd162df8b534`.
This does not advance accepted R125 Native CPU/test, R118B C1/C2/C3 or R116/V3
checkpoints. A1/A2, #182, full HIP/HSA parity and performance qualification remain
open. See the [implementation contract](../../runtime-generated-issue-v1.md).

## Implemented Boundary

The existing owner-only generated driver now retains one Context mutation
attempt and submission identity before native entry, and one indexed backend
submission under its existing shell/native owner. Exact source/currentness gates
bracket publication and physical observation. Explicit pre-effect retries retain
the same IDs; generic poll never publishes. Returned pending/completed custody
is rooted before closing lane restoration. A consuming lower failure without a
returned receipt preserves an explicit handoff marker, not a reconstructed token.

Physical completion parks the operation without typed result success. Ordinary
drain retains it until C4; an exhausted drain seals Context under existing policy.
Actual Stop first stops observers, then permits bounded observation-only cleanup.
Ready uses pristine abort; recycled work uses recycled detach. DATA disposal,
exact backend identity release, Context records/credits and stream hold remain
ordered. Unknown/terminal work is retained, never replayed or refunded as NoEffect.

## CPU Validation

Both GNU and musl passed **892 runtime-library tests**, with **15 ignored** out
of identical 907-name rosters. This adds 23 CPU tests and two opt-in native tests
to the previous roster. The final accepted runs are `gnu-final-runtime` and
`musl-qualified-runtime`, using their `*-qualified-build` Cargo JSON artifacts.
Strict all-feature/all-target Clippy, workspace formatting, 33 runtime doctests,
no-default compilation and unsafe-source policy (five passes, one maintenance
ignore) passed. No lower-library source changed in this packet; it is not a new
full KFD test or formal qualification campaign.

Coverage includes returned receipt ordering and consuming handoff failures;
exact retryable publication/recycle; Context attempt/ID/roster and substituted
live-token rejection; zero/duplicate returned handles; source substitution and
closing currentness; capacity/cold-SDMA exclusion and early release; real owned
engine drain-versus-Stop behavior and retained error/panic paths.

Receipt tests use generic drop-counted owners. Context fixtures execute real
registration/admission/identity code over mock backend metadata. Async fixtures
use injected hooks and the real owned engine. They do not constitute original
lower-owner fault composition or protected Worker-to-native execution. Complete
native fault matrices, each stopped-retirement failure boundary, production
journal correspondence, aggregate residency, C4 decoding/reply and C5/C6 remain
open. No successful generated drain or public activation is claimed.

## Native Execution

Four exact ignored tests passed on MI300X physical GPU 1, UID
`0xab83d2ffef0d3cdf`, PCI `0000:26:00.0`, using the final musl test executable:

`46d233f46b5ae8eb6c5d09a98ff1ec8e682a591c703462c46fee0d8e52b7db9e`

- N5 cold primary/AUX/rebound adoption, pristine abort and shutdown.
- N5 bootstrap primary/AUX/rebound adoption, pristine abort and shutdown.
- I2 cold primary/AUX publication, exact output bytes, rebound completion and shutdown.
- I2 bootstrap primary/AUX publication, exact output bytes, rebound completion and shutdown.

Each case disposes nine DATA resources. I2 cases independently assert bootstrap
state, distinct primary/AUX handles and the original primary handle on rebound.
They check primary/AUX output bytes before detach; rebound checks completion,
not output. Generic Ready polls stay unpublished and premature release rejects.
Physical completion reports quiescence without a delivered generated result.

These use the repository's authenticated exact vecadd artifact, actual checked
device, loader/ABI and original packet. They bypass generated carrier/Context
registration and establish no protected Worker application authority, typed
future result, physical overlap, native injected-fault behavior or performance
comparison. Debug probe durations are not benchmark results.

`native-guard.sh` checks the executable hash, exact UID/BDF, 0% use, low VRAM and
the ROCm PID/device map immediately before each case. GPU 0's existing workload
was left alone. Every native process finished normally, and GPU 1's post-case
reported VRAM returned to 298,647,552 bytes. The owned directory
`/tmp/fe2o3-i2-harsh-20260917.mUcACc` contained only the binary and guard; both
were removed, the directory removed, and absence verified. No foreign files or
processes were deleted or signalled. These observations are not aggregate-memory
proof or an exclusive reservation against other users.

## Reproduction And Exclusions

`source-files.sha256` pins all 18 changed Rust files. `source.patch` and
`source-base.txt` identify their base/delta. `record.sh` preserves commands,
timestamps, output and exit statuses without overwriting prior attempts.
`audit.sh` checks the final source, exact test rosters/results, binary identities,
native test names/hash and cleanup records. `SHA256SUMS` seals archive membership.

Earlier successful `musl-build`, `musl-final-build`, `gnu-runtime` and
`musl-runtime` commands precede the final explicit poll-result representation
and/or native route assertions; they are not final-source qualification.
Initial `clippy` exited 101 for inline returned-custody sizes. The final code
uses an explicit pending/completed enum and a scoped allowance preserving
allocation-free recycle failure custody. `gnu-qualified-runtime` exited 127
because an incorrect executable path was supplied; it ran no tests. The later
`gnu-final-runtime` uses the exact executable from Cargo JSON and passed.

Exploratory, pre-archive work also encountered compile errors and two incorrect
async test expectations (transferable preparation already rejects, and exhausted
drain already seals Context). Those were corrected before final qualification;
their partial runs are not counted. Source edits after earlier builds required
new `*-qualified-build` binaries and re-upload before any native invocation.

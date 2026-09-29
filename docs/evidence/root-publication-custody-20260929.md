# Original-Attempt Publication Custody

Checkpoint for [#272](https://github.com/harsh-nod/fe2o3/issues/272).
[M0](../issue272-capability-adr-v1.md) now records the accepted contract and
unqualified baseline. **M1-M7 remain incomplete; the complete protected
production-to-safe-GPU-launch matrix remains 0/47.** Three later manifest
selections are recorded separately, not silently substituted into that cohort.

## Implementation

The original `NativeAttempt` now consumes and retains the actual publication
owner through a phase-checked `CompilerTrace` adapter. Acquisition, revalidation,
issuer removal, outer accounting failure and unwind retain the original compiler
trace and cleanup slot. Lease/token installation precedes subsequent fallible
work. Durable retirement and authenticated production RPC remain unfinished.

The existing V5 recovery engine now has an explicit inert byte ceiling checked
before payload allocation/read. Shared quotes cover invocation/image observation,
filesystem and registry work, overlapping scratch, both artifact owners and
independent cleanup funding. The old V5 storage ceiling is unchanged.

The native publication fixture supplies genuine process descriptors, sealed
invocation metadata and filesystem locks over **inert** V5 content. It never
resumes rustc, executes Verus, produces accepted semantic evidence or runs a GPU.
This is ownership and failure-path validation, not M1 or kernel qualification.

## Local Validation

The first combined run at `91d4b0a2675f5f8b4449768ae89060c06f83689e`
completed artifact, broker and capability suites, then found a new fixture flag
appended after the required final backend selector. Commit
`e4b86465bbd0a59e529e38cd29a9d66b7564e1c3` corrected that fixture ordering without
weakening invocation admission. The remaining five libraries passed after the
fix. The following are distinct top-level tests across those split runs, not
one all-green invocation or repeated subprocess counts:

| Library | Passed | Ignored |
| --- | ---: | ---: |
| Artifact transaction | 341 | 0 |
| Broker authority service | 392 | 20 |
| Compiler closure capability | 271 | 4 |
| Compiler execution coordinator | 254 | 6 |
| Compiler execution issuer | 31 | 2 |
| Compiler execution protocol | 110 | 0 |
| Process identity | 16 | 0 |
| Protected service spawn | 258 | 7 |
| Total | 1,673 | 39 |

The corrected source snapshot was
`e3c9c6d6dc60b7f8a874b265a8c4547bc4d651e50b39d4b789201ddee26c74a2`.
Guarded builds used locked/offline nightly `2026-04-03`, one Cargo job, disabled
GPU visibility, a 12 GiB process-memory ceiling and a 1,200-second deadline.
Source and tool hashes were unchanged across each guarded run.

Artifact/broker/spawn documentation tests subsequently passed 32/92/65 tests
(189 total). The source snapshot for that run was
`96d570c25288a2cc6d7f5c680ce3d1f026c3808fa7ae32979101ae110a30d129`.
After the unrelated upstream Pliron/debugger merge at
`ab34692db27490113b35f378294efdd738e50576`, a guarded broker/coordinator
`check --all-targets` passed, snapshot
`ae56ea8243c602e49e7efdb77547b5c40a92fbc8a7dd5f566f5fbd89421a31fe`.
The native results below do not certify that later merged binary. Existing
compiler warnings remain; this is not a warning-free or Clippy-clean claim.

The live manifest initially rejected four stale Cargo.lock bindings after the
new test dependencies. Refreshing only those bindings and derived digests
restored source validation. The full 79-test manifest suite then passed 77,
with two expected whole-snapshot digest mismatches. After updating those two
golden digests, both tests and the unchanged original47-obligation test passed
on a focused rerun. No source roster, feature selection, required gate or
qualification status was changed. This too is split-run evidence, not a single
clean full-suite invocation. The nine M0 baseline tests and exact pinned-source
audit passed separately; neither executes a compiler or GPU.

## MI350 Validation

Fresh static issuer/helper/daemon images were built from the corrected source.
Each passed the secure-entrypoint/static-image checker before and after removing
nonessential symbol tables while retaining `fe2o3_secure_start_v1`. Original
build outputs were preserved. Packaged image SHA-256 values:

| Image | Bytes | SHA-256 |
| --- | ---: | --- |
| Issuer | 11,892,160 | `f2bb33d7eefa125e3ee0a62922314814b62e83b2e4db98caa1d61c2fa4fb0afa` |
| Helper | 1,528,168 | `5df3473466e446e216f679a5a6416709f565ac202ff78a63e6d0e72fa6cb65ba` |
| Daemon | 1,483,112 | `4b7769e0d00938598c7e292975dd7da0090535b67e31ab863920d86f798e713f` |

Both runs used test binary
`26440158129fddff6d0b5c45ea564a976534ac380a1d871abb7aeda05e5c7b17`
and isolated image
`sha256:fd5370f370708f6a02cec6d44818a4295609e5bc68aa42455e53f141168a9d5f`.
Containers had no network or GPU devices, a read-only root, one CPU, 2 GiB memory,
32 PIDs and a 600-second execution deadline. No failed or unconfirmed job was
automatically restarted.

- Publication: **6/6 passed**: drop, issuer removal, unwind, outer accounting
  refusal, invocation mismatch and short handoff ceiling. All used the same
  original 256 MiB request account. Maximum recorded request peak was
  158,518,711 bytes; cleanup peak was 80,006,871 bytes. Complete observation
  work/scratch quotes bounded the measured operations.
- Startup: **10/10 passed**, retaining the original readiness, continuity,
  substitution, timeout and resource-refusal cases. This separate existing
  fixture uses a 1 GiB account; its peaks must not be attributed to the
  publication fixture or a real compiler invocation.
- Both containers exited successfully without OOM. Container, private remote
  scratch and SSH control directory removal were independently verified;
  both cleanup reports contain zero errors.

Local evidence lives under the owned `fe2o3-issue272-production-next-evidence-20260921`
directory. The publication/startup status records are respectively
`root-publication-native-r24/status.json` and `root-startup-native-r25/status.json`,
with SHA-256 values
`2c8b69eead45e582bb27f536421ccd592f7d26153e9553c92f956872b1a365cc` and
`3fc6b1792a7f8875b1de2d8143e8b708eb7430d773566d393a7f51c04b7c924b`.
Packaging is recorded in `root-publication-packaged-r23/packaging.json`, SHA-256
`cb9b252049c82930f742830ac46b9baf3fe733d3e323976158125a23709c218d`.

## Remaining Production Work

The actual production wrapper does not yet call this original-attempt path.
Runtime enforcement, authenticated per-compilation intake, protected proof RPC,
durable publication/retirement, generic finalization and safe-host activation
remain required before even the vecadd production vertical can close.

Historical pinned-size evidence records LLVM at 199,520,544 bytes and
`librustc_driver` at 152,936,640 bytes: together 352,457,184 bytes, already larger
than 256 MiB before other retained owners. That evidence explicitly claims no
runtime-origin authority. The small fixture's success does not establish real
runtime affordability. Repeated backing charges need review, and an explicitly
bounded artifact allowance must compose with the original whole-runtime account
without resetting its ledger or weakening either ceiling. That work is separate
from this validated checkpoint. No tutorial-site deployment is claimed.

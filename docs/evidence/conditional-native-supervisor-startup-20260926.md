# Native Supervisor Startup Checkpoint

Date: 2026-09-26. Continuation of the
[compiler preparation checkpoint](conditional-native-compiler-preparation-20260926.md)
for [issue #272](https://github.com/harsh-nod/fe2o3/issues/272).
**No whole M0-M7 milestone or 47/47 production-to-safe-GPU-launch completion is
claimed.** Successful protected native startup remains unvalidated.

Base: `2983a86a30130c779f4f00fd74885aa9cb29cdf7`.
Integrated code: `ac74070883e8dafa4500a522824e10d4a081fee4`.
Subsequent documentation changes do not change executable behavior.

## Implementation

- `0ec4700db`: worker's bounded eleven-role raw descriptor intake and readiness
  transport, sharing the existing mechanical bootstrap checks without upgrading
  any V1 admitted authority.
- `18087267c`: worker's isolated staging EOF probes. Creating the pipe inside an
  exact-filter subprocess prevents unrelated test forks from inheriting writers;
  the strict immediate EOF assertions and all eighteen failure stages remain.
- `ac7407088`: shared nominal V2/V3 startup, dedicated static binaries, persistent
  cleanup guard, startup/accounting/custody tests, and musl ancillary layout fix.

The native entrypoints consume slots 3..=12 and 220. They charge full inherited
images and duplicate overlap before admitting the actual policy, contextual
deployment, process/namespaces, running image, root-bound lifecycle lease,
signing key, program and external anchor. Listener activation follows admission.
Exact 88-byte readiness and dispatch use the original request account; context,
profile, image and lease continuity are rechecked around readiness and dispatch.
Both families use one closed implementation, not a second production compiler.

The raw-source guard exists before the first resource refusal, validates all
slots before duplication, marks a source consumed before closing it, and closes
the remaining sources on refusal or unwind. Readiness has finite attempt and
deadline limits. These are logical work/storage quotas and syscall-attempt
bounds, not hard elapsed-time, generated-stack, allocator or RSS guarantees.

Review found that local lifecycle ownership could end before deferred child
cleanup. Startup now transfers an actual admitted lease alias into the existing
persistent cleanup pool before any child launch. Both original accounts are
charged. Only an empty, admission-open pool accepts the guard; there is no
replacement or detach operation. Controller Drop, error, unwind, quarantine and
work exhaustion retain it. Successful shutdown releases it only after checking
every slot empty and the storage invariant. This is process-lifetime custody,
not survival of process death; independent root custody and recovery are still
required. The generic guard API itself grants no deployment authority.

`build-static-compiler-execution-supervisor.sh [v1|v2|v3]` selects the binary;
omitting the argument and Cargo's default binary still select V1. The native
executables compose startup and separately funded cleanup, with a finite terminal
cleanup loop and silent fail-closed exit. Building them does not provision or
activate a native root deployment.

The musl build exposed a pre-existing `cmsghdr.cmsg_len` type assumption. The
parser now uses checked native length conversion, and its backing is word-aligned
for rustix's Linux syscall header even when libc's musl header is int-aligned.
Fixtures initialize explicit padding and use native field limits. Read-only
worker review found no further ABI, bounds or double-close defect under the
documented kernel-generated, single-receive invariant.

## Validation

Pinned `nightly-2026-04-03`, frozen/offline dependencies, HIP disabled, one bounded
Cargo command at a time, source frozen during builds. The worker performed only
source edits and read-only review; the primary integrated and executed tests.

| Check | Result |
| --- | --- |
| Four-crate GNU unit suite, R4, four test threads, no filters | 419 passed, 50 failed, 42 existing ignored; exit 101 |
| New supervisor IO/startup tests | 22 passed, included in R4 |
| New guard custody subprocess | Five real-flock schedules passed, one R4 test |
| Four-crate doctests, R2 | 26 positive and 255 compile-fail passed; exit 0 |
| Fifteen-package all-target check, R2 | Passed with existing warnings; exit 0 |
| Musl release ancillary parser/custody tests | 10 passed, 290 filtered; exit 0 |
| Static V1, V2 and V3 gates | All passed, including production image checks and missing-descriptor smoke |

R4 consists of compiler coordinator 52/7/0, supervisor 216/42/42, external anchor
coordinator 81/1/0 and shared spawn 70/0/0 (pass/fail/ignored). Nested helper
outputs are not counted twice. The suite is not green: 49 failures encounter
socket-operation EPERM, and one encounters ACL fixture EINVAL. These environment
failures also occurred before this slice. No test was disabled or production
predicate relaxed. Rust all-targets is not GPU architecture coverage.

The guard schedules use real flock exclusion and synthetic slot events. Existing
actual-child subprocess tests additionally check lock retention through exec,
late request-budget refusal, unwind, busy shutdown and exact-child reaping.
Neither kind establishes successful protected deployment. New startup tests
cover genuine sealed policy/deployment contexts, all policy substitutions,
descriptor custody, full input floors, exact/short quotas and denial history.
They do not exercise the complete privileged startup path.

The ten musl ancillary tests execute the parser and descriptor cleanup, including
unwind, in a musl executable. **Live mixed ancillary receive through rustix's
`RecvAncillaryBuffer` remains a coverage gap.** This was not a full musl unit run.
Each static gate checked ELF64 ET_EXEC, the secure entrypoint, no interpreter or
dynamic dependencies, non-executable stack, no undefined symbols, production
image admission, and silent exit 1 with all inherited descriptors missing.

Earlier local runs exposed four incorrect startup test work expectations and an
EOF test fork race; both were corrected without weakening quotas or assertions.
The guard tests were also isolated after review identified fork-inherited locks.
The first cold static build timed out, a second was interrupted, and the third
exposed the musl type error. All three final static gates passed after the fix.

Logs are retained under
`/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921/`:

```text
10d1ebc93b92472b0305beea1f2757d54f15a6fd6b6885dd9aade10105a543fa  conditional-native-supervisor-startup-full-r4.log
45e452059749451e1495fc9bfd30015f1cf9eeb99ebf62ede33f7f10b27b3671  conditional-native-supervisor-startup-docs-r2.log
2865572dfee78e3a92bba47eb8d0ce3d3a8f93067d98eb84055e10f3368e6631  conditional-native-supervisor-startup-all-targets-r2.log
0048ccefb1fe57b659dba196eddbb491ab2a29258240849949d6c8043dcef045  conditional-native-supervisor-musl-ancillary-r1.log
a2bb3097fa1e81cfcfdb334d28296c5cf586360873f77e36e3c0a2a39cad91b2  conditional-native-supervisor-static-v1-r1.log
b9217f75f4569904e6fca3c1c6f893ff165e3e18fdb0714d13ed7630f5f95c12  conditional-native-supervisor-static-v2-r5.log
b36908cceaaabf54d63c22bba18ddd22559841577dfe3187019d5a763a04c184  conditional-native-supervisor-static-v3-r1.log
```

Final musl executable SHA-256 values (V1, V2, V3 respectively):

```text
85b8a62bce9a4828074f9698360de24f8d1d4cb6437554b4a607380480f5e879
4b9c0edbad7e03463dec10ad3c8cd09291c5bf38439ebab8261721a23e5ff1a1
304792af262404e4dc12ce1b5f1dec5b8b61b3f4387fbee61f8e9a158f008685
```

## Remaining Gates

1. Compose native root inherited inputs and consuming supervisor launch through
   the shared spawn primitive. Retain root cleanup guard custody before anchor
   launch; join its actual lease and final staged Files. Reuse bounded transport
   mechanics for gate release, exact readiness/EOF/liveness and managed shutdown.
2. Validate genuine protected startup, composed post-clone/error/unwind paths,
   recovery and live ancillary receive. Lower-level fixtures and missing-FD
   smoke tests cannot replace these checks.
3. Complete compiler admission/postchecks/finish/revalidation, V5 publication,
   SubjectV3 transport, native backend receipt admission and conditional
   finalization through the single production compiler path.
4. Validate source/MIR/KIR/machine/numerical proof, generic non-AMD behavior and
   all 47 tutorial kernels on gfx942/gfx950. No protected proof or GPU run is
   credited here.

Fresh SSH attempts to mi350, mi350-2 and mi300x failed DNS in this session. No
remote job or scratch was created. Local commits and test evidence are distinct
from publication: both remote main refs and the issue update must be verified
separately.

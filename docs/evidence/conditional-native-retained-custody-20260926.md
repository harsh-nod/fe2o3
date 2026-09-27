# Native Retained Dependency Custody

Date: 2026-09-26. Continuation of the
[guard and transport checkpoint](conditional-native-root-launch-transport-20260926.md)
for [issue #272](https://github.com/harsh-nod/fe2o3/issues/272).
**No whole M0-M7 milestone or 47/47 production-to-safe-GPU-launch completion is
claimed.** The consuming compiler supervisor launch remains outstanding.

Base: `cf41653789a378636ccaf6b522aa8588f36af783`.
Integrated code: `148e4c318c8b1b338693b823b155757bae684b56`.
Subsequent documentation changes do not change executable behavior.

## Implementation

The existing fixed-capacity cleanup pool now retains a complete dependency
payload in its reserved slot before atomic clone. The original request account
funds the typed view; the original service account independently funds the full
persistent payload. Both pay logical work before allocation. Checked quota
queries include payload alignment, reference-count/mutex metadata and complete
caller-declared transitive storage. No new worker, pool or authority path exists.

`spawn_retaining` uses the same staged-image checks, root gate, clone, pidfd and
artifact-lease custody as ordinary spawn. Its child wrapper exposes no raw owner
extraction or replacement. Exec confirmation releases only the artifact spawn
lease. Pending cleanup, ECHILD, absent pidfd and quarantine retain dependencies;
an unused pre-clone reservation or exact consuming terminal wait can retire them.

Retirement keeps the slot occupied while destroying dependencies outside the
child, payload and pool-mode locks. Releasing preparation may therefore defer
its own anchor into the same pool without deadlocking. The service account
retires storage before the slot becomes reusable. An accounting mismatch or
panicking destructor quarantines the slot; accounting mismatch also stops new
admission. The unsafe bridge still requires bounded, funded, nonpanicking Drop
and complete transitive ownership. Recovery preserves the original account.

`RetainedResourcesV2<T>` uses `Arc<Mutex<T>>`, requiring `Send`, not `Sync`.
Budgeted read-only callbacks receive the original request ledger; they cannot
extract ownership or outlive the view. Mutex poison refuses without recovery.
The API does not make an unsafe interior-extraction implementation sound, and
recursive access to the same owner is prohibited. A real prepared-type test
caught the initial `Sync` requirement: its listener contains a `Cell`. The
implementation was corrected without unsafe `Sync` or weakening the listener.

Worker commits `4c8d448e9` and `36c908cde` supplied the bounded retained owner
and tests in two files. The primary integrated pool/spawn custody and actual
child probes. The worker's final source review found no concrete defect; the
worker ran no builds, tests, SSH or network operations.

## Validation

Pinned `nightly-2026-04-03`, offline dependencies, HIP disabled, one bounded Cargo
command at a time and source frozen during builds. Unit/doc/check commands used
`--frozen`; the existing static script uses `--locked` with offline Cargo.

| Check | Result |
| --- | --- |
| Four-crate GNU unit suite, R2, four threads, no filters | 451 passed, 52 failed, 42 existing ignored; exit 101 |
| Four-crate doctests, R1 | 27 positive and 264 compile-fail passed; exit 0 |
| Fifteen-package all-target check, R1 | Passed with existing warnings; exit 0 |
| Full shared-spawn musl release unit suite, R1 | 119 passed, 2 failed, 0 ignored/filtered; exit 101 |
| Static V1, V2 and V3 gates | All passed, including production image admission and missing-descriptor smoke |
| Changed Rust files, pinned rustfmt check | Passed |

GNU totals are compiler coordinator 53/7/0, supervisor 216/42/42, external anchor
coordinator 63/1/0 and shared spawn 119/2/0 (pass/fail/ignored). Nested subprocess
results are not counted twice. Failures remain 51 socket-operation EPERM and one
ACL fixture EINVAL. Musl failures are the same two live SO_PASSCRED tests. These
suites are not green; no test was disabled or assertion relaxed. All-targets is
not GPU architecture coverage.

Tests cover exact/short request and persistent storage/work floors, arithmetic
and alignment, sticky funding, drop order, poison, unwind, guard cloning under
dynamic accounting, controller recovery and quarantine. A synthetic terminal
schedule proves nested dependency destruction can reenter the same pool without
its locks held. Actual atomic-clone probes retain a lock-bearing dependency
through cancellation, post-adoption pidfd failure and unwind until exact reap.
The isolated subprocess must write a completion marker after the entire probe;
an empty test selection cannot pass accidentally. Both real prepared families
satisfy the retained owner's `Send` bound and checked storage shape.

These child probes exercise the retention reservation plus atomic clone, not a
successful public `spawn_retaining` with an actual compiler prepared owner. Type
checks do not construct that owner or authenticate a deployment. Earlier R1
compiler failure exposed the `Sync` defect described above; the final R2, docs,
all-targets, musl and static checks validate the corrected implementation.

Logs remain in
`/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921/`:

```text
4b30508c93ad386b852ae91eb515ea89bbe77dc3b4c4d879d5fad8d383f65037  conditional-native-retained-custody-all-four-r2.log
e2661e8eaf0cfb2dfc7184927f0602b604563580129a6df7cfd7ed2fea045707  conditional-native-retained-custody-docs-r1.log
b74df1aab8d0bae492561152fc03181e51d381b2f4ae0c11838cb06ee4900ab7  conditional-native-retained-custody-all-targets-r1.log
072f0b126244d59471b0a8f474a973825685d02606e56fb96aee02f8ee81f58c  conditional-native-retained-custody-musl-r1.log
507fc5706e5e936f74674f29d184db1567110d8ec7c6871b4a3f369995d48bdc  conditional-native-retained-custody-static-v1-r1.log
a0d7d0b36172a13ec24ee0468eb4547b511dc42ccfb0fd3a9d69c96bc79b5968  conditional-native-retained-custody-static-v2-r1.log
35cbce498ce732995cace99bdd037197576ff7a9078069b779c4391590686b0f  conditional-native-retained-custody-static-v3-r1.log
```

Static executable SHA-256 values, V1, V2 and V3 respectively:

```text
3dd3305245afb50f35e1ed41c905ead3a7fc0865497509ade3cb8d60f7f92884
abf523c6a2cab475c5194ba5a9d3919321b0e8bb8feef9df22aa84473d40931a
67a150612c65b02c1f52189ac8ebd2f3a06132cf1462ce3f8b1f4be801e424ec
```

## Remaining Gates

1. Transfer actual compiler preparation through retained spawn. Join its actual
   lifecycle/root to the already installed cleanup guard; an independently valid
   anchor guard is insufficient. Preserve that guard through nested anchor reap.
2. Compose consuming native supervisor launch, complete original-ledger quotas,
   final staged-file validation, profile/gate checks, nominal 88-byte readiness,
   EOF/liveness and continuity. Cover every post-clone refusal and unwind.
3. Validate protected startup, live terminal framing, recovery and runtime custody
   on the intended host. Rootless probes and missing-descriptor exits do not
   authenticate a protected deployment or trusted parent provenance.
4. Complete compiler admission/postchecks/finish/revalidation, V5 publication,
   SubjectV3 transport, backend receipts and conditional finalization. Validate
   semantic/machine/numerical proof, generic non-AMD behavior and all 47 tutorial
   kernels on gfx942/gfx950 through that production path.

SSH attempts to mi350, mi350-2 and mi300x failed DNS during this checkpoint. No
remote job or scratch was created. Local commits and evidence are distinct from
publication; both main refs and the issue update require separate confirmation.

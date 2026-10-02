# Pending Peer Segment Composition Qualification

This increment adds a distinct whole-list producer record to the Context journal
and unified compute/XGMI backend. A fully queued compute -> ordered overlapping
segment list -> compute -> scalar peer return -> D2H chain can be driven from
only its final readback stream. Direct list -> D2H is also supported. Public
events can be released after dependent admission without releasing custody.

Baseline: `131569ccac7958d5a45ae6b80902edc31b25b4e7`. This is a runtime
composition increment, not A3 completion, arbitrary kernel authority, HIP/HSA
parity, measured performance, or a new formal refinement of the adapter.

## Supported Profile

The Context must have a version journal and explicit backend capability. The
source is the exact logically Pending producer-aware Ordinary compute result,
with one full-allocation Write binding and no other alias of that allocation.
The destination is a previously initialized, settled PUBLIC DeviceLocal owner.
The descriptor snapshot, source producer, endpoints, stream and dependency rank
remain bound to one private immutable list identity.

The list retains a conservative source-envelope reader and a whole-destination
writer. A downstream Read-only compute binding or D2H may read the preserved
initialized destination frame, including holes outside the copied descriptors.
An envelope is never treated as scalar produced coverage. Output is promoted
only after whole-list success, producer reconciliation and native owner
restoration. ReadWrite/Write consumer aliases of the list destination are
rejected; a separate compute output Write remains supported.

An exact successful, quiescent backend producer receipt also covers the race
where physical compute has completed but Context still regards it as Pending.
Completed Deferred producers retain their original, once-charged launch until
result release. The Context journal supplies historical content currentness;
the raw backend receipt is not a new allocation content epoch.

Quiescent-without-result errors release certified stopped dependencies but leave
writer contents Unknown. Post-admission observation rejection, terminal
uncertainty and unwind retain custody. Preeffect admission rejection and
prepublication cancellation refund the corresponding consumer retains.
Publication prevents cancellation, and a later segment failure may
leave an applied prefix; a list is not an atomic transaction.

## CPU Qualification

Final generation `02`, with no intervening implementation changes:

| Check | Result |
| --- | --- |
| Runtime library | 2,245 passed, 32 existing hardware-only ignores |
| New runtime regressions | 10 Context tests and 8 backend tests |
| Deferred-chain example | 18 passed, including 5 new segment witness tests |
| Settled-segment example | 3 passed |
| Runtime doctests | 61 passed |
| Strict Clippy | Runtime library/tests and both examples passed |
| No-default-feature library | Passed |
| Source controls | All 32 passed |
| Formatting and whitespace | Passed |

The eight backend test functions exercise 31 parameter cases. They cover
prequeued/late consumers, exact event and producer binding, immutable plan
substitution, initialized complements, writable aliases, bounds, cancellation,
prefix failure/unwind, physically completed Native/Deferred producers, and
single-charge lifetime accounting. The Context tests additionally check nested
identity/rank drift, callbacks, logical reconciliation, source write reservation
and stale events after a later write.

The lower `fe2o3-kfd` source and workspace manifests are unchanged. Its previous
1,943-test qualification remains in the
[settled-list checkpoint](../dev-unified-peer-segments-2026-10-02/README.md);
those tests were not freshly rerun here. All 76 referenced executable proof
files are unchanged. Source guards changed only reviewed identity/count
literals, including the duplicate owner pin. No solver was executed and no
new adapter or whole-runtime formal qualification is claimed.

## Native Matrix

All 14 cases passed on MI300X GPUs 6 and 7 in both directions. The independent
offline receipt audit accepted the exact matrix, byte oracles, build/source
identities, 133 native command receipts, final baseline observations and owned
cleanup. The selected devices were UID `0x10a254ce4987e716` at
`0000:C6:00.0` and UID `0x53691ef168a0147d` at `0000:E5:00.0`.

The predeclared matrix covers:

- Prequeued compute/list/compute/peer/readback with 4, 65 and 4,096 descriptors.
- A 4-descriptor consumer admitted after observed list publication, while the
  exact list is retained and still Pending.
- Direct compute/list/full-frame D2H with 4 descriptors.
- Settled-owner regressions with 4 descriptors and packet-spanning descriptors.

The existing finite R57 V2 constructor is unchanged. Full chains perform two
setup launches and two pipeline launches; direct readback uses two setup
launches and one pipeline launch. Pipeline outputs are never installed by the
host. Only the final readback stream drives an all-prequeued chain; late mode
first drives the list just far enough to observe its retained publication.

The independent Python oracle uses integer-quarter arithmetic, applies ordered
descriptors to an initialized destination, and checks hashes of all logical
source/input/output/return/host bytes as applicable. Duplicates and overlapping
writes are retained; reversing the four-entry list changes its output. Return
and host guards and the destination complement are included. Accepted output
requires all 48 fields for a new witness or all 28 fields for a settled control.

## Evidence and Limits

The raw archive includes exact commands, environments, source manifests,
build artifact identities, stdout/stderr, byte oracles, audits and the staged
candidate patch. Earlier failed diagnostics and the superseded source-control
generation are retained, but are not accepted qualification results. The first
aggregate audit also rejected an incorrect carried-forward doctest total: 114
was the preceding combined KFD/runtime count, whereas this fresh runtime-only
run has 61. The final audit checks the exact prior runtime doctest roster.

Each hardware case requires fresh selected-device UID/BDF, activity, VRAM,
process/DRM attachment and host-memory observations. These are point checks,
not an exclusive reservation. Each process is bounded to 300 seconds with
core dumps disabled. Cleanup targets only the owned scratch executables and
directory after confirming no owned executable remains active. No device
reset or foreign process termination is permitted.

Both owned executables and `/tmp/fe2o3-pending-segments-20261002-MPzmQk`
were removed. No owned executable remained active, selected-device activity
returned to zero, and VRAM, process and DRM attachment observations matched the
baseline. These checks do not establish exclusive use during a run.

Remaining gaps include standalone settled-source list -> prequeued frame
consumers, list writes behind pending destination writers, a concrete
application-kernel evidence provider, native generated-argument qualification,
native partial-failure isolation, eight-GPU coverage, physical overlap and
matched HIP/HSA performance. One context per process and logical allocation
bytes are tested; repeat-context lifetime and hidden pool padding are not.

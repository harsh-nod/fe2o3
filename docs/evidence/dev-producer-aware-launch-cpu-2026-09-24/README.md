# Producer-Aware Typed Launch CPU Qualification

All seventeen CPU qualification stages pass. GNU and musl each pass the 27
focused tests, 1,392 runtime tests, 1,561 KFD tests and 1,021 runtime-model tests.
The twenty runtime hardware-only ignores and eighteen model manual performance/
scale ignores remain explicit. All 100 GNU doctests, default-feature checks,
formatting, strict Clippy and tool-identity brackets pass. The 3,960 source inputs
are unchanged across the campaign, with exactly twenty runtime-source paths
changed from the authenticated preceding baseline.

The retained campaign and byte-exact copies replay successfully. The original
collector failed while writing its final cleanup receipt after removing the
owned cache. A separate non-deleting recovery records actual absence without
relabeling that attempt as successful. This is CPU qualification only; native
execution, formal production refinement, performance and A1/A2 closure are not
established.

## Production Scope

The [ordinary typed-launch profile](../../runtime-producer-aware-typed-launch-v1.md)
adds exact event/producer pairs to Context, the existing frozen async owner and
the existing single-/multi-device KFD compute paths. It does not add another
executor, journal, queue implementation or Worker protocol.

Context retains each original binding, module, producer dependency and input
lease until conclusive settlement, including consumers without output writers.
Stable and pending reads share capacity; original read footprints must be
covered by the named producer's writable interval union. Public event release
and dropped observers do not release dependency custody. Typed launches and
directed copies share bounded producer-first reconciliation. Unknown or
uncertain outcomes cannot become successful logical completion.

The multi-device router authenticates both halves of a producer/event pair
against one child. The new accepted-routing fixture exercises both children,
checks the untouched child's accounts and verifies cancellation/refund. Its
pending-producer backing is synthetic; it is not evidence of native GPU
publication or multi-GPU compute.

## Fixed Campaign

`runner.py` records seventeen serial stages using offline/locked Cargo,
nightly 2026-04-03, two build jobs and four test threads. This campaign reuses its
owned development Cargo cache; it is not a cold-build claim. The selected source
map contains 3,960 inputs. Replay requires identical opening/closing maps and
an exact twenty-path runtime-source delta from the authenticated prior topology
CPU packet. Production KFD and runtime-model crate sources are unchanged.

The acceptance policy requires, on each GNU and musl target:

- 27 focused tests, also present in the complete runtime roster;
- 1,392 runtime tests with the same twenty hardware-only ignores;
- 1,561 KFD tests without ignores;
- 1,021 runtime-model tests with eighteen ignored manual performance/scale cases; and
- a default-feature, all-target runtime check.

It also requires strict all-feature/all-target Clippy for all three packages,
formatting, all 100 GNU doctests, and opening/closing compiler and Cargo identity
checks. GNU/musl named outcomes must agree. The runtime roster is exactly the
authenticated baseline plus the 27 new tests; KFD and model rosters must match
their authenticated baselines. Every fixed stage satisfies these requirements.

## Retained Development Iterations

`raw/iterations` contains six completed development transcripts. They were
collected while source was changing and are not frozen-source qualification:

| Transcript | Outcome |
| --- | --- |
| `check-initial.log` | All-feature library check passed. |
| `clippy-library-initial.log` | Failed: large error variant and complex tuple type. |
| `focused-initial.log` | 24 focused tests passed. |
| `runtime-initial.log` | 1,389 runtime tests passed; twenty ignored. |
| `focused-final.log` | 25 passed; one synthetic backing fixture failed admission. |
| `focused-corrected.log` | 26 focused tests passed after fixture correction. |

The Clippy fixes use a bounded private cleanup count and a named preflight
record, without lint suppression. The fixture correction first asserts that
unbacked memory rejects without changing custody, then explicitly marks the
synthetic pending allocation as backed. Production admission was not relaxed.
The subsequently added routed-child test brings the final focused roster to 27.

## Evidence And Cleanup Policy

The verifier authenticates captured helper bytes before executing them and
rejects duplicate JSON keys, nonfinite values, missing/extra command records,
changed controls, failed/unreaped commands, changed output hashes and incomplete
named rosters. The collector runs the full acceptance policy before collection
or deletion, checks command-group absence, copies the campaign byte-exactly,
and verifies the copied campaign before touching the cache.

Only the exact owned private `target` directory may be removed. Its descendants
must be ordinary, owned files/directories on the same device, without symlinks
or mount points. The cleanup receipt records path-accounted allocated bytes and
absence; replay checks actual absence. Nothing on `mi300x` is created or removed
by this CPU campaign. The independent owner-inspection directory is not part of
this packet. The removed cache accounts for 1,366,761,472 allocated bytes.

The original collector used the exclusive-create JSON writer twice on
`raw/cleanup.json`. Its provisional `absent: false` receipt is preserved exactly;
the second write raised `FileExistsError` after cache removal. There is no
archived command/traceback receipt for the original collection attempt. Its
failure attribution is reconstructed from the pinned original collector and
provisional receipt, observed cache absence, and deterministic reproduction of
the original function with the actual JSON helper. The failed collector,
verifier and tests remain in `raw/collection-failure`.

The corrected collector uses distinct immutable initial/final receipt paths.
It was not rerun on the collected packet. `recover.py` performs no deletion: it
authenticates the original inputs, complete campaign, exact copied bytes and
command-group absence, requires the owned target already absent, then writes
separate final and recovery records. `raw/cleanup-result.json` and
`raw/cleanup-recovery.json` retain that distinction. `artifacts.json` covers all
67 raw files, including both the failure inputs and recovery records.

Recorded audit directories bracket packet inputs and capture replay and all
eleven evidence-test groups. `audit1` passes; later numbered audits retain final
documentation checks separately. Tests include actual bootstrap helper
authentication, complete campaign/iteration rejection before deletion, cleanup
failure precedence, reproduction of the original exclusive-create bug, and
rejection of fabricated recovery histories. They do not prove native or formal
runtime behavior.

This is recorded-process evidence under a stable local workspace, not a
hermetic build or hostile-concurrent-filesystem attestation. The frozen runner's
source-finalization `finally` block can replace an earlier top-level exception
if finalization itself fails. Command receipts retain their own dispositions;
the packet does not claim general failure-precedence correctness for that runner.

Replay the recovered packet from its qualified source checkout:

```sh
python3 -I -B docs/evidence/dev-producer-aware-launch-cpu-2026-09-24/verify.py
python3 -I -B docs/evidence/dev-producer-aware-launch-cpu-2026-09-24/test_verify.py
```

## Remaining Gates

The R57 three-binding persistent DeviceLocal path still requires ready backing
at admission. It rejects a consumer submitted after its producer moves shared
bindings into `ComputeInFlight`. Qualifying queued producers does not qualify
already-published producers. That requires separately authenticated deferred
eligibility and genuine ready admission before eventual publication.

No new hardware execution, physical overlap, formal production refinement,
protected Worker execution, aggregate-memory bound or performance result is
established here. Existing individual journal proofs do not prove the new mixed
Context transaction, footprint coverage, shared reconciliation or async
carriage. Native chains, those composition proofs, generated graphs and matched
HIP/HSA performance remain required. A1/A2, issue #182 and the broader accepted
checkpoints remain open/unchanged.

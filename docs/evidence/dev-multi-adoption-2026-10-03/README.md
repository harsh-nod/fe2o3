# Multi-Device Nonpublishing DATA Checkpoint

Source baseline: `2ff3e85451e5f1912651d97912f816c147e2af52`, plus the candidate
source patch in this packet. Qualification date: 2026-10-03 UTC.

## Accepted Scope

The multi-device backend now routes generated reservation, shell registration,
DATA adoption and retirement to the original child. Immutable global and local
plans preserve original carrier ownership, logical IDs, native admission and
member ordinals. Private generated route tables never make generated allocations
eligible for ordinary reads, writes, releases, asynchronous copies or peers.
Both namespaces are rooted before child control transfer. Readiness and failure
quarantine capture the selected child before effects, including commit/disposal
unwinds and retained-route corruption.

Concrete Context implementations share the existing shell, credit and version
journal logic. Multi-device generated preparation tickets now carry reservation,
preflight, readiness, adoption and retirement hooks; issue remains `None` and
the completion API remains unavailable. Ordinary inert tickets remain unchanged.
Pristine adopted DATA prevents conflicting peer extraction and coherent capture
even when no generated-submission index exists.

## Native Witness

The exact repository vecadd fixture was re-admitted against real retained GPU
owners. A test-only packet insertion bypasses protected carrier registration,
but uses the same private child and multi-device commit cores as production.
Production preparation/binding, DATA adoption, retirement, route disposal and
explicit shutdown then execute normally. No `TestAuthority` or fabricated
checked device is used in the native test.

| Property | Observation |
| --- | --- |
| Devices | MI300X GPU 6, UID `0x10a254ce4987e716`, BDF `0000:c6:00.0`; GPU 7, UID `0x53691ef168a0147d`, BDF `0000:e5:00.0` |
| Orders | 6 then 7; 7 then 6, separate bounded processes |
| Each run | Both devices retain DATA; retire first while second survives; re-adopt first on its original primary lane; retire both |
| DATA disposal | Three complete three-buffer rosters, nine releases per run |
| Publication | Zero ordinary/generated submissions, pending compute or completion reservations |
| Shutdown | All generated routes, allocations and stream leases removed; both native queues shut down |
| ELF | SHA-256 `92aca8d18c08152d3a2b3db121d5744f129712d14fe51d14575355573857dbef`, 48,677,576 bytes |
| Host observation | Eight fresh selected-endpoint observations, eighteen bounded command receipts |

The controller pins the executable inode/digest and retains each owned process
group through cleanup. Observations reject active selected GPU use or attached
processes; they do not establish an exclusive performance reservation. No resets,
fault injection, all-GPU examples or unrelated cleanup were performed.
The owned `/tmp/fe2o3-multi-adoption-20261003.nh8mSBSZ` directory was removed
after evidence retrieval; cleanup found no remaining owned processes.

## CPU Coverage

Nineteen new CPU regressions cover colliding child-local handles, independent
retirement, range/capacity rejection, wrong plans and replay, private and ordinary
aliases, source substitution, captured-child commit/disposal quarantine, terminal
latching, nonvacuous public copy rejection, per-device credit/journal accounting,
atomic second-roster capacity rejection, equal-size cross-device credit swaps,
and pristine DATA peer/capture exclusions. Metadata fixtures do not claim native
DATA acquisition.

Final-source qualification passes 2,378 runtime tests with 33 explicit hardware
ignores, 170 example tests, 71 doctests and all 33 source-control commands.
Strict library and preparation-example Clippy, the no-default-feature library
check, formatting and whitespace checks pass. The captured runtime test roster includes all
2,411 cases; the separate native controller runs only the new exact ignored
two-device DATA test, once per device order.

## Evidence Packet

`qualification.tar.xz` contains the exact candidate source patch, source
inventories, bounded command receipts and raw output, test roster, metadata-only
checker audit, native controller, executable identities, observation replay and
cleanup receipt. Executables themselves are excluded. `MANIFEST.json` binds
every archived file and `SHA256SUMS` binds this README and the archive.

The final evidence audit passes: 6,374 unchanged source files per command,
51 accepted command receipts, all 2,411 listed runtime cases accounted for,
18 native command receipts, two native cases and verified remote cleanup.
The observation replay verifies both device identities, empty selected process
sets and the before/after timing brackets against the actual command records.

## Limits

This closes the nonpublishing multi-device DATA routing work item, not A3 or
HIP/HSA parity. The native test composes privately opened finite qualification
children and descriptive test Context IDs. It does not qualify public Context
registration, protected Worker execution, generated dispatch/completion, a
generated-storage peer handoff, physical overlap or performance improvement.

Seventy-six existing proof files remain unchanged. Source-control metadata
refreshes bind the new source inventory without changing checker logic. No new
solver run or whole-Context/backend formal refinement is claimed.

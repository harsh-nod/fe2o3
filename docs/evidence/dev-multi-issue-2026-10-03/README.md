# Multi-Device Generated Issue Checkpoint

Source baseline: `24aa9f38b9e022b479bde267c1028d84235961e2`, plus the candidate
source patch in this packet. Qualification date: 2026-10-03 UTC.

## Accepted Scope

Generated issue, physical progress, full-roster readback and receipt release now
route through the multi-device backend. A private global submission map captures
the original child and immutable global/local shell plans. Child-local submission
IDs may collide without aliasing. Ordinary submission capacity, handle creation,
shell disposal and shutdown account for retained generated receipts.

The global entering record precedes child preparation. Malformed returned IDs,
post-mutation rejection, terminal errors and unwinds retain custody and quarantine
the captured child. A rejection removes only an authenticated empty entering
record. DATA retirement precedes receipt release; release validation deliberately
survives the transition from Adopted to Retired.

Single-device and multi-device Contexts share issue/completion bodies, journal
readers/writers, closing-authority checks and completion settlement. Multi-device
generated async preparation now installs issue/completion hooks and supports
owner-thread activation. Ordinary inert preparation remains hook-free. Generic
poll/wait/drain only observe: Ready cannot publish, and physical Recycled reports
Quiescent without a delivered result, never Succeeded. Generic generated events
and cancellation remain unsupported.

## Native Witness

The exact repository vecadd fixture was re-admitted against retained native GPU
owners. Test-only packet insertion bypasses protected carrier registration using
the same private commit cores as production; it does not fabricate a checked
device or use `TestAuthority`. All issue/readback/retirement operations then use
the new multi-device routing methods and existing child implementation.

| Property | Observation |
| --- | --- |
| Devices | MI300X GPU 6, UID `0x10a254ce4987e716`, BDF `0000:c6:00.0`; GPU 7, UID `0x53691ef168a0147d`, BDF `0000:e5:00.0` |
| Orders | 6 then 7; 7 then 6, separate bounded processes |
| Each run | Issue on both children, retire first independently, re-adopt and dispatch on its original primary lane while sibling DATA remains readable |
| Native work | Three real dispatches and twelve DATA releases per run |
| Readback | Complete left/right inputs, exact output plus 16-byte guard, untouched fourth 80-byte read-only buffer; destination pointers/capacities unchanged |
| Rejection checks | Foreign roster identity and premature readback leave destinations unchanged; premature release and unsupported event/cancel reject |
| Observation checks | Ready poll/wait/drain/flush/progress cannot publish; completed generic observation remains Quiescent |
| Shutdown | Private/ordinary submissions, generated storage and stream leases empty; both queues explicitly shut down |
| ELF | SHA-256 `5d3293206f523aa9df4d4297d7578de4e37bb23b15d4c44d87cc059f78d3fc48`, 49,218,280 bytes |
| Host observation | Eight fresh selected-endpoint observations and eighteen bounded command receipts |

The controller pins the executable inode/digest and retains each owned process
group until reaped. Selected-endpoint checks reject active use or attached GPU
processes; these observations are not an exclusive performance reservation.
No resets, disruptive fault injection or all-GPU examples were run. After evidence
retrieval, `/tmp/fe2o3-multi-issue-20261003.fl6a0xk2` was removed; cleanup found no
remaining owned processes.

## CPU Coverage

Twelve new tests cover exact routing with colliding child-local IDs; global
capacity/identity rejection before mutation; independent retirement and replay;
wrong plans, corrupted routes and aliases; retained entering custody after bad
returned identities, terminal errors, misclassified rejection and unwind; per-child
Context readers/writers/credit settlement; cross-child settlement rejection;
duplicate issue/identity failure; and actual Context/async convenience entrypoints
rejecting synthetic devices before carrier lending, source access or decoding.

Context positive-tail fixtures explicitly assume native settlement and test only
bookkeeping. Backend metadata fixtures contain no native handles or fabricated
DATA release counts. They are not native execution or Worker admission evidence.

Final-source runtime qualification passes 2,390 tests with 34 explicit hardware
ignores, 170 example tests and 71 doctests (12 runnable/compilable, 59 compile-fail).
All 33 source-control commands pass, as do strict library/preparation-example
Clippy, the no-default-feature library check, formatting and whitespace checks.
The separate native controller runs only the new exact ignored two-device issue
test, once per device order.

## Evidence Packet

`qualification.tar.xz` contains the candidate source patch, source inventories,
bounded command receipts and raw outputs, complete test roster, metadata-only
checker audit, native controller, executable identities, observation replay and
cleanup receipt. Executables themselves are excluded. `MANIFEST.json` binds every
archived file; `SHA256SUMS` binds this README and the archive.

The final evidence audit passes: 6,378 unchanged source files per command,
51 accepted command receipts, all 2,424 listed runtime cases accounted for,
18 native command receipts, two native cases and verified remote cleanup.
Observation replay checks both device identities, empty selected process sets
and exact before/after time ordering against the actual command records.

## Limits

This closes the generated multi-device issue-routing work item, not A3 or HIP/HSA
parity. The native fixture composes privately opened finite qualification children
and descriptive Context IDs. It does not qualify public Context positive-path
execution, protected Worker admission, original generated typed-result delivery,
a generated-result peer pipeline, physical overlap or performance improvement.

Seventy-six existing proof files are unchanged. Checker refreshes change audited
source-identity/count metadata only, not predicates or proof bodies. No new solver
run or whole-Context/backend formal refinement is claimed. Production Worker
authority and authenticated completed-result upload/peer composition remain the
next application-path prerequisites.

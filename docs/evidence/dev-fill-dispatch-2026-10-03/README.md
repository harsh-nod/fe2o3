# Whole Fill Dispatch Projection

This checkpoint composes the exact gfx942 fill wave model across an entire
full64 dispatch. It does not complete application admission, establish actual
hardware execution, grant GPU authority or demonstrate HIP/HSA parity.

Parent commit: `9526e023380db65da143e3e0cccb140a601ee431`.

## Implementation

`Gfx942FillKernelV1::check_dispatch` creates a move-only
`Gfx942FillDispatchV1` borrowing the inspected exact 68-byte kernel. It retains
the immutable 16-byte kernarg snapshot, its address, output region and geometry.
Four single-arm shared macros implement validation, projected entry construction,
group execution and byte observation in both Rust and Verus.

Acceptance is proved iff: grid X is positive and divisible by 64, grid Y/Z are
one, workgroup is [64,1,1], decoded length N does not exceed grid X, and decoded
output pointer matches the aligned output region of exactly 4*N bytes. Kernarg
is 8-byte aligned; checked region ends do not overflow and nonempty output is
disjoint from the 16-byte kernarg region. Empty output is allowed with a positive
full-wave grid. Short-circuit validation guards subsequent arithmetic. The
largest accepted grid is 4294967232 workitems; no grid-sized allocation occurs.

`execute_group` rejects out-of-range groups, constructs s0:s1 kernarg, s2 group,
v0 lane and full EXEC, then invokes the existing shared wave executor. Other
registers come from an arbitrary seed, not an observed native entry snapshot.
The projection is tied to the inspected descriptor's selected register profile.
`byte_after` derives the sole potentially writing group and observes its actual
sparse modeled stores. It uses constant storage and at most one 64-lane
execution, independent of grid or output length. Outside bytes retain their
input value.

The formal composition proves exact acceptance, entry preconditions, group
range, exact index stores, unique group/lane coordinates, disjoint four-byte
intervals, output coverage and little-endian byte values. Each modeled group
terminates at PC 0x44 after 10 or 14 instructions. Repeating `execute_group`
repeats a simulation; the API does not schedule work or prove actual exactly-once
hardware execution.

The original wave theorem bodies and shared executable wave body are unchanged.
Only its leading doc comment became an ordinary comment for proof inclusion;
the Rust parent exports the new child module. The existing wave harness pins
were updated for those two changes, not to weaken its checks.

## Qualification

| Campaign | Cumulative obligations | Logical mutants | Harness controls |
| --- | ---: | ---: | ---: |
| Complete wave regression | 30 | 16 | 14 |
| Dispatch composition | 44 | 11 | 11 |

Both campaigns accept, with positive runs before and after mutations, identical
before/after source snapshots, pinned Verus release checks and owned child-group
cleanup. The 44 dispatch obligations include the 30 wave obligations; these are
not 74 independent obligations. The pinned release closure contains 190 files
and 129019839 bytes. No assumptions, admits or external bodies were added.

The 11 new mutants cover zero/partial grids, underlaunch, region overlap, wrong
kernarg/group/lane registers, partial EXEC, missing last group, wrong byte-query
group and changed outside bytes. Controls reject parse/type failures, timeouts,
wrong obligations and unexpected diagnostics. Proof-annotation failures without
a macro-expansion location require the exact enclosing function and, for the
byte-group precondition, exact primary and secondary spans. Synthetic controls
exercise missing function notes and wrong secondary spans. The final dispatch
campaign contains 16 accepted stages.

Kernel-analysis library tests pass 63/63 with 2 default ignores, and its
ownership doctest passes 1/1. Six new dispatch tests cover N=0/1/63/64/65/4097,
extra empty groups, all output and guard bytes, maximum grid, near-u64-limit
regions, adjacent regions, 19 input mutations, exact initialized/preserved
registers and equal memory effects across contrasting seeds. The two ignored
tests were not run; no native worker or GPU execution is claimed here.

Strict library Clippy, targeted rustfmt and whitespace checks pass. The archived
`run-checks.sh` records locked/offline commands, the pinned nightly and enabled
overflow/debug checks. `run-proofs.sh` records the two qualification commands;
reruns need fresh owned output directories.

Development diagnostics are retained separately. The first maximum-grid test
correctly rejected a fixture whose kernarg lay inside its large output region;
moving the fixture address corrected it without weakening validation. The first
dispatch campaign correctly rejected an unrecognized proof-annotation failure;
the final classifier requires its exact diagnostic provenance and passes the
added controls. Only `qualified-dispatch-final` and `qualified-wave` are accepted
campaigns.

The code-only patch relative to the parent has SHA256
`baad3b4c8267d4141819dddf701ed73528d1a2cf42b59461ac6612b5d60d3a14`.
The [qualification archive](qualification.tar.xz) has SHA256
`dc8d3b7c1d0fcec79b4a6f1d9a5263f3411d80effc4302a161bb5639ec454c13`.
Its 858-entry listing and extracted code-patch digest were checked. No MI300X
resources were created or changed. Owned local scratch is removed only after
archive validation and terminal test/proof sessions.

## Remaining Critical Path

The snapshot and address regions are not authenticated reads or allocation
owners. Ordinary wave64 execution, disabled VSKIP/GPR indexing and 64-bit global
addressing remain explicit premises. This proof does not establish ISA
interpretation, native register values, real group scheduling, memory backing,
visibility, completion or source/compiler refinement.

Next join the checked whole source/neutral KIR/replayed target KIR owners and
protected finalizer lineage to the authenticated analyzer's exact HSACO,
selected descriptor and complete fill profile. Retain the universal machine
contract in a distinct conditional Worker artifact profile. Consume coverage
only at the transition bound to the actual prepared dispatch, selected device
and patched storage. Existing unconditional admission remains unchanged.

Then qualify genuine admitted fill -> tracked upload -> native XGMI -> guarded
readback for N=64/65/4097 in both selected-device directions. N=65 requires grid
X=128 in this deliberately full64 model. Existing native multi-GPU mechanics
and synthetic staging evidence do not substitute for the application path.

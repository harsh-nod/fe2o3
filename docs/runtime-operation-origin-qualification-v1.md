# Runtime operation origins: foundation and actual-source qualification

Recorded on mi350-2, 2026-09-19 UTC.
This qualifies an opt-in in-process simulator observation and its bounded
acceptance harness. It does not close #281 V2, change public debugger identities,
or qualify source ownership, production resume, physical registers or hardware.

## Implemented boundary

The existing SimulationDebugSinkV1 has default-compatible opt-in origin delivery.
One fixed-size context accompanies one existing debug record. Available contexts
bind full invocation, actual static KIR site, monotone frame activation and
operation attempt. Before, memory and After records for one attempted operation
share that token; repeated sites and reused helper slots get distinct tokens.
Caller call attempts remain pending across helpers/yields. Invocation selection
clears stale context, including selection of the same representative invocation.
Both barrier and transpose releases explicitly lack a single-lane origin.

Tokens are local to a live simulation and full invocation. They are not global
run identifiers, serialized frame handles, per-site loop-trip ranks, source
authentication or proof of successful completion. Attempt capture can precede a
step-limit failure. Context is allocation-free with a <=256-byte size assertion;
a retaining sink must independently bound its own storage. Existing fixed
runtime fields participate in the resident census through actual sizeof values.

No SimulationDebugRecordV1/SimulationDebugFrameV1 wire layout changes. Existing
sinks adapt to the old callback by default. Legacy execution/results/records and
Stop/Drop behavior are preserved. General per-frame/caller contexts, debugger
retention/query integration, step-over/out changes, allocation generations and
CLI frame/occurrence semantics remain outside this foundation. Resource V1
call_frame/operation_occurrence/source_association remain NotRepresented.

## Recorded foundation/control gates

All ran on mi350-2 under the primary's serialized resource guard.

| Retained gate | Result | Scope |
| --- | --- | --- |
| phase11-debug-state-r2 | 19 passed | Private state transitions, counters, suspension and invariants |
| phase11-debug-origin-r2 | 8 passed | Actual interpreter over synthetic KIR: loops/helpers, memory, yields, aggregate releases, legacy compatibility and fault attempts |
| phase11-debug-full-r1 | 273 passed | Full simulator test suite on that frozen foundation snapshot |
| phase11-debug-example-r4 | 13 passed | Synthetic observer/CFG/coordinate/oracle controls |
| phase11-debug-pure-r7 | 9 passed | JS validator/identity-domain/coordinate/process controls |

Counts overlap and are not added. The foundation full-suite Rust census was
eeead382f8924e21657bdec7aff895eb00ec0322c02a462605b8a219e58fbb24.
The final observer/control Rust census was
ee077fddcaf1084a728a6b840353445f6c2e3d64d57a5dea00c761db48fd06b6.
Each named gate retained equal before/after censuses. These are working-tree
snapshots based on f5e81f985ff3e2771ad0f132d483f5cf74976ad6, not retroactive clean
commit or runtime-closure attestations. Later integrated regressions/publication
must be recorded separately by the primary.

## Actual ordinary-source run

phase11-runtime-source-r8/receipt.json passed all joins and 12 successful command
stages. Receipt size213,039 bytes; SHA-256:
9c825a890ea68d80081d3e1f5d66c537f0b1b6884c6b33ed0c414cabfcb4bac1.

The fresh 718-byte no_std source declares a typed gfx942/Wave64/WG64 kernel,
control_flow(loop_bounds(3)), trips = rounds % 4, a checked iteration += 1, and
a pure inline(never) helper computing (value ^ salt) & 0xffff. Normal source
export produced a verified Bundle V6 / canonical KIR V11. The observer first
required an actual entry-reachable CFG cycle containing the retained pure
helper call; source syntax or symbol names were not substitutes.

Six contextual CPU executions and six explicit opt-out executions use canonical
and seeded71 schedules, four logical invocations, seeds abcd1234/0f0f55aa, and
trip cases0/1/3. Each compares complete execution results and compact legacy
records, all four output words and initialization, and both surrounding canaries.
The independent oracle retains rounds & 3 rather than copying the source's
modulo expression. This is differential acceptance, not universal proof.

| Trips | Contextual schedules | Expected output word | Steps / records per contextual run | Observed helper activations per run |
| --- | --- | --- | --- | --- |
| 0 | canonical, seeded71 | 0xabcd1234 | 88 / 116 | 0 |
| 1 | canonical, seeded71 | 0x0000479e | 140 / 180 | 4 |
| 3 | canonical, seeded71 | 0x0000479d | 244 / 308 | 12 |

The six contextual runs retain1,208 records and32 checked helper activations.
Those counts are derived from retained rows, not copied into a second success
count for the opt-out runs. Before/After call intervals, helper activations,
memory origin and all canary/output joins are checked independently in JS.

### Distinct identity/coordinate domains

Actual runtime call site is [function0, BlockId7, operation1]. Its authoring
coordinate is [function0, block roster ordinal3, operation1]. The entry cycle's
BlockIds are [10,7,8,4]. The observer emits both coordinate forms from the same
decoded Module enumeration; source inspection uses only explicit authoring
coordinates. No name/span search or positional BlockId assumption remains.
Synthetic controls use nonpositional block IDs17/42/99.

Canonical bytes are1,531 bytes. Their raw SHA-256 is
34a2712cca4342144ca5c2657fb32102a22745a09f4150f43a65456a64188d4b.
The canonical model's domain-separated digest is
a1d719ea7e418e3872a1ff90b63c043f85f67170ac4bc9078e835bb5455691cd.
These are distinct contracts: inspection joins the latter, never the raw hash.

Other exact retained inputs:

| Input | Bytes | SHA-256 |
| --- | --- | --- |
| Source | 718 | 8dde544187f900e690a74bc550bca6c00ce75f48149b867d39f27d9841875725 |
| Bundle | 38,348 | dec8950fafd14d1322a999f7ff1b92b957f6664d135549ed8d6a4a3f19ac9bbf |
| Same-run census | 4,820 | e1cc8958e4b9b426e21e244f912dfa2efa95463b74ba15cce5392eb9fcd0ddab |
| Observer output | 39,874 | bd4aa4a9a3b8ac408c64dce4c68db2a96cf410636148f27d8cac00933f7375ad |

The census supplies measured same-run source attribution, not source-to-SSA
ownership or authenticated producer custody. The receipt measures selected
runner/compiler/executable/driver inputs before/after; it does not attest the
full runtime closure.

## Failed attempts remain retained

| Run | Actual stopping point; never counted as expected semantic rejection or success |
| --- | --- |
| r1 | Export setup repeated --locked already supplied by the exporter |
| r2 | Frontend correctly required the explicit control_flow loop declaration |
| r3 | Wrapping increment did not match the admitted checked induction latch |
| r4 | BitAnd bound was outside exact total uniform-index projection; equivalent modulo4 uses its existing nonzero-remainder rule |
| r5 | Export passed; observer omitted a real Switch at BlockId4 with two cases. Exact retained-bundle forensic decode identified it; exhaustive case/default successors and bounded negatives were added |
| r6 | Export and12 CPU runs passed; JS incorrectly compared raw canonical SHA-256 against the domain-separated model digest |
| r7 | Export and12 CPU runs passed; attribution confused runtime BlockId with authoring block ordinal |
| r8 | Fresh source export, observer, inspection and exact final JS joins all passed |

The retained-r5 forensic decode is not another fresh source success. No compiler
admission/projection check was weakened, and no failed receipt was relabelled.

## Bounds and reproduction

The harness uses source16KiB, bundle256KiB, census256KiB, report512KiB,
per-stream1MiB and total capture4MiB limits; export300s/observer60s deadlines;
two build jobs,40GiB free disk,64GiB available RAM and20GiB combined private
cache ceilings. CFG limits are64 blocks/256 operations/256 SSA values,64 cases
per switch and256 actual case/default edges. Closure work is bounded by64^3.
Observer rows are <=128 bytes, at most4,096 with actual Vec capacity charged
against512KiB. Exact simulation/capture limits and command inputs are retained
in the scripts and receipt.

Build/test the example and scripts with the pinned nightly2026-04-03 toolchain,
offline/locked, then run scripts/debug-runtime-origin-source-v1-smoke.mjs with
a fresh output path and explicit bin-dir, observer, rustc, cargo, rustc-driver,
cargo-home and both resolved cache roots. The checked-in runner records the
exact commands and resources. Reading retained evidence does not launch a GPU.

The next private retention prototype is outside the repository and UNRUN.
It does not alter this foundation receipt or satisfy production debugger,
public CLI, allocation reuse, live service, physical-state or full V2 exits.


## Final isolated regression pass

The final observer source snapshot also passes the full simulator suite
(`phase11-debug-full-r2`, 273 tests), debugger/protocol/CLI regression suites
(`phase11-debug-regression-r1`, 296 passed, 3 ignored), workspace formatting
and workspace policy (`phase11-debug-format-r1` / `phase11-debug-policy-r1`).
All use the final `ee077fdd...` Rust census with unchanged before/after inputs.
These isolated results do not replace combined-tree validation after integration.

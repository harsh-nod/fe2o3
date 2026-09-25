# Constructor-Origin Mixed-Reader Lifecycle Qualification

Status: the second signed-source campaign passes all sixteen stages, followed
by retained replay, ten calibration groups, byte-exact collection and owned
cleanup. The first campaign stopped at
`missing-atomic-event`: its postcondition failure was accompanied by solver
resource exhaustion, so the checker refused it as a logical negative. The
opening 1,271-obligation positive and eight preceding logical negatives passed;
the closing positive and regressions were not reached. Its complete artifacts
remain retained as rejected history, not qualifying negative evidence.

Qualified proof source: `33b4606f4134045757ce6f6d340e829ba79e2314`, signed on
`codex/r65-runtime-drain-versions` and published to both topic remotes.
Production runtime sources are unchanged by this proof extension.
The rejected first campaign remains bound to
`145c19485eb5dce46ceea6df7aebc8e363b2a933`. The correction retargets only the
atomic-event mutation and strengthens its calibration; proved source, contracts,
verifier limits and expected counts remain unchanged.

## Scope

The existing owner lifecycle now composes mixed stable/producer input
acquisition with both reader histories. Constructor-origin traces cover late
input rejection, full shared capacity, reused stable and producer arenas,
producer settlement and slot reuse, exact release-trace continuity, and Unknown
retention. Input matching does not assume the result or final output vectors;
the independent logical correspondence derives those and final represented
ownership. Overflow remains modeled rather than excluded by admission.

The release witnesses supply identity-matched quiescence evidence. They prove
the journal's response conditional on that evidence, not native quiescence or
its production provenance. Context map custody, completion reconciliation,
async carriage and compiler/device refinement remain separate boundaries.

## Qualification Policy

The signed checker binds 462 source inputs and the pinned 190-file Verus
release. Its sixteen stages require both complete 1,271-obligation positives,
nine logical mutation rejections, unchanged 738-obligation mixed and
720-obligation stable regressions, checker calibration and tool-closure
brackets. Timeouts, frontend errors and solver resource exhaustion do not count
as successful negative tests. No solver limit is increased for this packet.

Publication additionally requires relocated read-only replay and all ten
checker-calibration groups. `audit.py` authenticates inherited controls before
import, verifies exact publication receipts and replays the retained campaign.
The four focused publication tests exercise metadata, timing/order, boolean
substitution, command, roster and transcript corruption; these tests do not
replace the Verus campaign.

## Results

| Gate | Result |
| --- | --- |
| Whole-root positive before/after | 1,271 obligations each, zero errors |
| Logical mutations | All nine rejected without frontend/resource failure |
| Frozen mixed/stable regressions | 738 and 720 obligations, zero errors |
| Source and tool continuity | 462 source inputs; pinned 190-file Verus release |
| Retained replay and calibration | Passed; all ten checker groups |
| Publication corruption tests | Four groups passed |

The earlier development attempts and rejected first campaign are archived in
`raw`; only `raw/campaign2` supplies the accepted campaign results. Default
solver limits and the fixed four-thread/wall-time bounds are unchanged.

## Retention And Cleanup

`collect.py` held both campaign locks, retained all 1,068 files from the exact
owned scratch tree, replayed and calibrated the copy, then rechecked source
metadata, controls, inventories and thirty recorded terminal process groups
before removal. Independent path absence passed. It removed 23,113,728
path-accounted allocated bytes. Failed development attempts remain in `raw`;
they are not counted as accepted logical mutations. Cleanup makes no global process-
absence claim, and allocated bytes are path-accounted rather than filesystem-
wide free-space measurements. No MI300X resources were created by this packet.

The separate [CPU regression](cpu-regression/README.md) adds a four-case
discarded-producer-result test, not production behavior. GNU/musl each pass
1,404 runtime tests with 22 hardware-only ignores; 46 doctests, strict Clippy and
formatting pass. Its raw logs and source patch are retained and its two owned
build/log directories were independently confirmed absent. These ordinary CPU
results are not part of the signed Verus source profile or native qualification.

Replay from the repository root at the signed commit that publishes this packet
(or a descendant with the same 462 source inputs). A later production change
requires checking out that historical qualification commit; replay deliberately
rejects changed inputs rather than qualifying them by inheritance.

```sh
python3 -I -B docs/evidence/dev-owner-mixed-lifecycle-2026-09-24/audit.py
```

The auditor verifies the signed evidence roster and bytes, the full packet seal,
publication receipts, retained failed history and the qualified proof campaign.
It does not rerun Verus or infer hardware correctness from model proofs.

## Remaining Gates

A1/A2, #182 and full HIP/HSA parity remain incomplete. This work adds no native
device, physical overlap, fault, aggregate-memory or performance qualification.
The next proof boundary is production completion reconciliation,
including partial cleanup prefixes rather than an assumed atomic settlement.

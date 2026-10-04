# Budget decision proposal

Status: **unaccepted proposal; no implementation limit changes.** All numbers below identify their evidence class. Existing logical compiler/session/import limits continue to apply independently. A larger proposed RSS or wall-time ceiling cannot override a smaller source, wire, ledger, parser or profile limit.

## 1. Retain the existing proposed compiler/action policy

These are the explicit proposal rows in the pinned `docs/assembly-authoring-integration-contract-v1.md`, not newly measured performance or accepted service guarantees. Retain them for owner decision instead of silently replacing them with qualification-runner timeouts.

| Workload | Proposed wall-time ceiling | Proposed memory ceiling / bound | Measurement or decision still needed |
| --- | --- | --- | --- |
| Pinned two-architecture catalog regeneration | p95 30 s | Peak RSS 512 MiB | Current reproducible regeneration sample and catalog owner acceptance |
| 1-, 3-, 16-instruction generation and validation | Warm p95 250 ms per stage | Retained logical payload 64 MiB | Per-stage source/target/version-specific samples; do not count whole compiler startup as warm stage time |
| Exact inspect/select/materialize action | Warm p95 250 ms per action | Retained logical payload 64 MiB | Original source/region identity and create-new outcome must be retained through timing |
| Named fixed recipe, up to 64 eligible operations | Warm p95 500 ms | Retained logical payload 128 MiB | Exact/rebind/refusal/cancellation paths and current policy, not arbitrary transformations |
| Resource page / complete bounded sweep | p95 25 ms / 250 ms | Existing page and scan bounds | Separate real query work from UI rendering and transport |
| Original import size class, at most 2 MiB | p95 250 ms parse/hash/guard | JS heap delta 128 MiB | Pre-read size guard, post-decode bounds, complete validation; not an end-to-end first-open claim |
| Guard / render | p95 25 ms guard; 100 ms actual render; 250 ms bounded synthetic render | At most 64 rendered rows | Browser/environment/fixture pins and accessibility-visible result |
| Selection/page/reverse observation | p95 100 ms | At most 64 visible rows | Exact current revision/page anchor, not speculative old content |
| Invalidate/cancel | 100 ms UI invalidation; 1 s cooperative authoring acknowledgement | Original bounded owner lifetime | No guarantee of hard process cancellation or rollback |

The integration contract separately records existing safety ceilings of a 64-operation/256-value region, 64 KiB source snippet and 256 KiB report with bounded paging. These broader ceilings are not permission to widen the smaller 16-step ordered-program or any current physical profile. Admission stays at its existing smaller bound unless separately allocated and implemented.

## 2. Existing measurements that can inform, not decide, the policy

The pinned integration contract records the old resource-window workload: three warmups and twenty measured sweeps. For 4/64/128 invocations, page p95 was 0.581/0.664/0.652 ms; whole-sweep p95 was 0.715/6.404/12.691 ms; recorded RSS was 16.10/20.67/27.27 MiB. Actual guard/render observations were 0.4/4.2/5.0 ms. A separately synthetic 256-row/64-rendered-row case measured 4.6 ms guard and 26 ms render. These are named historical workloads, not current V20/V21/V22 measurements or compiler-wide RSS accounting.

The site production baseline at historical `b3272c2` used twenty fresh browser contexts and five samples per workflow/viewport. It measured expected DOM plus two animation frames, **not paint completion**:

| Workflow | Desktop p95 | Narrow p95 |
| --- | ---: | ---: |
| First navigation | 356.0 ms | 351.2 ms |
| First program open | 437.7 ms | 451.5 ms |
| Warm navigation | 34.3 ms | 30.3 ms |
| Warm program open | 69.4 ms | 73.4 ms |

Heap medians at selected checkpoints were approximately 8.3–9.7 MB, not a measured peak. The two retained fixtures totalled 959,617 bytes. Evidence is the exact published `docs/authoring-ui-production-baseline-20260919.md` pin in EVIDENCE.json; this packet performs no new timing.

## 3. Explicit new proposals needing owner approval and measurement

1. **Existing small UI workload:** propose p95 **500 ms first open** and **100 ms warm navigation/open** for the same at-most-1-MiB combined fixture class. This is end-to-end expected-DOM timing, separate from the original 250 ms parse/hash/guard stage. The historical maxima above fit the proposal but do not qualify the current site, new panels, mobile devices generally, or every imported file.
2. **V22 large recorded-import class:** propose p95 **1,000 ms parse/hash/decode/guard** for the existing at-most-11-MiB aggregate import and p95 **100 ms warm bounded page/render**. Propose **128 MiB heap delta** for that operation, measured separately from compressed input size, retained validated state and process RSS. These values are explicitly **unmeasured**; approval and fresh representative/worst-bound measurements are required. This is a new named size class, not a relaxation of the legacy 2-MiB parser class, compressed-stream caps or expansion checks.
3. **Live CPU route:** retain actual transport/session deadlines below; do not invent a 100 ms end-to-end debugger-response SLA from the rendering target. Measure transport, query execution, projection and rendering separately before agreeing such an SLA.
4. **Compiler versus public action:** retain source eligibility and fixed logical ledgers. A developer qualification taking minutes or a 40-minute outer runner is not evidence for a 250 ms public action. No new worker/process pool or warm-cache model is assumed here.

Recommended measurement protocol for an accepted successor: pin source/site/compiler/browser/tool/fixture/configuration identities; record cold and warm series separately; at least five warmups and thirty measured operations per workload/viewport; retain every sample, refusal, timeout and outlier; report p50/p95/max, logical work/retention, JS heap delta and RSS as separate quantities. Use monotonic original-start timing through final decode/projection/publication where applicable. Define cancellation and possible-publication outcomes before testing. This protocol itself is proposed, not retroactively imposed on older evidence.

## 4. Existing hard caps: retain separately from latency proposals

| Closed profile | Actual existing envelope recorded in shipped docs | What it does not establish |
| --- | --- | --- |
| V20 live bridge | 128 KiB canonical input; 16 KiB request file; outer request 128 KiB; inner line 64 KiB; 255 commands, four connections; 30 s request/900 s session/5 s HTTP/35 s client post-decode deadlines; stdout 8 MiB, stderr 64 KiB | No throughput/latency promise or successful continued-page qualification |
| V20 live queries | Step/reverse 1–64; seek 0–8193; value page at most64; memory at most256 bytes; fixed output allocation1/generation0 | Browser-supplied allocation/token cannot become source or memory authority |
| V21 recorded import | Container 256 KiB, request16 KiB; at most256 pairs, request line8192/response65536 bytes; at most8192 captured records,64 SSA/page,256 memory bytes/query;4096 bytes/backing,16384 total SSA values,65536 memory cells | Largest retained actual container119035 bytes and48 pairs is not a worst-case cap measurement |
| V22 CPU session/index | Original512 MiB cumulative storage and2^29 work ledger;16384 record bound; index8 MiB,8 allocations,768 bindings and8 pending values/row, row16 KiB; index workspace8,454,144 bytes and50,462,720 work prepaid in the original ledger | These are logical budgets, not allocator/RSS or wall-time bounds; index reservation persists until teardown |
| V22 recorded viewer | Compressed1 MiB/expanded8 MiB index; aggregate11 MiB; request16 KiB; JSONL1 MiB each,192 pairs;8192 projected SSA and8192 memory cells;16384 index rows | Decompression cap is checked before parser/projection; no new live route or runtime memory ownership |
| Fixed source-local-order recipe | Recipe8192 bytes; exact named policy and source/target/launch applicability | Real report's max logical storage67,797,289 and work91,681,379 are one campaign's ledger observations, not proposed general64-MiB policy compliance or process measurements |

Do not combine incompatible profile maxima into a fictitious single session. Existing V20/V21 limits are unchanged by V22. All parser/work/storage one-short, overflow, caller-floor, stale/refused and cancellation tests remain separate from wall-time measurements. The recorded recipe maximum is slightly over64 MiB but below the proposed recipe128-MiB row; it must not be presented as evidence for the separate64-MiB action row.

## 5. Owner decision required

Performance/compiler/source owners must accept or replace each applicable compiler/action row with a named measured workload. Viewer/tutorial/query owners must accept the actual timing boundary, small/large size classes, viewport and memory definition. Exact selected fixture pins and new measurement receipt hashes belong in the acceptance record. If measurement fails, retain refusal/currentness semantics and revise the proposal explicitly; do not silently increase a safety cap or drop slow/refusing samples.

M0/U0 require agreed generation/action budgets; V0 expressly requires measured UI budgets. Existing historical measurements can be accepted as named baseline evidence only by an explicit owner decision about their applicability. This packet cannot decide that for them.

## 6. Later retained route evidence — 2026-09-26

This dated addendum preserves the proposal and measurements above; it accepts no
ceiling and changes no implementation limit. The
[retained route report](https://github.com/harsh-nod/fe2o3-kernels/blob/6d71cfd0fe811da98d06c0c865f5dc2b9414ef88/docs/ui-route-performance-20260926.md)
and [numerical summary](https://github.com/harsh-nod/fe2o3-kernels/blob/6d71cfd0fe811da98d06c0c865f5dc2b9414ef88/docs/evidence/ui-route-performance-20260926.json)
record eight cells at measured site
`36e8e130b07fef92f7a2b1e1f47642f8814775dd`: five calibration and thirty
measured pairs per cell, across navigation, ordered-program and two real
historical V22 CPU recordings at desktop and narrow viewports.

Action-to-expected-DOM-plus-two-animation-frames p95 was 348.0–381.6 ms
for the four navigation/program first opens and 49.2–81.9 ms for their
warm opens. The four V22 first opens were 298.1–314.7 ms, warm opens
281.9–282.5 ms and bounded interactions 49.1–49.4 ms. These are not paint,
isolated parser, debugger execution or GPU measurements. In particular,
a complete V22 warm open is not the same operation as section 3's proposed
100 ms bounded page/render target; do not declare either compliance or failure
by substituting one timing boundary for the other.

The V22 recordings expand to about 3.6 and 3.55 MB, not the full 11 MiB
import class. Signed CDP used-heap checkpoint deltas, including negative
deltas, are retained; they are not peak heap or RSS. The summary explicitly
records `accepted_performance_budgets: false`,
`isolated_parser_measured: false` and `worst_11mib_class_measured: false`.
Thus the earlier unmeasured full-class proposal must not be read as absence
of all V22 route measurements, nor may these measurements qualify that
full class or a later site build.

The retained outer receipt SHA256 is
`2d9cce57577ac918a561590ef40e76cd05c83b6bde40c2ae15aa035381acacf8`;
the campaign report SHA256 is
`223df7e9d7f377dbd76a5707503abc06f0f70c7fe30fee8d3d908849117a9bff`.
Existing compiler/query/viewer owners still decide the applicable declared
workload and budget, citing compatible fixture/build identities and the
actual timing and memory boundary. This addendum records already available
evidence; it requests no new harness, adopts no proposed measurement protocol,
and closes none of M0/V0/U0.

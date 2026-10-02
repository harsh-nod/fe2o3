# Late Native Peer and Compute Admission Qualification

Status: CPU and sixteen-case MI300X qualification accepted by the independent
audit on 2026-10-02 UTC. Baseline:
`079f84235bf6be48286618d80a9ac4e0fff12899`.

## Implementation

- Directed full-buffer peers can select native transport while an exact started
  directed predecessor retains an endpoint. Selection authenticates the routed
  endpoint, owner, pair reservations and directed identity, then prepares an
  empty successor root without moving or cloning the parent's physical owners.
  Existing dependency/shared-read checks remain ahead of selection. Ordinary
  profiles, partial ranges and missing routes retain their existing fallback.
- Producer-aware compute can enter router-only deferred custody behind its
  published directed native producer. Exact stream, depth, profile, endpoint and
  region identities remain retained and revalidated. The prepublication native
  permit path remains unchanged; late admission acquires no child compute or
  SDMA ownership. Child admission and authority are checked again after all
  required producers succeed and restore their owners.
- Deferred progress can service an authenticated already-started directed peer
  occupying its child, before or after compute handoff. This is resource
  progress, not a sibling success dependency. Wrong-child, dangling or
  inconsistent directed ownership fails closed; uncertain custody is retained.
- `retained_compute_xgmi_copies_v1` is a read-only stored count of native roots in
  Published/Ready with retained ownership. It samples no fence and drives no
  progress. Hardware may already be complete; the count grants no access
  authority and does not demonstrate overlap.

## CPU Verification

Final-source qualification passes 2,138 runtime tests, zero failures and the same
32 hardware ignores. All 53 example tests, ten focused groups, strict Clippy,
feature/format checks and all 32 source-control commands pass. The 13 new runtime
tests include exact Published/Ready peer ownership, zero-staging successor
selection, preserved fallback, independent cancellation, corruption, terminal
failure/unwind retention and deferred blocker progress. All 81 compute-XGMI
tests pass.

CPU native-owner fixtures are scripted. Deferred consumer fixtures deliberately
hold compute behind an unpublished child gate and qualify admission, custody
and handoff, not compute arithmetic or native driver faults. The live compute
witness supplies the full output check under its existing finite authority.

The selection is `attempt-01`, metadata `proposal-01`, source workflow
`attempt-01-after`, and `hardware-01`. Preliminary compiler/lint diagnostics are
preserved separately and are not accepted final-source evidence.

The independent audit checks all 5,876 source files and exact test rosters. The
1,925-test KFD qualification is authenticated historical reuse, not a fresh run:
the executable, complete roster and all 344 KFD source files match the pinned
previous evidence archive.

## Hardware Verification

All sixteen cases pass on MI300X GPUs 5/6/7. Six new cases cover late three-device
chains and shared-source fanout in both orders, and late two-device
`directed peer -> compute -> ordinary peer -> D2H` in both orders. Ten existing
directed/readback/deferred/live-batch cases are current-source controls.

Each new case admits only its first native peer, observes the retained count
change from zero to one, then admits its successor without another progress
call. This uniquely identifies publication of that first peer while its owners
remain retained. It does not sample current GPU activity or infer publication
from a fixed number of progress calls.

The four late-peer cases use the production copy-only constructor with deny-all
compute authority. Each reuses one Context, six allocations and four streams
for two changed rounds. Copies span 8,388,581 bytes in three packets, including
an odd 37-byte tail. Full host output, source preservation, destination sentinels,
exact callbacks, event release and explicit logical/native cleanup are checked.
These four cases total 16 native peers, eight D2H copies, 64 full verification
reads and 24 callbacks. The second round's separate late D2H admission still
uses a bounded tail seed; that readback parent's publication is unobserved.

Each late-compute case uses the unchanged R57 finite artifact authority and
checks all 65,536 output elements. Three setup launches precede one pipeline
launch, two native peers, one dependent D2H and four exact callbacks. Once the
first peer is published and the tail admitted, only the final readback stream
drives the pipeline; public events are released. No host output is installed.
These are single-chain cases, not two-round compute qualification.

Including controls, the campaign records 58 native peers, 16 compute launches,
23 dependent D2H copies and 88 completion receipts. It runs from 13:54 UTC
through the final 14:03 UTC host checks. The audit confirms no owned processes,
removal of all three executables and
`/tmp/fe2o3-multigpu-late-admission-20261002-D7An8w`, and restored selected GPU
use/VRAM and complete PID baselines. GPU 0 was not selected; no reset or
foreign-process termination occurred.

| Artifact | SHA-256 |
| --- | --- |
| Runtime test executable | `45c95cf1bb54993adadd27c1aee3cc05fff408687c0790a01bccd1146e63b223` |
| Directed/readback witness | `4ea429a7bd40718b21620c2e60f0e79ec78735d726d0e2e2b6ff98cae9a19368` |
| Late-compute witness | `669916cd33c0e51721dc080122055ef638940d0ee3b77d566a7dc0aa2983d45a` |
| Full source manifest | `4724254d0c173531089901fefa6e6308b6f9f10a1bea4887eae308d2154fa90f` |
| Independent auditor | `339ab8d899149acf1c69b378b70aa164fdcd77549013f9a733e5c99159b089aa` |
| Qualification receipt | `71a4f8ace3982bd12c44809eb199a93679f1fd40ea2936b862386baab9f031d1` |

## Boundaries

This is not arbitrary mixed-graph admission, general application-kernel
authority, native fault isolation, eight-device qualification, physical overlap
or HIP/HSA performance parity. Native ambiguity still fail-stops the router.
Same-process reopen remains a separate device/VM lifetime problem.

The source guard refresh changes nine files: 16 identity hashes and seven exact
roster counts. All 76 proof files, proof predicates and expected proof/mutant
counts remain unchanged. No solver was run; historical proofs do not establish
formal refinement of these new adapters. A1/A2 and issue #182 remain incomplete.

MI300X is shared. Point-in-time UID/BDF/VRAM/PID and attachment checks are not an
exclusive reservation or performance-isolation guarantee. Future runs need fresh
checks and must remove only their own processes/files without device resets.

## Reproduction

The [raw bundle](raw.tar.gz), authenticated by [SHA256SUMS](SHA256SUMS), retains
controllers, exact commands and clean environments, source/executable identities,
rosters, outputs/exits, metadata audits, hardware admissions and cleanup receipts.
The read-only auditor independently checks complete payload expectations and the
exact case maps:

```sh
python3 -I -B audit.py attempt-01 proposal-01 attempt-01-after hardware-01
```

It requires the recorded source-freeze workspace with baseline HEAD, candidate
files and retained artifacts. The bundle is an evidence record, not a portable
self-contained build environment or an auditor for a later HEAD.

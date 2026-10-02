# Pending Compute Peer Windows

Implementation baseline: `090beb6f8eda93e68c43726e0be594ae20c014c2`.
Status: current-source CPU and finite native qualification passed on 2026-10-02 UTC.

## Scope

Ordinary peer copies may read a checked contained window from one pending
producer-aware, full-allocation Write and write an independently checked window
in a larger or smaller initialized PUBLIC DeviceLocal allocation. The native
queued/eager producer and router-deferred producer paths retain the original
producer identity and an immutable window with actual logical extents and offsets.
Context reconciliation retains the exact events, writer, lineage and read lease.
Whole-owner exclusion, paired custody, bounded progress and failure retention
remain unchanged. A slice neither creates a separate owner nor admits concurrent
slice writes. Partial Write, ReadWrite and aliased compute outputs remain rejected.

The existing deferred-peer witness adds `--return-window`, compatible with
`--late-compute`. It preserves the unchanged R57 V2 kernel authority and four-root
peer -> deferred compute -> peer -> D2H pipeline. The final readback stream drives
completion after public event release; no intermediate host compute join occurs.
It checks complete D/E/host snapshots, not just the copied bytes:

- D is 262,144 bytes; its peer source is `[20, 262100)`.
- E is 262,913 bytes; its destination/readback source is `[131, 262211)`.
- Host is 263,297 bytes; its D2H destination is `[97, 262177)`.
- E and Host have distinct A5/5A sentinels and untouched guard bytes.

The independent Python oracle derives the R57 arithmetic directly and checks a
domain-separated, length-framed SHA-256 over all three full logical allocations.
No expected computed output is uploaded. Logical guards do not qualify hidden
physical allocation padding.

## Qualification

The final complete runtime suite passes 2,160 tests with the unchanged 32-test
hardware-ignore roster. All 15 new test functions pass: seven Context tests and
eight backend tests. They cover independent bounds and exact ends, unequal logical
extents, original owner restoration, full byte guards, released public events,
final-readback-only progress, logical parent-before-child reconciliation,
cancellation, invalid/aliased writers, retained-window drift, and scripted native
failure/unwind custody. Scripted owners exercise real copy routing and custody,
not compute arithmetic or physical DMA.

Strict Clippy and no-default/hardware-qualification feature checks pass. The
selected source-control replay is `source-ci/attempt-03-after`: all 32 commands
pass. Its reviewed refresh changes only 16 hash literals and seven source counts
in nine files. All 76 historical proof-closure files remain unchanged.

The initial whole-record endpoint snapshot passed the runtime suite but enlarged
a shared enum enough to fail strict Clippy. The final implementation retains
regions and logical extents without additional allocation; complete record
identity remains checked by the existing allocation table and producer binding.
The final suite was rebuilt and rerun after that change. Superseded runs and the
initial in-flight compile diagnostic remain retained but are excluded from final
acceptance totals. Directed/deferred/live-sharded examples pass 11/8/5 tests;
the current-source CPU total is 2,184 passing library/example tests. Historical
KFD passes below are not added to that freshly executed total. Selected CPU
receipts are `attempt-02/{build,runtime,checks}`.

Lower KFD sources, the exact KFD test executable and arithmetic proof inputs are
unchanged. `reuse-check.py` authenticates the prior native-peer-subranges archive,
the complete 1,934-pass/no-ignore KFD result, 3,499 conservative dependency-source
pins and 14 arithmetic proof inputs. This is historical evidence reuse, not a
fresh KFD test or solver execution. It does not prove the changed runtime adapter.

## Native Results

All 12 MI300X cases pass: four new pending-compute window cases and eight
current-source controls. The campaign ran from 17:27:31 to 17:34:08 UTC on
2026-10-02. New cases use GPUs 6/7 in both device orders, with all four operations
queued before progress and with compute admitted after observed first-peer native
publication. Controls cover three-GPU peer-window readback (GPUs 5/6/7), both
full-buffer deferred-chain variants and repeated live sharded execution, all in
both device orders.

Each new case records four exact completion receipts, two completed native logical
peer copies, released public events, retained queryable results and final-only
readback progress. Complete D/E/host snapshots match the independent byte oracle
and the common digest
`ce60155c367eb1f028ec4cea77b86838b99470a8e73737849b56e47c8ed89c8f`.
The deferred executable SHA-256 is
`21b8160141974ccb581b5d8f5052b6c006a1626d11bb4e4ec6b00d599fdb72a8`.
Local and remote executable identities agree before and after the campaign.

Fresh UID/BDF, activity, VRAM, complete process/attachment census and host-memory
checks bracket every case. Final observations match the initial shared-host
baseline. No owned executable process remains; all three uploaded executables and
`/tmp/fe2o3-pending-compute-windows-20261002-gQjYdc` were removed. No device reset,
foreign process termination or native fault injection occurred.

## Provenance

The working evidence root is
`/home/harsh/.codex-tmp/fe2o3-pending-compute-windows-20261002`.
The accepted final receipt is `audit-01.json`; it rechecks current source and ELF
identities, complete test rosters, exact source-control commands, authenticated
historical reuse, and every native case's argv/oracle/admission/cleanup chronology.
The auditor reuses the reviewed Python oracle; it is not a second independent
host oracle. `raw.tar.xz`, `raw-manifest.json` and `SHA256SUMS` retain the commands,
outputs, exits, source inventories, controllers, failed/superseded attempts and
`candidate.patch` against the baseline above. No test executable or verifier
distribution is included.

Read-only follow-up notes in the archive prioritize ordered gather into one
destination and the concrete production artifact/proof-provider integration.
They describe remaining work, not newly implemented or qualified behavior.

## Limits

This is a contained read from a full compute output, not partial kernel-output
authority. The native compute witness still uses finite qualification authority,
not a general compiler/effects production provider. Ordered gather into a single
destination, arbitrary dependency graphs, concurrent slice ownership and native
fault qualification remain outside this increment. Shared-host point observations
are not an exclusive reservation. Physical overlap and HIP/HSA performance parity
are unmeasured; no whole-adapter or DMA-engine formal refinement is claimed.

# V6 Generated Writer Journal Integration

CPU/test development above canonical base
`d91e470f15bc2c42078842d983a4bb2f36ed4a8c`. Accepted checkpoints remain Native
R125 CPU/test, Admission R118B C1-C3 and Resources R116/V3. This packet does not
complete V6, A1/A2, issue #182 or HIP/HSA parity.

## Change

Protected generated ISSUE now begins one journal writer over the original
WriteOnly/ReadWrite shell allocation IDs before backend preparation. The retained
attempt, writer root, submission marker and private native-settlement receipt
bind the exact original writer, with a separate stream/hold/shell domain.
Generic submission observation cannot settle this writer. Protected completion
settles Success only after checked readback, native retirement and closing
currentness, before shell retirement. Stop marks Unknown and composes exact
whole-batch writable disposal with independent read-only retirement. Original
credits remain charged through uncertain outcomes and bookkeeping failures.

Generic retained-submission operations reject an existing held stream but still
allow ordinary quiescent submissions to outlive a destroyed stream. Ordinary
writer disposal reuses the factored final whole-roster commit. See the
[contract](../../runtime-context-version-journal-generated-v1.md) for ordering,
complexity and remaining boundaries.

## Qualification

The frozen source ran locally in
`/home/harsh/.codex-tmp/fe2o3-c4-completion-20260917`. GNU and scoped musl each pass:

| Package | Passed | Ignored |
| --- | ---: | ---: |
| fe2o3-runtime | 1009 | 17 |
| fe2o3-host | 271 | 4 |
| fe2o3-runtime-model | 779 | 2 |

Each target has 2,059 passes, 23 ignored, and zero failed, measured or filtered
tests. All ten new journal test functions pass on both targets. They cover mixed,
all-read-only and all-writable rosters; exact Begin membership; inner capacity
rejection; blocked generic observations and backend-entry routes; assumed-C4
settlement and neighboring invocation isolation; pre-retirement Success lineage;
Stop disposal and read-only preflight; no-handle error/panic quarantine; four
marker/root/roster substitutions; and post-model credit-fault retention.

The 22-test focused generated suite and an initial strict Clippy pass preceded
source freeze. Exploratory tests corrected fixture signatures and quarantine
accounting expectations. The first full GNU campaign then found six ordinary
post-stream-destruction regressions in the new guard. Its original source patch,
manifest and raw failure are preserved in the separately sealed
[first attempt](../dev-v6-generated-journal-2026-09-17-attempt-1/README.md).
The final source narrows the guard without changing those existing tests.

Strict all-feature/all-target Clippy, scoped formatting, no-default checks,
86 doctests and unsafe-source policy (5 passed, 1 ignored) pass. All sixteen
serial qualification records exit zero.
Musl uses `FE2O3_HIP_SYS_DISABLE=1`; unrestricted musl/HIP linkage is not
qualified. This is runtime/host/model coverage, not a fresh complete lower-KFD
campaign.

The roster parser validates exact package/target identity, unique test rows and
matching summaries. Its calibration accepts both immutable baseline targets and
rejects ten malformed-log mutations. GNU/musl complete rosters match across all
2,082 named entries, SHA-256
`7770f3bc4f2d661f2ebd0dfb50242f79f259ae364ad24c5183f0df1cd527a88f`.
The runner hashes all six executed test binaries and rechecks them after the
quality gates. Serial raw records retain argv, UTC times, output and exit status.

The source manifest covers sixteen changed files: thirteen Rust and three docs.
Its SHA-256 is
`68831a0fe2c02073642154b5df553f1fc7e661f32c75940f39996e6e11d0b5bf`.
The exact base-relative patch has SHA-256
`69a69344b0cc7be60e3a0001b5e8f3ab970d25752d84ecd824315b46dd789628`.
Independent read-only reviews checked the production join, test oracles, contract
and retained-stream correction. The seal checks source, patch, binaries and
record ordering before manifesting every archive file. Sealed evidence is
immutable.

## Limits

No native GPU, formal solver or matched HIP/HSA benchmark was run for this
packet. No remote files or jobs were created. C4 and Stop tests exercise the real
Context/journal bookkeeping tails under explicit test-only native-settlement
assumptions. They do not produce protected native receipts or establish machine
refinement. Credit-fault injection is CPU bookkeeping evidence, not native
disposal/failure or physical pool-residency evidence.

The outer protected ISSUE failure path still conservatively terminalizes an
inner capacity rejection. The inner test does not claim public retryability.
Production protected authority/refinement, input/read leases, backend aliases,
ordered overlapping writers, cross-run versions, content reuse, aggregate native
residency and native high-depth/overlap/failure/performance qualification remain
open. No new execution or content-reuse authority is granted.

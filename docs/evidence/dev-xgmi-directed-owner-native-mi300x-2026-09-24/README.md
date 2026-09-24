# Native Directed XGMI Owner Diamond

Status: one source-bound MI300X native correctness campaign passes. This closes
the shared-source admission failure for the unchanged four-copy owner witness;
it does not close A1/A2, formal refinement, fault qualification or performance.
The [earlier refusal](../dev-xgmi-directed-owner-refused-mi300x-2026-09-24/README.md)
remains failed and is not replaced by this result.

## Source And Result

Signed source: `87a9a4c90af12f09457249608dcbb215e7028847`, built in the clean,
owned detached worktree recorded in `verify.py`. The example and campaign bytes
are unchanged from the refused `1d1992d07` witness. The source correction admits
exact directed source readers and publishes allocation-disjoint FIFO prefixes.

The release musl binary runs on GPUs 5/6, uniquely bound to
`b7baafd0fb173d8e` / `10a254ce4987e716` and PCI `0000:a6:00.0` /
`0000:c6:00.0`. The original dependency diamond uses one owner thread, four
streams and five DeviceLocal allocations. Root, left, right and tracked join
all succeed. Producer events are released after join admission and before
progress; the source journal is enabled and pending dataflow is admitted.

Independent complete-buffer checks cover 327,680 bytes: 131,072 destination
payload bytes, 131,072 guard bytes and the unchanged 65,536-byte original source.
All canaries pass. Runtime shutdown returns Released with complete cleanup;
this is distinct from the earlier retained-until-process-exit failure.

All six host observations pass: both GPUs at fresh preflight, both after the
fixed two-second settling delay, and both after the fixed twenty-second delayed
observation interval. These are sequential point-in-time observations, not an
exclusive GPU reservation or proof of uninterrupted host idleness.

The controller collects the byte-exact remote inventory, removes only its
marker-owned directory, and independently confirms path/process absence. All
recorded child groups are absent. The removed remote directory was
`/home/harsh/fe2o3-xgmi-directed-owner-20260924.d22ed808efad4dc1`.

## CPU Prerequisite

`qualification/` is an exact copy of the CPU archive in the signed source:
1,341 runtime tests per GNU/musl target, twenty hardware-only ignores each,
46 doctests, six example tests per target, fourteen Python tests, formatting and
strict all-feature/all-target Clippy. All sixteen new regression names pass on
both targets. Fifteen successful/reaped stages bind 3,904 unchanged inputs and
unchanged Rust/Cargo identities. The campaign separately passes its six release
musl example tests and fourteen Python checks with the actual release CLI.

The raw musl CPU transcript preserves one child-banner interleaving. Replay
recognizes only that exact captured fragment; it does not count child invocations
as additional top-level successes or accept a changed result there.

## Offline Replay And Scope

Run `python3 -I -B docs/evidence/dev-xgmi-directed-owner-native-mi300x-2026-09-24/verify.py`.
The verifier authenticates the signed source, transitive helpers and CPU archive,
then checks exact commands, environments, deadlines, payload bytes, ELF identity,
complete signed source inventories, raw result and endpoint transcripts,
timestamp-contained observer freshness, fixed delays and cleanup records.
`test_verify.py` mutates disposable copies, including coherently rehashed inputs.
The separate CPU-content cases exercise deeper checks after bypassing only the
test's signed-copy gate; production replay always authenticates that copy first.

This packet validates the specific native diamond and its complete-buffer and
cleanup oracles. CPU fixtures supply hostile admission/custody coverage; the
hardware campaign does not inject native failures. External kernel/firmware/GPU
behavior remains contracted. No machine-code or Rust/model refinement theorem,
aggregate memory bound, multi-GPU compute integration, HIP/HSA ratio or speedup
is established. Native R125, Admission R118B and Resources R116/V3 remain the
broader accepted checkpoints; A1/A2 and issue #182 remain open.

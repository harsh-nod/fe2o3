# R61 Async Owner Validation

This is evidence for a local integration slice of
[issue #182](https://github.com/harsh-nod/fe2o3/issues/182), not completion of
that issue. It does not establish distributed execution, generated-kernel
production authority, HIP/HSA parity, or a performance improvement.

## Implementation Scope

- One owner thread constructs, operates, and finalizes the complete direct-KFD
  context without transferring a thread-affine backend or adding unsafe Send.
- Bounded nonblocking context commands and standard futures integrate with the
  existing progress loop. Typed launch, local copy, and peer-copy operations
  retain progress independently of observer lifetime.
- Registry reservation precedes submission. A rejected completion poll retains
  the original submission and retries only observation, never dispatch.
- Independent cyclic stream flushing prevents starvation under partial poll
  budgets. Scripted tests cover 2,048 outstanding operations and dropped futures.
- A panic or incomplete cleanup retains the whole context until process exit.
  Shutdown does not promise a drain or bounded adapter-call duration.
- Hardware testing exposed an SDMA logical/physical extent mismatch. The fix
  requests existing checked page-rounded device backing while retaining the
  original logical copy bound. Host buffers and admission policies are unchanged.

The [architecture and remaining acceptance](../../runtime-async-execution-plane-v1.md)
document specifies the bounds and property-level proof/adapter distinctions.

## Source Identity

| Source | Meaning |
| --- | --- |
| `c2fa9bad03e58eddc93c54d28ac1406742f4df1f` | Owner engine, typed operations, regressions, eight abstract R61 obligations and eight negative mutations. |
| `898523072c8210b1f64890b3d25e09260ada0bce` | Musl qualification profile, static-symbol checks, linker/target-library provenance; runtime and proof source unchanged from the first commit. |
| `9fc4062dbd5a1269ae86990f80b67ba6cd4a9101` | Page-rounded device SDMA backing with logical copy bounds; accepted two-run MI300X source. |

These commits are SSH signed and DCO signed. Remote qualification verifies the
signed source against the explicitly supplied allowed-signers file before
archiving and rebuilding it. The raw records retain that trust input, signature
output, source-file hashes, commands, environments, and rejection diagnostics.
Changes after the qualified `9fc4062d` source are two test-only assertion-style
fixes plus documentation/evidence. The accepted binary hash belongs to
`9fc4062d`, not the later evidence commit. Complete CPU suites were rerun after
the assertion edits; logs use the `r61-completion-*` names.

## CPU And Proof Validation

The initial GNU and musl runs each passed 1,824 tests across 39 top-level Cargo
test targets, with five tests ignored. Each target also passed 33 doctests.
Those results predate the SDMA sizing fix; subsequent validation is recorded
separately rather than silently replacing them.

After the sizing fix, both complete suites passed **1,827 tests**, with five
ignored on each target. The three added regressions cover extent planning,
production allocation wiring, and scripted device allocation/map/promotion,
successful last-byte admission, rejected padding copies/windows, demotion and
exact release. The host side of that lifecycle test is a metadata fixture.
Strict Clippy passed after replacing two test-only `err().expect()` assertions
with `expect_err`; the rejected style check and final passing log are both kept.

The original async implementation passed strict Clippy. Runner tests passed
8 cases, inherited host-guard runner tests passed 16, and the unchanged runtime
closure auditor passed 28 self-tests. The raw host logs are in [raw/host](raw/host/).

The authenticated Verus run passed 50 positive files, 1,268 obligations and
592 expected-negative rejections. R61 adds eight abstract obligations and eight
negative mutations to that suite. The 190-file pinned toolchain closure was
checked before and after execution. The proof transcript SHA256 is
`15f6522ab60779ddbb31f828f064a080d9a6ca7e397e33560305479f2907b28d`.

These are abstract proofs, checked helper predicates, and executable tests with
explicit boundaries. They are not a theorem about the entire threaded engine,
the sizing adapter, Linux, KFD, firmware, GPU, executors, or a network transport.

### Reproduction Commands

All commands ran in this worktree, with repository-pinned
`nightly-2026-04-03`, offline and locked Cargo resolution, and two build jobs.
CPU tests use `RUST_TEST_THREADS=2`; none of these commands opens a GPU.

```sh
RUST_TEST_THREADS=2 cargo test -p fe2o3-runtime -p fe2o3-runtime-model -p fe2o3-kfd \
  --all-features --all-targets --offline --locked -j2
RUST_TEST_THREADS=2 cargo test -p fe2o3-runtime -p fe2o3-runtime-model -p fe2o3-kfd \
  --target x86_64-unknown-linux-musl --all-features --all-targets --offline --locked -j2
cargo test -p fe2o3-runtime -p fe2o3-runtime-model --doc --all-features --offline --locked -j2
cargo test -p fe2o3-runtime -p fe2o3-runtime-model --doc --all-features \
  --target x86_64-unknown-linux-musl --offline --locked -j2
cargo clippy -p fe2o3-runtime -p fe2o3-runtime-model -p fe2o3-kfd \
  --all-features --all-targets --offline --locked -j2 -- -D warnings
cargo fmt --all -- --check
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover \
  -s benchmarks/runtime_gfx942 -p test_run_r61_owner.py -v
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover \
  -s benchmarks/runtime_gfx942 -p test_run_r60_pipeline.py
PYTHONDONTWRITEBYTECODE=1 python3 scripts/tests/runtime_pure_rust_audit.py
VERUS=/path/to/pinned/verus VERUS_TIMEOUT_SECONDS=120 \
  crates/fe2o3-runtime-model/verus/verify-verus.sh
```

The Verus executable used locally was
`/home/harsh/.codex-tmp/r56-verus-0.2026.08.09/install/verus-x86-linux/verus`.
The transcript retains authenticated identities; the placeholder above is not
permission to substitute a different verifier or solver.

## Rejected Hardware Attempts

| Source | Rejection | Evidence |
| --- | --- | --- |
| `c2fa9bad` | GNU thread startup references prohibited `dlsym`; rejected by the existing ELF policy before GPU execution. | [GNU rejection](raw/gnu-rejected/) |
| `89852307` | Musl passed Cargo/ELF/full-symbol gates, then device allocation promotion rejected the non-page-sized canary extent. The first phase failed; there is no accepted qualification set. | [Promotion rejection](raw/musl-promotion-rejected/) |

The GNU policy was not relaxed. The musl profile uses a different standard-library
thread implementation, target-filtered Cargo metadata, explicit linker identity,
full symbol checks including local/weak/defined symbols, and target std/CRT
hashes checked before and after building. A static ELF pass alone is not the
claimed evidence.

The canary remains 1,048,832 bytes: a 1 MiB body and 128 bytes of padding on each
side. Neither page-padding the canary nor weakening promotion admission was used
to avoid the failed case. The subsequent allocator fix preserves logical bounds.

## Accepted Hardware Qualification

Two runs from signed source `9fc4062d` passed on GPU1 of `mi300x`, unique ID
`0xab83d2ffef0d3cdf`, PCI `0000:26:00.0`, KFD GPU ID 23018. The canary used one
owner thread, one stream, H2D and D2H, with nonuniform data and full unchanged
input/output/padding comparisons. The upload future was abandoned before
completion; runtime-owned progress completed it without observer-owned polling
or flushing. Explicit logical and native cleanup completed before either PASS.

Each process emitted exactly:

```text
PASS schema=fe2o3.runtime.r61-async-owner-copy.v1 bytes=1048832 owner_threads=1 abandoned_upload=completed canaries=complete cleanup=complete
```

The [accepted captures](raw/musl-accepted/captures/) retain two monitor seals,
five consistent topology records, four telemetry boundaries and 32 recorded
commands. Both monitors observed selected-device queues, zero foreign or terminal
selected-device queues, successful target reaping and absent process groups.
Maximum observed census gaps were 5,938 and 5,966 microseconds, below the unchanged
10,000-microsecond bound at a requested 2,000-microsecond cadence.

The admitted artifact SHA256 is
`08368202109f880fea2d864e4cbd09a8c9ac19dbf36cfe379b31dd4dade7b05d`.
The static audit checked 4,766 distinct full symbol names, no unresolved imports,
no interpreter, no dynamic dependencies and no GNU min-stack lookup. This is the
musl canary profile only, not a GNU-host production pass or every application.
No performance result, compute overlap or high-depth hardware result is claimed.

## Integrity And Cleanup

The three task-owned enclosing MI300X stages were removed after archiving.
[Final independent SSH checks](raw/host/remote-cleanup.json) confirmed all three
paths absent and selected GPU1 at zero utilization and zero allocated VRAM.
GPU0's pre-existing 44% VRAM allocation remained untouched. There were no resets,
foreign-process termination, or all-device tests.

The compact accepted set retains the original `sha256.json` unchanged. Its
`source.tar` and `owner-binary` are intentionally omitted from Git; both remain
in the complete local archive. Rejected sets similarly omit `source.tar`; they
have no accepted manifest or retained rejected binary. Complete archives are
local evidence, not published release assets or downloadable attachments.
Independent read-only audit verified all 73 original manifest payloads, all
6,118 embedded source files, exact streaming Git archive equality, and the
signature against the prior independently trusted R60 signers. It rechecked
the archived binary's ELF/full symbols and the 41-package Cargo closure without
executing the binary or using a GPU.

| Attempt | Complete local archive SHA256 |
| --- | --- |
| GNU rejected | `dac9ecace68719132cafbd42b6fc7f50c2f301c5a1b44ad8e63346f218468e3b` |
| Musl promotion rejected | `dca875200b5f1c65b4d2365ec6ee84bef811198c96f722b9c378fd92c77cdbf2` |
| Musl accepted | `f1fe581996f204571fbc92171493f29347a411c9cd887a896743195dc8683356` |

The archives reside under `/home/harsh/.codex-tmp/` as
`fe2o3-r61-hardware-{c2fa9bad,89852307,9fc4062d}.tar`.
The [compact retention manifest](raw/retained-files.sha256) seals the 184 raw
files actually retained here and
does not replace the original runner manifest. Directory renaming and local
directory write permissions for archival handling do not modify capture bytes.

## Remaining Acceptance

#182 remains open. A1 still needs generated-kernel production admission,
operation-addressable cancellation, timeout policy, end-to-end byte budgets,
multiple executor integration, and high-depth mixed-duration GPU qualification.
A2-A7 still require admitted graph execution, data versions, all-device placement,
authenticated two-host control/data transport and collectives, distributed
failure qualification, and precommitted performance gates. Abstract proof success
and a one-device copy canary cannot substitute for those deliverables.

Only `mi300x` is authorized for this task. GPU0 has a foreign allocation and must
not be disturbed. A second AMD GPU host and an agreed all-device testing window
are still needed for the issue's distributed and all-GPU hardware gates.

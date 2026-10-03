# Authenticated Host Multi-Device Preparation

Development checkpoint on baseline
`2761f359468d60c6814e265ddaa21bff50b0c499`. This removes the single-backend
restriction in the normal host preparation API. It does not complete A3,
authenticate an ordinary application kernel, or add a formal adapter proof.
Three read-only agents reviewed API/gating, generated fixture coverage and the
remaining native application path. Primary owns implementation and qualification.

## Implementation

- Added `prepare_generated_multi_context_invocation` and its async counterpart
  for the retained `KfdMultiDeviceRuntimeBackendV1`. They use the same protected
  evidence gate, selected-device callback, private payload and persistent
  projection as the original single-device methods.
- Added `GeneratedWorkerV3ContextInvocationV1::validate_multi_context`, delegating
  to the original Context validator and private preparation identity.
- Preserved the original result-credit account, decoder, private carriers and
  all existing public signatures. No device reopen, extra authority provider,
  new public backend trait, or runtime/KFD implementation change is introduced.

Protected evidence is checked before Context effects, argument callbacks, async
budget cloning or enqueue. Preparation is not execution. Input arguments and
the executable are consumed even on rejection; this API does not promise their
recovery. Async progress remains owner-driven. Compiled documentation preserves
reservation/activation failure tickets and does not equate dropped observers or
timeouts with physical quiescence.

## Qualification

`qualification.tar.xz` contains bounded commands, environment, stdout/stderr,
before/after source inventories, host ELF identity, the candidate source patch,
and the replay audit. Rust uses `nightly-2026-04-03`, four build jobs,
single-threaded tests, test opt-level 1 with debug/overflow checks,
`FE2O3_HIP_SYS_DISABLE=1`, and no incremental build or debug info.

The accepted ledger is `final-*` plus `controls-02-*`: **43 receipts** against
one **6387-file** source inventory. Earlier captures are retained but not accepted
as final-source qualification. In particular, the diagnostic-marker test edit
overlapped the earlier `host-no-default` and `controls-final-11` commands; both
commands exited zero, but their controllers correctly rejected changed source.

- Host library: **315 passed, 5 ignored**, with all 320 roster identities matched.
- Host doctests: **35 passed**, including both new public usage examples.
- Host without default features: **198 passed, 1 ignored**.
- Macros library: **64 passed**. Generated-adapter fixture harness: **2 passed**,
  covering the positive downstream library and **37 negative cases**.
- Strict host Clippy, package formatting, whitespace and all **33 source-control
  commands** pass. No Cargo manifest or lockfile changed.

The downstream library compiles genuine generated `transform_gpu::RuntimeArguments`
through all four single/multi sync/async preparation methods. Four new negative
cases reject executable reuse and wrong-kernel arguments on both multi paths.
Existing negative cases retain move-only, thread-affinity, storage privacy and
nonexecuting-carrier checks. Three independent source-wiring tests compare both
forwarding bodies and verify gate/callback/projection order. A renamed existing
test plus two added tests produces one removed and three added roster names.
These are compile and source-wiring checks, not newly executed concrete-KFD
admission rejection tests.

The host ELF is 24,938,776 bytes, SHA-256
`d74ad7a48848a716b2dfb600420c67bf7bbf7072107231a69cb4e71bfe2e5a7c`.
The audit confirms exactly three modified and five new source files. All runtime,
KFD and accounting sources, the original protected gate/payload/projection bodies,
and 76 selected proof/declaration/shared-body files are unchanged. No solver ran.

## Remaining Acceptance

This checkpoint is CPU-only: no MI300X process or scratch directory was created.
The prior [native staging checkpoint](../dev-result-staging-2026-10-03/README.md)
still establishes synthetic charged data -> settled H2D -> native XGMI in both
directions on GPUs 6/7. It does not establish an authenticated native kernel
completion through this new host API.

The next [critical-path packet](../../runtime-multi-gpu-critical-path.md) is a
real Rust device-entry output kernel with complete semantic/physical ABI,
entry-to-exit machine refinement and authenticated evidence production. Partial
helper proofs, test-only positive providers and finite native qualification must
not bypass that gate. Full admitted compute -> peer -> compute/readback,
zero-copy DATA promotion, physical overlap and matched HIP/HSA performance remain
separate open acceptance items.

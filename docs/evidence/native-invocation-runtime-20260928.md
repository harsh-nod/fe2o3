# Exact Invocation and Runtime Accounting Checkpoint

Date: 2026-09-28 (Pacific). Continues the
[native proof helper bootstrap checkpoint](native-proof-helper-bootstrap-20260928.md).

Next checkpoint: [native compiler exec and capture](native-compiler-exec-20260928.md).

**M0-M7 and the 47/47 production-to-safe-GPU-launch gate remain incomplete.**
No tutorial entry changes classification. This checkpoint adds exact invocation
custody, approved helper configuration and bounded runtime admission, not an
integrated compiler/helper attempt, proof RPC or GPU execution.

## Implemented

- Fixed rustc and ELF-interpreter transfers use the existing retained compiler
  inventory. Each duplicate has a separate full backing charge. Validation
  checks the original object, complete inventory, approval and original budget;
  matching bytes or a caller-selected role/path cannot substitute for custody.
- Exact V3 invocation staging preserves argv order, repetitions, empty arguments,
  argv[0] and the complete sorted environment without inheritance or lossy
  conversion. Capacity measurement, byte/count checks, allocation and overlapping
  owners are prepaid. Cwd remains descriptor text, not an admitted directory.
- Private `CompilerInvocationBacking` consumes the exact descriptor and one
  genuine runtime, retaining both executable transfers and staged inputs.
  `ProofHelperBacking` now consumes that compiler backing, rather than competing
  for the same runtime. Full compiler revalidation surrounds sealed-helper
  validation. Neither owner provides an execution entrypoint or public import.
- Compiler approval V2 authenticates explicit helper UID/GID values and rejects
  zero, invalid or colliding service identities. The sole positive approval
  loader and native authority launcher select fixed `policy-v2`, bound to the
  existing V3 client profile. The V1 codec remains inert; there is no V1 approval
  fallback. Helper startup and root-side staging derive credentials from this
  retained approval, never from caller arguments or the helper's observed IDs.
- Helper-local Verus admission and revalidation now use its existing resource
  budget. Charges include complete retained files, interpreter backing, metadata,
  bounded scans and temporary reopen overlap. The original ledger and Budget
  address must agree. Bounded owners reject legacy unmetered revalidation,
  attempt acquisition and execution dispatch before those operations run.

The root-side components are **not yet called by the real per-compiler attempt**.
A pinned interpreter FD does not establish dynamic-loader or DSO resolution.
Configured role credentials do not establish deployment provenance or an
independent whole-domain custodian. The installed V1 provisioner/systemd graph
does not provision this helper role or publish V2 compiler approval; migration
must update those consumers together. Cross-process original-root-account
metering and bounded proof execution remain unfinished.

## Local Validation

Code commit: `2f234ba0cc06a4e941f6c500cee719ef4050ab63`. It includes the separately
published MIR fix `3991a6d6cc350f856038d813674cd1e63b617adb`, merged without
replacing upstream history. The final guarded runs used the same 9,237-file
snapshot:

```text
c60e865ae181cfe8131e574d2ed4576af8613155e093c12e9202250d05be7e2b
```

Pinned `nightly-2026-04-03`, locked/offline dependencies, one build job, serial
tests, fixed resource limits and unchanged before/after source/tool hashes were
retained. Results are scoped, not workspace-wide or warning-free validation.

| Check | Result | Evidence Label |
| --- | --- | --- |
| Compiler closure / coordinator / rustc invocation unit tests | 263 / 217 / 99 passed | `attempt-final-core-rtwo` |
| Build-authority integration suites, including both policy codecs | 55 passed | `attempt-approval-final-rone` |
| Build authority / closure / coordinator / invocation doctests | 221 passed | `attempt-final-doc-rtwo` |
| Retained Verus runtime and process-control tests | 123 passed; 8 ignored locally | `attempt-runtime-resources-rthree` |
| Helper entry I/O tests | 4 passed | `attempt-helper-entry-rone` |
| Merged nominal V35 MIR tests | 16 passed | `attempt-mir-nominal-rone` |
| Unsafe source inventory | 5 passed; maintenance command ignored | `attempt-unsafe-policy-rone` |
| Cargo driver, codegen, verifier, runtime protocol and coordinator | All-target checks passed | `attempt-integration-check-rtwo` |

Nested subprocess results are not counted twice. The protected-runtime test
below is one of the eight locally ignored tests, executed separately, not eight
additional passes. The authority-launcher shell suite also passed. Scoped
formatting, whitespace, all eight hygiene-policy tests and delta hygiene against
`15dd803a2` passed without a waiver. New credential tests were split into a
focused module to retain the file-size limit.

Tests cover exact invocation bytes and capacity charges, wrong roles/inodes,
policy downgrade and identity collisions, missing/short reservations, changed
budgets, sealing and transfer overlap, and refusal before unmetered dispatch.
They do not manufacture a public positive approval owner or prove a successful
composed production launch. An initially overbroad fixture iterator, an empty
policy test filter and the legacy-access gap were corrected before these runs.

## Real Protected Runtime

On `mi350-2`, the actual ignored test
`retained_functional_refinement_runtime_v1::resources::tests::protected_runtime_retains_complete_backing_on_original_budget`
passed: **1 passed, 0 failed, 0 ignored**. It opened the fixed root-protected
runtime, checked complete retained charges, revalidated the genuine owner,
rejected missing storage and foreign/moved budgets, rejected legacy access,
and dropped custody before releasing storage.

The test container was configured for a non-root process in the pinned read-only
image, no network, all capabilities dropped, no-new-privileges, one CPU, 2 GiB
memory and 32 PIDs. Shared runtime and interpreter directories were mounted read-only.
The first probe's individual loader-file mount correctly failed the retained
parent's `NO_XDEV` check. Mounting its containing protected directory fixed the
test environment; no runtime safety check or manifest was changed.

| Identity | SHA-256 |
| --- | --- |
| Pinned container image | `fd5370f370708f6a02cec6d44818a4295609e5bc68aa42455e53f141168a9d5f` |
| Executed verifier test binary | `36aece4ec87bf62b62be59ff165169969993b396c2a238ec7c6ae9fa7061343b` |
| Corrected remote probe driver | `21c572d1f551c332316cb7dcbb239ebf6728d49c3659e30112a7dfc5a937d415` |
| Successful remote JSONL | `8224f8dd0da7db91d635cb36dceb17e91ce2606622fde5d96a1d60f8644d0ec4` |

The final local snapshot's verifier test binary was independently checked to
match the executed bytes. Both probe containers and private scratch directories
were removed, and a separate SSH check confirmed their absence. Shared images,
volumes and installations were preserved. Evidence is retained under
`attempt-runtime-probe-rone`, `attempt-runtime-probe-rtwo` and
`attempt-runtime-probe-cleanup-20260928.json`.

This is **static protected-runtime admission/accounting evidence only**. No
proof child, proof receipt, authenticated helper launch or GPU workload ran.
The optimized static helper image from the preceding checkpoint was not rebuilt
in this batch and is not evidence for the new source snapshot.

## Next Integration Gates

1. Extend the existing native exec stage for actual rustc argv/environment,
   directory-object cwd, stdio, backend/output/input descriptors and validated
   loader mappings. Do not introduce a second compiler launcher.
2. Retain actual compiler/helper siblings through one attempt, with separate
   cleanup domains and one consuming wait owner per child. Integrate tracing
   before releasing the compiler's pre-exec gate; pending cleanup retains all
   transitive backing and spawn obligations.
3. Migrate provisioning and installed entrypoints coherently, establish actual
   deployment provenance/custodian admission, bind the authenticated attempt
   commit and exclude peer writers and backing aliases.
4. Add bounded proof RPC and execution through the helper-local lease, resume
   the same compiler, and finish original-account metering. Existing production
   authority refusals remain until their exact obligations are implemented.
5. Complete generic safe launch, the 47-kernel positive/negative matrix,
   target-matched hardware validation, tutorial updates and release gates.

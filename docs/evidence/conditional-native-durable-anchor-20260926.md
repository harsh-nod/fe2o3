# Native Durable Anchor Checkpoint

Date: 2026-09-26. Continuation of the
[anchor provisioning checkpoint](conditional-native-anchor-provisioning-20260926.md)
for [issue #272](https://github.com/harsh-nod/fe2o3/issues/272).
M0-M7 and 47/47 production-to-safe-GPU-launch completion remain open.
This is direct durable-service integration, not protected startup, compiler
semantic equivalence, numerical refinement or GPU execution evidence.

Base: `d56b128402d0f50be56a710148987ffac91aa204`.
Integrated code: `e6769d5d0c110ed2149b5b1e0231a324a8d7162c`.

## Implementation

- `36758e1f5`: extracts one key-free durable core used by V1 and native wrappers.
  It owns root/lock, public key, sequence/head and poison status. Poison is set
  before persistence, so both errors and unwinds require reopening. Fixed state
  I/O rejects short transfers/EINTR; NONBLOCK state open rejects FIFOs without
  waiting. V1 successful state and observation wire semantics are preserved.
- `e6769d5d0`: adds nominal V2/V3 native key-owning wrappers. Constructors and
  exchanges require the actual deployment, exact nonroot service credentials,
  sealed key validation and original-ledger accounting. No raw-key export or
  V1-owner upgrade is available. An advance persists before signing; late
  refusal returns no response but leaves the exact commit recoverable.

The [state contract](../compiler-execution-native-anchor-state.md) specifies
ownership growth, nested resource charges and failure effects. These wrappers
reuse both the existing key capability and the existing state engine. No second
persistence or recovery implementation was added. Cargo adds only the existing
workspace kernel-IR dependency for the original budget type; versions are unchanged.

A native worker implemented the core in a private worktree; the primary
implemented wrappers/tests, integrated and built. Read-only review found no
production-wrapper defect. It found two test defects: numeric FD identity could
race with descriptor reuse, and response accounting was retired too early.
Tests now compare device/inode and retain a move-only charged observation until
use completes. Review also prompted drop/reopen/recovery after late refusal.
The follow-up source review found those issues addressed. Workers ran no builds,
SSH, remote jobs or permission prompts.

## Validation

Pinned `nightly-2026-04-03`, frozen/offline dependencies, one bounded Cargo job at
a time, HIP disabled, no incremental compilation, source frozen during builds.
Tests used one thread except the explicitly four-thread native run. Logs:
`/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921/`.

| Check | Result |
| --- | --- |
| Initial native run R1 | 20 failed at fixture setup: temporary roots were mode0755, not required mode0700 |
| Corrected native run R2 | 20 passed; 26 filtered |
| Final native run R3, four test threads | 24 passed; 26 filtered |
| Full service library R1 and final R2 | Each: 41 passed, 9 failed; no ignored/filtered tests; exit 101 |
| Service doctests | 2 positive and 10 compile-fail passed |
| Final isolated entrypoint diagnostics | 5 passed; 45 filtered; excludes the failing socket setup |
| Eleven-package all-target check | Passed with existing warnings |
| Changed Rust formatting, whitespace, hygiene delta | Passed |
| DCO through integrated code | 101 signed-off commits from `9350f2f6b73da5e249a56a297097c4919378b363`; no exceptions |

Focused/diagnostic tests overlap full suites. All 24 native tests and all 13
existing durable-state tests pass in final R2. Native coverage includes each of
12 persistence boundaries under error and unwind, exact retry, prior/proposed
recovery, V1/native state and response compatibility, every state-byte mutation,
invalid lengths/metadata/symlinks/FIFO, second writer, deployment/credential/key
refusal, exact/one-short accounting, historical peaks/denials, overflow, input
retirement, and late failure followed by drop/reopen/recovery. These are injected
faults against live filesystem operations, not machine power-loss tests.

The nine full-suite failures match the previous checkpoint: socket send/domain
inspection return `EPERM`, response-direction shutdown fails, wrong-endpoint
testing does not reach its expected category, and two entrypoint failures follow
the first failure's poisoned test mutex. The isolated entrypoint run again
passes those two tests. They are not relabeled as a green full suite. Static-image
execution and protected-root startup were not run. The prior supervisor socket
failures remain unresolved.

The all-target check covers supervisor, issuer, client, coordinator, closure
capability, anchor coordinator/service/provisioner, host, cargo-fe2o3 and backend.
Cargo target kinds are not GPU architectures. Doctest code was unchanged after
its successful run; subsequent changes corrected tests only. R1 fixture failures
were corrected by creating mode0700 private roots, not weakening production checks.

## Evidence Digests

SHA-256 of logs in the directory above:

```text
504475bd685c1985f82a9a47d50e770745b5bc04790a8c00a43e8328d3ecdc08  conditional-native-durable-anchor-tests-r1.log
12ac475eb33dff31ca5cf3ebeb3c6c4de76910256d86591db4d786823479bf81  conditional-native-durable-anchor-tests-r2.log
e56279d49837638c553b9e8e671e602827f7e97a1a3257a6ae2d4f9c10e8fc4b  conditional-native-durable-anchor-tests-r3.log
96ed0e08f8f7f909b9cd749e059353f4bf38fcfd3ff5d02e284c5fe73d62bb24  conditional-native-durable-anchor-service-suite-r1.log
bda90b51a765e3798992d8753f9f7b049e8f933814af1aa4f56a04cf5e3664c4  conditional-native-durable-anchor-service-suite-r2.log
aed851f2a8b591393379879a385b7caf86979d03013ccd639436f0424362608e  conditional-native-durable-anchor-doctests-r1.log
54531561df8d9eafe52d7378c08d4a7154db9044dc0fe82acbf9fb4faa421c07  conditional-native-durable-anchor-entrypoint-isolated-r1.log
28557925854b8f4d1e32ad7c28ac9d06bfea8b996c1d9e1dacf2167211afb5ee  conditional-native-durable-anchor-integrated-check-r1.log
e468e71a2caddddd881adf699ebceb129326c232420f69c9357e29b3e451e91f  conditional-native-durable-anchor-hygiene-r1.log
fc5b2c4a0700a50bb740cc3147549ded9d06f30dbe404fcdd359f0f516084b48  conditional-native-durable-anchor-dco-r1.log
```

## Remaining Gates

1. Drive native durable owners from the shared peer loop, then the inherited
   daemon, helper and root coordinator. Preserve lifecycle and exact descriptor
   custody, including sealed key descriptors during startup cleanup. Current
   helper and daemon still extract raw V1 keys.
2. Execute actual protected-root reissue, startup/readiness/recovery and cleanup.
3. Complete native admission/preparation, postchecks, invocation finish,
   revalidation, V5 publication and SubjectV3 transport in the single production
   compiler pipeline on its original ledger. Backend receipt admission remains
   V1, and conditional production finalization still refuses.
4. Complete source/machine/numerical proofs and all 47 target-matched GPU runs,
   including generic non-AMD validation. Numerical differences need explicit
   compiler-proved bounds, not unchecked tolerances.

Fresh SSH probes of mi350, mi350-2 and mi300x failed DNS resolution. No remote
job or scratch directory was created; no shared cache/source/report cleanup was
performed. Neither remote main nor the tutorial site is credited as updated by
this local checkpoint. Normal pushes to both remotes remain required.

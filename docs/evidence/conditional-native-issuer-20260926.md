# Conditional Native Issuer Checkpoint

This continues the [service/profile checkpoint](conditional-service-profile-20260926.md).
Source and test snapshot: `85b283668f7c4bf2335da393c87b53cd42ba072e`.
All [#272](https://github.com/harsh-nod/fe2o3/issues/272) milestones M0-M7 remain
open. This is issuer-side implementation and component validation, not protected
deployment, machine equivalence, GPU execution, or 47/47 end-to-end completion.

## Implemented

- Actual V3 launch-input custody separately admits sealed PolicyV3 and ManifestV3
  at fixed slots 6/8, retains private CLOEXEC duplicates, and revalidates both
  objects and the exact policy join on the original budget. The identity-only
  manifest wire is not a family selector or provisioning proof.
- WorkerAnchorJournalV3 retains an actual TransactionV3. Its version, magic and
  hash domain are distinct. A private shared body preserves V2 stages, signature
  checks, early error precedence, nested charges and legal successors.
- ProtectedCompilerExecutionIssuerAdmissionV3 consumes actual PolicyV3/KeyV3
  with the existing protected process and transport custody. It measures the
  running executable, retains the original Work lifetime, and rejects a foreign
  ledger before revalidation I/O. No legacy owner conversion is available.
- Prepare/Issue observe the compiler independently, recover its actual V5
  publication, and retain the publication lease and locked consumption token.
  The published invocation must exactly equal the observed descriptor before
  SubjectV3 is derived. Observation, currentness and admitted custody are checked
  around signing, durable commit and response construction.
- Both families use one packet-loop implementation and the existing singleton,
  recovery planner, durable mutation engine and journal filenames. Conditional
  issuer/Worker records use distinct V4 framing and identity/signature domains;
  the conditional anchor journal is V3. Foreign or mixed records are refused
  before recovery mutation, never treated as absence or automatically migrated.
- Publication still requires an independently signed anchor transition, durable
  Worker custody and exact issuer advancement. Fresh currentness explicitly uses
  the V3 carriage/anchor methods on the same cumulative account.
- The V3 inherited entrypoint consumes fixed FD3..11 through the shared hardening
  and service path, including closing original readiness fd9 before emission.
  The separate `fe2o3-compiler-execution-issuer-conditional` binary calls it.
  Default executable, runtime family selection and trusted deployment are unchanged.

These changes share existing implementations rather than add another executable
IR, proof graph, GPU finalizer or service recovery algorithm. No source/kernel
name, environment override or decoded-byte fallback selects the conditional family.
Issuer provenance/currentness is not a proof of CPU/GPU semantic equivalence.

A native source-only worker implemented launch inputs and anchor journals in a
private worktree and independently reviewed the primary integration. No actionable
source defect was found. The review checked V2 executable-body preservation,
actual locked V5 custody, original-ledger lifetimes, distinct records in one
namespace, V3 currentness selection and unchanged fixed-FD ordering. It did not
run protected tests or grant authority.

## Validation

All commands used the pinned `nightly-2026-04-03` toolchain, offline locked
dependencies, one Cargo job, serial tests, disabled GPU visibility, a 12 GiB
virtual-memory bound, an outer timeout, and the existing private cache/runtime.
Source was frozen during each command. Logs are retained in
`/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921`.

| Command scope | Result |
| --- | --- |
| Protocol crate tests | 238 unit/integration tests, 104 compile-fail docs and 7 positive docs passed; none failed or ignored. |
| Issuer library tests | 19 passed. Two subprocess helper roles are marked ignored as standalone tests but are explicitly executed by the passing fixed-slot parent tests. |
| Broker library `native` filter | 83 passed, 20 failed, two existing opt-in/helper tests ignored, 222 filtered. All 66 issuer admission/service/readiness/Worker/cross-family cases passed. This broader command is **not green**. |
| Broker and issuer docs | 77 + 11 compile-fail docs and one positive example per crate passed. |
| All-target check: protocol, broker, issuer | Passed. |
| All-target downstream check: client, issuer, broker, cargo-fe2o3, rustc-codegen-fe2o3 | Passed. Cargo target kinds, not GPU architectures. |
| Conditional binary | Normal local dynamic binary linked; not executed or admitted as a sealed-static image. |
| Source hygiene | Changed Rust files pass pinned rustfmt; diff whitespace and hygiene-delta policy pass. |

The 20 broker failures are socket send/inspection `EPERM` and resulting
earlier-`PeerDomain` error-kind assertions, including existing V2 and unchanged
transport tests. No socket check, timeout, permission boundary or test was weakened
to obtain a pass. The complete protected/live V5 service flow remains unvalidated.

The first all-target check found a test helper borrowing an attestation whose
verification API consumes it. The helper now takes ownership, without cloning;
the subsequent check passed. This did not require changing production verification.

| Log | SHA-256 |
| --- | --- |
| `conditional-native-issuer-check-r1.log` (initial test compile failure) | `587de638983455c945c529ae4df9e905d83cb9842855833ad78e5482c68dc5dd` |
| `conditional-native-issuer-check-r2.log` | `5cfc2a116651a5cef4c6adc1d7b944adfb8f873d60e2b35ce31265f431d5ef87` |
| `conditional-native-issuer-service-tests-r1.log` | `70b8bc9d2c4725029fcc72b9627c75b70b3a45a6000f9a670e6bc14767ced5dc` |
| `conditional-native-issuer-protocol-tests-r1.log` | `9f0d593c8535fd9915ba2682e98cf29e1aa0be73c58bc53a7bce86e61edaf110` |
| `conditional-native-issuer-input-tests-r1.log` | `835b50c464e01034790d9d8bc477cb3316cfb7da96f4725c10c1a688ecc58ab0` |
| `conditional-native-issuer-docs-r1.log` | `715815e5d433ea7bfc17f2090b3bc9c22ab4e7322bcc8a8edbfad37c7fa69525` |
| `conditional-native-issuer-downstream-r1.log` | `6995fbda84e8e58ee8d1952ee36d5e04ae1f7566969ac4515ea61108d5335ad3` |
| `conditional-native-issuer-binary-r1.log` | `99dc56431bc1fb781ef475256cbf6578b322493a4bee419aeefb45d0c1504b3b` |

## Still Required

1. Independently pinned conditional supervisor/launcher deployment and a measured
   sealed-static issuer image; protected inherited-entrypoint and live V5
   observation/issuance/anchor/currentness tests.
2. Early compiler client custody whose original TARGET resource account survives
   lowering, proof preparation and terminal exchange. A fresh temporary admission
   account or the terminal client's exclusive borrow cannot substitute for this.
3. The consuming normal conditional publication continuation and authenticated
   parent/Worker intake, preserving original owners, locked occurrence and budget.
4. Applicable LLVM/ISA/machine refinement, including explicit compiler-proved
   numerical error limits, finalizer/host integration and generic capability coverage.
   A CPU oracle or inert receipt cannot replace these proofs.
5. Complete simulator, negative and target-matched hardware evidence for all 47
   kernels, followed by the release gate and identical public mains.

## Environment And Publication

SSH to `mi350`, `mi350-2` and `mi300x` failed DNS resolution in this session.
No remote job or scratch directory was created. After a completed command,
six inactive prior test executables were removed only from our private local
cache, reclaiming 1,095,682,408 bytes; source, worktrees, reports and live builds
were preserved.

Normal fetches from both Git remotes fail DNS. Both public mains were independently
observed through GitHub metadata at `1d8ef2462d141ad257e59807ab8727fc297ed1c5`.
They contain concurrent work that must be fetched and reconciled before a normal
push. This local candidate has not been published to either main; no force push
or alternate Git transport was used.

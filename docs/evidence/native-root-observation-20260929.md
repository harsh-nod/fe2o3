# Root Observation And Actual Issuer Startup

Date: 2026-09-29 UTC. Code checkpoint: `b58965179dc9b2e49bba990dfc09fbbc585811b1`.
This follows the [root issuer launch checkpoint](native-root-issuer-20260929.md).

[Issue #272](https://github.com/harsh-nod/fe2o3/issues/272) and
[compiler occurrence #218](https://github.com/harsh-nod/fe2o3/issues/218) remain
incomplete. The seven-case actual root-to-issuer startup matrix now passes.
That is **not** seven qualified kernels, protected proof execution, or 47/47
end-to-end completion. The [compiler fixture](../../crates/fe2o3-protected-service-spawn/tests/fixtures/native_compiler_exec.c)
stays at its first exec stop. The ignored matrix's source documents its required
binary paths, opt-in environment and isolated-container prerequisites.

## Implemented

- A scoped root-task observation view borrows the original trace, account,
  thread and unreaped child. It exposes neither a pidfd nor wait/signal ownership.
  Descriptor duplication uses the original pidfd with continuity checks on both
  sides, exact CLOEXEC validation by consumers, and an explicit unreserved charge.
  Returned files remain inert inputs requiring independent role admission.
- Native broker observation now shares its existing invocation, executable,
  backend and artifact predicates between service and original-root sources.
  No caller-provided observation provider or PID-reopen constructor was added.
  The new root-specific occurrence methods are not yet connected to an installed
  authenticated root-control transaction.
- Nested occurrence, observation and spawn failures preserve actual resource
  errors without reclassifying nominal diagnostics. A closed ptrace operation
  enum accommodates both glibc and musl request types.
- Credential-bound bootstrap EOF before readiness has a distinct failure
  diagnostic. EOF does not prove reaping; malformed packets and unauthenticated
  one-byte stages still refuse.
- The shared secure entrypoint checks initial invocation shape after restoring
  confinement but before libc. This fixes actual non-root helper startup:
  nondumpability makes `/proc/self/environ` root-owned and unreadable to that
  service. The entrypoint checks a bounded nonempty argv0 and empty initial
  environment, then records a private atomic observation. Rust retains its bounded
  command recheck. No dumpability, capability, UID or sealing rule was relaxed.
  See the [Linux proc ownership rules](https://man7.org/linux/man-pages/man5/proc_pid.5.html).

## Local Validation

Bounded offline checks used pinned nightly `2026-04-03`, one build job and serial
tests, with source/tool snapshots unchanged during each command. GPU access was
disabled. These are unoptimized mechanics builds, not release qualification.

| Suite | Passed | Ignored |
| --- | ---: | ---: |
| Coordinator, profile and spawn unit/integration tests | 518 | 15 |
| Their doctests | 134 | 0 |
| Broker, supervisor, anchor helper and anchor service tests | 699 | 51 |
| Their doctests | 243 | 0 |
| `cargo-fe2o3` wrapper tests | 452 | 5 |
| Unsafe-source policy | 5 | 1 maintenance test |

Counts exclude nested subprocess summaries. The service run used source snapshot
`eb8a63f07485bbd25bed9b88d543f54a573cc762305d8797f7d547761322f02c`.
The final focused, wrapper, inventory and all-target checks used
`bbbca8066d8005b520fa717191469c63708836eb67f20aef0ed20c3ef56f1ffe`.
Between these snapshots, changes were observation tests, fixture budgeting,
comments and documentation, not service behavior.

Wrapper, codegen backend and coordinator all-target checking passed with warnings.
Scoped formatting, diff checks, eight hygiene self-tests and the change-set hygiene
gate passed. The reviewed unsafe inventory is 2,428 sites in 421 files.

## Native Validation

The normal, uninstrumented `root-issuer-native-rg` matrix passed on `mi350`:

| Case | Checked outcome |
| --- | --- |
| `ready-cancel` | Actual Ready120 plus EOF, original-account continuity and cleanup |
| `ready-unwind` | Foreground unwind retains cleanup custody through terminal drain |
| `same-uid` | Issuer/compiler credential collision refuses without issuer state |
| `corrupt-state` | Existing corrupt issuer state remains unchanged and startup refuses |
| `zero-timeout` | Refusal without issuer state |
| `short-work` | Original request's insufficient work refuses without issuer state |
| `short-storage` | Insufficient frame storage refuses without issuer state |

This uses actual static V3 issuer, anchor helper and anchor daemon binaries,
genuine prepared inputs, the original compiler trace, retained lifecycle leases
and independent cleanup funding. The test's unexecuted supervisor/launcher slots
use distinct admitted static fixture images. It is not an installed deployment.
The positive launch remained below its queried work/peak-storage bounds; logical
storage charges are not an RSS guarantee. Final cleanup verifies retired charges,
descriptor counts, compiler termination and lock release.

Three original-pidfd descriptor tests also passed on `mi350-2`: ordinary observation,
exact quoted work/scratch, and one-short work at the second continuity check. The
last verifies descriptor closure, restored storage and preserved denial history.
A local callback-exit test verifies rejection of late success without consuming
the caller's terminal wait ownership.

All native runs used read-only, networkless, CPU/memory/PID-limited containers
without GPU devices. Every owned container, scratch directory and private SSH
connection was removed, including failed diagnostic attempts. Temporary diagnostic
instrumentation was removed before the passing matrix. Earlier failures identified
unsupported executable-memfd flags on the older `mi350-2` kernel, a missing fixture
FOWNER capability, the invocation bug, an unsuitable unexecuted ELF fixture and an
undersized fixture work ceiling. None was treated as a pass or a reason to weaken
production admission.

Passing binary SHA256 identities:

| Artifact | SHA256 |
| --- | --- |
| Coordinator tests | `effbbb3d5f3291a866af715c02d85496ca1fb8b3a7b51ae550a14a0b92abaf10` |
| Spawn tests | `61dc33e9d7522b05777e503910a0ab20764ad09388fa4253951e234ebe523243` |
| Static compiler fixture | `5cd9ee2d9194b8302fc39d9ed940bd5a2da3166a130596a8e957e5acb2934af5` |
| V3 issuer | `fa6db2e34cf1b2c3abd60538cea82a6ed4dd6d3918dc128af66d721bc5de1398` |
| V3 anchor helper | `9c9b4de47b397f57f5d849b69d2caafe63374a799b91d1a4676bfe20153618a1` |
| V3 anchor daemon | `ba8cd94beb96b7bc8b174adbe267235b5d236acaf7031bb7a4c137afe80ef0ac` |

The three service images passed ELF64 static EXEC inspection, exact secure-entry
matching, non-executable-stack checks and absence of dynamic dependencies.

## Still Required

- Authenticate root/issuer control traffic and connect actual root observations
  to retained native occurrence leases/tokens across Prepare, Issue and replay.
  Durable publication must retire both owners before acknowledging retirement;
  recovery must not bypass outstanding retirement.
- Exercise root broker observation/revalidation and coordinator phase gates with
  actual invocation/backend/artifact inputs, including mutation and lost-custody
  negatives. Stolen-wait quarantine coverage remains separate work.
- Connect the owning installed production attempt, wrapper intake, protected
  runtime/helper association, descendant enforcement and safe host activation.
- Run protected proofs and target-matched GPU tests for the complete manifest,
  with exact receipts, artifacts and negative cases. No #272 milestone closes
  merely because this startup matrix passes.

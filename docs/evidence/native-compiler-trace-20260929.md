# Native Compiler Trace And Service Key Checkpoint

Date: 2026-09-29 UTC. Tested code: `70aa4bfd4b17b9f195a82fc84adcd9c47cd01618`.

Follow-up: [private root-to-issuer launch checkpoint](native-root-issuer-20260929.md).
The results and remaining work below describe this earlier tested code.

[Issue #272](https://github.com/harsh-nod/fe2o3/issues/272) remains incomplete.
This checkpoint establishes wrapper input custody and native process/key-transfer
mechanics, not a production compiler transaction, protected proof execution,
GPU execution, or 47/47 tutorial qualification.

## Implemented

- Protected wrapper custody retains the actual pinned directory used by the
  prepared `fchdir`, alongside captured stdio and invocation metadata. It does
  not reopen the cwd pathname or turn that observation into compiler authority.
- A private compiler trace consumes the original clone-owned child and its
  authenticated channel. Issuer input transfer is one-use, requires its own
  confirmed held exec stop, and retains the same transitive backing independently
  of compiler reaping. No replacement pidfd, second waiter, or fabricated public
  `AcceptedHandoff` is introduced.
- Transfer failure and unwind cancel the original child. Review found that the
  success state was committed before the budget scope's final accounting check.
  A native regression reproduced a rejected transfer followed by successful
  resume. The fix keeps cancellation armed until the complete scope succeeds.
- A fresh V3 service-owned signing-key image is derived from retained root key
  custody and the exact deployment/policy. The root template is unchanged. The
  new image is anonymous, exactly sealed, read-only and mode 0400; final staged
  descriptors must match its inode, owner, policy and source key. Receiver
  current-owner admission and public handoff credential rules remain unchanged.

## Validation

Final serialized tests and compiler checks used the same unchanged 9,268-file
source snapshot:
`a410f789251f3d42a5dae1d4d24a468dc65776f6dcb5bb251e23025b37ab544f`.

- Capability, client, coordinator, supervisor and spawn: **1,118 unit/integration
  tests and 430 doctests passed**; 55 environment-dependent tests ignored locally.
- `cargo-fe2o3` wrapper binary: **452 tests passed**, five ignored.
- Wrapper, codegen backend and coordinator all-target check passed with warnings.
- Unsafe-source policy: five passed, maintenance test ignored. Reviewed inventory:
  2,417 sites in 416 files. Scoped formatting, diff checks, eight hygiene-policy
  self-tests and the change-set hygiene check passed.

Counts exclude nested unit-test subprocess summaries. Doctest totals include
both runnable merged groups and the separate compile-fail groups.

Five isolated native diagnostics passed on `mi350-2`:

1. The final combined-build coordinator binary passed nine cases covering channel
   receipt, held exec, one-use transfer, retained backing, refusal, unwind and
   the final-accounting cancellation regression.
2. Fresh key ownership, source-template preservation and exact resource charges.
3. Wrong deployment/policy/key, equal-byte inode substitution and metadata drift.
4. Late refusal/unwind seed wiping, descriptor cleanup and storage restoration.
5. After retiring root custody and dropping GID then UID to the deployment's
   service identity, the unchanged receiver admitted and revalidated the image.

These used read-only, networkless, resource-limited containers. The key tests
required CHOWN only, except receiver admission also needed SETGID/SETUID. The
coordinator used a static fixture, not an approved compiler. Final binary SHA256:

- Coordinator: `0375e9ee537752a28e9267712f3e4eab97175ae580f96f1e0c51a6c9ee297fd5`.
- Key tests: `6488c07db09272a66743da24c19dcd5a7a587fedab56d02a5d42830d9169ab1e`.

Every attempt's container, scratch directory and private SSH connection was
removed. The reproduced failure and a rejected stale-binary zero-test run are
preserved as failures, not passes. The runner now enumerates the exact selected
test before remote execution. Final diagnostics used the final suite binaries.

## Remaining Integration

The installed coordinator still does not accept complete compiler attempts.
Actual wrapper intake, runtime/loader/input mappings, retained helper/compiler
composition, issuer launch and its 120-byte pipe readiness protocol, full proof
association, descendant enforcement and installed activation remain required.

The audit also identified a separate authorization gap under
[compiler occurrence #218](https://github.com/harsh-nod/fe2o3/issues/218).
Native observation currently calls `pidfd_getfd` inside the nonroot issuer.
That syscall requires ptrace attach authorization, not merely possession of a
pidfd. [Linux syscall contract](https://man7.org/linux/man-pages/man2/pidfd_getfd.2.html).
The current root-owned namespace arrangement does not establish that permission
for the distinct-UID issuer. This is a code-review finding, not a protected
production run. Integration must preserve live observation and publication-lock
custody across Prepare, Issue and replay; a startup snapshot or cached success
cannot replace it. Credential checks must not be weakened to bypass the gap.

Additional native coverage is still needed for consuming trace-seizure failures,
direct dependency-boundary resource/account substitutions and an actual issuer
cleanup slot retaining compiler backing through deferred/quarantined cleanup.
No milestone or kernel qualification is closed by these mechanics alone.

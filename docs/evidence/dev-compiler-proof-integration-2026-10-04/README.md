# Compiler Proof Integration Qualification

Base: `c84b1b41cf282e3af2cf0e6d21f7ab6a7437f5d3`.
This checkpoint removes the compiler-time proof placement blocker on the multi-GPU
critical path. It does not complete ordinary application admission, two-GPU
execution, formal verification of the transport, or HIP/HSA performance parity.

## Implemented

- Protected Cargo retains one admitted proof broker. An authenticated one-use
  preparation consumes the original wrapper exec-permit pidfd; the initial
  four-descriptor handshake remains unchanged. Proof work and source/ISA
  observation share the original attempt and run concurrently.
- A consuming spawn delegates the exact sealed invocation to the original rustc
  child before issuer completion. Failure paths preserve existing kill/reap and
  revocation handling. Session cancellation precedes scoped worker joining.
- Protected compiler custody owns one brokered runtime through both existing
  proof joins and publication. Only extraction mode can open a local execution
  runtime; synthetic process-observation test custody cannot execute proofs.
- Canonical invocation FD199 stays owned and close-on-exec through issuer
  inspection. The previous early close made genuine issuer pidfd_getfd fail with
  EBADF. Repeated admission cannot close an already retained owner's descriptor.
- The genuine harness requests code-object V6, matching production compiler and
  host contracts. Strict Worker version disagreement still rejects.

No proof-output importer, captured proof, permissive provider, or weaker seccomp
policy was introduced. Existing proof source, output checks and receipt joins
remain the producers.

## Validation

- Cargo frontend full unit binary: **420 passed, 5 ignored**.
- Compiler full library: **524 passed, none ignored**.
- Verifier full library: **188 passed, 11 ignored**. Some ignored entries are
  subprocess fixtures invoked by ordinary parent tests; others need deployment.
- Strict changed-production Clippy (`--no-deps`, `-D warnings`), scoped rustfmt,
  shell syntax, and `git diff --check` pass. No lint suppressions were added.
- A broad dependency-inclusive Clippy attempt stops at the existing unrelated
  `fe2o3-semantic-import/src/profiler_bundle.rs:791` `derivable_impls` warning.
  Whole-workspace lint cleanliness is not claimed.

The archive includes the source patch, commands, production binary hashes,
test/build logs, failed experiments and read-only review notes.

## Genuine Campaign

Four isolated installed-layout campaigns were run, in order:

1. Applying `RLIMIT_NOFILE=243` to the entire root deployment failed before Cargo:
   protected-service staging requires descriptors at 400 or above. This is not
   a composed wrapper low-file-limit qualification and did not change policy.
2. The initial integration completed actual generated proofs but exposed the
   original invocation FD199 lifetime bug at issuer publication.
3. After retaining FD199, real proofs and issuer-backed publication succeeded;
   Worker correctly rejected the harness's V5 request against compiler V6.
4. With the V6 harness, device compilation, issuer publication, Worker finalization
   and generation commit completed. Ordinary host-library compilation then failed:
   `#[kernel(typed)] must be compiled through cargo fe2o3 build, check, test, or clippy`.

The final campaign is **failed overall**: 0 passed, 1 failed, exit 101, 752.17 seconds.
Its positive evidence ends at the completed device phase. The host phase clears
wrappers and does not propagate the original device binding. The application did
not execute its FD195/current-record audit or retain its conditional proof.
All four campaign logs record removal of their exact outer cgroups. Namespace
deployment files and application build targets were ephemeral. No MI300X action
or GPU benchmark was performed.

## Next Gate

Carry the binding from the original committed device invocation into only the
matching ordinary host library through a distinct sealed projection. Preserve
receipt/profile/source/current-publication validation on success and failure;
do not derive a new host binding or hold a publication lock across the runner.
Then qualify application admission, followed by authenticated fill on two free
gfx942 devices and bidirectional PUBLIC XGMI with full guarded readback.

Additional transport coverage remains for production-shaped duplicate preparation,
mismatched second request, nonprotected preparation, and pre-delegation shutdown.
Codec tests and the genuine positive path do not substitute for all those cases.

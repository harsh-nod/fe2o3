# Genuine Application Startup: 2026-10-04

Base: `78e1f0e6f0b712f2e89f6cf41ca468d78a37ace1`. The archive records the
source patch, exact invocation, tool measurements and qualification logs.

## Result

The private-layout campaign reaches the actual selected rustc backend with one
fresh typed kernel. It does **not** pass compiler-time proof execution, emit an
admitted artifact, run the host application or execute a GPU kernel. The failure
is retained as a failing ignored integration test, not converted into success.

The campaign exposed three production blockers that are now fixed:

1. Executable sealing changed ownership before mode, requiring `CAP_FOWNER`
   absent from the shipped coordinator/manager profile. Set mode while the
   private memfd is still owned, then transfer ownership and seal/revalidate.
   A real-root regression with `CAP_CHOWN` and without `CAP_FOWNER` fails before
   the fix and passes afterward; owner, mode, bytes, read-only access and seals
   are checked. No production capability was added.
2. Authority metadata/config queries appended duplicate `--frozen`/`--offline`.
   Preserve caller arguments and add each required flag only when absent across
   all three query sites. Malformed caller input is not silently repaired.
3. `run --bin` forwarded its binary selection into AMDGPU, which cannot produce
   binary crate targets. Compile the package library for the device; preserve
   host binary selection and application arguments. Reject unsupported target
   selectors explicitly. The fixture now defines its typed kernel in `src/lib.rs`.

Campaign 11 passes real fresh-key provisioning, production resource inspection,
approval installation, actual manager startup and authenticated coordinator
readiness using its original 14 descriptors. The exact effective, permitted and
bounding capability sets match the shipped seven-capability profile. It then
enters protected authority release and reaches the selected kernel (3 CGUs,
1 candidate, `gfx942:xnack-`). The campaign uses no captured kernel namespace,
compiler/proof receipt or application handoff.

## Remaining Gates

The selected compiler fails with `retained generated-proof runtime failed:
Process`. Source tracing establishes an incompatible process contract:
`cargo_invocation_boundary` installs an inherited exec-notification seccomp
filter, whereas `validate_controller_security_v2` requires mode/filter count
zero before local proof execution. Proof-child observations subsequently require
exactly one verifier-installed filter. This is rejected before proof-child spawn.

Keep both protections. Compiler-time proof execution needs an independently
admitted unfiltered controller and custody of its exact generated input/result.
The post-publication application custodian cannot simply be called earlier: its
input requires finalized HSACO and the committed envelope. Next, implement the
retained device-binding projection for host library compilation, then qualify the
ordinary FD195/current-record/proof path. Only then run the two-GPU fill and
bidirectional native XGMI checks described in the critical-path document.

## Verification

- Cargo: 6 phase-plan, 8 project and 12 authorized-closure tests pass.
- Proof custodian: 26 ordinary unit tests pass; 10 privileged/helper tests remain
  ignored by default. Protected executable: 3 ordinary tests and the explicit
  restricted-root ownership regression pass.
- Existing installed-resource campaign still passes (53.79 seconds), including
  real retained proof execution and its rejection/cleanup controls.
- Scoped strict Clippy, scoped formatting and `git diff --check` are recorded.
  Unscoped dependency Clippy encounters a preexisting `derivable_impls` error in
  `fe2o3-semantic-import`; unrelated source is unchanged. Repository-wide format
  checking also has preexisting unrelated differences.

## Limits And Cleanup

This is not a systemd boot/package activation test or independent administrative
approval. The harness approves its own freshly inspected candidate. Bubblewrap
adds no-new-privileges; the service units' exact systemd composition is untested.
Unlike the resource-only campaign, the genuine campaign retains read-only host
source/toolchains, Cargo caches and GCC's `/usr/libexec/gcc` directory. That GNU
closure is not a hermetic distribution package. Keys stay in private tmpfs.

Cleanup kills/reaps service parents and drains the fresh enclosing cgroup,
including any quarantined descendants. This is containment qualification, not
graceful service shutdown. Logs confirm removal of each completed outer cgroup.
MI300X SSH attempts timed out before remote execution; no remote files, jobs or
services were created. No new multi-GPU performance or HIP/HSA parity is claimed.

Preliminary logs are retained: setup/profile errors, failed cache builds, and the
intentional before-fix regression. Campaign 12 additionally observed a fixture
README edit during compilation and rejected source-closure revalidation; it is
not the clean reproduction. Its first failure was the same proof preflight.
Campaign 13 uses stable source and the added zero-inheritable/ambient-cap
assertions. It reproduces the same proof failure in 121.03 seconds, without a
source-revalidation error, and removes its outer cgroup. The archive excludes
executable images, signing seeds and caches.

Source patch SHA-256:
`d76472e58a91694e6060fe87c6fe2843d8e06fbb98be67990e45a4c58227362a`.

Evidence archive SHA-256:
`0fee39e455b24b1a7d4eb2b410fe94ae20d48db8e85bbfb936e870f7e6684654`.

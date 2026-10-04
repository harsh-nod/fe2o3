# Proof Deployment Provisioning V1

This supplies the missing production approval installer for the ordinary
application proof route. It does not establish genuine compiler admission,
completed proof, native execution, multi-GPU operation or formal verification of
the installer. The existing 13-file compiler-only V1 bundle remains unchanged;
proof resource packaging and the final-path analyzer closure remain separate work.

## Build

`scripts/build-static-proof-custodian.sh` builds the manager, application controller
and `fe2o3-proof-custodian-provision` as musl static executables. It checks the
secure entrypoint, absence of loader dependencies and undefined symbols,
non-executable stack, invalid-entry rejection and the production static ELF parser.
`FE2O3_STATIC_PROOF_CUSTODIAN_TARGET_DIR` can select an existing shared Cargo cache.

Install the three qualified executables as root-owned, root-group, single-link
`0555` files under `/usr/libexec/fe2o3`. Provision the compiler client profile first.
The manager unit remains `deployment/systemd/fe2o3-proof-manager.service`; it must
be activated independently of the compiler service, after resource qualification.

## Inspect, Then Approve

The fixed CLI has two commands:

```text
fe2o3-proof-custodian-provision inspect-fixed-resources OUTPUT
fe2o3-proof-custodian-provision install CANDIDATE EXPECTED_SHA256
```

Inspection must run as the actual dedicated non-root proof UID/GID. It captures
and retains the same closed process profile as the controller: all four IDs,
empty supplementary groups and all capability sets, no_new_privs, zero securebits,
no seccomp, zero dumpability/core limit, umask 077 and default owned SIGCHLD state.
The static secure entry supplies dumpability, core-limit and no_new_privs settings;
the administrator must arrange the remaining profile, including `umask 077` and
credential/capability dropping. Inspection as root rejects before resource use.
Both UID and GID must be distinct from the installed compiler and anchor accounts.

Inspection accepts no alternative resource paths. It retains root-owned final-path
controller, manager, coordinator, Worker and compiler-profile objects. It invokes
the real bounded authenticated Worker inspection and opens the protected Verus
runtime at the controller's fixed path. It revalidates retained resources and its
process profile before creating the output exclusively, mode `0400`.

The output is exactly 440 bytes: the existing canonical 280-byte application
deployment followed by the existing canonical 160-byte manager deployment. The
manager record binds the application record identity. The printed candidate
SHA-256 is discovery data, not self-approval. Review the resource provenance and
candidate before supplying that digest independently to the root install command.
Do not blindly pipe discovery output into privileged installation.

Worker executable identity is domain-separated, not ordinary `sha256sum`. Runtime
closure identity includes actual mapped dependency paths. Verus identity binds
the protected manifest and target pins, not just the shell installer's manifest
hash. Development-tree measurements cannot be reused after relocation. A Worker
executed from a sealed memfd also cannot use `$ORIGIN` to locate its installed
dependencies. Complete and measure the final immutable analyzer layout first.

## Root Installation

Root installation captures one bounded regular candidate without following a
symlink or blocking on a FIFO. It checks the independent lowercase SHA-256 before
decoding either record. No candidate file, Worker or Verus process is executed.

It validates canonical crosslinking and credential separation, checks all three
installed static images against the candidate, seals them under the existing
production ELF contract, and rechecks the Worker's domain-separated image identity.
Original root-owned path objects and compiler profile remain retained through
publication. The analyzer closure and Verus identities are approved pins here;
they are not freshly executed or independently remeasured by root. Actual controller
resource admission must still validate both before ResourcesReady.

Installation creates a private staging directory under `/etc/fe2o3`, writes and
syncs exactly two `0444` records, and publishes the complete `0755` directory as
`proof-custodian` using a no-replace rename. Files and directories have no xattrs;
records have exactly one link. Parent sync and original-directory/record checks
follow publication. An identical existing installation is revalidated without
replacing its objects. Partial, conflicting, extra-file, writable or aliased
installations reject. This command does not perform upgrades or revoke active
proof custody.

Ordinary prefix errors attempt to remove only the command's own stage. Cleanup
failure or process death can leave `.proof-custodian-*`; later installs refuse
such residue rather than delete unknown objects. Inspect interrupted staging as
administrator before recovery. Failures after the final rename explicitly report
that approvals may already be installed; never interpret them as a rollback.

## Qualification Boundary

`scripts/qualify-proof-deployment-install.sh` runs the real install executable
inside private real-root PID/mount namespaces. It requires absolute environment
paths for `FE2O3_PROOF_INSTALL_TEST`, `FE2O3_STATIC_PROOF_CUSTODIAN_DIR`,
`FE2O3_PROOF_INSTALL_COORDINATOR` and `FE2O3_PROOF_INSTALL_WORKER`.
It tests actual fixed images, independent pins, root/non-root role rejection,
credential conflicts, metadata rejection, approval publication, the production
application-deployment opener and object-preserving reinstallation.

That campaign deliberately uses inert analyzer/runtime facts. It does not
positively qualify inspection, service-profile resource execution, manager startup
or GPU execution. Follow with the genuine final-layout inspection, controller
resource campaign, ordinary compiler/application admission and two-GPU fill/XGMI
pipeline described in [the critical path](runtime-multi-gpu-critical-path.md).

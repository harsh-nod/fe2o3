# Committed Host Binding And Application Admission

Base: `560f67b0a26989914919936ba070cbdb3a16dfe0`.
The final genuine installed CPU campaign passes. This closes the original
device-to-host binding and composed application-admission prerequisites for the
two-GPU path. It does not qualify GPU execution, performance parity, general
Rust/device coverage or formal correctness of this frontend transport.

## Implementation

- A distinct canonical sealed production-host projection retains the original
  committed envelope lease, independently admitted compiler profile, source
  closure and source/directory descriptors. Receipt, compiler closure, target,
  source and current publication are checked before and after host Cargo,
  including errors. No publication lock is held across the application runner.
- Only the exact matching ordinary host library receives the original device
  binding. Crate, package, source, host target, edition, library role, cfgs and
  profile must match. Main, dependencies, build scripts and tests remain unbound.
  The projection grants macro namespace only, not proof/load/launch authority.
- Device invocation normalization requires the exact pinned Cargo build-std
  imports and unstable-options suffix. Static host Cargo probes admit only the
  exact reviewed static-link triple for query classification; rustc receives the
  original argv. Negative controls reject changed, missing, duplicate or extra
  options, response files and authority-bearing compiler selectors.
- The runner uses the retained sealed image FD200; the dedicated supervisor
  reexecs `/proc/self/exe`, not its deleted memfd display path. Child descriptor
  sweeps still expose only the intended application handoff, proof and FD195.
- Ordinary host Cargo no longer installs an unused compiler exec-permit observer:
  it has no compiler broker/permit consumer, and the inherited listener prevented
  the application's own listener from installing. Device compilation retains its
  mandatory boundary and broker; the application retains its original restrictions.
- The application filter admits `SYS_open`, equivalent to its existing unrestricted
  `openat`, because pinned x86-64 rustix uses that syscall for the production
  deployment directory opener. Process creation, new sockets, credential changes
  and exec replacement restrictions are unchanged. Fixture build jobs are now four.

## Validation

- Full frontend unit binary: **436 passed, 7 ignored**. Live-kernel regressions
  cover nested-listener rejection, the real rustix opener under the application
  filter and sealed supervisor reexec/custody. Both pre-exec probes always terminate
  before an unserviced exec notification could block them.
- Real pinned static Cargo cache test: **1 passed**, with A/A/B/B binding runs.
  The selected library is rebuilt, warm proc-macro dependencies remain fresh,
  dep-info records the binding and executed consumers observe the current value.
  Its manufactured inert projections test namespace/cache behavior, not authority.
- Strict production Clippy (`--no-deps`, `-D warnings`), scoped rustfmt and
  `git diff --check` pass. Dependency-inclusive Clippy still encounters the existing
  unrelated `profiler_bundle.rs:791` `derivable_impls` warning.
- Standard-library-only `production_build_config` integration binary, compiled
  with the pinned rustc and real CLI: **17 passed, 2 failed**. The new host/device
  boundary guard passes. Existing source-string assertions for absence of the
  word `selector` and presence of literal `false` fail in unchanged source bodies.
  This is not a clean full integration-suite claim. The feature-gated auditor
  fixture probe was updated, but that separate feature suite was not rerun.
- Compiler/backend/service sources were unchanged; their earlier full test
  results are not represented as rerun here. Independent native agents reviewed
  authority boundaries, lifecycle handling, syscall composition and next GPU work.

## Genuine Campaigns

Seven isolated fresh-key installed-layout runs were performed:

1. Rejected a canonical AMD target versus CPU-short-name comparison; corrected it.
2. Rejected the original Cargo build-std unstable-options gate; normalized it exactly.
3. Rejected Cargo's actual `noprelude,nounused` qualifier; matched the pinned source.
4. Reached host Cargo and rejected its static target query; added the bounded query path.
5. Compiled the ordinary host library/binary, then rejected nested seccomp listeners
   with `EBUSY` before application exec.
6. Executed the application and its no-mode/no-FD200/201 checks, then rejected the
   production deployment opener with `EPERM` because `SYS_open` was absent.
7. **Passed** real selected-rustc compilation, issuer/anchor publication, Worker
   finalization, ordinary host compilation, FD195/current-record audit and retained
   remote conditional proof admission. Manager/coordinator continuity checks pass.

Final result: **1 passed, 0 failed**, exit 0; test body 460.22 seconds, whole campaign
470.89 seconds. The log prints both `genuine compiler audit and retained conditional
proof admitted` and the selected-rustc-to-FD195 application success marker. The
later coordinator-quarantine message is emitted during teardown after success;
the owned empty proof scope and outer cgroup are drained/removed.
All seven campaign logs record cleanup. Final cgroup absence was checked.

Final CLI SHA256: `c959ad064c604e62a9b7a8b0c9e37b34a30f55dfe8278e54fe958161ad112e44`.
Backend SHA256: `2bfad4b0dcc64f4fba38a88dbb6123919da730a06d5867009397a7e0edc7102d`.
The archive retains commands, all experimental logs, image hashes, source patch,
new source files and review notes. It excludes compiled tools and downloaded
third-party source; pinned Cargo URLs/hashes remain in the notes.

Archive: [qualification.tar.gz](qualification.tar.gz).
SHA256: `684b5e6438c6dd233591ed846e56d1f7693e50223c7cbeef5715752e70fc5f60`.

## Next Acceptance

Implement the fixture's optional two-GPU path using one retained remote artifact,
the existing generated-only multi-device native-peer backend and the owned
current-thread async engine. Require exact receipts and full 65-element fill
readbacks before bidirectional PUBLIC XGMI, native-copy counter delta two, complete
source/payload/guard checks and explicit successful engine shutdown. The archive
includes the concrete API/ownership work order.

MI300X was inspected read-only through Windows OpenSSH. No GPU workload, remote
file, service or device-permission change was made. Required installed services
are absent; `sudo -n true` and `sudo -n -l` require a password and direct root SSH
is denied. Deployment is still an external prerequisite for that authenticated
hardware campaign. Do not substitute synthetic authority or old native witnesses.

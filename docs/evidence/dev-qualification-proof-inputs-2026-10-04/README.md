# Portable Proof Input Audit

Base: `f2c22da166c667faab2c54e6a76eb6cb629eb38f`.
This checkpoint adds a root-free preflight for the proof-runtime inputs used by
the genuine two-GPU qualification harness. It is not the complete transport
bundle, installed approval, a hardware result or A3/A7 closure.

## Implementation

- Add `audit-qualification-source VERUS_DIST RUST_TOOLCHAIN INPUTS` to the existing
  functional-refinement runtime script. Reuse the existing runtime manifest,
  target pins and qualification package lock; no accepted hashes change.
- Check both whole libc/zlib packages before parsing. Stream the exact DSO and
  loader members into size/hash checks without extracting or executing them.
  Keep the existing checks for host libgcc/libstdc++, Verus, Rust and excluded
  launcher/rustup provenance. Require the inherited root-owned `/lib64` link;
  the later private installation reconstructs `/usr/lib64`.
- Run this preflight before cgroup/private-state creation. Keep the later package,
  source, installed and native resource admission checks. Report the distinct
  `QUALIFICATION_INPUTS_OK`, never installed approval.
- Propagate failed checksum commands even after correct output; publish no success
  marker after a failed manifest hash. Iterate tagged manifest rows directly so
  a failed enumeration subprocess cannot silently skip checks. Installed inventory
  enumeration also propagates its parser failure.

Paths are reopened after hashing and between package-member reads. This is
point-in-time preflight, not immutable retained custody or a formal proof of the
shell installer. Supplied tools/packages are never executed by this audit, but
trusted host decoders and hashing utilities are used.

## Validation

- **22 shell controls pass**: binary member bytes, wrong sizes/hashes, missing and
  truncated members, decoder/tar/hash failures after output, manifest-report and
  inventory-parser failures, wrong whole-package hash before decoder invocation,
  failed whole-package hashing, symlink/hardlink/FIFO/missing input rejection,
  non-execution of a script-shaped inspected file and command arity.
- Real pinned source-package inspection passes. The final code also passes as
  UID1000 with empty supplementary groups, dropped capabilities and the complete
  filesystem read-only in private PID/network namespaces. No writable scratch
  or root installation is required by the audit.
- Ordinary `audit-source` still rejects the unmodified host libc mismatch. The
  portable marker does not imply that the host's interpreter is accepted.
- Full genuine harness with an invalid libc package rejects before cgroup
  creation; before/after owned-scope rosters match.
- Fresh genuine installed-service campaign: **1 passed**, exit 0, **481.00 seconds**
  in the test and **493.57 seconds** including deployment/cleanup. Actual selected
  compiler, issuer/anchor, Worker finalization, host linking, FD195/current-record
  audit and retained conditional proof admission pass. Manager/coordinator
  continuity passes. The later coordinator quarantine message occurs at teardown;
  the empty owned proof scope and outer cgroup are removed. Absence was independently
  checked, and all frozen source/binary hashes match after the run.
- Shell syntax and `git diff --check` pass. ShellCheck passes for the runtime audit
  and new test script, with one annotated indirect-function false positive.
  The unchanged harness sections still report declaration, unused-loop and jq
  quoting warnings; the complete harness is not claimed lint-clean.
- Native agents reviewed checksum/error propagation, authority boundaries and the
  next private input projection. Checksum/enumeration findings were fixed before
  the final genuine campaign. No Rust source changed; no Rust suites, solver or
  MI300X hardware campaign was rerun for this packet.

The production scripts and measured binaries stayed fixed through the genuine
campaign. Only test declaration/lint cleanup followed it, and all 22 controls
were rerun. Static test SHA256:
`fd0f2873595cfd0722af21407596bc5c0d96439faadefab4119eb0d1f621b463`.
CLI SHA256: `c959ad064c604e62a9b7a8b0c9e37b34a30f55dfe8278e54fe958161ad112e44`.
Backend SHA256: `2bfad4b0dcc64f4fba38a88dbb6123919da730a06d5867009397a7e0edc7102d`.

## Next Multi-GPU Work

Prepare the separately manifested source/toolchain/offline-cache projection,
leaving the exact 13-file compiler bundle unchanged. Preserve the CLI's baked
local macro path and tree digest. Hide the host home and make private ancestors
searchable by UID1000 without changing UID1002 host inputs. Preserve the pinned
Rust runtime's mode bits; its identity includes modes, so blanket `0444`/`0555`
normalization would change the accepted pin. Preserve exact registry/git bytes,
including the pinned Pliron checkout, and qualify the static host-link tool closure.

MI300X still needs a usable real-root service launch. Then run the admitted fills
and bidirectional native peer copies on two freshly observed free GPUs, inspect
full payload/source/guards and native shutdown, and qualify the failure controls.
This packet performs no remote action and makes no speedup or HIP/HSA parity claim.

The archive contains commands, environment, logs, source patch and review notes,
not executables, private keys or third-party package/source archives.
Archive: [qualification.tar.gz](qualification.tar.gz).
SHA256: `337634bef5bf6160bd9f5774080284709e34ed42c091c6f28fc37bf0e6ff7772`.

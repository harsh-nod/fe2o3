# Private Application Inputs for Two-GPU Qualification

Base: `ed0fe2b0bc4e018c08012992eddad039ef3e3819`.
This checkpoint qualifies the source/compiler/offline-cache projection used by
the genuine application harness. It does not qualify GPU execution, complete
deployment portability, A3/A7, HIP/HSA parity or a general formal-verification
claim. The next work remains the [multi-GPU critical path](../../runtime-multi-gpu-critical-path.md).

## Implementation

- Add a separate inert transport manifest. Include all tracked source except
  historical `docs/evidence/`, the selected Cargo/rustc and complete Rust library
  tree, complete registry/Git caches, CLI/backend, binding trampoline and jq.
  Do not modify the closed compiler bundle or any accepted authority pin.
- Normalize only copied source/cache/tool modes. Preserve Rust library modes and
  empty directories exactly because the production runtime pin includes them.
  Copy cache/tool hardlinks into independent files; reject output hardlinks,
  symlinks, special files, unknown inventory entries and noncanonical manifests.
- Bound entries, bytes, depth, paths, descriptors and Git inventory subprocesses.
  Charge the aggregate budget before component copies. Check the independent
  manifest hash before JSON parsing, then compare a fresh complete inventory.
- The trusted invoking harness verifies transport, creates a fresh root-owned
  private copy, verifies that copy again and only then executes bundled setup
  scripts. The next namespace makes it read-only and hides transport/driver
  aliases. A read-only bind of a mutable external tree alone is not sufficient.
- Hide original `/home` and `/root`; expose the copied source at the CLI's exact
  compiled-in root with searchable private ancestors. Keep the real runtime pin,
  exact macro identity and existing compiler/proof/application admission checks.
  Privileged Python calls ignore inherited Python import/runtime environment.

The trusted invoking harness, Python/helper and host setup/linker remain installer
premises. The transport hash is not runtime authority or continued custody of
the original source. GCC/binutils, CRTs, static libraries, linker scripts and
their runtime dependencies are explicitly outside this bundle. Proof/service
installation inputs retain their separate existing preflight and installed
checks. The staging byte-capacity preflight does not reserve space or inodes;
copy failures are fail-closed and clean only the originally created destination.

## Validation

- **35 Python tests pass as real root**. Coverage includes byte/mode/inventory
  mutations, malformed metadata and pins, empty entries, aliases/special files,
  changed sources, bounded/failed Git enumeration and reaping, aggregate limits,
  low descriptor limits, substituted cleanup roots, hostile Python environment,
  root staging ownership/alias separation, capacity and copy-corruption rejection.
- **436 frontend unit tests pass, 8 ignored**. The new ignored helper measures
  the copied compiler runtime with the actual production pin implementation;
  it does not execute rustc. No production Rust behavior changes in this packet.
- The full snapshot has **57,538 entries and 3,011,281,212 bytes**. Its independent
  manifest SHA256 is
  `d91f3edefaa61f29909a6e6c068ca8ea3f4a478d450d73d12a21f8ffded41ed6`.
  All transport copies were reassigned to UID1002; original inputs were untouched.
- With original homes hidden, UID1000, empty supplementary groups and dropped
  capabilities, the unchanged Rust runtime pin passes **1/1 in 5.48 seconds**,
  and both existing exact macro tree/path controls pass **1/1** each. Original
  Cargo cache paths are absent and projected source/cache/runtime inputs are
  readable but not writable by the application role.
- Fresh genuine installed-service campaign: **1 passed**, exit 0, **451.21 seconds**
  in the test, **510.33 seconds** including setup/private staging/cleanup, and
  **557.95 seconds** including transport construction and the projection probes.
  Real selected compilation, issuer/anchor publication, Worker finalization,
  offline host build, FD195/current-record audit and retained conditional proof
  admission pass. Manager/coordinator continuity passes. The later coordinator
  quarantine message is teardown; its empty scope and the outer cgroup were
  removed. Owned cgroup absence and private-storage release were independently
  checked after the command exited. This is a correctness run, not a benchmark.
- Existing **22 package-input controls**, two-GPU mount parser controls and
  four-layer synthetic character-node carriage pass. Synthetic nodes do not
  establish GPU admission or execution.
- Shell syntax, new-helper ShellCheck and `git diff --check` pass. The full legacy
  harness is not claimed lint-clean. Strict Clippy and unrelated integration/
  hardware/performance suites were not rerun. Frozen script/CLI/backend hashes
  matched after the genuine campaign.

Four earlier driver attempts stopped before genuine admission: the scratch bind
was hidden by `/dev` recreation, a same-invocation bwrap alias was used as a source,
the outer cgroup mount was read-only, and an intermediate probe mount directory
was not explicitly searchable. Driver fixes precede the successful fifth run;
none changed production admission. All private snapshots were released.

Native agents reviewed bounds/cleanup, staged-copy integrity, the existing
two-GPU path and remote readiness. The primary integrated and tested all edits.

## Invocation

Use the existing complete reviewed qualification environment. Its JSON form
supplies the same source/tool/cache paths and authority pins to the root-free
constructor. The destination must not exist, and its parent must exist.

```sh
python3 -E -s -B scripts/qualification_input_bundle.py prepare-application \
  "$repo" "$reviewed_environment_json" "$bundle"
```

Retain the printed digest independently of the transferred tree. Do not accept
an untrusted sibling checksum as authority. Supply the existing five authority
pins and proof/service inputs unchanged when running the real-root harness:

```sh
FE2O3_GENUINE_INPUT_BUNDLE="$bundle" \
FE2O3_GENUINE_INPUT_SHA256="$independently_retained_sha256" \
FE2O3_PROOF_INSTALL_CAMPAIGN=genuine \
  bash scripts/qualify-proof-resource-inspection.sh
```

The legacy no-bundle path remains supported. Bundle mode rejects the resource-only
campaign. Hardware mode still requires `genuine-two-gpu`, explicit selected UIDs,
usable real-root containment and two freshly suitable devices.

## Hardware Readiness

A read-only native agent reached MI300X through Windows OpenSSH with the corrected
`HostName=sharkmi300x-1` and existing strict host-key verification. WSL OpenSSH
timed out. All eight devices had live KFD users from another user's ComfyUI
processes; 0% sampled utilization did not establish a free pair. Noninteractive
sudo still required a password, root SSH was denied, fixed compiler/proof
services and resources were absent, and cgroup-v2 was not user-writable. No
remote workload, file write, permission change or cleanup was performed.

Next: qualify the host static-link inputs and real-root private deployment, then
run the existing admitted fills, bidirectional peer copies, complete readbacks/
guards, explicit native shutdown and failure controls on a suitable pair.

The archive contains the manifest, source patch, commands, environment and logs,
not toolchain/cache bytes, executables or private keys.
Archive: [qualification.tar.gz](qualification.tar.gz).
SHA256: `a3c131c63b79110083606b4e10c3a80af0842e45141ce801c5d4525f9e1f5db1`.

# Host-Link Qualification for the Two-GPU Path

Base: `9b8f9b0f5f5ae754eeeb5f529c6bd125282653bf`.
This checkpoint adds an optional diagnostic to the existing genuine application
campaign. It does not implement a new runtime authority path, execute GPUs,
complete A3/A7, certify general formal verification or establish HIP/HSA parity.
The [critical path](../../runtime-multi-gpu-critical-path.md) remains real-root
deployment followed by an admitted two-device workload on an unoccupied pair.

## Implementation

The [observer](../../runtime-host-link-observation-v1.md) retains the real GCC and
bundled-LLD route, runs each original link unchanged, then captures an
output-equivalent replay. The ELF must be byte-identical. Root postflight checks
measured CRTs, archives, linker scripts and Rust target libraries, including
dynamic build-script/proc-macro links and the final static application.

The proxy is independently pinned, copied into a fresh private root-owned inode,
and compared against that same pin after copying. Capture storage is a bounded
private tmpfs. Parser/control reads, metadata, entries and aggregate reports are
bounded; inputs are not extracted or executed. Existing authority pins, Cargo
restrictions, the application ELF policy, service capability sets, selected GPU
mounts and the default no-observer command are unchanged.

Dynamic-loader resolution remains an installed-host premise. A new general DSO
packager is not a prerequisite for the two-GPU smoke. The observer reports
recorded input equality, not continued custody of every transient input from
the original invocation or a hermetic build proof.

## Validation

- **15 Python tests pass** as UID1000 in the prepared private namespace. Fourteen
  cover archive/input/constructor controls; the optional live-proxy test checks
  twelve invalid control spellings against the actual binary and exact diagnostic.
  Without that namespace, the live test skips.
- **8 shell integration tests pass**: disabled mode, exact mount/command vectors,
  malformed/partial options, wrong pins, resource-mode and symlink rejection,
  failed campaign exit preservation and failed postflight propagation.
- **3 standalone Rust tests pass**, including bounded timeout and normal exit code
  behavior. The static musl proxy builds with `-Dwarnings`.
- A real selected-rustc static probe passes original/replay byte comparison and
  measured-input auditing. This is a small compiler probe, not genuine admission.
- **35 application-input tests pass as real root**, **22 proof-input controls**
  pass, and the existing selector and four-layer synthetic device-node checks
  pass. Synthetic nodes are not GPU execution.
- Final genuine installed-service campaign: **1 passed**, exit 0, **478.55 seconds**
  in the test, **550.46 seconds** including setup/audit/cleanup, **588.62 seconds**
  including input transport. It includes real selected compilation, issuer/anchor
  publication, Worker finalization, ordinary host compilation, FD195/currentness
  checks and retained conditional proof. All **30 links** pass postflight.
- The final application has **36 measured external inputs**, **226 individually
  recognized generated inputs**, and CRT order `crt1.o`, `crti.o`, `crtbeginT.o`,
  `crtend.o`, `crtn.o`. Its ELF SHA256 is
  `6c22887d7ea776b350fac9f456dc3432e1ba555c883815c3ad451e295e425ccb`.
  The previously completed campaign also passed 30 links and produced that same
  final ELF; this is an observation of these runs, not a general reproducible-build
  guarantee.
- The final input snapshot contains **57,543 entries / 3,011,336,813 bytes**,
  manifest SHA256 `f146027510656cc06753103667fd4aab4afd531841597cc7a72d694ca4942ee1`.
  Installed host-link premise digest:
  `8815f82587796b7f483d331ae4948e07ed632b3c378d0e9149d840a5cd27adea`.
  This observed digest is not independent prior approval.
- Frozen implementation/CLI/backend hashes match after qualification. Additional
  black-box tests were added after snapshot capture without changing those inputs.
  Shell syntax, helper ShellCheck and diff whitespace checks pass. Rust unit/Clippy
  suites for unchanged production crates and hardware/performance suites were not
  rerun; no runtime performance claim follows from these wall times.

An initial driver quoting error stopped before setup. The next attempt exposed
capture-directory chmod after ownership transfer with no CAP_FOWNER; reordering
chmod before chown fixed it without adding capabilities. A successful preliminary
campaign preceded the final copied-proxy pin regression and full rerun. All owned
qualification cgroups were removed; the final cgroup and released storage were
independently checked. The manager's later quarantine message is teardown, not
evidence of native GPU settlement.

Three native agents provided read-only review and hardware/deployment inspection;
the primary owned edits, integration and tests. MI300X still had live users on all
eight GPUs and no noninteractive real-root launch path. Its requested Ubuntu 24.04
package/layout prerequisites are present, but exact remote profile acceptance is
unverified. No remote workload, file mutation or cleanup was performed.

The archive contains commands, manifests, source patch and logs, not compiler
cache bytes, executables, captured library archives or private keys.
Archive: [qualification.tar.gz](qualification.tar.gz).
SHA256: `12ca6bcdcf5b666a8ff09bc615de173589ed2f0c01170b58ea0a893d4334ba01`.

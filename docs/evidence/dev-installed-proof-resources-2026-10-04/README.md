# Installed Proof Resources: 2026-10-04

Source: `07dae1cbbd705b45f20167a37a9c22e3775fec81`, based on
`dac764837296a440edf0fd3c71c37b5af3db8ed8`.

The real production inspector now passes under UID 61002/GID 61003 with the exact
closed proof profile and final canonical resources. The same private layout then
runs the actual installed application controller through ResourcesReady,
activation, genuine conditional analysis/Verus proof, retained probing and
application-EOF quarantine. Homes, setup sources and host-library aliases are
hidden; no loader cache/preload, hwcaps alternatives or ROCm tree is present.

## Results

- Two inspections produce identical 440-byte candidates, SHA-256
  `e42df3f0e0758e43b413ca8ba24bd076403acb51385c11b89819df5ab62d62ef`.
- Missing Worker, compiler profile, Verus runtime or libzstd rejects without
  creating a candidate. Restored original objects support installation/reopening.
- Actual controller cases pass: proof/probe/EOF retention, payload length
  mismatch, duplicate input descriptors and stale-session rejection.
- Final combined test: 56.35 seconds; prior full run: 67.87 seconds.
- Original children are contained/reaped and unrelated siblings survive. Both
  completed campaigns remove their fresh outer cgroups.
- 25 unit tests and 7 documentation tests pass. Eight privileged/helper tests
  are ignored by default; this campaign explicitly executes the new root test
  and existing application-controller/helper tests. Strict Clippy and formatting pass.
- Direct invocation of either internal shell setup phase rejects before writes
  without the required private namespace.

The current qualified Worker statically links LLVM/LLD; its external closure is
seven base DSOs. No Worker rebuild or libLLVM relocation was needed. The actual
conditional analysis branch consumes finalized HSACO in memory, not ROCm bitcode.
The harness also fixes and guards a nested-mount issue: recursive root binds made
the original private `/dev/null` unusable through `nodev`; recreating private
devices at each nested stage restores the intended behavior.

## Boundaries

This is component qualification, not production package activation, independent
administrative approval, authenticated manager registration, genuine compiler
receipt/current-record admission, ordinary sandboxed application admission or GPU
execution. The compiler profile and registration are fixtures. The install digest
is derived inside the test. The host-derived DSO setup is measured by the real
inspector but is not an immutable distribution package or exhaustive packaging
mutation campaign. Trusted root test code sees the cgroup tree; cleanup acts only
on its fresh subtree. No host service/dependency was changed or MI300X job started.

## Reproduce

Use [the qualification script](../../../scripts/qualify-proof-resource-inspection.sh)
with the absolute input paths documented in
[provisioning](../../runtime-proof-deployment-provisioning-v1.md). `evidence.tar.gz`
contains the exact local WSL invocation, source patch, qualified image hashes,
toolchain/host details, tests, final and preliminary logs, and `SHA256SUMS`.
It contains no executable images. Preliminary failures and the stale-helper run
are explicitly distinguished from final controller qualification in the notes.

Source patch SHA-256:
`44632a94430ae31976adead9886fe097070ad9fd359a23410c0764d6f16b8eb7`.

Evidence archive SHA-256:
`ef4fc504bde4cb6f78d0ced79aa1f7194ccf2721fe6067780c5d59b90e04dce4`.

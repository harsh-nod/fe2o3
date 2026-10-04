# Independent Proof Manager Handoff Qualification

Date: 2026-10-04. Scope: authenticated published application registration to the
independently installed manager and fixed staged controller, including
Ready/Activate and failure custody. This is not ordinary application proof or
GPU execution, a new formal proof, production systemd qualification, or HIP/HSA
parity evidence.

## Source And Archive

- Source: `21594d8cebbdeeec72f7237222e859fe8dce108e`.
- Base: `a71d254767fbd7d3050ce25fae4df37d7726161c`.
- Source patch SHA-256:
  `cdf8de4b9ee873b46463b4e7be3b6c8c90e672896ea00b0990eee970f0d72fd4`.
- [Archive](evidence.tar.gz) SHA-256:
  `d48a08506c8270bbdd6443a1e6c770166c41d45a606e6e463c174b62dc205e08`.

The archive contains the source patch, verified source signature, per-file hash
manifest, exact scripts, final CPU/build/Clippy logs, five root-case logs, installed
image measurements and independent namespace checks. Preliminary setup failures
and the superseded review build are retained, not counted as final qualification.
`raw/final-source-check.log` ties the final source, formatting and unit contracts
to the signed commit. Archive extraction and every manifest hash were checked.

## Results

- 353 CPU unit/integration tests and 86 doctests passed across broker authority,
  coordinator, host-link closure, proof custodian and protected static executable.
  The default run left 27 opt-in tests ignored; subprocess helper result lines
  are not counted twice.
- Strict Clippy passed for broker authority, coordinator and proof custodian,
  including all test targets. Both systemd contract scripts passed.
- Final static musl manager and broker test images built successfully. Their
  hashes remained unchanged throughout the root cases.
- Seven opt-in test invocations passed in five isolated campaigns: 35 PASS groups.

| Case | PASS Groups | Boundary |
| --- | ---: | --- |
| Active | 27 | 16 legacy registration controls, 8 pending handoff controls, stopped-manager bootstrap, full Ready/Activate, control-EOF custody |
| Before Activate | 3 | Nonblocking bootstrap, app Ready before activation, control-EOF custody |
| Coordinator Copy | 1 | Byte-identical image on a different inode rejects before Hello |
| Delayed Listener | 3 | Initially absent manager remains pollable, then authenticates and activates; control-EOF custody |
| Post-Connect Removal | 1 | Removing the actual socket immediately after successful connect returns terminal ENOENT, never Pending |

The actual fixed manager launched the previously qualified, exactly approved
application controller. A measured static broker libtest image occupied the
coordinator path; it was not the production coordinator under systemd. The C
application/compiler records were fixtures. No proof Request was sent, no Verus
proof ran, and no GPU was used. Control EOF was tested while the coordinator test
process remained alive; this is not a coordinator process-death campaign.

## Reproduction And Cleanup

`raw/build-final.sh` records the pinned nightly, offline dependencies, test settings
and static linker arguments. `raw/prepare-runtime.sh` and `raw/isolated-runtime.sh`
use the existing pinned analyzer and Verus inputs from the earlier
[application-controller qualification](../dev-application-proof-controller-2026-10-04/README.md).
Large toolchain/image inputs are not duplicated in this archive. Repoint the
scripts' fixed repository/scratch paths to a fresh owned directory before use.
`raw/campaign.sh` defaults to all five cases and accepts explicit case names.

Each campaign used a private PID/mount/network namespace and a newly owned outer
cgroup. Every final namespace check found no surviving task, and every outer
cgroup was removed. Retained proof subgroups were contained by the external
qualification scope; that cleanup is not native settlement. No host service,
configuration or MI300X installation was changed. Owned local scratch was removed
after archiving; unrelated workspace files were preserved.

## Remaining Gates

The standard compiler-only bundle still lacks manager/controller resources,
host-specific approval provisioning and complete service activation. The ordinary
host/supervisor route remains legacy. Next: consuming CustodianReady/proof client,
remote conditional ownership, existing per-device native preparation, complete
deployment, then ordinary two-GPU compute/upload/PUBLIC-XGMI/readback. Manager
release/reuse and settlement-driven reclamation remain absent. See the
[design and deployment boundary](../../runtime-proof-manager-v1.md).

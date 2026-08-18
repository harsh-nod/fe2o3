# R1 MI300X Identity Oracle V1

Status: measurement-only hardware lane for issue #137. This lane supplies the
R1 differential observation required by the runtime architecture. It does not
admit a device, prove a runtime property, or grant any runtime authority.

## Isolation and Order

`scripts/runtime-identity-oracle.sh` performs this fixed sequence:

1. audit the four production Cargo roots with the pure-Rust dependency policy;
2. build and ELF-audit the pure-Rust `kfd-device-identity` example;
3. execute that example with `--all` in a bounded subprocess;
4. execute `/opt/rocm/bin/rocminfo` in a different bounded subprocess with a
   cleared environment; and
5. pass the two completed output files to the standalone Python comparator.

The pure-Rust process exits before `rocminfo` starts. Oracle bytes are never an
argument, environment value, file descriptor, expected digest, proof record, or
other input to the pure-Rust process. The comparator is a repository script, not
a Cargo package, and neither it nor ROCm is reachable from a production Cargo
edge. The production metadata and final ELF audits are repeated in the hardware
lane so this separation is an executable gate rather than a documentation-only
rule.

## Exact Profile

Both inputs are bounded, ASCII, newline-terminated regular files opened with
`O_NOFOLLOW` and revalidated after reading. The parser bounds bytes, lines, line
length, agents, fields, and ISA records. Duplicate or missing identities and
security-relevant fields fail closed.

The V1 comparison requires:

- exactly eight unique GPU UUIDs, with the sorted UUID sets equal;
- the pure-Rust profile digest, boot ID, Linux `6.8.0-124-generic`, amdgpu
  `6.16.13` plus its pinned srcversion, DRM 3.64.0, firmware 192/25, SPX/NPS1,
  `gfx942:xnack-`, and an observed wavefront size of 64;
- ROCm 7.2.4, HSA runtime 1.18, the loaded ROCk/amdgpu module 6.16.13, global
  XNACK `NO`, eight `AMD Instinct MI300X` GPU agents, `gfx942`, wavefront 64,
  `BASE_PROFILE`, `KERNEL_DISPATCH`, chip ID 0x74a1, ASIC revision 1, and
  firmware 192/25;
- exact UUID, KFD node, and PCI-to-BDFID agreement for each sorted GPU pair; and
- the exact ROCm 7.2.4 ISA set containing
  `amdgcn-amd-amdhsa--gfx942:sramecc+:xnack-` and
  `amdgcn-amd-amdhsa--gfx9-4-generic:sramecc+:xnack-` for every GPU.

The parser produces canonical records sorted by lowercase 16-digit GPU UUID.
Any disagreement or parser failure produces no measurement record.

## Evidence Format

The comparator command is:

```text
scripts/runtime_identity_oracle.py \
  --pure-rust-output PATH \
  --rocminfo-output PATH \
  --rocm-release PATH \
  --pure-rust-executable PATH \
  --rocminfo-executable PATH
```

Successful stdout uses schema
`fe2o3-r1-device-identity-oracle-measurement-v1`. Its fixed header includes:

```text
claim_status=Measured
claim_scope=device-identity-differential
authority=none
proof_effect=none
runtime_authority_effect=none
result=match
```

It then records the exact profile and platform identities, raw-output digests,
both executable digests, comparator digest, and eight sorted GPU rows containing
UUID, KFD node/GPU ID, PCI address, render minor, oracle agent/BDFID, target,
wavefront, firmware, exact ISA, and match result.
`Measured` is the only claim status. In particular, a matching record cannot be
used as `Checked`, `Proved`, `ProvedUnderContract`, or aggregate `Verified`
evidence. There is no runtime API that accepts this record.

## CI

Generic CI runs all parser and comparison fixtures through
`scripts/ci-local.sh runtime-policy` without ROCm or a GPU. The manual
`runtime-identity-oracle.yml` workflow runs only after an explicit operator
confirmation on the GPU hardware environment, has read-only repository
permissions, clears any previous result before measuring, and uploads the
non-authoritative measurement only after success. A failed run uploads logs but
not an old or partial evidence file.

The live command is:

```bash
FE2O3_ALLOW_RUNTIME_IDENTITY_ORACLE=1 \
  scripts/ci-local.sh runtime-identity-oracle
```

## Limits

This is a differential measurement of two userspace observation paths, not an
independent hardware attestation. `rocminfo`, ROCr/HSA, the kernel, firmware,
sysfs, and hardware are external and may share defects. The observations are
sequential rather than atomic. XNACK is process-global per subprocess, so the
oracle confirms its own process state and an ISA feature string; it cannot prove
the state retained by the earlier pure-Rust process. UUID agreement does not
prove physical anti-substitution, reset absence, liveness, progress, memory
coherency, queue semantics, kernel execution correctness, or performance. V1 is
deliberately restricted to this eight-GPU MI300X and exact software profile.

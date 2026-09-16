# MI350-2 Engineering Observation Profile

This additive profile supports the explicitly enabled `engineering-gfx950`
feature. It does not change the gfx942 profile or turn gfx950 observations into
protected load, dispatch, model, or service authority. The existing unsafe,
disposable engineering worker remains the only consumer with execution APIs.
Without the feature, the original gfx950 platform admission is unchanged.

The initial platform-only manifest was observation-v1. Observation-v2 also
binds the explicit SPX/XCP0-to-parent-render correlation below. Its digest
changes with the new contract; the original host's manifest is unchanged.
This page retains its initial filename for existing links.

## Exact Platform

The observations below were collected read-only on `mi350-2` on 2026-09-15:

| Field | Exact value |
| --- | --- |
| Kernel | `5.18.2-mi300-build-140423-ubuntu-22.04+` |
| AMDGPU module | `6.16.13` |
| Module source version | `975C4B2AA8AD01E2EA472C0` |
| Driver source package | `/usr/src/amdgpu-6.16.13-2278356.22.04` |
| PCI device / revision | `1002:75a0` / `00` |
| Target / waves / XCC | `gfx950` / `64` / `8` |
| SIMDs / SIMD per CU / arrays | `1024` / `4` / `32` |
| LDS / waves per SIMD / compute queues | `160 KiB` / `8` / `24` |
| Compute / SDMA firmware | `41` / `12` |
| Partition | `SPX/NPS1` |
| `mes` / `sched_policy` / `cwsr_enable` | `0` / `0` / `1` |

The kernel, module version, and module source version are matched as one exact
tuple. A kernel from one profile cannot be combined with the other profile's
module source version. Every existing target, PCI, firmware, DRM, XNACK,
partition, aperture, descriptor, and currentness check still applies. Queue
admission separately retains its exact topology and module-parameter checks.
No setter changes XNACK, partitions, module parameters, clocks, or another
process's resources.

The bound device now retains its selected manifest and exposes its SHA-256 via
`observation_profile_sha256_v1()`. The original free function still identifies
only the original profile. The read-only example reports the bound profile,
not a misleading constant for another host.

## Source Review

`crates/fe2o3-kfd/src/device_gfx950_mi350_2.rs` records the exact source hashes.
They identify the reviewed files on disk; they do **not** authenticate the
running module, firmware, or hardware. Kernel and driver behavior remain
contracted dependencies, not a machine-code proof.

The following critical sources are byte-identical to the sources in the
original gfx950 engineering contract:

| Source | SHA-256 |
| --- | --- |
| `include/uapi/linux/kfd_ioctl.h` | `b3721c1a428a32bb9994af579432af48c44fa65abb860049f11a63a5c093235d` |
| `include/uapi/drm/amdgpu_drm.h` | `9d7ff60a211d2aa73a6c15b2da49e050cebe518fc059ee93e31d61288f7b60dc` |
| `amd/amdkfd/kfd_queue.c` | `fb4b2a5c9e6981222873bcd7aca7e9c1397cba8f1a6b33634d2a48d4427fe062` |
| `amd/amdkfd/kfd_doorbell.c` | `de30437ee1ed9ccbdaf855899482c0bebb7f55adc120ac712c96cadef1a0ec6d` |
| `amd/amdkfd/kfd_mqd_manager_v9.c` | `21166e9dbe2a4c24cbcd6f9ff6193aa093230e91fbafc8b4ac4eee1465cd2c9e` |
| `amd/amdkfd/kfd_process_queue_manager.c` | `8526e258824dbe145e4209cf0fed26463729234ba24369f39e3413e7e6e028db` |
| `amd/amdgpu/amdgpu_amdkfd_gpuvm.c` | `c7cca2ee47a08c99bb73906662d82dd7d0b5738468fbef54848e5e6dd62ba50d` |

The different `kfd_chardev.c` was reviewed for the used GET_VERSION, aperture,
SMI, query-only XNACK, ACQUIRE_VM, CREATE_QUEUE, and mmap selectors. The queue
creation path retains process locking, checked buffer acquisition, target-bound
doorbell encoding, and cleanup on failure. Negative XNACK input remains a query
and does not enter the setter/no-live-queues branch. ACQUIRE_VM accepts an
existing binding only for the same DRM file, otherwise rejects it.

The different `kfd_process.c` retains one process-device VM, its DRM-file
reference, CWSR initialization, and unwinding of failed initialization.
`kfd_events.c` retains bounded signal-page mapping with `VM_DONTCOPY` and
`VM_DONTDUMP`. The byte-identical SMI reset-event source remains a prospective
notification mechanism, not an all-reset/ABA guarantee. Internal mmap selectors
in the different `kfd_priv.h` retain the type shift 62 and GPU-ID shift 46.
Their separate hashes and the device-queue-manager hash are recorded rather
than borrowing another package's provenance.

The unchanged `kfd_queue.c` derives 32 CU/XCC, 1280 saved waves/XCC, control stack
`0x3000`, context/XCC `0x15a3000`, debug total `0x50000`, and total CWSR mapping
`0xad68000`. Existing queue-profile tests independently calculate these sizes.
The unchanged gfx9 MQD path retains AQL `NO_UPDATE_RPTR`; observing completion
does not grant ring-slot reuse. Existing fail-terminal resource-retention rules
are unchanged.

## Layout Checks And Reproduction

Both C programs below only inspect C constants/layouts and print results. They
perform no KFD ioctl and launch no GPU work. Run from the fe2o3 repository on the
host; `$scratch` must be a caller-owned directory.

```sh
driver=/usr/src/amdgpu-6.16.13-2278356.22.04
kernel=/usr/src/linux-headers-5.18.2-mi300-build-140423-ubuntu-22.04+
scratch=$(mktemp -d)
mkdir -p "$scratch/include/drm"
cp "$kernel/include/uapi/drm/drm.h" "$kernel/include/uapi/drm/drm_mode.h" "$scratch/include/drm/"
cc -std=c11 -Wall -Wextra -Werror -D__user= \
  -I"$driver/include/uapi" -I"$scratch/include" \
  crates/fe2o3-drm-uapi/tests/oracles/drm_amdgpu_identity_v1.c -o "$scratch/drm"
"$scratch/drm"
cc -std=c11 -Wall -Wextra -Werror -I"$driver/include/uapi" \
  crates/fe2o3-kfd-uapi/tests/oracles/kfd_event_uapi_1_18.c -o "$scratch/kfd-events"
"$scratch/kfd-events"
```

The DRM header copies preserve the original bytes. `-D__user=` removes only the
kernel pointer-address-space annotation, as exported userspace headers do; it
does not substitute different structure definitions. The observed DRM sizes,
offsets, and requests match `drm_amdgpu_identity_v1.rs`: 64-byte `drm_version`,
32-byte `drm_amdgpu_info`, version request `0xc0406400`, info request `0x40206445`,
and identity prefix offsets `0,4,8,12,16`. Full device-info size is 448 bytes;
the runtime queries its independently checked 20-byte identity prefix only.

The event oracle reports KFD 1.18 and matches all constants, sizes, and offsets
in `run-kfd-event-uapi-oracle.sh`, including 48-byte event data, 40-byte context
save header, 24-byte wait arguments, and 16-byte runtime-enable arguments.
This does not make the original source-closure script applicable unchanged to
the new package; its old package/source pins are deliberately not rewritten.

```sh
cargo test -p fe2o3-kfd --lib device::gfx950
cargo test -p fe2o3-kfd --features engineering-gfx950 --lib device::gfx950
cargo test -p fe2o3-kfd --features engineering-gfx950 --lib engineering_gfx950_profile
cargo run -p fe2o3-kfd --features engineering-gfx950 \
  --example kfd-gfx950-device-identity -- "$KFD_DEVICE_UNIQUE_ID"
```

The tests reject missing/mutated platform fields, mixed original/new tuples,
cross-target and mutated device identities, and mutated queue geometry/module
parameters. A no-feature test rejects the new tuple. The read-only example
must retain the actual new profile hash, pass repeated currentness checks,
return to its original descriptor count, and report all execution authorities
false. This profile alone is not GPU execution or numerical/performance evidence.

The host is shared. A foreign process retained GPU memory during this audit.
Do not reset the GPU, kill other jobs, or infer exclusive access from sampled
zero utilization. Device dispatch and performance qualification require a
separately coordinated reservation and separate evidence.

## Initial Rejection And Follow-Up

Initial platform-only validation (before adding XCP0 correlation), collected
on 2026-09-15 using `nightly-2026-04-03` on this host:

| Check | Result |
| --- | --- |
| gfx950 tests, feature disabled | 5 passed |
| gfx950 tests, engineering feature enabled | 12 passed |
| Full KFD library, engineering feature enabled | 487 passed, 1 ignored |
| Exact-driver C event/layout oracles | Match the frozen Rust UAPI definitions |
| Actual read-only device binding | Rejected before platform admission |
| GPU dispatch / numerical execution | Not attempted |

Set `KFD_DEVICE_UNIQUE_ID` from local topology without publishing its value.
The probe reports the following rejection; hardware identifiers are redacted
from public evidence under the repository contribution policy. Original logs
remain in the caller-owned directory on the test host:

```text
Topology(RenderCorrelationMismatch {
    node_id: 1,
    field: "unique_id",
    kfd: "<redacted XCD/XCP identifier>",
    render: "<redacted board identifier>"
})
```

This is not a decimal/hex parsing discrepancy: KFD reports an XCD/XCP
identifier, whereas the DRM parent `device/unique_id` reports a different
board identifier. Both observations point to the same PCI function and
render minor; SPX still has virtual XCP render siblings.

The exact driver explains the different identifier domains:

- `amd/amdkfd/kfd_topology.c:548` selects `gpu->xcp->unique_id` when an XCP
  exists, otherwise the parent GPU's unique ID. Source SHA-256:
  `47f2ed7121b64af169cbaf1edbdd408be46c2e07efaae5ffa6c009b469ea9737`.
- `amd/amdgpu/amdgpu_xcp.c:123` initializes that XCP identity using the
  `AMDGPU_UID_TYPE_XCD` identity of its first enabled GFX instance. Source
  SHA-256: `4043b92fddc8caf29d5db98768dbffd53495fcd18bc15b0f8fdafa9583a21307`.

Independent source review established an existing-ABI relationship rather than
an equality between the two UID bit patterns:

- `kfd_topology.c:2162-2172` derives KFD's PCI address and XCP render minor from
  the same parent AMDGPU device.
- `amdgpu_xcp.c:293` makes XCP0 share the parent DRM device; its open path binds
  the opened minor to a valid partition.
- `amdgpu_gfx.c` provides the current partition query; `aqua_vanjaram.c`
  assigns all XCCs to the single XCP in SPX. Their reviewed hashes are in the
  new manifest, alongside the topology/XCP source hashes above.

Observation-v2 implements `Gfx950EngineeringXcp0ViaParentRenderV1` as a distinct
correlation kind, selected by the exact platform and feature, never by a failed
UID comparison. It requires gfx950/full-device geometry/firmware, SPX/NPS1,
canonical parent PCI render ancestry, exact BDF and render number, one KFD node
per PCI endpoint, nonzero identifiers, and non-symlink XCP evidence. Partition
child/virtual render devices reject. The XCP directory and metrics-file
identities are retained; changing telemetry contents are not read.

Both UID domains and the correlation kind remain in the retained snapshot and
are independently compared on currentness checks. Existing FD, DRM, aperture,
boot, process, reset-event, and generation checks remain in force. The default
gfx942 and original gfx950 routes continue to require same-domain UID equality.
This is contracted driver/sysfs correspondence, not hardware authentication,
proof of UID equivalence, or an all-reset/ABA guarantee.

The follow-up read-only probe **passes** with observation-v2, including two
currentness checks and descriptor cleanup (four descriptors before and after).
It reports checked-observation-only authority: explicit VM acquisition,
allocation, queue, dispatch, and XNACK setter authority remain false. This is
not a production execution receipt or numerical evidence. No GPU work was
launched in either probe.

| Observation-v2 follow-up check | Result |
| --- | --- |
| Focused XCP0 regressions | 11 passed |
| Default KFD library | 422 passed, 1 ignored |
| Engineering KFD library | 498 passed, 1 ignored |
| Strict Clippy, both feature configurations | Passed |
| Reviewed installed-source hashes | All 25 matched |
| Read-only bind/currentness/cleanup | Passed |

The successful example executable SHA-256 was
`5d6deead4e675d1a9470d86d0098b990a07227e89e021a9268543f0af66b7504`.
Original probe output stays private on the test host; public evidence redacts
GPU/unique IDs and render minor, and is explicitly labeled as redacted.

The initial rejected example executable SHA-256 was
`78a1ea77d9ab7f740da36d1359e4a78d8d3895410d362a1bd03ad6a9ea63f1cc`.
The initial rejection occurred during topology discovery. Production source
atomic lowering, protected allocation/queue admission, source proofs, and
hardware/runtime execution qualification remain separate open obligations.

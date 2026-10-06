# Gfx950 Read-Only Queue Geometry

This slice implements independently branded planning, not native gfx950
admission. `gfx950_queue_resources::plan_gfx950_aql_queue_resources_v1` consumes
an exact read-only host topology snapshot. It produces no device token, memory
allocation, mmap, ioctl, header write, queue or dispatch authority. Existing
gfx942 types, resource constants and manifest bytes remain unchanged.

The exact source and observation premises are retained in the
[planning manifest](../crates/fe2o3-kfd/src/gfx950_queue_resources/profile.manifest).
Kernel files were copied read-only from
`mi350:/usr/src/amdgpu-6.16.13-2303411.24.04`. ROCr files are from
[ROCm 7.2.1 commit 820d835](https://github.com/ROCm/ROCR-Runtime/tree/820d83572bd8a098ba8366b84943c760ecce8ca5).
Source hashes identify reviewed source bytes, not a source-to-loaded-binary
proof. No native device admission digest is asserted or borrowed from gfx942.

## Derivation

The deployed `kfd_queue.c` explicitly selects 512 KiB VGPR state and the observed
LDS capacity for target `90500`. SGPR and hardware-register state contribute
16 KiB and 4 KiB per CU. Its queue-buffer validation requires an exact control
stack size, a sufficient per-XCC context size, and a page-aligned total mapping.

For the retained SPX/NPS1 observations:

| Quantity | Derivation | Bytes or Count |
| --- | --- | ---: |
| CU per XCC | `1024 / 4 / 8` | 32 |
| Waves per XCC | `min(32 * 40, 32 / 1 * 512)` | 1280 |
| Control per XCC | `align_4096(40 + 1280 * 8 + 8)` | 12288 |
| Workgroup per XCC | `align_4096(32 * (524288 + 16384 + 163840 + 4096))` | 22675456 |
| Context per XCC | control plus workgroup | 22687744 |
| Debug per XCC | `align_64(1280 * 32)` | 40960 |
| Total mapping | `align_4096((22687744 + 40960) * 8)` | 181829632 |

Each XCC header starts at `index * context`. Its debug offset is relative to
that header, `(8 - index) * context`; its debug size is the complete debug
region, `debug * 8`. Debug storage follows all eight context blocks. The
control-stack page count derives from `control / 4096 * 8`, yielding 24
nonoverlapping shadow-page offsets. The planner checks arithmetic and wire
width bounds before retaining geometry; it does not allocate those pages.

ROCr computes fallback context using its original derived control size, then
independently selects positive kernel `CwsrSize` and `CtlStackSize` values. Zero
means fallback. The deployed kernel does not export either property through
topology sysfs. The public planner therefore admits only the observed fallback
profile, and closed topology parsing rejects either unreviewed property.
Private derivation tests and the C oracle cover precedence and reject
unaligned, undersized or overflowing combinations. These test inputs do not
extend the public observation profile.

## Qualification

The independently compiled
[C oracle](../crates/fe2o3-kfd-uapi/tests/oracles/kfd_gfx950_queue_resources_1_18.c)
uses the retained KFD/ROCr headers and checks their actual structure layout.
Its source-hash-checking runner accepts a flat directory whose filenames match
the manifest's `source.*` entries:

```sh
sh crates/fe2o3-kfd-uapi/tests/oracles/run-kfd-gfx950-queue-resources-oracle.sh SOURCE_BUNDLE
cargo test -p fe2o3-kfd --lib queue_resources:: --locked
cargo test -p fe2o3-kfd --doc --locked
```

Rust tests compare all eight test-only header byte images and all 24 shadow
offsets with the C oracle record. No production header writer is introduced.
Negative cases cover host/source/driver parameters, target, partition, each
capacity premise, ring boundaries, divisibility, arithmetic overflow and size
precedence. A compile-fail example prevents substituting the new plan for the
gfx942 plan. An explicitly ignored live test observes and plans all eight MI350
devices without opening KFD/DRM handles or performing native mutations.

This is not native queue qualification, machine semantic refinement, a model
proof of this Rust implementation, multi-GPU application support or performance
evidence. Follow the [native admission work order](runtime-gfx950-native-admission-work-order.md)
before using these dimensions in a native ownership path.

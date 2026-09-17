# Exact Asrock Engineering Profile

The `engineering-gfx950` feature adds one separate checked-observation profile:
kernel `5.15.160+`, amdgpu module `6.16.15`, and module srcversion
`9462451703604FCD7EC2365`. All three strings must match together. Default builds
reject this tuple; neither earlier profile nor its retained manifest changes.

The source manifest in `src/device_gfx950_asrock.rs` records the reviewed
`amdgpu-6.16.15-2267428.22.04` inputs and kernel headers. The review compares
the used contracts with the frozen Rust adapters, not every driver operation.
C layout oracles cover the used KFD/DRM requests, offsets, and widths. The
driver's full DRM device-info struct is 448 bytes, but the adapter requests
only the unchanged 20-byte identity prefix; the driver bounds the copy by
the requested size. Existing wire schema identities therefore remain intact.

The asrock audit retained its CPU oracle sources, binaries, outputs and hashes
under `evidence/uapi-layout-v1` in the task workspace. `source-inputs.sha256`
has SHA-256 `39eb9f9fc25ae296a9daca14ffb743957e92c63408671c4eae3ba70cc7532f03`;
`oracles.sha256` binds the local oracle inputs and outputs. These are audit
records, not runtime admission tokens.

## Identity And Runtime Gates

Under SPX this driver publishes the parent device UID in KFD topology. The
profile retains the ordinary `SameDeviceUid` correlation and requires equality
with the PCI board UID. It does not reuse the earlier MI350-2 XCP UID exception.
Missing XCP metadata cannot trigger a fallback; a UID mismatch still rejects.

Unchanged gates include gfx950, PCI `1002:75a0` revision 0, firmware compute 41
and SDMA 12, wave64, 1024 SIMDs, 8 XCCs, SPX/NPS1, KFD 1.18, DRM 3.64.0,
disabled XNACK, retained descriptor identity, complete bounded apertures,
topology/currentness rechecks, prospective reset events, and poison on error.
The engineering queue additionally requires the existing exact capacity
roster, MES 0, scheduler policy 0, CWSR enabled, and 4096-byte host pages.
Queue geometry, allocation flags, completion handling, and teardown behavior
are unchanged. PUBLIC VRAM is not relabeled as coherent memory.

## Scope

Source hashes identify reviewed inputs, not a cryptographic attestation of
the running module or firmware. The trusted kernel/driver assumption remains.
Checked observations grant no model-device, memory, queue, dispatch, gfx942,
or protected-runtime authority. The existing explicit engineering worker is
the only execution route enabled by its separate feature and checks.

Tests cover the exact tuple, missing/changed/mixed fields, default-feature
rejection, distinct retained profile hashes, strict UID equality, and failure
to borrow the old XCP exception. CPU tests and a worker build do not establish
GPU execution, numerical correctness, ordinary-memory publication, or progress.

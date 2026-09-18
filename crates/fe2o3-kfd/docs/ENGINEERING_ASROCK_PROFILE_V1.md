# Exact Asrock Engineering Profile

The `engineering-gfx950` feature admits the exact kernel `5.15.160+`, amdgpu
module `6.16.15`, and source version `9462451703604FCD7EC2365` tuple through
a separate observation manifest. Default builds reject it. The original
gfx950 profile and all gfx942 admission checks remain unchanged.

This profile reuses the strict equality check between KFD and PCI board UIDs.
It does not add an XCP exception, even if extra XCP metadata is present.
Missing, modified or mixed platform fields reject. A bound device retains
the selected manifest; `observation_profile_sha256_v1()` reports that profile
rather than the original static profile's hash. Full currentness checks
compare the complete retained topology, including kernel/module observations.

The manifest records the mixed header contract from DKMS package
`amdgpu-6.16.15-2267428.22.04` and kernel `5.15.160+`: DKMS KFD/amdgpu UAPI
headers and kernel core DRM headers. Used requests, widths and offsets must
match the frozen Rust adapters. DRM device-info is 448 bytes in this driver;
the adapter requests only its unchanged 20-byte identity prefix, with the
driver bounding the copy by the requested length. Source hashes describe
reviewed inputs, not attestation of the loaded module or firmware.

Unchanged checks include gfx950, PCI `1002:75a0` revision 0, compute/SDMA
firmware 41/12, wave64, 1024 SIMDs, 8 XCCs, SPX/NPS1, KFD 1.18, DRM 3.64.0,
XNACK disabled, descriptor identity, complete bounded process apertures,
reset-event/currentness checks and permanent poisoning on error. Engineering
queue geometry, allocation flags, completion and teardown are unchanged.
PUBLIC VRAM is not relabeled as coherent memory.

Checked observations grant no protected-runtime, model-device, allocation,
queue, dispatch or gfx942 authority. Engineering execution still requires
its separate explicit opt-in and full lifecycle checks. Source review and
CPU tests alone establish neither native execution nor numerical correctness.

# Reviewed Cargo Vendor Manifest

`fe2o3-device-cargo-vendor-v1.toml` contains the exact unmodified Cargo-produced
manifest for fe2o3-device from revision
`c4c5cdd0f69f3844386440a5addb4d4c3dce0e4b`. The device package is unchanged at
the refresh patch base `ca72d44be3248b8c4dfee01e9329ff9694792a25`.

The producer used `nightly-2026-04-03`; its Cargo executable SHA-256 is
`c9ad606cb1dbb4a65aa27c80be88ed61eb2b811b6450eeec6794f60ed78b94a3`.
The retained vendor producer and final package snapshots match. The 1,922-byte
manifest SHA-256 is
`a5505445b6b63f1e46b7fca58aa25450de19f3cec1c8d44ce444e64d441a22b4`.
The canonical manifest SHA-256 is
`da7bbcd2f3dac75b968f45137c920d94658b5600ff52bf9d843d494bcdd8afb7`.

All 27 `src/**` files match the canonical package. Neither representation has
`build.rs`. The vendor package has no `Cargo.toml.orig`; its generated header
does not establish that such a file exists. Tests copy the actual canonical
sources and substitute only this manifest, then check the full package digest
against the separately pinned vendor closure. No runtime manifest normalization,
original-manifest substitution, or semantic-equivalence exception is used.
The reviewed vendor closure over the manifest and 27 source files is
`343fa53b50a154db8ff930ed2b875d9cf83306e8eb9bfcd493b6fb474990fb34`.

This refresh adds the Cargo-generated `ordered_program_api` and
`ordered_region_api` test stanzas. Cargo discovers these targets even though
the canonical manifest has not changed. A separate regression queries Cargo's
actual SDK target metadata and checks that every test target appears exactly
once in the fixture. Missing and duplicate target stanzas are rejected. The
test target roster check supplements, rather than replaces, exact manifest
bytes and full source-closure admission.

Canonical and vendored materializations retain different observed closure and
semantic-transcript identities. Review and update the exact fixture and full
closure together when the device package or pinned Cargo output changes.

## 2026-09-22 bounded-repeat source refresh

The producer/revision facts above describe the historical fixture. This source
snapshot adds only the reviewed bounded const-repeat helper and its macro hook
to the device source closure: 28 regular src files, unchanged canonical
Cargo.toml, and no build.rs. Repetition expands into the existing flat marker;
terminal, role, effect, ABI and authentication policies are unchanged.

The canonical closure is
`8931e46d7cd42cec7469af30be82da7cd031d919d8a3dfd617b484ff1fff8d11`.
The Cargo-vendor closure is
`8ad658d614a1aca9bc6ee7b3830008a33e692c5664e688bbace2ea1d6bbfec3d`.
Both use the unchanged raw-byte, relative-path-sorted length-framed algorithm;
the accepted set remains exactly these two complete materializations. Old
closures are not appended as compatibility exceptions. New source/semantic
identities are expected, not carried over from prior captures.

The mirror adopted the already-reviewed canonical fork's exact 1922-byte
Cargo-produced fixture and actual Cargo metadata target-roster regression.
The old 1770-byte manifest omitted the two existing ordered-program/region
test stanzas. The adopted artifact retains the historical producer provenance
from revision c4c5cdd0f69f3844386440a5addb4d4c3dce0e4b, pinned nightly
2026-04-03; this refresh does not claim a fresh Cargo producer run.

The retained vendor manifest is 1922 bytes, SHA-256
`a5505445b6b63f1e46b7fca58aa25450de19f3cec1c8d44ce444e64d441a22b4`.
Repeat tests are nested under the existing ordered_program_api target, so
no target or generated-manifest stanza is added by the repeat implementation.
Actual Cargo target discovery and full canonical/vendor materialization tests
remain mandatory; their dated execution evidence is separate from these pins.

### Final publication policy comments — 2026-09-22

Two narrowly justified hygiene-policy comments now precede the unchanged
checked-arithmetic overflow rejections in the const repeat expander. Because
the closure uses raw source bytes, these comments require fresh reviewed pins:
canonical `9bb9a616c53766b493970e39f4e294785337c3a038645848f313e5a3038229f5`,
Cargo-vendor `539ddb6cf0632c936ea168f7556795b61dac309806177b6c5d502e661b5ba20e`.
The earlier Phase22 pins above describe the retained pre-comment captures;
they are not additional accepted entries. The source roster remains 28,
terminal/ABI semantics are unchanged, and the historical vendor manifest and
producer provenance remain unchanged. Fresh matching compiler/source gates
are required for these final bytes; an old DSO is not a current provider.

## 2026-09-23 concrete conditional source refresh

This snapshot adds one reviewed const-selection helper and the new const_if
macro arm: 29 regular source leaves, 411206 raw source bytes, the unchanged
398-byte canonical manifest, and no build.rs. Both complete flat alternatives
are validated before selection. Five direct const-generic projections avoid
introducing caller-scope const item names; the existing single terminal134
marker, five constants and eight runtime arguments are unchanged.

The exact current canonical closure is
`ffbfcf5fceccdad6f01b4b761502c94845829a26e8ead7496c992122d2ef1dbc`.
The current Cargo-vendor closure is
`b543e896235b425e328c53c158c207882b20e5512fcde455066c0573e8d13991`.
Only these two complete materializations are accepted; earlier entries above
are historical observations, not compatibility fallbacks. Raw bytes, sorted
relative paths and length framing remain unchanged. No authentication rule,
terminal admission, ABI, lowering or production policy is broadened.

The 1922-byte vendor manifest and its producer provenance remain unchanged.
Selection tests are nested under ordered_program_api, so no new Cargo test
stanza is expected. Actual target discovery and canonical/vendor positive and
mutation tests must still pass against this source. Fresh matching compiler
DSOs and normal-source captures are required; older repeat/native captures do
not qualify this new kernel or refresh. Qualification results are recorded
separately from these reviewed source identities.

## 2026-09-23 inert complete-body const packing refresh

The reviewed source tree now has 31 regular source leaves and 423135 raw source
bytes. Two no_std const-data modules and their public module registration add
bounded complete-body numeric packing and transactional building. They do not
call or register a marker, create executable MIR/KIR, change an ABI or admit a
source kernel. The canonical 398-byte manifest is unchanged; no build.rs exists.

Current canonical closure:
`570816af9f221a87dfe87d7be9257da3812835de665707db968e1dd36b56e952`.
Current Cargo-vendor closure:
`c90b0fbce11611eaba245ad8d5456f55628600406972f58db6882c8a6f6e6f92`.
Only these two complete materializations are accepted. Earlier hashes above
remain historical, not fallback exceptions. Identity derivation, old terminal
selection and CombinedV4/V5 domain bytes are unchanged.

The separate normal-dependency parity fixture is a nested workspace, not a new
device test target or device dependency. The exact 1922-byte Cargo-produced
vendor manifest and its original producer provenance are unchanged. Actual
Cargo target-roster checks, canonical/vendor materialization controls and fresh
matching backend/source qualifications are required for these final bytes;
test results belong in the dated qualification ledger, not in this pin update.

## Exact MIR36 complete-body terminal source refresh

This source adds the hidden, no-inline marker for the exact ten-constant and
ten-runtime-argument complete-body source ABI. The existing terminal inventory
is preserved; only actual authenticated calls select terminal144, CombinedV6
and MIR36/intrinsic92. Older profiles do not auto-admit this marker.

The final nightly-2026-04-03 rustfmt (edition 2024, skip_children=true) marker
bytes produce canonical closure `970f321b61bf4320f4af4bb00089088baa92e7104c34b9ebbb71067dfe3369bd` and Cargo-vendor closure
`52d648ec3d06ab04453b7146dcda03a575d399b620f8fc92376e7cf76f5c8897`. Only these two whole materializations are accepted; previous
entries above are historical and are not appended as fallbacks. The exact
1922-byte Cargo-produced vendor manifest and its historical producer provenance
are unchanged, as are the raw-byte sorted-path length-framed identity rules.

Fresh compiler/source and provider mutation gates are required. This pin entry
does not claim successful compilation, source qualification or GPU execution.

## Exact MIR37 physical-entry terminal source refresh

The reviewed source tree now has 32 regular source leaves. The only device
source delta from the MIR36 image is the registered physical_entry_v1 module
and its three no-inline typed markers: actual authenticated calls select
terminals145..147, CombinedV7 and MIR37/intrinsics93..95. The unchanged
diagnostics.rs remains 7490 bytes, SHA-256
`803b2178d789c18875b8d3a816af355abfd6a3f8f29cbe7b3262c1ba6e75111d`.
The new physical_entry_v1.rs is 7685 bytes, SHA-256
`5b74180b97caad3e0f0009dd2a803bc6bb5277e12b01dba9e4c839ac36b7af9d`.

The exact current canonical closure is
`0aa7faec0cf4fc2adb16d97ea5b40a1128bd0242865780e0e3fbb948248b2d91`.
The current Cargo-vendor closure is
`33bdfc4e101107b75385c03a4d5857106f605f0671ca0177c6e3843e9c88b998`.
Only these two complete materializations are accepted; earlier pins above
are historical, not compatibility exceptions. Source-file and package bytes,
sorted relative paths, length framing, source-definition identity and exact
provider ABI checks remain mandatory. No path whitelist or digest bypass is
added. The 1922-byte vendor manifest and its historical producer are unchanged;
this refresh does not claim a new Cargo vendor producer run.

Materialization controls cover changing, removing and renaming the new leaf
in canonical and vendor images, in addition to the existing manifest and source
controls. Fresh matching compiler/source gates are still required. These pins
are reviewed inputs, not evidence of successful compilation or execution.

## Exact MIR38 global-copy terminal source refresh

The new reviewed image contains 33 regular source leaves. The only device
changes from the MIR37 image above are the registered physical_global_copy_v1
module and its three no-inline typed markers. Actual authenticated calls select
terminals 148..150, CombinedV8 and MIR38 intrinsics 96..98; earlier identities
remain unchanged. The source root has precisely an immutable shared u32 slice
and an exclusive DisjointSlice<u32>. Input/output pointer and length components
are not four logical arguments and do not create a ghost kernarg allocation.

The new physical_global_copy_v1.rs is 7031 bytes, SHA-256
`5c4ded93cfecb22512fd74b619a8c237063af21ec5a44c4c727b69be3c1742d0`.
The exact canonical source closure is
`fb5338c85bc5110fade2dce0cadf905b2a94f23e15c7033ca951254e4d06a8ba`; the Cargo-vendor closure is
`13747fdbf81c6b7de2813c0cd86866c994683e1c031de3d966cdafe788559577`.
Only these two complete current images are accepted. The MIR37 image hashes
above are now historical, not fallback exceptions. The unchanged diagnostics.rs
remains 7490 bytes with the same SHA-256 pin above. The 1922-byte vendor manifest
and its historical producer are unchanged; no new Cargo vendor run is claimed.

Materialization controls additionally change, remove and rename the global-copy
leaf in both canonical and vendor images. Complete source bytes, sorted paths,
length framing, actual source definition, FnABI and occurrence checks remain
mandatory. These are reviewed input pins, not evidence of source qualification,
native compilation, GPU execution or a completed memory-validation milestone.

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

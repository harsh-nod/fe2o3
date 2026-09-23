# Reviewed Cargo Vendor Manifest

`fe2o3-device-cargo-vendor-v1.toml` contains the exact unmodified Cargo-produced
manifest for fe2o3-device from revision
`272af56d6032f25019156e27a0ba50d16f85830c`. The device package is unchanged at
the initial patch base `263267463d33e0e722827c670091a681ab8491df`.

The producer used `nightly-2026-04-03`; its Cargo executable SHA-256 is
`c9ad606cb1dbb4a65aa27c80be88ed61eb2b811b6450eeec6794f60ed78b94a3`.
The retained vendor producer and final package snapshots match. The 1,770-byte
manifest SHA-256 is
`8ffc8a52272ff0866b3f68d78d0365f50af2da1be2e305891175539d5cde65b3`.
The canonical manifest SHA-256 is
`da7bbcd2f3dac75b968f45137c920d94658b5600ff52bf9d843d494bcdd8afb7`.

All 26 `src/**` files match the canonical package. Neither representation has
`build.rs`. The vendor package has no `Cargo.toml.orig`; its generated header
does not establish that such a file exists. Tests copy the actual canonical
sources and substitute only this manifest, then check the full package digest
against the separately pinned vendor closure. No runtime manifest normalization,
original-manifest substitution, or semantic-equivalence exception is used.

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

This fork now adopts the already-reviewed canonical fork's exact 1922-byte
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

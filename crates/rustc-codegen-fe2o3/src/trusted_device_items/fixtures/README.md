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

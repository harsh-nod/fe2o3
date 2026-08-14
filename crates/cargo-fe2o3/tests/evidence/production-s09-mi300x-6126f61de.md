# Production S09 observation: mi300x, 2026-08-13

This is a reproducible test observation for implementation commit
`6126f61de4c1eb9e14525ccc736c4944f282a07f` (tree
`119b2c2303995433d2ddd61d06404cd8c836a0a6`). It is not an authenticator,
authority decision, protected attestation, or execution receipt.

## Environment

- Host: `sharkmi300x-1`, reached through the `mi300x` SSH alias
- Target admitted from the exact inspected HSACO metadata: `gfx942:xnack-`
- LLVM build ID: `7.2.4`
- Worker build ID:
  `fe2o3-worker-v1-sha256-234d22f9fb347c86495e7156e53ef8eab55e939d6514973a6df373aee12f77a9`
- Exact source materialization: `git archive
  6126f61de4c1eb9e14525ccc736c4944f282a07f | tar -x`
- `production_s09.rs` SHA-256:
  `e1ab068918803f52d3577020a1e7237e22a3a85e01434d90cef1c24376e4136f`
- Policy-approved retained leaf:
  `/tmp/cargo-fe2o3-s09-retain-6126f61de4c1eb9e14525ccc736c4944`
- Retain sentinel SHA-256:
  `93bb741cd4306de025c24d4559fadc99117630db9c190c3f86106cf0916ad6b4`

The retained leaf was pre-created empty. The test created its sentinel and did
not remove or replace the caller-selected directory.

## Commands and result

The exact archived backend was built with the pinned nightly Cargo/rustc and:

```text
cargo build --locked --offline -p rustc-codegen-fe2o3
```

That build passed in 24.50 seconds. The ignored production test was then run
from the same exact archive with the explicit pinned Cargo, rustc, rustc
library path, backend, Cargo home, Worker, Worker build ID, LLVM build ID, and
retained-leaf variables documented in `crates/cargo-fe2o3/README.md`:

```text
cargo test --locked --offline -p cargo-fe2o3 --test production_s09 \
  production_s09_compile_captures_and_publishes_worker_output \
  -- --ignored --exact --nocapture
```

Result: 1 passed, 0 failed, completed in 132.45 seconds. The test traversed
`binding_wrapper::run`, brokered pinned Cargo/backend/artifact capabilities,
closed-environment materialization, inert descriptor capture, pinned rustc
spawn, Worker selection, and durable publication. It decoded the durable
envelope and nested canonical publication record, validated their checksum,
scope-derived filename, published state, and complete identity chain, and
bound the record's finalized-output identity to the exact content-addressed
HSACO bytes inspected for COV6, `gfx942:xnack-`, and exactly kernel `alpha`.

## Exact observation

```text
FE2O3_S09_PRODUCTION_OBSERVATION_V1
capture_sha256=1b405a34fba9d451427cb92d586745017b9d10819ecf751b6905553f8b076fef
rustc_sha256=08dfef109ad22d90556dbd2f964543cd93843dcd75a2e9792c173667392a1950
backend_sha256=2791637815fb9271cedfd4565b14c7a2f1363294e1f6d47a0dff3ea833d59d51
cargo_sha256=c9ad606cb1dbb4a65aa27c80be88ed61eb2b811b6450eeec6794f60ed78b94a3
worker_sha256=764c7309af90b7c11b9a8ca14a84d449ab9f0a7f5eaf39b82b2d316ad4f3235a
hsaco_sha256=5902632c5c249be05855ae5cef62bb9096a1f9277cfb0c58b4384594d6ee61de
publication_record_sha256=df498afa857a248f8f95f500fe0ffe0cf26e4be7ed0967d57cb98e9a2d91fae1
publication_kernel_set_identity=55907e9008e132c1b5a50a4812e67bfd2df932ba04af22d71aad170b52eba078
publication_target_identity=5367e4d254830f0daa09d008c37465590b186ee4ffc9b7b344bafd23b6577cb5
publication_request_identity=7f9c62ea4120633f1f0f69d77274f1ab782f171bf52cba5f1e4fa22a37dae0b5
publication_worker_identity=9488930e856e41d03a86d1ea0f2c9acb0a111e9fe4b410da98e91599aef815ef
publication_identity=bf0e36616644d1affa8966f91ccd3eecef7e7bf1eb3fe8eba65f9d25e3739a05
target=gfx942:xnack-
```

Independent `sha256sum` over the retained files reproduced the HSACO and
publication-record digests above. The record filename was
`.fe2o3-link-publication-v1-73346cb23a45375bf936fed9e573e95c8143a568f0ebcfa154db4ada9067bc92.record`.

## Scope and limitations

The captured V2 descriptor is inert, retained only in memory across spawn, and
does not persist plaintext environment values. The publication record and
these observations grant no loading, launch, compiler-origin, or verification
authority. Canonical cwd pathname binding does not claim a pathname-to-object
identity join; process consistency binds the separately pinned object. The
scalar profile remains restricted to its reviewed exact unit/source/cwd shape
and does not establish a general source or output-object association.

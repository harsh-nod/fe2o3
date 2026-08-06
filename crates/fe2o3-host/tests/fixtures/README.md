# Published HSACO inspection fixtures

`gfx1151-typed-vecadd-v1.fe2o3` is a canonical artifact container locally
recorded from `examples/vecadd` in a checkout at source commit
`30f6d75cf1c10c5dc18e2f9de6eb33015f6aab80` with the repository's real
`cargo-fe2o3`/`rustc-codegen-fe2o3` pipeline:

```text
FE2O3_TARGET=gfx1151 cargo run --locked -p cargo-fe2o3 -- build -p fe2o3-vecadd
```

The container was extracted byte-for-byte from the generated host executable's
embedded `FE2O3AC\0` record. Its sole payload is the generated
`target/fe2o3/vecadd.hsaco`.

The source commit is a developer-recorded annotation and is not bound by
container V1. It does not authenticate the compiler or establish provenance.

- Container size: 6045 bytes
- Container SHA-256: `c74ee3f593b0bc302f67312e415a867f541fe3e3e79973ee596bc7bbf98a22d1`
- HSACO size: 5328 bytes
- HSACO SHA-256: `053551d6a21604bec295acf1aedb4e3b2dedefa7f904a5fa160660b889b480fa`
- Target: `gfx1151`
- Metadata export/descriptor: `vecadd` / `vecadd.kd`

This fixture establishes compiler-produced container and HSACO parsing only.
It records no filesystem freshness, compiler authenticity, hardware execution,
module-load, launch, or device-safety claim.

The ignored `gfx942` and `gfx950` hooks are parser coverage only. They do not
construct a G7 admission or inspection token and must not be consumed as G8,
provenance, release, or hardware evidence. Each target requires all four exact
environment inputs:

```text
FE2O3_GFX942_PARSER_CONTAINER_V1
FE2O3_GFX942_PARSER_CONTAINER_SHA256
FE2O3_GFX942_PARSER_PAYLOAD_SHA256
FE2O3_GFX942_PARSER_SOURCE_COMMIT

FE2O3_GFX950_PARSER_CONTAINER_V1
FE2O3_GFX950_PARSER_CONTAINER_SHA256
FE2O3_GFX950_PARSER_PAYLOAD_SHA256
FE2O3_GFX950_PARSER_SOURCE_COMMIT
```

Container and payload SHA-256 values are recomputed and must match their
64-digit lowercase pins before parsing. The source commit must be a full,
nonzero 40-digit lowercase Git ID. Container V1 does not bind that commit, so
the commit remains an unauthenticated operator annotation and establishes no
compiler provenance. Missing or malformed inputs and digest mismatches fail
closed.

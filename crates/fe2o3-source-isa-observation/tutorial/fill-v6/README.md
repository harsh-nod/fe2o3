# Source-exported inspection fixture

Generated on `mi350-2`, 2026-09-17, by
`node scripts/authoring-v6-smoke.mjs NEW_OUTPUT_DIRECTORY`, using the pinned
`nightly-2026-04-03` toolchain and the #280–#282 first implementation slice based
on compiler commit `fe406b0c3275c33b92f81734a4e6e3852b891073`.

`fill-v6.fe2sim` came from the real `examples/fill/src/lib.rs` production
extraction command, not a hand-built KIR graph. The retained receipt hashes the
source and exact bundle. The bundle is still extraction-only, self-contained
content custody: it does not authenticate protected compiler execution or grant
proof, production resume, artifact, hardware, load or launch authority.

The fixture covers read/select and rejection of stale or unsupported promotion.
It does not establish typed-assembly frontend admission or U2 round-trip support.
The companion script additionally checks CPU output words/canaries and captures
a source-bound debugger memory window. Its full execution artifacts remain in
the script's output directory; `receipt.json` records their hashes.

These are exact retained observations, not a requirement that future toolchains
produce identical IDs. Regenerate into a new directory, review changed source and
operations, and update fixtures explicitly rather than resealing old evidence.

The read-only snapshot, operation and region JSON projections were refreshed for
the current capability matrix and additive inline-assembly source-reference
field using this same retained bundle. Every source-reference field is null for
this ordinary fill kernel, which still has no eligible bitwise/ISA selection.
The original bundle, its
source/build receipt, and its historical stage-availability claims are unchanged.

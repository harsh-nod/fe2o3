# S09 source-debug pilot V1

This pilot preserves source metadata for the authenticated General V3 `alpha`
kernel on exact target `gfx942:xnack-`. It is enabled only by Worker V2
configuration value `s09-alpha-gfx942-o0-v1`, which requires COV6, `O0`,
`strip-debug=false`, and per-stage LLVM verification.

The profile is deliberately closed. The compiler requires the checked-in alpha
source identity, function at line 68, local `i` at line 70, and the exact
`f32`, read-only slice, and `DisjointSlice<f32>` argument profile. It emits
DWARF for:

- function `alpha` and its source file/line;
- scalar argument `scale`;
- the physical slice components `input_data`, `input_len`, `output_data`, and
  `output_len`; and
- scalar local `i` after the trusted global-index operation.

Rust slice and `DisjointSlice` values are represented by their physical
pointer/length components. Aggregate reconstruction, structs, tuples, arrays,
optimized debugging, arbitrary source profiles, and user-provided debugger
commands are unsupported. Requesting the profile with optimization or debug
stripping fails before compiler execution. A source, ABI, SSA-shape, target, or
pre-existing debug-metadata mismatch fails before Worker V2 publication.

The compile test emits one COV6 HSACO and requires `llvm-dwarfdump --verify` to
accept its linked DWARF. The fixed ROCgdb runner and transcript checker are a
separate debug evidence boundary. S09 remains `Missing` until a production
debug result from that runner is signed and accepted by the parity gate.

# Retired Wave64 Source/KIR Adapter

Current source-to-KIR qualification: unsupported.
No current source-to-KIR receipt is available.

The two adjacent .rs.txt files preserve the old adapter and its integration
tests byte-for-byte outside Cargo module/test discovery:

- source_kir_refinement_v1.rs.txt: 4a72320247f4b7433a58a982166c36b75ed70ff140fdec010658e2f3ce49a63a
- source_kir_refinement_v1_tests.rs.txt: 10831fdb25f252572bfb42ec728f3545078baa3b072d8f2d189343599a02b1b3

The adapter contains two unit cases and the test artifact contains nine cases.
These 11 retired Rust cases are not current passing coverage. They require an
absent, retired workload-profile schema and cannot qualify the general compiler.

Historical source SHA256:
7c6ead1e7c01a61a8f31a010c9e8cb9bd1c21a905ba61e9d90c6c077c748ffd4
Historical schema SHA256:
da2722bd3ce349228644300b13bb45d4683d1ebd60f8b7749e7764ec6569e894

The historical schema digest is not the identity of the current general Kernel IR.
It is recorded, not rebound to another file. The unchanged historical Verus
theorems and three negatives are abstract-model evidence only.

No feature or environment selector re-enables this adapter. Live module/receipt
exports are absent, with compile-fail and host absence tests. There is no
replacement success constructor or always-failing compatibility API. Existing
tutorial qualification remains pending/unqualified; exact source/CPU evidence
does not grant compiler, artifact, runtime, machine, safety or parity authority.

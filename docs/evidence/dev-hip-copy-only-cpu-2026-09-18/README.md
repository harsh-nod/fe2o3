# HIP Copy-Only CPU Qualification

Development above `9b9265c6919cb8dff9506f2c6ffa7b7f2538905f` adds the explicit
`diagnostic-copy-only` mode to the existing HIP comparator. The
[protocol documentation](../../../benchmarks/runtime_gfx942/HIP-COPY-DIAGNOSTIC.md)
defines its bounds, full per-round validation, raw timing output, explicit
resource-release gate and comparison limits. The legacy invocation retains its
allocator workload and output schema. Exact gfx942/XNACK-token checks also reject
malformed or contradictory target strings previously accepted by prefix matching.

Ten CPU-only mock test groups pass, including every round and output field,
the 10,000-round limit, argument/identity/allocation/submission/wait failures,
first/middle/last-byte corruption, cleanup failures and late output failure.
The unchanged argument-helper regression passes two tests. The actual comparator
is compiled with strict GCC warnings against CPU-only HIP symbol implementations,
not linked to the native HIP runtime. ROCm headers are explicitly selected from
`/opt/rocm`; this campaign did not skip the adapter tests.

Before/after snapshots cover the complete benchmark source directory and selected
compiler/HIP header identities. Unrelated concurrent Rust bootstrap development
is outside this scoped C++/Python qualification. The selected external hashes
are not a hermetic compiler/SDK closure or authenticated formal toolchain claim.
Ten receipts record actual commands, start/end timestamps, statuses and combined
output; the final auditor checks complete Python test results, source stability
and closure. Exploratory development test runs preceded this retained campaign.

No GPU was used for these tests. Native HIP execution and a matched KFD/HSA/HIP
campaign remain required. This packet reports no throughput, parity, formal
refinement or milestone acceptance. Future runs must never link
`hip_copy_diagnostic_mock.cpp` into a hardware executable.

`verify.py` checks current scoped sources and selected external inputs against
the retained snapshots. After later changes, use `SHA256SUMS` for offline archive
integrity; the historical verifier receipt does not become a current-source pass.

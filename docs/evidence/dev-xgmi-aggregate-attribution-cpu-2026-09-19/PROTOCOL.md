# Aggregate XGMI Attribution CPU Qualification

This packet qualifies bounded CPU behavior of the aggregate attribution recorder,
the existing aggregate batch path, profiling equivalence, the older attribution
recorder, and the peer benchmark argument/reporting surface. It is not native GPU
execution, a timing acceptance gate, a hardware refinement proof, or HIP/HSA parity
evidence. In particular, native dual-enable and mixed-path diagnostic capture are
not established by this packet.

## Execution

From the repository root, with GNU and x86_64-unknown-linux-musl Rust toolchains
available, run:

```sh
python3 -I -B docs/evidence/dev-xgmi-aggregate-attribution-cpu-2026-09-19/test_cpu.py -v
python3 -I -B docs/evidence/dev-xgmi-aggregate-attribution-cpu-2026-09-19/cpu.py --record
python3 -I -B docs/evidence/dev-xgmi-aggregate-attribution-cpu-2026-09-19/cpu.py --live
```

Recording requires a fresh packet containing only the three checked-in tools and
this protocol. Existing raw data, bindings, or seals are never overwritten. A
failed attempt remains unsealed and must be preserved outside this packet before
another attempt. The pinned recorder bounds each child process group, records
stdout/stderr and exact commands, and verifies group absence after every stage.

All Cargo test and clippy stages use two build jobs, optimization level 1, debug
information disabled, debug assertions and overflow checking enabled, incremental
compilation disabled, and two Rust test threads. No GPU, SSH, or native comparator
is invoked. Rust formatting is restricted to the seven changed Rust files, with
child traversal disabled. Clippy checks the runtime library and benchmark example
with all features and warnings denied.

## Exact Coverage

The 21 ordered stages include full source snapshots and Rust/Cargo identities;
GNU/musl full library listings and runs filtered by
`kfd_backend::xgmi_batch` and `kfd_backend::xgmi_diagnostic`; GNU/musl all-feature
and GNU feature-off example listings/runs; unsafe-inventory listing/run;
verifier calibration; scoped formatting; and clippy.

Frozen source-derived allowlists require 38 selected aggregate tests and 9 older
diagnostic tests on each target, and 10 example tests in each configuration.
Complete GNU/musl library listings must agree. Every execution must contain each
selected name exactly once, all passing, with the exact filtered-out count derived
from its full listing. No prefix-only or count-only success is accepted. The
unsafe-source test runs five tests and ignores only the explicitly named baseline
refresh command with its exact maintenance reason. It does not rewrite the
baseline. Seven Python tests calibrate the verifier without invoking Cargo.

Existing scaling tests run as semantic tests with ordinary captured output. Their
timings are not extracted, compared, or treated as performance acceptance.

## Binding and Replay

The recorder and source selector are imported/executed only after verification
against the SHA-256 constants in `cpu.py`. No historical evidence is modified.
The selector includes Cargo manifests/lockfile, toolchain/configuration, complete
crate/example source, relevant benchmark sources, and the unsafe-source baseline.
Required inputs explicitly include both scaling test modules, profiling
equivalence, and the aggregate recorder. The byte-identical before/after source
snapshots bind every selected path, not merely the changed files.

`tools.json` freezes the three new packet files and both authenticated helpers
before recording. Binding preparation verifies exact receipt schemas, commands,
working directory, integer types, stage ordering, stream hashes, successful exits,
and child-group absence. Duplicate JSON keys and nonfinite numbers are rejected.
The binding includes full source and test rosters, tool identities, and toolchain
output hashes. All evidence is included in the exact `SHA256SUMS` closure; extra
files/directories, missing files, and symlinks are rejected. SHA-256 is an integrity
seal, not a claim of external cryptographic signing.

`--record` prepares the binding, seals the completed archive, and verifies live
source equality. For a separately recorded complete raw packet, `--prepare-binding`
and then `--seal --live` provide the same checks without overwriting prior files.
Plain invocation and `verify()` replay the archive; `--live` also requires the
current selected source map to match. The recorded source base commit is retained;
a later evidence-only commit is allowed when the complete selected file map is
unchanged. Rust and Cargo output hashes remain available to native comparison
verification. Calibration fixtures and mutations are generated only in temporary
directories and are not evidence of actual Rust execution.

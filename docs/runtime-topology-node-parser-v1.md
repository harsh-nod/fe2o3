# Fixed-Slot Topology Node Parser

The direct-KFD topology reader stores its closed set of 39 node properties in
a `[u64; 39]` and a presence bitset. This replaces the private
`BTreeMap<String, u64>` representation, following the existing link parser.
Successful property insertion and lookup no longer allocate key strings or
use tree nodes. The bounded input text still allocates, as do error payloads.
Absent properties remain distinct from properties whose value is zero.

## Contract

This is an in-memory parser change, not a currentness fast path. It preserves:

- Every filesystem observation, its order, and its bounded read.
- The exact closed key set and all ranges, including recognized unused fields.
- Final-newline, line-count, literal-space, and canonical decimal validation.
- Error precedence: syntax, known key, numeric conversion, range, duplicate.
- Partial CPU-node property sets and the existing GPU required-field order.
- All three optional SDMA properties and their absence semantics.
- Fresh whole-host discovery, directory/file identity checks, all-GPU render
  correlation, and generation/host-identity closing checks.

All historical schema minima are zero. The fixed parser therefore checks the
same upper bounds on `u64` values without a redundant lower-bound comparison.
The separate platform and link parsers are unchanged. No public API or
execution authority is added.

## Verification

`topology/tests/node_properties/reference.rs` retains the parser and range
table from commit `563f5a2b17f9ab0dcbee41c4c436a8e768515f47`; only the shared file
read is factored out for differential tests. The new tests cover all 39 slots,
bounds, zero/absence, permutations and sparse subsets, malformed/unknown and
duplicate inputs, overflow, and error precedence. They also check mixed CPU/GPU
discovery, every required GPU-field omission and omission pair, all optional
SDMA subsets, and identical selected Rust I/O boundaries for successful and
failed reads. The boundary trace is not a kernel syscall trace.

Run the focused regression suite with:

```sh
cargo test --locked -p fe2o3-kfd --all-features --lib topology::tests::node_properties
```

Differential tests are regression evidence, not a formal proof of the parser,
kernel, driver, or machine code. This change does not establish a native copy
latency improvement or HIP/HSA parity. Existing measurements identify sysfs
discovery as the main cost, and its observations are intentionally unchanged.
Any performance claim requires a separate matched baseline/candidate run.

## CPU Qualification (2026-09-20)

On pinned nightly `2026-04-03`, the following six test filters passed together
on GNU/all-features, musl/all-features, and GNU/no-default-features: 205 passed,
zero failures and zero ignored tests in each configuration.

```text
topology::tests::
currentness::
currentness_diagnostic::
shared_memory::pair_currentness::tests::
device::tests::
sdma::tests::
```

Pass the filters after `--` to `cargo test --locked -p fe2o3-kfd --lib`, with
the relevant feature and target options before `--`. Tests used opt-level 1,
debug assertions and overflow checks enabled, incremental compilation disabled,
and two test threads. Warnings-denied library/test Clippy passed with both
all-features and no-default-features. All six changed Rust files passed rustfmt.

Broader GNU and musl runs of all 1,524 tests were deliberately terminated during
unrelated, CPU-heavy queue-construction fault matrices. They reported no
assertion failures before termination, but exited unsuccessfully on SIGTERM:
**full-suite completion is not claimed**. Their owned processes were confirmed
absent afterward. This increment did not run hardware tests or benchmarks and
created no remote resources.

# Matched XGMI Hot Batches: CPU Qualification

This packet qualifies the benchmark changes for matched persistent-hot XGMI
batches at depths 1, 16 and 32. It does not qualify native execution at the new
depths, GPU concurrency, copy performance, A1/A2 closure or HIP/HSA parity.

## Change And Results

HIP/HSA hot callbacks now prepare, enqueue and validate every allocated slot.
Every enqueue precedes every wait within a timed batch. Validation reads every
complete source and destination, including both guards, even after a payload
mismatch. The KFD non-diagnostic hot-only gate admits depths 1 through 32;
diagnostic modes remain depth-one. Ordinary and ordered paths are unchanged.
The maintained result parser requires independently supplied trial controls
and computes bandwidth from the full one-direction batch byte count. See the
[contract](../../runtime-xgmi-hot-batches-v1.md) for timing boundaries.

The accepted `cpu2` run passes all 21 command stages:

| Scope | Result |
| --- | --- |
| GNU example, default / all features | 13 / 14 passed; no failures or ignores |
| musl example, default / all features | 13 / 14 passed; no failures or ignores |
| Native argument controls | 2 passed |
| Common lifecycle, controls and buffer helpers | 3 passed, including normal and UBSan C++ execution |
| Actual HIP/HSA hot callbacks with instrumented CPU APIs | 7 passed, including normal, UBSan and three expected-negative compiled mutations |
| Maintained result parser | 7 passed, covering all nine matched backend/depth cells and invalid records |
| Existing ordered-segment controls | 1 passed; 1 optional Rust/C++ differential test skipped |
| Real native comparators | HIP gfx942 and HSA ELFs compiled with strict warnings; both retained |
| Rust formatting and scoped strict Clippy | Passed |

The Rust feature/target counts overlap; they are not 54 unique tests. The five
Python commands total 20 passing tests and one skip. The existing optional
ordered-segment differential requires `FE2O3_SEGMENTS_RUST_BINARY`, which was
not supplied; its separate C++ normal/UBSan control still ran. No full runtime
library, doctest, Verus or native GPU suite was rerun in this packet.

The callback harness includes the actual producer functions and replaces their
GPU APIs with instrumented CPU functions; it does not link HIP/HSA runtimes.
It checks exact slot pointers, byte lengths, enqueue/wait order, complete buffer
readbacks and HSA signal reuse. Expected-negative variants corrupt HIP source
selection, HSA source selection and shared validation traversal. Pattern
directions are exercised, but reversed native endpoint/agent arguments,
hardware scheduling, API failures and GPU teardown are not qualified here.

## Source And Prior Qualification

Opening/closing maps bind 3,955 selected inputs. Exactly ten benchmark/example
source paths differ from the previous
[production CPU packet](../dev-topology-link-scratch-cpu-2026-09-24/README.md).
Production runtime libraries, manifests, lockfile and toolchain are unchanged.
The previous packet is present in signed parent
`32f560f0887b2be29a492a16b9700b053c929e27`; its runner identity is separately
pinned by this checker. Its GNU/musl topology, currentness and runtime results
are reused only at that unchanged source scope, not described as new runs.

The runner uses `nightly-2026-04-03`, locked/offline Cargo, a fresh exclusive
target directory, two build jobs and two test threads. Test optimization is
level one with debug assertions enabled and incremental compilation disabled.
Rust, Cargo, g++ and hipcc identities agree before and after the accepted run;
Rust/Cargo identities also match the prior qualification. The retained runner
defines environments, working directory and command deadlines. Compiler
libraries, system headers and the host are not hermetically archived.

Each substantive command has argv, timestamps, exit status, output and a
process-group absence receipt. The runner's source-enumeration Git calls are
not separately bounded or receipted; this packet does not claim every runner
action has that treatment. The offline checker independently re-enumerates
selected inputs with scrubbed Git and a timeout.

## Rejected Run And Cleanup

`raw/rejected-cpu1` preserves the initial frozen qualification. Its test/build
stages passed, but formatting rejected two assertion layouts. Only rustfmt's
changes to those assertions separate it from the accepted source. It is not
accepted as a complete qualification. `cpu2` reran the entire sequence using
a fresh target directory after the correction.

Collection verified full retained inventories and byte equality before
removing the two owned Cargo targets and two preliminary callback binaries.
`raw/cleanup.json` records all four exact paths and their absence. Cleanup
reclaimed 1,438,052,352 allocated bytes; logs and real comparator ELFs remain.
No SSH or GPU workload ran, and no foreign resources were removed.

## Replay And Limits

Authenticate this packet's signed Git commit before executing its scripts.
From the original qualified checkout:

```sh
python3 -I -B docs/evidence/dev-xgmi-hot-batch-cpu-2026-09-24/verify.py
python3 -I -B docs/evidence/dev-xgmi-hot-batch-cpu-2026-09-24/test_verify.py
```

The replay checks the exact 123-artifact roster, source maps/delta, runner
identity, command rosters, tool continuity, reported results, retained ELF
identities, cleanup and the rejected format outcome. Current qualified source
and historical cleanup-path absence are required; this is not an arbitrary
future-checkout replay. `validation/` captures the bounded final replay and
20 checker tests: one valid baseline and 19 hostile-record controls. Its input
bracket covers this README and the packet scripts/manifest.

Next is a fresh signed-source native campaign on admitted free endpoints,
retaining each backend/depth/process/direction result separately. Batch latency
divided by depth is amortized cost, not a per-copy latency percentile. Neither
CPU callback tests nor final canaries prove every repeated intermediate copy.
Whole-runtime formal refinement, aggregate retained-memory bounds and A7
performance acceptance remain open.

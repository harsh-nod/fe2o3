# Fresh Link Scratch: CPU Qualification

This packet qualifies the [fresh link scratch change](../../runtime-topology-link-scratch-v1.md)
in signed source `1e13a90cd5e604a4612b079b48f629005a0a047b`. It reduces
allocation churn without caching topology observations. It is not native
performance evidence, whole-runtime formal refinement or HIP/HSA parity.

## Accepted Checks

`raw/cpu1` passes all thirteen serialized stages. It uses the pinned
`nightly-2026-04-03` toolchain, two build jobs, four test threads, optimization
level one for tests, debug assertions, no debug information and no incremental
compilation. Opening/closing source maps agree on all 3,951 selected inputs;
opening/closing Rust and Cargo output bytes also agree.

| Gate | GNU | musl |
| --- | --- | --- |
| KFD topology selection | 83 passed, no ignores | 83 passed, no ignores |
| KFD currentness selection | 30 passed, no ignores | 30 passed, no ignores |
| Full runtime library, all features | 1,365 passed, twenty existing hardware-only ignores | 1,365 passed, twenty existing hardware-only ignores |

The host doctest command passes 27 KFD and 46 runtime tests (the latter emitted
as four plus forty-two). Formatting and all-feature/all-target strict Clippy
pass for both packages. This is selected KFD coverage, not the full KFD library
roster; no hardware test was enabled by the twenty ignored runtime tests.

The six new regression functions cover long-to-short reuse, error residue,
opening rejection without allocating read storage, bounded I/O observation
order, competing error precedence and actual allocation counts. The count test
checks 1, 2, 8 and 64 successive healthy reads and observes exactly N-1 fewer
Rust allocation calls with one shared buffer. It tests the reader mechanism;
the production traversal's one-buffer-per-link-set wiring is separately
reviewed. It is not a latency measurement or a kernel-syscall-count proof.

`raw/focused2` separately passes all twenty prechecked-reader tests. This is a
subset of topology coverage, not twenty additional unique tests. The earlier
`raw/focused1` failed compilation because the existing fixed-schema differential
test still used the old reader signature. Its raw failure and source bracket
are retained; the test call was updated before the accepted campaigns.

The byte-exact runner has SHA256
`b1e15db084398e4e19cbb0f4f7a0588cf2594b89611d49248a1426243c339b4a`.
Each command retains its argv, timestamps, exit status, process-group absence
and original output bytes. The bracket covers the runner-selected source
inputs, not a hermetic toolchain or the complete process address space.

Native comparison and performance acceptance remain separate. No new Verus
solver run, executable-refinement theorem, aggregate memory bound, A1/A2
completion or parity claim is made by this packet.

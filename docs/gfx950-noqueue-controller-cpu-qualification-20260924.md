# gfx950 no-queue controller CPU qualification — 2026-09-24

The new fixed-purpose debugger controller and entry-rendezvous observer build
as normal executables and pass the CPU checks below. They have **not** been
run together against ROCgDB or a GPU in this qualification. This does not
complete #281 V4 or prove runtime debugger acceptance.

The controller drives only a newly launched, owned observer through fixed
host breakpoints. Its code-object observation compares the original ELF with
the bounded memory object reported in the same stopped inferior. There is no
arbitrary attach, caller-selected MI, register capture, queue creation or GPU
dispatch interface. Even a successful content comparison retains
`runtime_loaded_success=false`, `debugger_acceptance=false` and
`physical_capture=false`.

## CPU and build results

- Controller example: 24 tests passed, including the three reused MI-parser
  tests, exact startup environment, required nested library-range syntax,
  identity/stop/content mismatches and independent exit/reap/stream outcomes.
- Shared parser/state controls in the debugger library: 18 passed.
- Entry-rendezvous sibling example: 15 passed, including reused controls.
- Unsafe-source inventory: 5 passed; its explicit refresh operation remained
  ignored.
- Both selected examples passed strict Clippy with `--no-deps`.
- Both normal examples built successfully; test harnesses were not substituted.

These counts include overlapping parser/fixture coverage and are not a count
of distinct end-to-end cases.

The required MI library-range shape needs syntax depth 12. Depth 11 rejects
that exact shape; over-deep, duplicate and unknown range fields still refuse.
The other parser and transcript bounds were not relaxed. The debugger's
cleared environment contains only the two locale settings and
`PYTHONNOUSERSITE=1`, `PYTHONSAFEPATH=1`,
`PYTHONDONTWRITEBYTECODE=1`. These flags do not disable system Python startup
hooks or establish process isolation.

The broader strict Clippy invocation initially failed in dependencies
(`large_enum_variant`, `too_many_arguments` and `nonminimal_bool`).
The selected-example result above is not a claim that the entire dependency
graph is warning-free. Four findings in the new controller/parser were fixed
without warning suppressions and the CPU tests were rerun.

## Retained identities

At the successful selected-example gate, compiler HEAD was
`76fe660d9ef27961ec764c5cec1b7b79e6321a33`. Its exact dirty-source census
was 7,258 files / 108,069,170 bytes, SHA-256
`742d58c8728bbdb5d66325534c094d33546e01ecaf960d7467f7f101b4d9b1ec`.

| Record | SHA-256 |
| --- | --- |
| CPU example/library/sibling checks, before broader dependency-lint refusal | `6ae0115d9560bd9ef20f35ea8f0ede768dbb023042d7cffb7a7a41e05c7109f6` |
| Selected-example strict lint, inventory, CPU rerun and normal builds | `553f89d61b9a5fa1bf5f35377e5cd002814e99e7589ecdbf414c4f03c2648d30` |
| Controller executable, 2,280,232 bytes | `dae850d70e801798b6cc6bd55a2612444d0e6647e3b66734e96b6977b279c2f4` |
| Observer executable, 3,678,008 bytes | `383e06f79693dc10636f344d34eab3361d5224f606038ec25692d2cf3431870f` |

See the [controller README](../crates/fe2o3-debug-cli/examples/gfx950_noqueue_debugger_acceptance_v1/README.md)
for its exact closed arguments, limits and cleanup contract.

## Remaining native work

The controller separately observes inferior termination, debugger termination,
direct-child reaping, stream EOF and reader joins. It does not claim to reap
the debugger's inferior or prove launch-family quiescence. The ordinary task
runner's process-group cleanup is not a substitute for that missing family
supervision.

Before an actual run, qualify the dedicated family supervisor with benign
process fixtures, join the exact managed service/cgroup ownership, and retain
the effective executable/Python/native startup dependency closure. An eventual
host-stop ELF match still does not prove the complete runtime callback path,
live trap/queue support, same-stop register state or a hardware kernel result.
Those remain separate integration steps.

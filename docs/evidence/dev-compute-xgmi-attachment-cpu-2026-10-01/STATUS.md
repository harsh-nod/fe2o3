# Completed Integration Checks

This supplements the unchanged original [packet](README.md) and its manifest.
The original manifest covers `records.tar.gz` and the original README only;
the files described below were added after that packet was prepared.

## Runtime Regression

The all-feature runtime library suite completed with exit 0: 1,928 passed,
32 ignored, zero failures and no filtering. The test phase took 454.35 seconds
after a 5 minute 30 second build. `runtime-command.json` retains the command,
environment, working directory and observed exit status. `runtime.stdout` and
`runtime.stderr` are the complete, unmodified command output, including nested
subprocess-test output. This command did not run native GPU tests or a solver.

The integration used source baseline
`5778c9d09711055271c7f9c8f01afd53d056ad82` plus the ten frozen KFD files and
four guard updates identified by `guards/proposal.json` inside `records.tar.gz`.
The same source changes passed the 32 recorded source-CI commands. Only
documentation and evidence packaging changed during the runtime test.

The integrated runtime library also passed `cargo check --no-default-features`
with exit 0 in 1 minute 26 seconds. `no-default-command.json` records the exact
command; `no-default.stdout` and `no-default.stderr` preserve its complete output.

## Incomplete KFD Suite

The separate full KFD suite exceeded its 900-second bound and exited 124.
`full-kfd-suite-terminal.json` preserves its command and final tool-output chunk.
It is explicitly **not** a complete stdout capture: earlier tool output was
truncated. No suite-wide pass or failure-free qualification follows from this
incomplete run. The complete 19-test focused KFD result and strict KFD Clippy
result remain independently recorded in the original archive.

## Remaining Qualification

Native queue creation/retirement, peer-mapped compute buffers, runtime XGMI
routing and full-byte compute-to-XGMI-to-compute execution remain unqualified.
The CPU results establish neither formal refinement nor HIP/HSA performance
parity. Read-only MI300X snapshots found foreign activity and did not admit a
free pair, including the closing check at 20:23:30 UTC on 2026-10-01. No native
run was admitted. The previous owned remote CPU scratch root was removed only
after its evidence and binaries were durably recovered.

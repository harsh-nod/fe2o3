# HIP Copy Payload CPU Qualification

Development qualification on 2026-09-18, based on commit
`1890a64e1911a2a346c5a9e70a8ff5ad45b19231` plus the recorded working-tree changes.
This packet adds no native execution or performance acceptance.

## Results

- Eleven strict payload-parser calibration groups passed.
- Ten actual C++ comparator / CPU HIP-mock groups passed, including validation
  of comparator stdout through the new parser.
- Two native benchmark argument-helper tests passed.
- Ruff lint, formatting and ShellCheck passed.
- Eight complete command receipts exited zero, with no unittest skips.
- All 90 benchmark source identities and three selected external identities
  were identical before and after the campaign and matched at sealing.

The payload checker binds independently supplied workload/device/target values,
the exact ordered round roster, full-buffer checks, positive bounded raw
intervals, release counts, native exit status and empty native stderr. It never
accepts performance or replaces host observation and process-custody checks.
The CPU mock uses installed HIP headers but performs no GPU work; its call
trace is separately checked and is not represented as native stderr.

## Reproduction And Integrity

`qualify.sh` records commands, start/finish times, complete combined output and
exit status. It refuses to overwrite an existing raw directory. `verify.py`
requires exact full unittest rosters, receipt closure and unchanged current
source/compiler/header identities. It uses the hash-pinned input snapshot
helper from the preceding copy-only CPU archive without modifying it.

The external snapshot includes `/usr/bin/g++` and two selected HIP headers,
not the compiler's complete transitive dependency closure. This is a local CPU
qualification, not a hermetic or reproducible native build.

Run `sha256sum --check --quiet SHA256SUMS` from this archive for historical byte
integrity. Run `python3 -I verify.py` for the stronger current-tree/current-tool
check, which is expected to fail after relevant source or tool changes. The
manifest includes the complete raw receipts and all files except itself.

Shared-host native qualification, matched HIP/HSA comparisons, production
refinement, A1/A2 and issue #182 closure remain outside this packet.

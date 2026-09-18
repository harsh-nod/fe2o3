# Combined SDMA Retained Release: CPU Development

This packet qualifies the production-shared retained-release driver and
composition preflight for Directional plus Striped (2, 4, 6, 8, 10, 12, 14)
owners. It is R126 development, not full runtime acceptance or HIP/HSA parity.

The archive binds a single source cohort based on `fca96f832d890f2caa0ddd239c6535596a5fcdc2`.
The before/after inventory covers 5,557 selected source/build inputs, including
the runtime release contract. Evidence and build outputs are excluded. Raw
commands, UTC times, statuses, selected rosters and complete outcomes are kept;
`verify.py` rechecks their exact scope without executing archived commands.

## Scope

- 32 selected KFD tests on GNU and musl: five combined tests, existing generic,
  directional and standalone-striped tests, public selector/drop tests and six
  lower SDMA cleanup tests. Unselected tests are not claimed as rerun.
- All seven combined counts with and without attached dispatch, original vector
  and token identities, nonzero striped cursor, exact ordering, backing refunds
  and resource counts (`11 + 3*N`).
- Borrowed rejection of invalid profiles/counts/cursors, cross-set duplicate IDs
  and 22 structural/pending mutations in each set. LogicalMux stays unsupported.
  Invalid secondary composition is a hard error even when the public parent is busy.
- Error/panic injection at every occurrence of SDMA topology, currentness,
  destroy, doorbell and resource callbacks in the maximum 14+2 composition.
  Representative primary destroy, resource, platform and signal failures check
  both roots stay terminal, including untouched and completed siblings.
- Real memory-cleanup fixture prefixes at resource positions 1, 14, 15 and 16;
  these are injected local native-interface effects, not Linux GPU faults.
- Retry snapshots cover both retained sets. The existing inline-size regression
  still bounds each SDMA custody at 32 KiB and the public root at 128 KiB;
  this does not prove total stack consumption.
- Four runtime shutdown/terminalization CPU smoke tests, strict all-feature,
  all-target KFD Clippy, formatting and diff checks.
- Four archive-parser calibration tests, including 13 rejected malformed
  harness/roster variants.

## Rechecking

```sh
python3 -B docs/evidence/dev-combined-sdma-release-cpu-2026-09-18/verify.py
python3 -B docs/evidence/dev-combined-sdma-release-cpu-2026-09-18/test_verify.py
```

`--live` additionally compares current selected source hashes with this cohort.
`--seal` writes a manifest exactly once. Do not rerun `qualify.sh` inside the
sealed archive; it refuses to overwrite receipts. A different source requires
a new packet. The inventory and harness parser are reused with pinned hashes.
For a fresh unsealed packet, run `qualify.sh` first, then record the calibration
with `record.sh calibration python3 -B <new-packet>/test_verify.py` before
verification and sealing. Calibration is separate from the source-test interval.

## Limits

These fixtures execute the shared production driver but do not execute Linux
ioctls, the real public combined constructor/selector workflow, submitted GPU
work, concurrency or copy throughput. No new MI300X run, formal theorem or
machine-code refinement is claimed. Standalone native striped receipts remain
historical evidence for their own source. Combined native qualification,
LogicalMux retained teardown, terminal-creation custody and broader R126
integration remain separate work.

Before qualification, development compilation exposed an overcaptured Rust
2024 opaque snapshot lifetime; the fixture now explicitly captures no lifetime.
A development fault test also used the generic callback hook for primary
DESTROY, whose fixture has a separate outcome selector; it correctly failed
when no fault was injected. The final test uses that selector. Those exploratory
runs are not represented as passing qualification receipts.

## Preserved Recovery History

`interrupted/` contains the original successful Clippy and 32-test GNU run,
both target rosters, and the incomplete musl attempt. Its recovered transcript
ends during the fifth combined test with a NUL-padded tail; no finished/exit
receipt exists, and it is not accepted as a passing harness. The verifier pins
the exact recovered transcript hash. Runner/test process and temporary build
directory absence was observed during recovery, but those command outputs were
not archived and are not independently checked by this packet. The cause of the
interruption is unknown. An archived recovery source inventory matched the
original byte for byte.

`setup-rejected/` preserves a subsequent Clippy invocation that exited 101 before
compilation: the replacement build symlink had no destination directory. A fresh
persistent private build directory was then created. Neither this setup failure
nor the interrupted musl transcript counts as qualified test evidence. `raw/`
contains the complete new qualification, rerun from the beginning with the same
source. The verifier checks all three source maps and preserves both rejected
attempt classifications; the seal covers their original retained bytes.

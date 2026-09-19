# XGMI Retirement CPU Qualification

All 21 recorded commands passed with no owned process group remaining at command
closure. The exact source inventory was unchanged across the run. GNU and musl
each passed 75 KFD tests, 30 runtime tests, and four benchmark-control tests, with
identical test membership across targets. Feature-off benchmark controls passed
four tests. The unsafe-source policy passed five tests with its one explicit
maintenance test ignored. Strict all-features/all-targets Clippy, no-default
compilation, formatting, and diff checks passed.

This packet qualifies CPU implementation tests only, not native execution,
formal refinement, driver fault injection, parity, or performance. The design is
described in `docs/runtime-xgmi-retirement-v1.md`; that descriptive document is
not a compiler input in this source inventory.

The fixed 21-command driver reuses the prior sealed recorder and command
definitions, adds retirement and ordinary SDMA release regressions, and limits
test concurrency to two CPU threads. The recorder retains exact commands,
timestamps, output digests, return status, and owned-process-group closure.
The run tools are frozen before commands start. The post-run acceptance tool
independently pins source and test rosters; it is not part of the run-tool set.

The qualification ran against base
`0402a9f6b8fe873eb162ddb998f48c011b4b65aa` plus the recorded implementation changes.
Its 5,568-file source snapshot SHA-256 is
`524e4c3cf2e61a03495d75cbc61979ed8724e9d8ee81d0d7fcd6723f8e05415c`.
The inventory binds selected compiler inputs, not arbitrary host state.

The verifier passed 14 calibration tests, including offline verification from a
relocated checkout. Historical receipt paths remain bound to the original
execution root; `--live` instead compares the current checkout's selected source
files with the qualified file map. The final `SHA256SUMS` seals the complete
archive, including its post-run verifier, calibration tests, and this report.

Acceptance:

```sh
python3 -I docs/evidence/dev-xgmi-retirement-cpu-2026-09-18/verify.py --live
python3 -I docs/evidence/dev-xgmi-retirement-cpu-2026-09-18/test_verify.py
```

Prior CPU/native packets remain unchanged and do not qualify this revision.

# Matched Hot-Batch MI300X Campaign

The [prospective protocol](PROTOCOL.md) fixes the matched depth-1/16/32 workload,
source identity, trial ordering, admission, timing interpretation and cleanup.
Native qualification and performance results are not yet claimed by this
checkpoint.

## Preparation

The source benchmark changes and their complete CPU qualification are signed
at `8dc128357ecd55e1ba4f2866eb075899481f9aa0`, published on both topic remotes.
The controller requires the exact signed CPU evidence packets, source maps,
retained parser and a fresh cold-build target. No production source change is
introduced by this protocol.

`protocol3` passes 31 tests across three bounded commands: fifteen prospective
campaign tests, nine existing authenticated bootstrap/cleanup controls and
seven maintained result-parser tests. All process groups are absent and the
five-file protocol input bracket is unchanged. The exact protocol scripts are
also retained with that run. The earlier protocol1/protocol2 records are
superseded development controls, not native acceptance; their input hashes are
retained but their intermediate script bytes were not independently frozen.

These CPU tests qualify declared argument, source, identity and failure-path
controls. They do not establish real GPU execution, hardware concurrency,
performance, complete native teardown or formal correspondence. The next step
is one fresh native campaign, followed by exact collection, owned cleanup and
independent result replay. A1/A2, A7 and general HIP/HSA parity remain open.

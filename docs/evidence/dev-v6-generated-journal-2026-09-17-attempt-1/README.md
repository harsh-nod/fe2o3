# Preserved First Generated-Journal Qualification Attempt

This is failed development evidence, not acceptance. The recorded GNU campaign
returned exit 101: host 271 passed / 4 ignored, runtime 1003 passed / 6 failed /
17 ignored. It stopped before runtime-model and musl qualification.

All six failures concern existing ordinary retained-submission semantics after
stream destruction. The new held-stream guard incorrectly required that stream
to remain present for cancellation, drain and consuming release. The corrected
candidate checks only whether an existing stream is held; identities and the
existing terminal-state rules remain unchanged. See the separate
[final campaign](../dev-v6-generated-journal-2026-09-17/README.md).

The original 16-file source manifest, base-relative patch and raw commands,
outputs, timestamps and exit codes are preserved. Commands are not rerun or logs
overwritten here. No native GPU, solver or performance qualification was run.

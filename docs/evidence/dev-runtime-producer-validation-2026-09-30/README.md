# Integrated Producer Validation CPU Regression

The all-feature `fe2o3-runtime` library suite at signed integration
`624101b57146ae979f641e3b4d3e0ef938ad7c75` passes 1,901 tests, with zero failures
or filtering and 32 existing ignored native tests. All 1,933 output test names
are unique. Build time is 4 minutes 56 seconds; test time is 89.49 seconds.

The command uses pinned nightly-2026-04-03, offline locked Cargo, two build jobs,
two test threads, test opt-level 1, and no incremental compilation. Cargo
recompiles changed crates using an existing target cache. Complete selected
source, tool and PID-namespace continuity checks pass; the owned child group
closes. This is development regression evidence, not native execution, a proof
campaign, an independently authenticated qualification, or a milestone exit.

`raw.tar.xz` contains the original eight files: opening inputs, closing source
inventory, result, runner, process-owner source, and owned stdout/stderr/receipt.
Archive members are checked against their original bytes on readback. The raw
records retain historical absolute paths. They do not include the toolchain,
dependencies or test ELF and are not a standalone rebuild package.
Archive SHA-256:
`df50328bb932a802eecf56ce7482455b77bad35c47b8ae1bf56791e412a6d5bc`.

The separately retained ELF is 43,605,824 bytes, SHA-256
`cc2e201c3c6f29880227493bcc3fcb9f13ad855486b01e80eb7065c5bf5ac453`.
Opening inputs SHA-256 is
`6283f22746d8dd57de01331b966e5de89c3383c73a813286ede2f3a01721c5a1`;
stdout SHA-256 is
`7f43e1d62614be21e1a2508585a8bb71aa80008a80c197e3fd58f50be6c5869b`.
The older 1,893-test success and rejected KFD attempts remain separate evidence;
this run does not supersede or qualify the incomplete KFD suite.

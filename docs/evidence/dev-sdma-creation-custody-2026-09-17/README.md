# Returned SDMA Creation Custody Development Evidence

This CPU development packet repairs returned-owner retention during SDMA creation
settlement above `5db1450574a12053d46d890a0bfdc2a8c4f17e9c`. It does not advance
R126/N5 acceptance. Native R125 remains accepted at its CPU/test boundary;
Admission R118B C1/C2/C3 and Resources R116/V3 are unchanged. A1/A2, issue #182,
hardware/formal qualification, aggregate memory and HIP/HSA performance remain open.

## Behavior

All six public creation profiles use one shared production settlement helper.
It retains single/combined returned owners or lower failure custody before model
retake unwinding escapes and before the adapter's final poisoning or metadata
destruction. Lower failure construction may already poison its process gate.
The original panic outranks later retake, metadata and poisoning panics.
Ownerless retryable lower rejection stays retryable only after successful model
return. Vacant-owner preflight prevents overwrite and terminal retry is inert.

Creation alone defers the shared model envelope's poisoning to the outer
settlement helper. Other callers retain their original policy and original
restore/retake call sites; `model_loan.rs` is unchanged. Successful public adapters
install their owners before inspecting or returning observations. Terminal
conversion moves existing rosters without a new allocation or arbitrary callback.

No-returned-output remains distinct from retained custody. This does not recover
owners consumed inside a panicking lower creator, add native destruction/recovery,
or qualify Generic, Striped, LogicalMux and terminal-creation teardown.

## Coverage

Eleven new tests exercise the production-shared driver with constructed primary
parents and real model loans. They cover single/combined returned-success rosters,
all profile rosters and returned lower failures with primary/attempted custody
and an empty secondary roster,
opening/retake errors and panics, actual reclaim rejection after certificate
regression, missing callback output, ownerless retryable/terminal classification,
metadata destruction, first-panic precedence and normal Directional teardown.

Borrowed snapshots preserve original owner/token identities, roster pointers,
lengths and capacities, backing/model/accounting state and doorbell observations.
Terminal retry checks the complete backing snapshot, loan tuple, foundation
location, poison state and poison-callback history. A source assertion connects
the driver to all six public adapters and the production model/poison callbacks.

SDMA backing fixtures use the existing Vec-backed memory backend. Doorbells and
primary platform resources include owned local anonymous mappings. Queue IDs,
creation outcomes and attempted native observations are scripted. No native
CREATE_QUEUE, hardware-fault behavior, formal implementation correspondence or
performance result is established by these fixtures. Local fixture disposal is
not production recovery or proof of kernel queue destruction.

## Reproduction And Scope

`source-base.txt`, `source.patch` and `source-files.sha256` pin all eight changed
Rust files, including three new files. `freeze-source.sh` captures this cohort;
`qualify.sh` runs the final commands through the non-overwriting `record.sh`.
`audit.sh` verifies exact source, commands, chronological results, compiled
identities, per-test names and final artifact membership. `SHA256SUMS` seals the
completed archive.

The final lower selection is 249 tests: 238 existing and 11 new, with 1,151
filtered out of the full 1,400-test roster. This includes the live model envelope,
queue source gates, SDMA and Linux queue tests, selected live-foundation tests and
the new creation tests. Libtest substring matching also includes 47 existing
persistent-SDMA tests. It is not a full lower-library qualification run.

GNU and musl each passed all 249 selected KFD tests and all 869 runtime-library
tests, with 13 runtime tests ignored. The full KFD rosters match at 1,400 names;
the runtime rosters match at 882. Strict all-feature/all-target Clippy,
workspace formatting, 27 KFD and 33 runtime doctests, no-default compilation and
the unsafe-source policy (five passes, one maintenance ignore) passed.

The audit checks all 27 command records, including excluded earlier attempts,
exact final vectors and chronological ordering. KFD Cargo JSON, per-target
executable paths, binary hashes, unique roster/results and complete summary rows
are checked together. The source/test and scope reviewers found no blocker to
this bounded development repair. Neither ignored runtime tests nor the stopped
expanded constructor campaign count as passing native or full-suite evidence.

## Excluded Earlier Attempts

- `focused-preliminary` exited 101: the first panic-payload test used a non-Send
  Rc counter. The final test uses an Arc atomic counter.
- `focused-revised` exited 101 with seven passes and one failure: its dispatch
  snapshot wrongly required the poison bit to remain unchanged. The corrected
  existing oracle permits only the required poison-bit transition.
- `focused-corrected` passed eleven tests before the final lint/fixture changes;
  final qualification uses the subsequently frozen source instead.
- `clippy` exited 101 on the existing inline failure-custody representation and
  a large test-only snapshot variant. Scoped inline-custody lint allowances
  preserve the allocation-free production error path; the test snapshot now
  boxes its observation array.
- `gnu-selected` was an extra 512-test constructor campaign, stopped by the
  primary agent with SIGTERM (143) to keep this packet within the reviewed
  change-specific scope. Its partial log has no completion summary and is not
  counted as a pass. The final bounded campaigns are separately named
  `gnu-final-*` and `musl-final-*`; their Rust source is unchanged.

This packet performs no remote staging, GPU execution, remote deletion or remote
process signaling. Existing closed evidence archives are unchanged.

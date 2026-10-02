# Asynchronous Compute-XGMI CPU Checks

## Source and Scope

Source base: `d2ff52f63932a4765306528fefd5e82de5b87b12`, already pushed to
both repositories. The raw packet records the exact source/guard patch, changed
source hashes, toolchain, clean environments, commands, complete test rosters,
outputs and exit codes. Documentation and evidence are outside the source patch.

The persistent compute-XGMI adapter now separates begin, one-shot completion
sampling and finish. The original owners, identities, generations, physical
extents, mapping records and completion ticket remain rooted across Pending.
Both model foundations are restored before each return. Ready retains custody
until finish; finish rejects occupied output slots before native effects.
Admitted failure or unwind terminalizes both endpoints and retains ownership.
The synchronous adapter remains available to existing callers.

The runtime replaces its fixed 30-second GPU wait with these phases in the
existing cooperative-copy ledger. Whole-child reservations last from before
owner extraction through queue retirement and both allocation restorations.
Conflicting native work remains busy; logical bookkeeping and stored observers
remain available, and disjoint device pairs can progress. Pending does not
increment cooperative progress. A started copy cannot be cancelled. Success
and the native completion counter follow retirement and both restorations.

This remains an opt-in R57 qualification route, not arbitrary-kernel support.
Native selection requires initialized PUBLIC storage, equal full logical
extents within one packet and quiescent endpoints. Other copy shapes retain
host staging. No uncertain native prefix falls back to staging. Deadlines are
checked between phases, not inside topology, mapping or queue syscalls; this
does not establish hard syscall deadlines or same-VM compute/copy concurrency.

## Coverage Boundaries

Runtime tests use a scripted transport inside the production cooperative ledger.
They cover repeated Pending, zero-deadline nonpublication and retention,
observations, whole-child exclusion, pending dependencies, disjoint four-child
pairs, corrupt reservation ownership and errors or panics at all six stages.
Scripted success deliberately does not increase the native completion counter.

KFD tests exercise production persistent detach/restore and paired model-loan
sequencing with injected transfer callbacks. Eight mapped-arena tests sample
the actual CPU-visible fence with synthetic mappings and check pending/ready
custody, malformed tickets, stale generations and closing-currentness errors
or panics. They do not execute the entire native two-session map, publish,
Pending, Ready, remap and finish path as one composed test. Hardware and
complete composed native fault qualification remain required.

The updated two-GPU witness requires pre-flush poll and zero-time wait to stay
Pending. It checks four exact launches, destination sentinel replacement,
unchanged source, native completion count, 13 full-buffer readbacks and explicit
logical/native cleanup. Its two CPU tests are CLI and sentinel checks, not GPU
execution.

## Retained Corrections

Earlier attempts remain in the packet and are not promoted to final passes:

- `attempt-01/runtime`: 16 focused passes and one failure; full run has 1,945
  passes, four failures and 32 ignores. The new occupied-dependency test lacked
  three scripted scratch steps for its final staged write. The fixture now
  supplies exact Allocate/Write/Recycle steps, verifies nonzero copied bytes
  and checks remaining teardown steps. Production code was unchanged.
- `attempt-02/runtime`: all 17 focused tests pass; full run has 1,946 passes,
  three existing `InspectSocket` permission failures and 32 ignores. Subsequent
  test-only lint and formatting corrections require fresh final binaries.
- `attempt-02/checks`: strict Clippy rejects a manual divisibility check; pinned
  formatting also rejects three lower-module layouts. Equivalent test syntax
  and mechanical formatting repair those checks; preimages are retained.
- `attempt-03/checks`: strict Clippy rejects cloning a Copy generation array
  in the new sampler test. Direct copying repairs it; its preimage is retained.
- The initial source-pin proposal rejects an unaccounted whole-file SDMA hash
  before editing guards. The corrected proposal authenticates that exact hash
  update and verifies the guarded cadence/currentness function slices unchanged.

## Final CPU Results

The final combined library test build in `attempt-03/build` exits zero in
3m55s. Tests use optimization level 1 with debug assertions and overflow checks,
one build job and serial execution. These are not unoptimized-debug results.

Final runtime ELF SHA-256:
`20b0e451457850dca976c56a6d2be1b2fac327ae57b9e2a7a58e2c9476867cd2`.
The focused run passes all 17 tests, with no failures or ignores, in 0.02s.
The unfiltered run records **1,946 passed, three failed and 32 ignored** in
96.62s, exit 101. All three failures are the existing `InspectSocket` `EPERM`
at `authorized_execution.rs:1317:91`:

- `cooperative_debug_telemetry_emits_only_bounded_logical_records`
- `failed_session_end_is_explicit_and_terminal`
- `pre_native_telemetry_failure_is_returned_and_poisoned`

This is not a full-suite pass. No tests were skipped to hide those failures.
The runner's overall exit is one; both test stderr files are empty. Full
rosters and matching before/after ELF hashes are in `attempt-03/runtime`.

Final KFD ELF SHA-256:
`671170962c1b6c3c81250a44a765c8318638eae0eb38f6364c5431aed20dbe00`.
All six focused list/run pairs exit zero with no failures or ignores:

| Filter | Passed | Filtered out | Seconds |
| --- | ---: | ---: | ---: |
| `compute_xgmi` | 39 | 1,863 | 0.06 |
| `xgmi_single_poll` | 8 | 1,894 | 0.00 |
| `model_pair_loan` | 7 | 1,895 | 0.00 |
| `initialized` | 63 | 1,839 | 102.04 |
| `public_sdma` | 6 | 1,896 | 2.44 |
| `constructed_sdma_allocation_success_preserves_extents` | 1 | 1,901 | 1.62 |

These filters overlap and must not be summed into a suite total. The broad
KFD suite remains incomplete. Every roster is nonempty, every stderr file is
empty and all before/after hashes match; records are in `attempt-03/kfd`.

Final tooling in `attempt-04/checks` passes strict combined all-feature
library/test/example Clippy with `-D warnings`, the no-default runtime check,
both example tests, the normal runnable witness build, touched-Rust formatting
and whitespace checks. The no-default check retains one feature-specific
`Route::Native` dead-code warning. Clippy takes 36.95 seconds.

The witness SHA-256 is
`5000db26ecd5bc967962b013e1a33a3d843409da527d0248681589745c657167`.
It was built, not executed on GPUs. Tooling ran on final Rust bytes before the
last unit rebuild, so its `elf-final.sha256` still records earlier unit ELFs;
the final unit identities come from `attempt-03/build` and the final test
before/after hashes above. The packet's top-level `final-elf.sha256` checks
both final unit executables and the witness together after testing.

## Source Controls

The final `source-ci/attempt-05-after` passes all 32 existing workflow commands,
exit zero in 86.95024134 seconds. All 7,695 inventoried files remain unchanged
during execution, no child groups remain and no timeout occurs. It runs no
build, GPU command or solver. Prior passes and rejected proposals are retained.

Against the source base, metadata changes are exactly 19 SHA literals across
12 scripts, with no count or predicate changes. All 76 associated executable
proof files remain unchanged. The final metadata report is
`source-ci/proposal-final-04/metadata-report.json`, SHA-256
`41875bc2787041daaf04c7c577d179ed85724484f47559f9d6c50a639b4d13a7`.
These checks maintain source bindings; they do not formally verify the new
asynchronous route or refine it to machine code.

## Hardware and Remaining Work

The read-only MI300X attempt at `2026-10-02T00:35:03Z` exits 255 because SSH
cannot resolve `sharkmi300x-1`. No remote command executes, no device pair is
admitted, no GPU workload runs and no remote scratch is created. The packet
retains the exact command and failure receipt under `admission/`.

The next gate is the complete-output two-GPU native witness, followed by
composed native faults, outstanding group drain/shutdown, additional pairs,
workload partitioning and matched scaling measurements. Persistent peer
mappings and multi-packet transfers remain open. No A3 milestone exit, general
HIP/HSA parity, hardware overlap, formal refinement or speedup is accepted.

## Raw Packet

`SHA256SUMS` identifies `raw.tar.gz`. The archive includes initial and corrected
attempts, source-control inputs and receipts, preimages, scripts and source
hashes. Executables and Cargo caches are not bundled. Replay requires the
recorded toolchain and locked dependencies; source is recoverable from the
base plus `source.patch`. Archive-local historical reference files support the
recorded source audits but are not new hardware or proof evidence.

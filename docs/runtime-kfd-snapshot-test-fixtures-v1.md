# KFD Lossless Test Snapshots

## Scope

Integrated at `273b964d2` from signed candidate `1cac2b791`.

The private `MappingBytesV1` fixture stores a mapping's original length,
ordered nonzero 4 KiB chunk indices, and the exact contents of those chunks.
Capture examines every chunk, omitting it only after a full zero comparison
or retaining every byte, including a partial final chunk. The canonical
representation preserves full byte equality without sampling or digest-based
comparison; a dense reconstruction supplies an independent test oracle.

The shared-memory insertion and pristine-abort fixtures use this representation
instead of retaining mostly-zero dense copies. For the initialized gfx942 CWSR
fixture, it retains eight 4 KiB payload chunks instead of a 186,019,840-byte dense
snapshot. Capture remains linear in mapping size. Production mapping sizes,
initialization, queue behavior and native resource ownership are unchanged.

## Qualification

Five new CPU tests cover chunk boundaries, dense pairwise equality, every-byte
changes, source independence, and the actual CWSR headers. The existing cleanup
oracle and the full creation-unwind matrix remain enabled.

One ordered baseline/candidate run of that same matrix measured 305.88 seconds
and 103.49 seconds respectively, excluding compilation. This is a single CPU
fixture measurement, not a statistical benchmark, a GPU runtime speedup, or
evidence of HIP/HSA performance parity.

The pre-format candidate's complete 1,820-test KFD run finished with 1,819 passes
and one failure, with no ignored or filtered tests. All 1,304 tests in the affected
shared-memory, live-queue, dispatch-binding and SDMA groups passed; those groups
are subsets of the full run, not additional passes.

The failing test is
`target_debug_telemetry_v2::tests::credential_bound_channel_accepts_typed_failure_before_publication`.
Separate executions of both retained baseline and candidate binaries reproduce
`SocketAdmission` at the same assertion. The underlying syscall and errno are
not established by those transcripts. The complete suite remains rejected;
production socket checks and the test have not been weakened or disabled.

After module/import formatting and a comment correction, a fresh all-feature
build passes all five new tests and the existing cleanup oracle. Its actual
1,820-name test roster is unchanged. Strict all-feature/all-target KFD Clippy,
four-file formatting and whitespace checks pass. The eight stages close their
fresh process groups with unchanged source, tool and executable identities.
This final binary differs from the pre-format binary; both are retained. The
full suite and timed matrix were not rerun on the final bytes.
All 6,449 selected source/build inputs match the signed candidate. CPU execution
preceded signing; this is exact-byte evidence reuse, not a post-signing rerun.
The merged test files match that candidate exactly. The two older KFD proof
source guards include only the reviewed four-file delta, with unchanged proof
bodies, expected counts and mutation policies. No new older-proof run is claimed.

## Evidence Boundaries

The [compact evidence packet](evidence/dev-kfd-snapshot-fixtures-2026-09-30/README.md)
retains all attempt histories, exact source inventories and a minimal Git bundle
for the detached signed candidate. Its 470-member archive is byte-checked on
readback. The bundle requires baseline `5cf8266b6`; neither a complete checkout
nor the retained test executables are included. Publication has recovered:
on 2026-09-30, both remotes' `codex/r65-runtime-drain-versions` refs were confirmed
at `44c9266dfef479804cc222c3c9a528e42a788841`, which contains the packet and its
required baseline history. This is topic-branch publication, not a main merge
or an additional qualification result.

The local records are retained under
`/home/harsh/.codex-tmp/fe2o3-kfd-snapshot-qualification-20260930-audit`.
`first-pair-attempt-1` contains the ordered matrix comparison;
`follow-on-attempt-1` contains the completed, rejected full suite; and
`socket-replay-attempt-2` contains the baseline/candidate diagnostic.
`postformat-attempt-1` contains the final fresh build and focused/static gates.
The initial socket-replay classifier rejection and the earlier full-suite
timeout remain preserved as separate rejected attempts.

The earlier measurements do not become fresh executions of the revised bytes.
No native execution, machine-code proof, distributed qualification or milestone
exit follows from this test-fixture optimization.

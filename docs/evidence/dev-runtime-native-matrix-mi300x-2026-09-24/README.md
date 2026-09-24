# Current-Source MI300X Runtime Qualification

All 22 native test commands passed across two campaigns. **The original strict
matrix remains rejected**: the primary-panic immediate observer recorded 1%
GPU busy. The separate sixteen-case suffix passed. A later idle observation
does not override the earlier refusal, and no failed cell was retried.

## Source And Build

Signed source: `3540325123fa30b5208d57ba950922a707eb07b8`. A cold, locked,
offline, all-feature `fe2o3-runtime` musl library harness was built with
`nightly-2026-04-03`, two Cargo jobs and incremental compilation disabled.
The actual selected ELF ran the exact 1,385-entry CPU roster: 1,365 passed and
twenty hardware-only tests were ignored. Those twenty names form this native roster;
three prefix variants expand one test to 22 invocations. Two isolated child
envelopes add two harness frames, not two additional test commands.

The archive retains all 6,015 signed source-export files in
`raw/build/build-source.tar.gz`, the actual ELF, public signer, exact tool output,
Cargo artifact-selection output and source maps. The separate 3,956-file
prior CPU-qualified subset is checked against signed historical evidence.
Source, runner and tool identities are bracketed before and after execution.
No production Rust changes are part of this packet.

The frozen protocols' phrase "1,365-test CPU roster" refers to passing tests,
not the total roster length. Their original bytes are retained unchanged;
the full roster has 1,385 entries. The replay summary's `cpu_tests` field also
counts passes, with `cpu_hardware_ignores` recorded separately.

## Attempts

| Attempt | Result | Retained boundary |
| --- | --- | --- |
| `campaign` | Local prelaunch rejection | Fourteen protocol tests pass, then a stale topology-helper pin rejects. No owner or SSH command was created. Frozen bytes independently reproduce the pin mismatch; no historical traceback is fabricated. |
| `campaign2` | Strict campaign rejected | Six native commands and eight harness frames pass. Seventeen endpoint observations admit; the primary-panic immediate observation refuses. Delayed observation passes but cannot rehabilitate it. |
| `campaign3` | Separate suffix passes | Sixteen native commands/frames, 48 endpoint observations and all collection/cleanup steps pass. `complete_matrix=true` means only this explicitly frozen suffix. |

Both native cohorts use the same ELF, source, device, full original roster,
shared helper bytes and build environment. Only the frozen orchestration,
case-selection tests and protocol description change for the suffix. The
first six cases and remaining sixteen are disjoint and exhaust the 22 variants.

The first cohort covers two-stream dispatch, auxiliary allocation prefixes
0/1/2 and installed-primary error/panic retention. The suffix covers cold and
bootstrap generated abort/issue/roster paths, typed dispatch, allocations while
one/both compute lanes are pending, auxiliary retry, device/host capacity
rejection and retry, allocation release, device promotion, zero-cache disposal
and 256-MiB directional-copy backing refund.

GPU 1 is bound to `0000:26:00.0`, UID `0xab83d2ffef0d3cdf`, CPUs 0-47 and
NUMA node 0. This is a shared host, not an exclusive reservation. All 22 fresh
preflights admit; 65 of 66 total endpoints admit. Immediate and fixed delayed
windows are T0+[0,1] and T0+[20,21] seconds after reap and process-group absence.
The original 1% sample, subsequent zeros, SMI captures and absent selected PID
are all retained. No causal attribution to telemetry, foreign work, teardown
or a runtime defect follows from those observations.

## Replay And Cleanup

`verify.py` checks signed Git blobs and archive bytes/modes, the actual ELF,
11 build commands, the prelaunch attempt, sixteen local campaign commands,
92 remote command receipts, active/terminal custody bijections, exact test
transcripts, case-specific markers/profiles, raw observer decisions, time
windows, cohort identity joins, byte-exact collection and cleanup ordering.
The refused endpoint is replayed through the original authenticated observer,
not normalized to success. `test_verify.py` exercises rehashed malformed
records, source/ELF/helper substitution, cohort mismatch and false cleanup.
The frozen `test_protocol.py` has sixteen parser/orchestration tests.

```sh
python3 -I -B docs/evidence/dev-runtime-native-matrix-mi300x-2026-09-24/verify.py
python3 -I -B docs/evidence/dev-runtime-native-matrix-mi300x-2026-09-24/test_verify.py
```

Both marker-owned remote directories were collected, removed and independently
checked absent. Process absence is limited to recorded groups and accessible
process path/cwd observations, not inaccessible foreign descriptors. Local
collection retained 556 files plus its cleanup receipt before removing exactly
627,740,672 allocated bytes of owned scratch. The extracted source and Cargo
cache were removed; source archive, ELF and every attempt remain. `artifacts.json`
binds the 557 retained files. Audit command receipts are kept separately so
they do not recursively authenticate themselves.

## Limits And Next Work

This is native assertion evidence and strict suffix qualification, not an
accepted original full matrix. Generated fixtures bypass carrier registration;
they do not qualify protected Worker V3 application execution. Error/panic
injection is at the runtime envelope, not actual KFD ioctl or device faults.
Terminal-retention cases reclaim through isolated process exit, not ordinary
resource disposal. Pending native work is not a physical overlap measurement.

No production Rust/model refinement, aggregate memory bound, high-depth kernel
qualification, performance parity or A1/A2 closure follows. Native R125,
Admission R118B C1-C3 and Resources R116/V3 remain the broader accepted
checkpoints. Next runtime integration is producer-aware typed launches with
exact pending-input leases and success-gated reconciliation. Protected graph
execution, broader faults, aggregate accounting and matched performance remain
separate gates. Any investigation of the busy reading requires its own fixed
diagnostic protocol, not a relaxed replay of this campaign.

# R126 SDMA Recycle And Release: Development Receipt

This packet implements retained ordinary SDMA recycling/release and typed
runtime failure settlement above `7da2a6a3ec1e7ca353e3d45b2bdd7e7eafa7abd1`.
R125 remains the accepted CPU/test checkpoint. R126, A1/A2, issue #182, formal
implementation correspondence and HIP/HSA parity remain incomplete.

## Source And Ownership

`source.patch` contains the complete KFD/runtime source and test delta. SHA-256:
`d2983452ffc7c5a29b9fff433b6546fcbf28b635ef47ecc8952e3ff7ea93dc9e`.
Documentation is separate. The source stayed unchanged during the complete
regressions and native probes; executable identities are in `binary-sha256.txt`.

The shared lower driver roots the original matching-owner buffer before
admission, policy or native effects. Reservation rejection returns the exact
unchanged healthy buffer. Successful cache insertion follows reservation and
generation advancement, then removes one logical outstanding debit. Generation
overflow now preserves the original host certificate. Existing host/device
policy functions still determine whether configured pressure requires disposal;
explicit release bypasses the cache policy.

Disposal converts the rooted buffer into borrowed data-cleanup custody before
the model loan. Its original metadata and native cleanup prefix remain rooted
through model retake. Complete backing disposal can refund the backing account
before a failed retake, but the completed receipt and outstanding logical debit
remain retained on failure. Only successful complete settlement removes that
debit and root. Retake errors outrank ordinary cleanup errors; the original panic
survives secondary retake/poison panics. Inline custody avoids introducing an
allocation while transferring or returning an owner.

Foreign input returns unchanged. Healthy root-free foreign attempts still set
the existing irreversible activity latch, including on disabled SDMA sessions;
terminal or retained-root foreign attempts remain inert. A second matching input
with occupied recycle custody aborts instead of overwriting, dropping an owner,
or falsely reporting retryability through the legacy result type. Public guards
cover SDMA enable, allocation, checkout/trim, queue release and Drop.

Runtime input is rooted before driver selection. Native failures cross the
adapter as typed errors, and returned custody is rooted before diagnostics.
Indexed healthy recovery restores only the original-kind synchronous placeholder
and returns Quiescent; transient recovered and ambiguous outcomes remain terminal.
Successful release remains the final logical allocation/accounting/profile commit.
The scripted driver retains ambiguous/panicking input in its own root, rather
than simulating a release, so tests observe exact lower ownership.

## CPU Evidence

Nine constructed tests start with genuine freshly allocated host/device mappings.
The transaction fixtures retain their original directional pair and use real
model loan/reclaim. Public guard/Drop ingress instead transfers genuine buffers
to engine-less public session shells; those probes intentionally never enter
native cleanup or model loan/reclaim. They prove guard and failed-admission
retention, not successful constructed-parent public admission. One additional
unit test checks generation-overflow certificate preservation. Covered cases include:

- 17/4097-byte cache, configured cache, zero-capacity disposal and explicit
  release; exact backing refunds, generation and outstanding accounting.
- Existing cached neighbors, healthy reservation rejection and successful retry.
- Admission/policy failures and panics, quarantine, underflow and generation
  exhaustion, with original mapping and account observations.
- A 120-case cleanup/retake/secondary-poison matrix, including actual model
  revision regression and completed-but-unsettled disposal receipts.
- Forty-four injected native-operation/currentness/host-projection failures and
  panics, with exact cleanup metadata, native records and account prefixes.
- Foreign public recycle/release attempts on healthy and terminal sessions;
  initially empty disabled/terminal roots, inert public guards, second-input
  collisions and Drop in core-disabled subprocesses.

Nine scripted runtime tests cover exact owner/neighbor bytes and identities,
actual authenticated certificate preservation, device scrub/demotion retry,
independent wrong-kind and occupied-slot rejection, driver selection and lower
panic, typed diagnostic panic with original payload identity, indexed
placeholders, public Context error classes and credit quarantine, and terminal
retries that preserve both roots and indexed shadow allocation addresses.
Configured Context retains eight requested bytes, one record and one quarantined
record after terminal failure. The unconfigured facade seals on its next valid
backend call following panic; the backend is already terminal.

Final complete library regressions pass on the unchanged source:

| Toolchain | Crate | Passed | Failed | Ignored | Duration |
| --- | --- | ---: | ---: | ---: | ---: |
| GNU | KFD | 1,344 | 0 | 0 | 1,118.86 s |
| GNU | Runtime | 789 | 0 | 6 | 24.78 s |
| musl | KFD | 1,344 | 0 | 0 | 1,842.97 s |
| musl | Runtime | 789 | 0 | 6 | 31.79 s |

The six ignored runtime tests are opt-in hardware probes, exercised separately
below in eight process invocations. The lower GNU/musl runs overlapped on the
same host; durations are provenance, not a performance comparison. Nineteen
focused tests and strict all-feature/all-target Clippy pass. Final formatting
and no-default-feature runtime compilation pass. Unsafe-source policy passes
five tests with its explicit inventory-maintenance test ignored. This is not
full-workspace, compiled-negative, formal-proof or full R126 qualification.

Commands:

```sh
prlimit --core=0:0 -- cargo test --locked --offline -p fe2o3-kfd -p fe2o3-runtime --all-features --lib -- --test-threads=4
cargo test --locked --offline -p fe2o3-kfd -p fe2o3-runtime --all-features --target x86_64-unknown-linux-musl --lib --no-run
prlimit --core=0:0 -- <musl-kfd-test> --test-threads=4
prlimit --core=0:0 -- <musl-runtime-test> --test-threads=4
cargo clippy --locked --offline -p fe2o3-kfd -p fe2o3-runtime --all-features --all-targets -- -D warnings
cargo check --locked --offline -p fe2o3-runtime --no-default-features
cargo fmt --all -- --check
cargo test --locked --offline -p cargo-fe2o3 --test unsafe_source_policy
```

The focused development history retains a fixture API compile failure, incorrect
completed-receipt and host-projection test expectations, a test observation helper
using the wrong allocation-table iterator, and the initial large-inline-custody
Clippy failures. Those failed logs are not passing evidence. Earlier passing
focused runs predate the final foreign-attempt and assertion changes. The last
nineteen-test focused pass precedes the final inline-custody lint annotations;
the complete regressions qualify the final frozen source. Exploratory formatting overlapped
two early builds; those are not final-source qualification. Some preliminary
inspection failures were not archived, so this is not a complete attempt inventory.
Two independent bounded reviews cleared the final lower driver and strengthened
runtime assertions. Reviews do not substitute for execution or formal proofs.

## Native Evidence

The stripped musl runtime executable SHA-256 is
`086875a68409621f391568e408a56bf781134fce9aa1039a4c87d59a86123b0f`.
The completed upload hash matches. `remote-binary-sha256-inflight.txt` records an
earlier observation made before SCP completed and is excluded. No probe ran until
SCP exited successfully and the completed remote hash matched the local binary.

All eight probes passed in separate sequential processes on MI300X GPU 1,
unique ID `ab83d2ffef0d3cdf`:

- The new zero-cache probe configures both host and device cache limits to zero
  before resources. Public allocation/write/release and handled internal native
  download verify 17 logical bytes on 4096 physical bytes and 4097 on 8192.
  Device backing falls from 12,288 bytes/two records to zero before trim, with
  no reserved/retained/quarantined device remainder. Cached buffers/bytes and
  subsequent trim count are zero. Host backing is control-only at 532,480
  bytes/three records before shutdown, then zero after retained primary release.
- The default-cache roundtrip retains four cached buffers and 12,288 device
  backing bytes/two records before trim, then refunds backing after trim.
  Both roundtrip probes validate their eight-event public runtime histories.
- The HostVisible allocation/shutdown, primary and two-stream vecadd regressions
  pass. Vecadd checks three/six readbacks and 22/41-event profiles.
- AUX budget rejection after zero/one/two initialized owners retains exactly
  38,281,216/42,475,520/46,669,824 backing bytes and 12/13/14 records until process
  exit. These three probes are not successful native-cleanup observations.

Patterned readback SHA-256 values:

- 17 bytes: `cca448791d4bcee8fe07acb2b42c1ea727893454bf2d47cfbd43b644d9c6fd76`.
- 4097 bytes: `7d9f8ee9ea61461d10b2b71c408cb7153390786855259a9ded5872aa87b514e3`.
- Vecadd: `79fd0768604fe9de0ced87297f7d653343e998926b59e2cce7df5e38194c52b3`.

The patterned read uses the internal native download helper and requires its
handled result; it is not additional public read/profile coverage. This campaign
does not inject native recycler faults, demonstrate copy/compute overlap or
compare performance with HIP/HSA. Host timings in profiles are observations,
not matched-workload benchmark results.

Every remote invocation used an exact test name, `prlimit --core=0:0`,
`--ignored --nocapture --test-threads=1`, the explicit device ID and
`timeout --signal=TERM --kill-after=10s 120s`. AUX cases additionally selected
`FE2O3_TEST_NATIVE_INITIALIZED_PREFIX=0`, `1` or `2`.

## Cleanup And Remaining Work

Only the stripped executable was uploaded to
`/tmp/fe2o3-r126-recycle.6KBBML`. After all probes exited zero, an anchored
process query returned status 1, confirming no matching executable. The exact
file and empty directory were removed; absence is recorded in
`native-cleanup.txt`. The local stripped copy was also removed; no executable
is archived. GPU 1 reports zero use/VRAM percentage before and after. GPU 0 had
unrelated existing VRAM use (46% at the recorded preflight, 44% afterward);
no reset, service stop or unrelated deletion was performed.

Next is retained directional demotion, then synchronous-copy phase ownership
through prepare/publish/wait/retire and typed healthy backing-credit rejection.
Allocation during pending compute remains disabled until those prerequisites and
genuine borrowed directional-owner preflight are complete. Generated
DATA-ADOPT/ISSUE/COMPLETE, readback/typed replies, Stop/drain/graphs, production
Context/resource proofs, other queue profiles, native faults, aggregate memory,
multi-device/distributed behavior, device-language/collective refinement and
matched HIP/HSA performance remain separate requirements of the full objective.

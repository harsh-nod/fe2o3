# M1 Reference Enrollment: CPU Checkpoint

2026-10-08. Ordinary developer validation only, not protected compiler admission,
proof execution, GPU execution, M1 completion, or 47-kernel qualification.

## Tested Source

- Commit: `a10c4b0c2711d70c6bca9304f76dc1c85d760a1d`.
- Tree: `73b92a938a9e93c2645cc2647f2d322e9d54a42c`.
- Pinned toolchain: `nightly-2026-04-03-x86_64-unknown-linux-gnu`.
- Host: MI350; CPU affinity 20/21, nice 10, 64 GiB aggregate memory ceiling,
  swap disabled, 256 tasks, one test thread, bounded systemd units.
- Source snapshots before and after each successful stage matched
  `f597431c74516ef8908eeb4a17e1dbf396ea837f2b8e56f904c3d5fa76f66d8c`.
  This is the recorded tracked-file mode/type/content snapshot, not a sealed
  whole-host or continuous isolation claim.

The joined source includes original-invocation enrollment loans and identity
checks, actual rustc source resolution/custody, original CPU-origin projection,
strict explicit-origin codecs, and the shared inert enrollment-request decoder.

## Actual Results

| Stage | Result |
| --- | --- |
| Default test-binary build, both libraries | PASS, 1.10 seconds, cached artifacts, no CLI profile override |
| All `fe2o3-rustc-invocation` library tests | 108 passed, 0 failed, 0 ignored |
| Focused `rustc-codegen-fe2o3` tests | 129 passed, 0 failed, 0 ignored, 4629 filtered |
| Non-test production-library check, both libraries | PASS, 45.99 seconds |

The compiler selection covers enrollment resolution, source custody, cumulative
work/storage boundaries, CPU-origin projection, ten isolated admission/loan flows,
native ownership refusals, source-reference obligations, and CPU effect replay.
Exact discovered names and selections are retained with the run. The other 4629
compiler tests were not executed in this checkpoint.

The build and check commands were:

```sh
cargo test --locked --offline -p rustc-codegen-fe2o3 -p fe2o3-rustc-invocation --lib --no-run --message-format=json-render-diagnostics
cargo check --locked --offline -p rustc-codegen-fe2o3 -p fe2o3-rustc-invocation --lib --message-format=json-render-diagnostics
```

The actual Cargo-produced binaries were then executed with exact discovered test
names, `--exact --test-threads=1 --format=json -Z unstable-options`. No extra
test-build profile override was supplied. Cargo's committed test profile optimizes
only `sha2@0.11.0`; debug assertions and overflow checks remain enabled.
Production profiles and the 60-second fixture / 120-second native deadlines
were not changed.

Here "default" means the committed profile without a CLI optimization override,
not a pristine Cargo environment: the builds retained `CARGO_PROFILE_DEV_DEBUG=0`,
`CARGO_PROFILE_TEST_DEBUG=0`, `CARGO_INCREMENTAL=0`, and `CARGO_BUILD_JOBS=1`.
The ordinary production check used the dev profile, where SHA remained opt-level 0;
it does not prove an unoptimized production backend meets admission deadlines.

## Preserved Failures

The preceding `60c2e5bc52e10e8cefe4c5d077d0cef78d76681e` run completed with
114 passes and 9 failures: three generic-CGU fixture failures and six child
timeouts. The original logs, terminal receipt, and executable were preserved.

The generic fixture requested metadata-only output and found no concrete generic
instances. It now requests object generation but still stops after analysis;
an assertion checks that no object was emitted.

Timeout observations showed sustained CPU use and repeated full-image reads.
The longest flow performs 30 fresh image checks of a roughly 533 MB test binary.
A separately recorded test-only SHA optimization experiment passed all focused
cases without extending deadlines or caching/skipping verification. Its profile
setting was then committed and the normal build/tests repeated successfully.
This is not a GPU or compiler performance benchmark, nor a recovered stack trace
proving the precise location of every earlier timeout.

Two existing native self-exec fixtures print six plaintext child-report lines.
The initial JSON-only result parser rejected that mixed stream. The supplemental
readback retains those lines and all 123 original parent results. Later reports
explicitly validate the known child diagnostics and exact parent test-name sets;
no failing run was relabeled as passing.

The immutable supplemental report's explanation says it ignores only blank lines.
That wording is inaccurate: its separately retained `bad` list also contains the
six human-format child-report lines. Its parent results and failed-suite counts
were independently checked against the original raw stream.

## Evidence

Remote evidence root:
`mi350:/home/harmenon/fe2o3-m1-integration-20261007.WWdApdE1/evidence/`.

| Receipt under that root | SHA256 |
| --- | --- |
| `r1903-focused-tests-20261008-1912/terminal.json` | `7d6f4e7d9cae05533234b12fcbe1730b8c4c7a686f6bed5a25781bfaaa1b9e11` |
| `r1903-focused-tests-20261008-1912/result-readback.json` | `b4cfe60a348613d529177f12d8d468f11ddd8f7ed2bff84faf5f9aee69d83cbc` |
| `r1908-default-profile-testbuild-20261008-1935/terminal.json` | `8b10cf207b65782814ff82a4745a3282a6e9d91e67052f7f32e34f15d3490d88` |
| `r1909-default-focused-20261008-1936/fe2o3_rustc_invocation/terminal.json` | `037db1c5177e965639d47962f5fbf2079520f8b4aa16d7282779fa03cb55b986` |
| `r1909-default-focused-20261008-1936/rustc_codegen_fe2o3/terminal.json` | `8f253d0de08252e1b4bc4e5a3586e48eb897abe744c368f8fdb9180da019b410` |
| `r1910-production-check-20261008-1938/terminal.json` | `23307e055714da82439cb2c6b5fc6512c0b5d6b85882d4da3907e233b40c0ea7` |

Actual compiler test executable SHA256:
`379c7062201a9215979eb071151cb24f11afae960f685a7418d4ffa3ccf44b5e`.
Invocation test executable SHA256:
`71283c04e6a57b5047ae21f32be5ef359fa4bf17e0c77685fa9bc2d87bb325e2`.
Default builds reproduced the already-discovered binaries byte for byte.
CLI exits, observed process/cgroup absence, boot identity, resource checks,
artifact pins, and original source snapshots are retained in the receipts.

## Remaining Gates

The isolated loan fixtures measure their real test executable, not an independently
installed production compiler/backend. Their process-consistency successes do not
establish protected compiler provenance or successful proof publication.

An authenticated selector/Instance-to-semantic-root association must still cross
the signed compiler handoff. Host and proof-worker consumers need original-account
raw Subject reconstruction, complete mapping recovery, and paid decoder storage.
Genuine signed cross-process tests, broader compiler compatibility, the protected
default-fill path, KernelContext typed-global vecadd, and supported GPU runs remain
open. Shared decoder values and explicit expected-origin inputs remain inert.

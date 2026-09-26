# Worker V3 Current-Thread Application Development

Status: scoped CPU development qualification. The broad suites have environment
failures; this is not complete CPU or native application acceptance.

Base: `89434c39eb051115a57280bf9fd8093acbd29a18`.

This change adds a one-shot inherited application transaction requiring the
refining verifier adapter. It composes the existing authenticated handoff,
generated-only KFD backend, version-journal Context, current-thread owner,
preparation/reservation/activation, original typed completion, stream disposal,
drain and owned shutdown. No new native or verifier authority is created.

The report keeps completed data, the primary error, drain and shutdown distinct.
Timeout is cooperative and selects Stop, not cancellation or rollback. Returned
Context-construction failure retains the backend until process exit; there is
no fabricated owned-shutdown report before an engine exists. Initializer and
caller-destructor panics retain the lower APIs' existing unwind contracts.

Sealed output bundles now cover zero through 64 observers. Empty selection does
not assert a zero-output invocation or eliminate storage/readback accounting.
It still requires the original completion receipt. Singletons share the original
all-or-nothing slot and gate checks.

The production verifier/refinement providers and proof artifacts are not supplied
by this helper. CPU tests use the actual current-thread command engine with an
empty non-Send backend, real charged-output storage, and explicitly scripted
observer metadata. They do not execute the private protected generated carrier
through the public application entry point and do not establish native execution,
sandbox composition, end-to-end refinement, performance or full HIP/HSA parity.

## Results

All Cargo commands used `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1` and `--offline`.
Library tests ran serially. Raw logs retain failures rather than excluding them.

| Gate | Result | Log |
| --- | --- | --- |
| Default host library | 183 passed, 2 failed, 0 ignored | `raw/host-library.log` |
| All-features host library | 269 passed, 33 failed, 4 ignored | `raw/host-all-features-library.log` |
| Default doctests | 28 passed (6 plus 22), including refining-adapter compile rejection | `raw/host-doctests.log` |
| Strict all-features/all-targets Clippy | Passed with `-D warnings` | `raw/clippy.log` |
| No-default-features check | Passed | `raw/minimal-check.log` |
| Restored current-thread driver/preflight tests | 5 passed | `raw/restored-driver-tests.log` |
| Restored bundle tests | 16 passed | `raw/restored-bundle-tests.log` |
| Formatting and whitespace | Passed | `raw/format.log`, `raw/whitespace.log` |
| Reply-retention mutation | Compiled; rejected by the intended cell-count assertion | `raw/mutation-retain-reply.log` |
| Source restoration | All seven source hashes match | `raw/restored-source-check.log` |

Coverage overlaps; these numbers are not additive. Ten test groups are new.
The driver regression uses real generic Context commands and one retained reply
with a two-cell budget. Its stage names do not turn those commands into generated
preparation/reservation/activation. The bundle tests exercise real charged slots
and the existing shared observer plumbing; scripted metadata is not a successful
runtime receipt. No successful protected application or `execute` transaction is
claimed by these CPU tests.

Both default failures occur in unchanged compiler-current-record tests when
descriptor admission returns `EPERM`. The all-features suite adds 30 direct
`EROFS` publication-fixture failures and one failed subprocess assertion. Running
that child explicitly with `FE2O3_TOKEN_AWARE_REVALIDATION_CHILD=1` reproduces its
`EROFS` failure at the same publication fixture (`raw/legacy-child-diagnostic.log`).
No permission exception, relaxed assertion, or production fallback was used.

The mutation deliberately forgets the consumed finite future after driving it.
The two-cell test then observes two occupied cells rather than one. The mutation
was removed, `raw/source-sha256.txt` checked, and both focused groups rerun. This
tests reply disposal, not native allocation accounting or formal correspondence.

## Reproduction

From the repository root, prefix each Cargo command with the environment above:

```sh
cargo test -p fe2o3-host --lib --offline -- --test-threads=1
cargo test -p fe2o3-host --all-features --lib --offline -- --test-threads=1
cargo test -p fe2o3-host --doc --offline
cargo clippy -p fe2o3-host --all-features --all-targets --offline -- -D warnings
cargo check -p fe2o3-host --no-default-features --offline
cargo test -p fe2o3-host --lib --offline production_application::current_thread::tests -- --test-threads=1
cargo test -p fe2o3-host --lib --offline generated_runtime_results::completion_tests::bundle_tests -- --test-threads=1
cargo fmt -p fe2o3-host --check
git diff --check
```

The source manifest binds the tested production/test files to this packet's
signed commit. It is not an authenticated Verus or native execution receipt.

## External Gates And Cleanup

MI300X still fails hostname resolution (`raw/mi300x-connectivity.log`), so no
remote job or artifact was created. Approximately 2.8 GiB of this worktree's
rebuildable `target/debug/incremental` cache was removed before qualification;
other worktrees and `/tmp` artifacts were left alone. The unrelated pre-existing
owner-inspection evidence directory was preserved.

Production verifier/refinement providers, owned proof artifacts, successful
inherited-handoff/native sandbox execution, post-activation fault campaigns and
matched HIP/HSA measurements remain required. No new Verus campaign was run.
Ordinary application examples remain disabled. Accepted lane checkpoints and
A1/A2/#182/full-parity status are unchanged.

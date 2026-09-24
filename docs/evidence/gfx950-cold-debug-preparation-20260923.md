# Inactive gfx950 debug preparation — 2026-09-23

One supervised disposable process on mi350 completed **real KFD VM acquisition and kernel/trap mappings**, retained version-zero metadata and exited. Seven separately supervised argument/artifact/device refusals also matched their exact expected phases. This is inactive native preparation, not trap installation/execution, runtime enable, metadata publication, queue creation, kernel dispatch, stopped-wave observation or explicit native cleanup acknowledgment. V4 remains open; no original milestone checkbox is closed by this result.

The [public contract](../gfx950-cold-debug-preparation-v1.md) specifies ownership and retention. The [walkthrough](https://github.com/harsh-nod/fe2o3-kernels/blob/main/docs/gfx950-cold-debug-preparation-v1.md) explains the opt-in command. This qualification is separate from the earlier [read-only artifact/device companion](gfx950-checked-artifact-20260923.md).

## Qualified boundary

The engineering-only owner consumes a genuine checked gfx950:xnack- device, immutable owned ELF bytes and one kernel name. It uses the normal COV6 loader and actual allocation/mapping path. An exclusive process gate is exposed before VM acquisition. After that boundary, errors, unwind and Drop retain native resources together until process exit and poison that reservation; no retry, close or activation API is offered.

The positive used explicit node 2, GPU ID 39903, unique ID 16366993098680759275 and device-profile SHA-256:

`6f859b0a67f8ee2497393206930ff35bf1a9ae69d33f172106a790a0c9226667`.

Those are measured host selections, not portable defaults. Checked-device currentness fences and exact final artifact/trap/file comparisons passed. They do not establish continuous currentness, atomic filesystem authentication or general reset/ABA resistance.

| Observed item | Bytes | SHA-256 |
| --- | ---: | --- |
| Cold preparation example executable | 3526520 | 1dc684d9135f6783ed4d399fa2a66681f073923ec72fb191365d332ef5fb5634 |
| Diagnostic gfx950 native artifact | 5536 | d10b592732d91cf4c0d4289890fdd0a5328e84cb8208817eaabac4c62c1a34c6 |
| Retained trap text | 1116 | 4ffea893ee53e018a629c19254855721882444517155251f1f45dd3519bc81fe |
| Actual preparation stdout | 1009 | 5bb4e69b34c26616217717499bef9153245f2688eda4bce5c487dc9e629ef5f1 |

The selected kernel was `fe2o3_gfx950_observation_fixture`. Its earlier native artifact was not regenerated or relabeled as authenticated Rust source. The owner reported **16384 logical backing bytes** and **5744 logical retained metadata bytes**, metadata version 0, target gfx950:xnack- and Wave64. These counts are not RSS or allocator-capacity measurements.

The fixed trap mapping was copied, CPU-read-only protected and read back with its zero-filled tail. No trap registration or execution occurred. CPU-read-only does not remove GPU write permission. The retained upstream source/license and unqualified TMA/PC-sampling execution assumptions remain explicit in the public contract.

The positive direct child exited 0, with an observed close event, empty stderr, no supervisor signal attempts and no abandoned stream drain. This establishes the observed process exit, not explicit KFD release acknowledgments or a descendant-quiescence proof. The output correctly keeps `cleanup_acknowledged:false`. The generic task receipt retains `hardware_observed:false`; no dispatched GPU behavior is claimed.

## Build, tests and lint

The passing build gate used base commit `0d16955476ed608bf40bb92394b39ffb42b2cb3c`, pinned nightly 2026-04-03 and the root's bounded offline/locked runner. It passed:

- No-default-feature KFD library: **444 passed, 1 ignored**.
- Engineering-gfx950 KFD library: **570 passed, 1 ignored**.
- Ordinary cold-preparation example: **6 passed, 0 ignored**, then built.
- Strict no-deps all-target KFD Clippy in both feature configurations, followed by `git diff --check`.

These are separate configurations, not a sum of distinct tests. The existing Cargo duplicate-target warning remained; no Rust warning was allowed through the strict KFD gates. This does not change the separately recorded merged-tree model Clippy findings.

Executed command sequence, under the runner's pinned compiler environment:

~~~sh
cargo test --offline --locked -p fe2o3-kfd --lib --no-default-features
cargo test --offline --locked -p fe2o3-kfd --lib --features engineering-gfx950
cargo test --offline --locked -p fe2o3-kfd --features engineering-gfx950 --example observe_gfx950_cold_debug_v1
cargo build --offline --locked -p fe2o3-kfd --features engineering-gfx950 --example observe_gfx950_cold_debug_v1
cargo clippy --offline --locked --no-deps -p fe2o3-kfd --all-targets --no-default-features --message-format=short -- -D warnings
cargo clippy --offline --locked --no-deps -p fe2o3-kfd --all-targets --features engineering-gfx950 --message-format=short -- -D warnings
git diff --check
~~~

The passing build and native preparation/refusal gates shared the unchanged source census: 7,116 files / 106,721,855 bytes, SHA-256 `e2bb531758496808b4a0590afb7e56011f72277e497de3c20062a278fe0f6392`. This is a measured source tree, not the eventual publication commit. No independent mirror rebuild or full browser rerun is claimed.

Publication adds the walkthrough/evidence and removes one trailing space in an upstream assembly comment, caught when the new file was staged for whitespace checking. The original offline-build input pin and normalized repository-source pin are both preserved in the public contract; the executable trap byte array and all compiled Rust sources are unchanged. No whitespace or Rust lint suppression was added.

## Exact native invocation and refusals

The positive's argv after the selected executable was:

~~~text
--allow-vm-mapping --retain-until-process-exit
/home/harmenon/fe2o3-authoring-280-282-mi350.4VZ42zNr/phase28-gfx950-native-artifact-r1/fixture.hsaco
5536
d10b592732d91cf4c0d4289890fdd0a5328e84cb8208817eaabac4c62c1a34c6
fe2o3_gfx950_observation_fixture
2
16366993098680759275
39903
6f859b0a67f8ee2497393206930ff35bf1a9ae69d33f172106a790a0c9226667
~~~

The supervisor imposed 30 seconds on that call. Each negative below ran in a fresh process with a 20-second direct-child limit, rechecking the exact unchanged executable/artifact and retained positive pins. All seven exited 1, emitted one bounded refusal with empty stderr, and reported `native_preparation_effects:not_attempted`. An earlier refusal was not credited as a later one.

| Change from positive | Exact phase / reason |
| --- | --- |
| Omit mapping acknowledgment | arguments / closed_grammar_and_explicit_effect_acknowledgments |
| Expected size 5535 | artifact_file / expected_size_mismatch |
| All-zero expected artifact hash | artifact_pin / sha256_mismatch |
| Kernel `missing_cold_probe_kernel` | artifact_admission / normal_kernel_selection_refused |
| Node 3, retaining selected unique ID | device_selection / exact_identity_or_profile_mismatch |
| GPU ID 39904 | device_selection / exact_identity_or_profile_mismatch |
| All-zero device-profile hash | device_selection / exact_identity_or_profile_mismatch |

Device-selection refusals may open/check owned device descriptors, but occur before entering VM/mapping preparation. The normal loader rejects the absent kernel before device opening. Refusal after actual cold-owner entry has a different conservative possible-retention classification; this campaign does not claim to exercise that native failure.

The seven negative executions each recorded post-exit SIGTERM/SIGKILL attempts reporting `group_absent`; they are not described as having no signal attempts. Direct exit/close events and completed drains were observed. Explicit driver cleanup and generic descendant quiescence remain unproved.

## Retained history and pins

Paths below are relative to the task root. Failed runs remain retained:

- Build r1 passed the test suites and example build, then engineering all-target Clippy rejected an immediately invoked closure in a new retention test. A lexical block preserved the assertion; build r2 reran the full sequence and passed.
- Refusal helper r1 stopped after its first child because underscore case labels violated its own literal-output-leaf rule. No negative suite success was claimed. Immutable helper r2 used hyphenated labels and a fresh output directory, then passed all seven.

| Receipt | Bytes | SHA-256 |
| --- | ---: | --- |
| logs/phase28-resume-r6-compiler-cold-debug-build-r1/receipt.json (failed) | 13016 | e1bbdeb4f8c7f9f18762e10f25d5dfaacf1d69f7f19ff323bc8a11d7f95c2040 |
| logs/phase28-resume-r6-compiler-cold-debug-build-r2/receipt.json | 20987 | d9656d1e9b7cc04d1cb62c495dd04a4d433e8d17da3fb838e72c29effb6289eb |
| logs/phase28-resume-r6-compiler-cold-debug-actual-r1/receipt.json | 23991 | 9124adf3d96419562c9edf7d81850752e30219fef3d4d2e9be51bbebc357522d |
| logs/phase28-resume-r6-compiler-cold-debug-refusals-r1/receipt.json (failed) | 15863 | 3c148b3574031bb3b13faae56cfd4050129090b0157ce12b2b003e70d7f607f2 |
| logs/phase28-resume-r6-compiler-cold-debug-refusals-r2/receipt.json | 27097 | 98c0b98651fcf14fab442c87af33d7d64a0886f0c5eadb29ae533a08cc1d40f4 |
| phase28-cold-debug-refusals-r2/report.json | 8037 | e4ef615e6aac202c22dc5ff8c01a512f0b24e58c36ec5974aec5a4bd95edecbc |

Helper r2 is 5,520 bytes, SHA-256 `0466d30f5db44293c3b375dac7a329de832221c75a84207a293983da90d2b9e4`. Independent read-only review rehashed the positive executable/artifact/request/streams and all seven report→execution→stream joins, and reviewed the actual producer/retained-file helper. That review did not execute the project or replace the root's runs.

No protected finalizer, source-proof route, public curriculum capability or compiler pin changes follow. Positive hardware register cells still require compatible trap/runtime publication, an actual stopped-wave producer, exact correlation and qualified teardown/retention semantics.

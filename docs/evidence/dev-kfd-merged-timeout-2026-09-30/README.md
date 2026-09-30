# Incomplete Merged KFD Regression

Development attempt on September 30, 2026, at signed source commit
`12782867c81e7d0a4de328355618e84f5feb25c8`. This is an incomplete run, not a passing
KFD qualification, native execution, HIP/HSA parity or performance evidence.

## Invocation And Result

```sh
cargo test --offline --locked -p fe2o3-kfd --all-features --lib -- --test-threads=1
```

The command uses pinned nightly `2026-04-03`, two build jobs, test optimization
level 1, debug information disabled, and the existing target cache. The exact
sanitized environment, absolute invocation, tool hashes and selected build-input
inventory are in `inputs.json`. Cargo reports a 3 minute 21 second rebuild.

The executable announces 1,824 tests. There are 716 completed `ok` lines and no
final libtest summary before the 1,800-second command limit. The partial log ends
during `constructed_sdma_creation_native_attempt_faults_retain_exact_mutated_arguments_and_resources`;
that location is not evidence that this test failed or caused the timeout.
The earlier completed KFD attempt used two test threads and a 3,600-second limit,
so its elapsed time is not a matched performance comparison.

The owner records `TimeoutExpired`, child status `-9` and successful closure of
its process group. The wrapper exits 1. Its generic final exception text does
not mean closure failed: `result.json` records unchanged sources, tools and PID
namespace, and `recorded_group_closed: true`, alongside the timeout error.
These are original development recorder observations, not an independent
process census or a historical/host-wide absence claim. There is no automatic
retry and no ignored or weakened test added by this attempt.

## Retained Artifacts

`raw.tar.xz` contains the exact command input and closing inventories, recorder
sources, result, owned command receipt and partial stdout/stderr. Original
absolute paths are historical metadata, not portable replay instructions. The
archive excludes the checkout, build cache, temporary directory and executable.
Readback checks all eight members against the retained originals. Archive
SHA-256: `e1f1daf764488d04701e990b731764bf409b93928b85e5ec3fa539bb6a22b176`.

The separately retained ELF is 34,639,296 bytes, SHA-256
`e138da8d67f347ae7dae942ccaa7b775f2f9e008c0056fb48b7d99ac8e0d8e1a`.
It was copied out of the mutable build cache after the owned command terminated.
Its identity supports future explicit diagnostics, not a successful test claim.

Result SHA-256:
`109a234b8d12437877ebc06fc1aefcac3ee832d77de196683210d475b4d2581b`.
Partial stdout SHA-256:
`768f37714a6bc11332f56e1e1ad4e129af36e52245465f85d8edac5dcc382165`.

All A0-A7 milestone exits remain open.

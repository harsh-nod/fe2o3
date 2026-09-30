# Retained KFD CPU Regression

On September 30, 2026, the retained KFD test executable passes all 1,824 library
tests: zero failures, ignored tests or filtering. This is a fresh execution of
the executable built at `12782867c81e7d0a4de328355618e84f5feb25c8`, not a fresh
build of the current branch or the later host-diagnostic candidate.

## Invocation And Result

```sh
retained-kfd-12782867c.elf --test-threads=2
```

The command has a 1,800-second bound. Its original owner records a start at
19:56:44 UTC and completion at 20:10:38 UTC, status zero and process-group
closure. Libtest reports 819.97 seconds. All 1,824 terminal `ok` names are unique
and exactly match the separately collected `--list` roster; stderr is empty.
Source, tool and PID-namespace continuity checks pass. These are development
recorder observations, not an independent process census or a host-wide absence
claim. Elapsed times are not a matched performance comparison.

Before the full run, a separate exact invocation of
`target_debug_telemetry_v2::tests::credential_bound_channel_accepts_typed_failure_before_publication`
passes one test with 1,823 filtered. This does not explain the failure of that
test in an older executable/environment. The separate listing, focused test and
full regression each have their own command receipt and closed process group.

The [earlier one-thread timeout](../dev-kfd-merged-timeout-2026-09-30/README.md)
remains incomplete and is not reclassified as successful. Its build and timeout
records supply the retained executable's development provenance. No test was
removed, ignored or weakened for this run. The command's recorded checkout HEAD
is `f1be6340c8e01ed671bd6fe8d82d04bbca49cc92`; that is its execution context, not
the executable's build source. Later producer-validation and benchmark-source
changes are not covered by this retained executable.

## Retained Artifacts

The 34,639,296-byte executable remains separately retained, outside the mutable
target cache. Its SHA-256 was checked before launch and after completion:
`e138da8d67f347ae7dae942ccaa7b775f2f9e008c0056fb48b7d99ac8e0d8e1a`.

`raw.tar.xz` contains the original eight files from each of the three commands:
input and closing inventories, recorder sources, result, owned command receipt,
stdout and stderr. It excludes executables, checkouts, caches and temporary
directories. Absolute paths are historical metadata, not portable replay
instructions. Archive readback checks every file byte and mode against its
original. Archive SHA-256:
`80003a8faccbf86377a5329c7914f5e3aca2b16afb13a899d3a237524aae0ad7`.

The full-run input inventory SHA-256 is
`13ec869441118ea4a5c452e1332258d09982c49ce600249ab968a2278a714832`;
stdout is `3e6664e92ba6ff14c5145fc3fd7b9d00798d73459a895f5d243101e8a121ccca`;
the owned receipt is
`e00ee418d605f5fd99d15e706d695c44ddc40ab989a29bd3eff841cb72d5ab35`.
The generic result JSON is identical across successful commands and must not
be used without these command-specific joins.

This result does not establish native execution, formal verification, HIP/HSA
parity, performance acceptance or any A0-A7 milestone exit.

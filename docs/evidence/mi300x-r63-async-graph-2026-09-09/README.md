# R63 Runtime-Bound Graph Qualification

Result: two guarded MI300X runs passed. This qualifies the local copy-graph
profile, not all #182 acceptance, compiler admission, distributed execution,
whole-executor formal verification or HIP/HSA performance parity.

## Exact Source And Capture

- Signed hardware source: `517f90f79ad4057a373a649658accc14fac5cf1d`.
- Branch: `codex/r63-runtime-graph-v1`.
- Full capture: `/home/harsh/.codex-tmp/fe2o3-r63-hardware-517f90f7.tar`.
- Capture SHA256: `7c6ab804e149885fa30437e3b16f1bb3fce7e2e172454458df6bc69b4ce1fb48`.
- Binary: 4,553,648 bytes; SHA256
  `76a13091d62dcccb42b6be8ae6fb7aa0ad652a4be65240a5c7c2a01f3c548566`.
- Source archive SHA256:
  `f9fb1c7337df86503e33d4c30a689eba3824a076922dc7def24a3d7bd004314e`.

The read-only capture audit checked all 73 manifest payloads, all 32 command
exit codes, exact output for both runs, the trusted SSH signature, signed commit
bytes and all 6,448 source files against a fresh Git archive.
The signer trust anchor was supplied independently from the capture:
`SHA256:q8oGVYZ11904aFzlMkSiEwyeSP+6hbuiZGbNVGRZVCg`.

`raw/musl-accepted` retains the original compact evidence unchanged, omitting
only `source.tar` and `owner-binary`; its original `sha256.json` still lists
both omissions. The full capture remains at the path above. The additional
`retained-files.sha256` covers files actually retained in this directory.
The subsequent evidence commit changes no hardware-qualified runtime source.

## Hardware Result

The deny-all compute authorizer prevents this example granting executable
authority. The graph has four streams, 12 logical nodes, five native copies and
host-side event joins. Two independent branches use disjoint native allocation
custody; the join overwrites only half of the right output before readback.

Each run checks the exact graph identity, all 12 successful node states, exactly
five successful native observations, unchanged inputs, both branch patterns and
every output/padding byte. Every stream has zero remaining submissions, followed
by explicit complete owner/native shutdown. Exact output:

```text
PASS schema=fe2o3.runtime.r63-async-graph-copy.v1 bytes=1048832 owner_threads=1 streams=4 nodes=12 copies=5 joins=host canaries=complete submissions=released cleanup=complete
```

Selected GPU: index 1, UID `0xab83d2ffef0d3cdf`, PCI `0000:26:00.0`,
KFD GPU 23018/node 3. Worker CPUs 0-47 and memory NUMA node 0; observer CPU 95.
The 2ms queue census recorded maximum gaps of 5,167us and 6,054us, below the
unchanged 10ms gate. Both runs had zero foreign/terminal selected queues, exit
code zero, reaped children and absent process groups.

The private offline two-job release build used the pinned nightly toolchain,
static musl target and the exact BFD link command. Cargo/ELF/full-symbol closure
audits passed: 5,036 symbols, no unresolved symbols, interpreter, dynamic
dependencies or GNU minimum-stack lookup. No dependency or census gate was relaxed.

The independent post-run SSH check confirms removal of
`/dev/shm/fe2o3-r63-owner.eeLI11k2` and target PIDs 2480180/2480244. GPU 1
returned to zero VRAM usage. GPU 0's foreign 44% allocation was left untouched.
No resets, fault injection or all-GPU reservation were performed.

## CPU And Proof Results

- GNU and musl: **1,931 passed, 5 ignored, 44 harnesses each**, covering runtime,
  runtime-model, KFD and completion with all features/all targets.
- GNU and musl doctests: **63 passed each**.
- Clippy with warnings denied, formatting and source/document diff checks passed.
  Verbatim raw evidence retains tool-emitted whitespace.
- R60/R61/R62/R63 runner tests: **32 passed**.
- Authenticated Verus suite: **52 positive sources, 1,284 verified obligations,
  608 expected-negative rejections**; the 190-file toolchain closure matched.

R63 adds eight abstract reservation-guard obligations and eight negative
mutations. They do not prove token privacy, ledger conservation, scheduling,
GPU completion truth or the entire executor. See the
[runtime graph contract](../../runtime-async-graph-v1.md) for these boundaries
and the remaining #182 roadmap.

Raw local logs and independent capture/cleanup audit scripts are in `raw/host`.

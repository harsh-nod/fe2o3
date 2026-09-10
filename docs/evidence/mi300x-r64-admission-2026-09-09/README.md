# R64 Payload Admission And Executor Validation

Result: CPU/executor regressions, authenticated arithmetic proofs and two guarded
MI300X copy-graph regressions passed. This advances local A1/A2 work for #182; it
does not complete the distributed execution plane, establish end-to-end memory
bounds, prove the executor or demonstrate HIP/HSA performance parity.

## Exact Source And Capture

- Signed implementation and hardware source:
  `2389df9060469dfec6f0043f9f660f9eb56f7285`.
- Branch: `codex/r64-runtime-admission-executors`.
- Full accepted capture:
  `/home/harsh/.codex-tmp/fe2o3-r64-hardware-2389df90.tar`.
- Capture SHA256:
  `7a1eec8047f935e91c6acf652356061014e48447e2e3699b0846078b9b2ad2a8`.
- Source archive SHA256:
  `d6510092044929c8c1a84e6b73b18e9e4e3cbf62e0f7bbac10700359d9d4f76d`.
- Binary: 4,558,392 bytes; SHA256
  `395d80daae23942e30317390e74c1112a0476d5ef50f5824591f656a8c36b089`.

The primary and read-only swarm capture audits checked all 73 manifest payloads,
all 32 command exit codes, both exact PASS records, the trusted SSH signature,
signed commit bytes and all 6,560 source files against a fresh Git archive.
The signer trust anchor was supplied independently from the capture:
`SHA256:q8oGVYZ11904aFzlMkSiEwyeSP+6hbuiZGbNVGRZVCg`.

`raw/musl-accepted` retains the original compact capture unchanged, omitting only
`source.tar` and `owner-binary`. Its original `sha256.json` still lists those
omissions. The full capture remains at the path above. The outer wrapper logs
are in `outer`; local validation logs and audit/wrapper scripts are in `raw/host`.
`retained-files.sha256` covers all 127 retained raw files. This evidence commit
does not change the hardware-qualified implementation.

## Implemented Scope

Frozen standalone launch requests evaluate argument getters once on the caller
thread, own compact payload slices and enter the existing owner-side validators.
A shared byte budget follows the retained payload until its actual disposal;
dropping or cancelling an observer does not refund a still-retained payload.
All six legacy standalone launch/copy/peer APIs also bound dependency lists,
reject duplicate identities and compact the lists before enqueue. Their argument
evaluation remains on the owner thread. Budget exclusions, additional bounded
validation scratch and the distinction from native resource retirement are
explicit in the
[admission contract](../../runtime-async-admission-v1.md).

Terminal graph reports include a context-bound local occurrence identity minted
from the private reservation generation. Repeated identical graphs receive
different generations. This is neither a data version nor a distributed epoch
and grants no execution authority.

Dev-only integration tests exercise Tokio current-thread and multi-thread
executors, genuine non-Send LocalPool observation, initially pending owner
wakeups, task abortion and real timer recovery of the same queued operation.
An independent watchdog-expiry flag prevents a timeout wake from masking a lost
owner wakeup. No executor dependency enters the production runtime closure.

## CPU And Proof Results

| Validation | Result |
| --- | --- |
| GNU all features/all targets, four crates | 1,948 passed; 5 ignored; 44 harnesses |
| musl all features/all targets, four crates | 1,948 passed; 5 ignored; 44 harnesses |
| GNU doctests | 64 passed; 0 ignored; 4 harnesses |
| musl doctests | 64 passed; 0 ignored; 4 harnesses |
| R60/R61/R62/R63 runner regressions | 32 passed |
| Clippy, all features/all targets, warnings denied | Passed |
| Workspace formatting and source/document diff checks | Passed |
| Production Cargo closure | 42 packages; 8 permitted build scripts |
| Authenticated Verus suite | 53 positive sources; 1,292 obligations; 616 expected-negative rejections |

The four tested crates are runtime, runtime-model, KFD and completion. Seventeen
new focused runtime/model tests cover budget contention, payload destructor
ordering, queue/registry rejection, cancellation, backend error/panic, frozen
encoding and shape limits, occurrence isolation and named-executor behavior.
Raw logs preserve tool-emitted whitespace.

R64 adds eight abstract arithmetic obligations and eight targeted negative
mutations. The overflow mutation uses wrapping addition on valid machine-domain
inputs. Production CAS updates call the corresponding checked Rust helpers;
machine-width tests and code review validate that correspondence. These are not
proofs of atomic linearization, permit ownership, threading, GPU completion or
the whole executor. The pinned 190-file, 129,019,839-byte verifier toolchain
closure and every pre/post authentication gate passed.

An initial verifier invocation was explicitly interrupted to strengthen the
overflow mutation; its log is retained separately and is not counted as proof
success. The complete fresh invocation in `raw/host/r64-verus.log` passed.
Three read-only swarm reviewers checked implementation, proof/authentication
inputs and hardware evidence. Their findings were resolved before final runs.

## MI300X Regression

The unchanged guarded R63 runner built the signed R64 source offline with the
pinned nightly, static musl and exact BFD link command. The copy-only example
retains its deny-all compute authorizer. It now additionally checks the graph
occurrence's context, structural identity and nonzero generation.

Both runs passed the four-stream, twelve-node, five-native-copy diamond, with
complete input/output/padding canaries, successful native retirement and
explicit complete shutdown. Each emitted exactly:

```text
PASS schema=fe2o3.runtime.r63-async-graph-copy.v1 bytes=1048832 owner_threads=1 streams=4 nodes=12 copies=5 joins=host canaries=complete submissions=released cleanup=complete
```

Selected GPU: index 1, UID `0xab83d2ffef0d3cdf`, PCI `0000:26:00.0`,
KFD GPU 23018/node 3. Worker CPUs 0-47 and memory NUMA node 0; observer CPU 95.

| Run | Root PID/PGID | Census Samples | Target Queue Observations | Maximum Gap |
| --- | --- | --- | --- | --- |
| 0 | 2662753 | 1,291 | 1,576 | 6,019 us |
| 1 | 2663128 | 1,284 | 1,571 | 5,134 us |

Both sealed 2ms censuses passed the unchanged 10,000us gap gate, with zero
foreign/terminal selected-device queues, exit zero, reaped children and absent
process groups. Five topology records agreed; four boundary telemetry checks
reported 0% selected-device GPU utilization and 0% VRAM allocation.

Independent inspection of the actual ELF matched all 5,042 unique full symbols and
found no undefined/dynamic symbols, dynamic dependencies, interpreter,
GNU minimum-stack lookup or Tokio symbols. The production Cargo closure was
unchanged at 42 packages and eight permitted build scripts.

This hardware evidence covers the copy-DAG regression and occurrence checks.
It does not hardware-qualify Tokio/LocalPool execution or nonzero frozen launch
payload budgets, exercise compute, measure overlap or compare HIP/HSA speed.

## Rejected Attempt And Cleanup

The first remote attempt stopped before GPU execution because its private
offline Cargo home lacked the existing pinned `pliron` Git checkout. The wrapper
was corrected to copy the existing Git cache privately; no network fallback or
acceptance gate relaxation was used. Its compact original diagnostics are in
`raw/rejected-offline-git`; only `source.tar` is omitted. Full rejected capture:

- `/home/harsh/.codex-tmp/fe2o3-r64-hardware-2389df90-rejected-offline-git.tar`
- SHA256: `8e34e10eed19332583d2515d9beb00dcd186ac48ede53d79652843db6efdc8d3`

The independent SSH cleanup check confirms removal of both private stages,
`/dev/shm/fe2o3-r64-owner.m0FdGlkA` and
`/dev/shm/fe2o3-r64-owner.NENnw7p5`, GPU child PIDs 2662753/2663128 and wrapper/
runner PIDs 2651377/2653117. The selected GPU reported 0% VRAM allocation. GPU 0's
foreign 44% allocation was untouched. The newly needed executor crate remained
absent from the shared Cargo cache; all staging stayed private. No resets, fault
injection, all-GPU campaign or second-host execution was performed.

## Remaining #182 Acceptance

Still open: compiler-authenticated #134 plans and #214 refinement inputs; exact
producer versions, invalidation, residency and measured overlap; complete
end-to-end/native memory admission and drain qualification; integrated multi-GPU
placement, shards, replicas and group quiescence; authenticated two-host
membership, publication receipts, transport and distributed collectives;
authorized reset/partition/recovery campaigns; and precommitted all-GPU/two-host
performance gates. No second GPU host or all-GPU maintenance window has been
provided. The local tests and arithmetic proofs grant none of that missing
distributed authority or qualification.

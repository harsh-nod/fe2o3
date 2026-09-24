# gfx950 native runtime-observer qualification — 2026-09-24

This is a completed **historical no-queue observation**, not a reusable runtime
acceptance, live stop, physical-register capture or GPU-dispatch capability.
The public source package is [separate GPL debugger tooling](../../tools/rocgdb-runtime-observation-v1/README.md);
its private fixed-host controller and process-scope harness are not portable core
APIs. No queue or kernel dispatch was introduced.

## Actual native relation

The fresh controller observed one inferior (PID 2550268), native process 1,
producer generation 1, one code object and six native records. It independently
checked the entry executable/start identity, object absence before runtime
activation, object presence after publication, the producer registration record,
and the original 5,536-byte ELF at the same owned host stop.

| Sequence | Native observation |
| --- | --- |
| 1 | Attach, same inferior/process/generation |
| 2 | Runtime event 1, kind 5, state 1, successful acknowledgment |
| 3 | Callback 1 / breakpoint 1 / thread 1, action 1, success |
| 4 | Code-object event 3, kind 3, callback 2, success |
| 5 | Callback 2 / breakpoint 1 / thread 1, action 2, successful acknowledgment |
| 6 | Terminal lifecycle disappearance, reason 14 |

The actual MI stream separately contained `*stopped,reason="exited-normally"`
before the terminal record. Reason 14 alone does not establish normal exit:
kill and failed-attach cleanup can also mourn an inferior. Explicit GDB detach
remains reason 13. Four CPU negatives reject killed/signalled exit, missing
normal exit, early attach rollback and explicit detach.

The observer process was not reaped by the controller itself; its pidfd exit
was observed, GDB was reaped as a direct child, reader threads joined and streams
completed. The independent credential-preserving scope joined controller PID
2549320/start 50926800 to client 2549309 and owner 2549308. Inner and outer wait/
reap/ECHILD, EOF, exact scope emptiness, current owner, request digest and terminal
ACK all passed; neither cleanup deadline expired and no kill attempt was needed.
Manager emptiness alone was not used as cleanup proof. Cleanup is not rollback.

## Source, build and startup

The producer package's three patches start from ROCgdb
`48b1d324e389d2ed5e19822d377ff9050770233d`. A separate fresh checkout applied each
patch with `git apply --check`, checked all intermediate selected-file digests,
and reproduced the actual build's seven final postimages. No complete debugger
tree or binary is vendored. The package's strict C++ controls passed 20+23 cases;
its source/MI/lifecycle controls passed 11+7+17 Node cases.

The actual debugger ELF was 198,563,552 bytes, SHA-256
`389a26fa47e7171b18970c6841136ae0dbfa0c8137762c93a115a8d960618a73`.
The fresh startup gate checked 78 generated data files and the source/build
chain; all five benign process-cleanup cases passed. The startup snapshot
contained 104 initial Python modules plus 8 collector-added modules, 50 mapped
ELF paths and 193 post-observed file records (170 present, 23 absent).
System Python site hooks were present, not disabled. This is not complete
import history, cache-execution provenance or complete MI runtime dependency
coverage.

The fixed controller passed 98 Rust controls, strict Clippy and a fresh build.
Its separate scope passed 30 Rust and 67 Node controls before the one actual
attempt. Artifact, static-candidate, observed-startup and historical-benign
prerequisites matched before/after. The actual debugger emitted one index-cache
directory warning; it did not weaken the event, object or cleanup checks.

## Retained evidence

All hashes below identify retained files, not signatures or exported authority.

| Evidence | Bytes | SHA-256 |
| --- | ---: | --- |
| Source-package report | 26,628 | `0cb9ce29df144b08adf7ce96ebec3724c9a2518449bd3f6e8f74a37769ef4765` |
| Source-package completed gate | 123,671 | `c5b0bc33982fb2dcf78a7321c4e6537e75a80295406f4cc0712f189f037b40ae` |
| Fresh startup completed gate | 131,466 | `05129f7f581dd746cbbe22b586513da27da3377e354a6a45dee017fb3e7d2679` |
| Controller CPU completed gate | 373,785 | `0e3fd125bf6767070b02c9efd97113c33373b4287ed3e51d6e93d55cc1e5f75f` |
| Scope CPU completed gate | 301,891 | `6521e53d87b0c2835de6f42d9aee07c40813460b8d9b149bac06a515b486a2c5` |
| Actual runtime summary | 1,961 | `1135f95cb29621fde651b97594a18e458dc23e1f14f0a6c4adbf9e8ddce1466f` |
| Actual runtime observation/transcript | 47,824 | `1d73de8da3b67c5f36d69038501fbf7d3e63313470c670c23cd3eb2e06dce907` |
| Actual completed root gate | 340,001 | `67943cd116aeb8f50c44c23344345f1ab76c1ad8bb3520a79e39821e922c6219` |

The successful fixed attempt nonce is `3c8c921f28a78b887b6910bb07f22afd`.
Earlier failed scope attempts remain failed. In particular, the old producer's
internal teardown recorded detach before normal exit; the corrected producer
classifies lifecycle cause at actual callers. No consumer rule was relaxed to
accept reason 13 as normal disappearance.

## Still unavailable

This result does not expose physical VGPR/SGPR/AGPR/EXEC values, GPU memory
snapshots, hardware wave state, a live browser adapter, target performance
predictions, source authentication, protected finalization or launch admission.
The old observer artifact is independent of the authored gfx942 LDS example.
A successful runtime metadata observation does not execute that kernel, discharge
its runtime conditions or close V4/M4/M6. Accepted original exits remain
M1/V1/V2/U1/U2/U3: 6/18.

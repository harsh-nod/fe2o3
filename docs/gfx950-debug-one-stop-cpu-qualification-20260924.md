# gfx950 one-stop target CPU qualification — 2026-09-24

This is CPU/static qualification, not a GPU stop, capture or launch permission.
The earlier empty-queue native result dispatched no kernel; it does not qualify
this packet-capable target or a new debugger executable.

## Implemented owner path

The engineering-only API consumes the original cold owner into a privately
boxed fixed one-stop preparation, the exact prepared packet, one in-flight owner
and local completion. It cannot be reconstructed from JSON, caller addresses,
Booleans or an empty-queue owner. Failure retains unresolved native custody and
possible-publication facts rather than retrying or claiming cleanup.

The fixed artifact is validated byte-for-byte and against its descriptor, ABI,
segments and 84-byte instruction sequence. A ninth allocation holds a 272-byte
logical output with 8-byte canaries on both sides of 64 XOR lane results. The
packet is published once with release ordering and one doorbell. The original
60-second clock survives the prepublication checkpoint, any debugger pauses,
completion, retirement and final descriptor close. Queue/event/metadata/runtime/
trap/doorbell/allocation retirement remains ordered; late failures retain truthful
closure facts. These implementations and pure controls are not observed ioctls.

The 776-byte checkpoint is owned by the same preparation. Its actual symbol is
`fe2o3_gfx950_one_stop_prepublication_checkpoint_v1`. Unsafe callers still owe
same-client pre-runtime attachment, actual LoadedSuccess and sole ACK, reviewed
trap/CWSR/TTMP setup, sampler exclusion, source/PC/resource joins, and consumed
native pre-resume gates. None of those facts comes from the checkpoint bytes.

## Completed CPU/static gates

The combined gate passed 2,057 test executions in 18 Rust result groups, including
638 engineering KFD tests, the separate fixed-HSACO bit-mutation test, 52 KFD
doctests, 444 default KFD tests, AQL and kernel-analysis suites, selected strict
all-target Clippy and the unsafe-source inventory. Configurations overlap.
Its receipt is
`logs/phase28-resume-r13-compiler-one-stop-repeated-calls-cpu-r3/receipt.json`,
25,917 bytes, SHA-256
`5b40adfaf8c6b68c7e6da997d7ac3b536c339e8ab39e36dc0d56d81624b20a54`.
Its 7,817-file source census is
`915c19f8cae83729f9ea61952378d161c50f0f17011762d9148b6dbae03bee3a`.

A separate root-bound private observer source passed 21 Rust controls, 12 Node
controls, strict Clippy, CPU build and readelf checks. Both the first-main entry
marker and real target-owned checkpoint exist; no dummy/dead-code substitute was
used. The target executable was **not invoked**, nor was any debugger or scope.
The 4,162,064-byte PIE has SHA-256
`1ec72e9d53df21a510089951a6bba9a0167d8b7e2323e3dcd1d1ec5add3f3bdb`.
Its entry/checkpoint symbol offsets are 0xc7800 and 0x19daf0 respectively; these
are ELF observations, not runtime addresses or a ready debugger profile.

The completed outer observer-build receipt is
`logs/phase28-resume-r13-compiler-one-stop-observer-cpu-r1/receipt.json`,
77,108 bytes, SHA-256
`7261b67fece74456763f360a16b88e12a9486d5fad0938ec56054536cbab6531`.
The inner `phase28-gfx950-one-stop-observer-tools-r3/built-cpu.json` is
3,533 bytes, SHA-256
`07e0c6be9f50149b96abfea5b64332636f47afd42f507417e9cfbba0b10d0fb4`.
Its source census `aef0567360fb6cb3cd588bef8cb5e49a79a453ea02eced32a27c343bac71743f`
predates two unrelated kernel-analysis test-only lint corrections. Receipts retain
that distinction rather than claiming a rebuilt executable on the later tree.

Fourteen pure GDB source-placement controls and a separate strict-C++ standalone
owner-state control program passed. Their receipts are respectively SHA-256
`2b984fe29cd7c221f3f3775b0e73ddf70dc9d3fd7f6e191339d33676c91186ec` and
`7a62b698c2ce5d215897ed4e3d696de154ea73bf85787cb2f62eb2050c799967`.
These private GPL increments are not yet a qualified native producer. Real
post-breakpoint-deletion retirement, bounded API adapters, generic/AMD resume
hooks, exact actual identity joins, loader/currentness checks and owned-family
cleanup still require integration and fresh qualification. Physical samples and
visualization data remain unobserved here. V4 and broader milestone exits remain
open; no global tutorial pin or support claim changes.

# rustc host-link handoff V1

This directory specifies the isolated compiler-to-static-linker handoff implemented by
`fe2o3-host-link-closure`. It is a foundation protocol and is not integrated with `cargo-fe2o3`.
The crate grants neither tool approval nor broker, publication, or runtime authority.

The compiler publishes every producer through `PublishedHostArtifactV1`, then creates a
`HostLinkHandoffV1`. The handoff is one canonical sealed plan plus retained sealed producer
descriptors. Reopening `target/` or any other diagnostic pathname is not an accepted transfer.

After decoding `HostLinkPlanV1`, the broker prepares `HostLinkClosureV1`. Launch is safe only when
the broker also supplies a move-only `ApprovedStaticHostLldV1`. Minting that capability is an
explicit unsafe authority boundary: the future W1 broker must first verify evidence for the exact
plan-bound tool ID, SHA-256, size, mode, release nonce, target, plan digest, and LLVM build identity.
This crate validates the binding but does not decide that a tool is trusted.

`HostLinkClosureV1::launch(approval)` uses `clone3(CLONE_PIDFD|CLONE_CLEAR_SIGHAND)` to obtain the
pidfd atomically, holds the child behind an exec-status handshake, installs only stdio, result FD
91, and input FDs 100+, normalizes inherited signal state, applies the no-descendant seccomp
filter, and executes the exact sealed tool descriptor with `execveat(AT_EMPTY_PATH)` and an empty
environment. There is no public child-socket accessor and no `finish_child_handoff` API.

Result polling is nonblocking and has a fixed 30-second wall deadline. Admission is a resumable
state machine: one public poll copies or hashes at most 256 KiB, performs at most 64 fixed-size
validation operations, and targets a cooperative wall check every 10 ms between operations. The
10 ms value is not a hard latency bound: scheduler delay and one kernel syscall can exceed it.
The byte and operation caps are hard per-poll work bounds. The absolute deadline is checked before
and after every cooperative work interval,
so a late 128 MiB or 512 MiB result cannot be synchronously copied or admitted after expiry.
Timeout and `Drop` send
SIGKILL through the pidfd and return without a blocking wait. One process-wide event loop retains
pidfds until nonblocking `waitid(P_PIDFD)` succeeds. At most 64 live executions/deferred reaps are
accepted; exhaustion fails closed. API return is bounded, while kernel reap is eventual rather
than claimed synchronous.

The static tool owns its output memfd, links directly into it using LLVM 22
`--mmap-output-file`, validates and seals it, and sends it over the result socket. The closure
never treats that sender-owned descriptor as the admitted artifact: it copies exact bytes with
positional reads into a fresh receiver-owned memfd, sets mode 0555, seals and revalidates it, then
returns the move-only `AdmittedHostOutputV1`.

The closure owns the wall deadline. The static tool currently owns its separate 60-second CPU
rlimit; this crate neither installs nor treats that tool-local limit as closure authority.

ELF parsing is an independent bounded V1 grammar rather than a permissive object-parser success
check. Inputs admit ordinary x86-64 ET_REL sections, GNU 32-bit archive indexes/long names, Rust
COMDAT groups, notes, symbol tables, REL/RELA, and `SHT_X86_64_UNWIND`. Compressed sections, CREL,
unknown section kinds, GNU `/SYM64/`, and BSD archive indexes/extended names are intentionally
unsupported. Outputs admit only the documented static ET_EXEC section subset and validate every
section mapping, note, and symbol entry incrementally.

No output directory, user namespace, ambient output path, COMGR, DSO input, plugin, dependent
library section, or LTO cache participates in this protocol. See [protocol-v1.md](protocol-v1.md)
for the exact wire grammar and limits.

Run the isolated contract and hostile suite with:

```bash
toolchains/rustc-host-link-handoff/test-v1.sh
```

An optional cross-lane identity hook can check a separately built static tool without changing
that lane:

```bash
FE2O3_HOST_LLD_COMBINED_TOOL_V1=/absolute/path/to/fe2o3-host-lld \
FE2O3_HOST_LLD_EXPECTED_SHA256_V1=<64-lowercase-hex> \
FE2O3_HOST_LLD_EXPECTED_LLVM_BUILD_IDENTITY_V1=<exact-build-id> \
toolchains/rustc-host-link-handoff/test-v1.sh
```

The hook checks the executable digest and `--fe2o3-identity-v1` contract. A full real-link
closure/tool round trip remains an integration test for the static/build lanes; this branch does
not claim it.

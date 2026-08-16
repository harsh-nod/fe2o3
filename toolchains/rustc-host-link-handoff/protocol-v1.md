# Host-link static-tool contract

## Authority and launch

`ApprovedStaticHostLldV1` is a move-only launch capability. Its only constructor is an explicit
unsafe/trusted boundary and binds the plan digest, tool artifact ID, SHA-256, size, mode, release
nonce, target, and LLVM build identity. `HostLinkClosureV1::launch` requires and revalidates that
exact capability before executing the plan's sealed `static_host_lld` descriptor.

The launcher uses `clone3(CLONE_PIDFD|CLONE_CLEAR_SIGHAND)`, an exec-status pipe, and
`execveat(AT_EMPTY_PATH)`. The child receives an empty environment and only canonical stdio,
result FD 91, and input FDs 100+. Caught/ignored dispositions and the signal mask are normalized
before exec, and the child cannot create descendants after the seccomp boundary. No safe API
launches a merely labeled `StaticHostLld` artifact, exposes the result socket, or asks a caller to
finish descriptor handoff.

The API wall deadline is 30 seconds. Each admission poll copies/hashes at most 256 KiB, performs
at most 64 fixed-size validation operations, and targets a cooperative wall check every 10 ms
between operations. The 10 ms target is not a hard scheduling or per-syscall latency guarantee;
the byte and operation caps are the hard per-poll work limits. The absolute deadline is checked
around every cooperative work interval. Timeout, terminal failure, and `Drop` use pidfd SIGKILL and
never perform a blocking reap. One shared event loop retries nonblocking pidfd `waitid`; EINTR and
all other wait errors retain the pidfd and consume capacity. Live executions plus deferred reaps
are bounded at 64. Return is bounded; process disappearance/reap is eventual.

## Canonical argv

Arguments are bounded printable ASCII and ordered exactly as follows:

1. `fe2o3-host-lld`
2. `--fe2o3-host-lld-elf-v2`
3. `--fe2o3-result-socket-v1=91:<dev_decimal>:<ino_decimal>`
4. `--fe2o3-request-v1=<plan_sha256>:<closure_sha256>:<nonce_sha256>`
5. Semantic options and typed inputs in original linker order

Each input is one argument:

```text
--fe2o3-input-v1=<fd>:<kind>:<sha256hex>:<size_decimal>:<mode_octal>
```

`kind` is exactly one of `elf-rel`, `archive`, or `rlib`. There is no `elf-dso` V1 input. Input FDs
start at 100. Repeated semantic inputs repeat the typed record at that position and reuse the same
FD. Bare proc-fd inputs, raw `-L`/`-l`, response files reaching the tool, alternate spellings, and
unknown options are rejected.

The stable closure digest is computed before result/socket identity and request nonce controls are
inserted. It binds the plan, fixed-root and authenticated procfs/mount identities, root tree
digests, retained artifacts, semantic argv, fixed result FD, and input assignments. Every
preparation obtains a fresh nonzero 256-bit request nonce with Linux `getrandom`.

## Input policy

Direct objects and all archive/rlib members must be x86-64 ELF64 little-endian ET_REL. Raw
bitcode, active embedded bitcode, `SHT_LLVM_DEPENDENT_LIBRARIES`, `.deplibs`, nested/thin/external
archives, path-shaped member names, malformed members, plugins, and LTO are rejected. An inert
`.llvmbc` section is accepted only with `SHF_EXCLUDE` and none of `SHF_ALLOC|SHF_WRITE|SHF_EXECINSTR`.
`lib.rmeta` is accepted at most once, only in an rlib, and must itself parse as ET_REL. Every
archive must contain a non-metadata linkable object.

The accepted ET_REL section grammar is finite: NULL, PROGBITS, SYMTAB, STRTAB, REL, RELA, NOTE,
NOBITS, INIT/FINI/PREINIT_ARRAY, GROUP, SYMTAB_SHNDX, and `SHT_X86_64_UNWIND`. Section zero,
alignment, nonoverlap, notes, symbols, local/global partitions, extended indexes, groups, and
relocation target offsets are validated structurally. Compressed sections, CREL, dependent-library
sections, and all unknown section types are rejected. Regular archives admit canonical GNU 32-bit
symbol tables and GNU long-name tables after validating every member offset/name boundary. GNU
`/SYM64/` and BSD symbol/extended-name encodings are intentionally unsupported in V1.

The closure independently parses all of this before launch; the static tool repeats validation.
Fixed-root inotify paths use a retained authentic procfs (`PROC_SUPER_MAGIC`) and retained mount
namespace and `/proc/self/fd` identities. Those identities are checked before/after watch install
and revalidation. The trusted initial mount namespace is a broker assumption. Inotify is only a
mutation signal; descriptor-relative content snapshots and tree digests remain authoritative.
Retained namespace identities detect substitution but do not freeze the namespace's mount table;
the broker must isolate and serialize mount-table mutation in that trusted namespace.

## Exact limits

| Resource | V1 limit |
| --- | ---: |
| Plan bytes | 4 MiB |
| Plan semantic arguments | 4,092 |
| Static-tool total argv | 4,096 |
| Bytes per argv element | 4,096 |
| Aggregate argv bytes | 1 MiB |
| Producer/unique input descriptors | 2,048 |
| Bytes per input | 256 MiB |
| Aggregate retained/link-input bytes | 2 GiB |
| Archive members per archive in closure parser | 8,192 |
| Archive member-name bytes per name | 1,024 |
| Aggregate archive-name traversal per archive | 1 MiB |
| Cumulative archive members across unique closure | 262,144 |
| Program headers per admitted executable | 1,024 |
| Sections per ELF input/output | 8,192 |
| Entries per ELF symbol/relocation/group/note/index table | 1,048,576 |
| Output bytes | 512 MiB |
| Concurrent live/deferred executions | 64 |
| Closure wall deadline | 30 seconds |
| Admission bytes per poll | 256 KiB |
| Admission operations per poll | 64 |
| Cooperative wall-check target | 10 ms (not a hard latency bound) |
| Static-tool-owned CPU limit | 60 seconds (outside closure authority) |

Bounds are checked incrementally before append, copy, or launch. The 2 GiB aggregate usually
dominates before large counts are reached.

## Result packet

The result channel is nonblocking `AF_UNIX SOCK_SEQPACKET`. The parent authenticates the endpoint
identity and exact direct child. The worker sends exactly one packet with exactly one SCM_RIGHTS FD
and this newline-terminated printable ASCII record:

```text
fe2o3-host-lld-result-v1\tplan=<hex>\tclosure=<hex>\tnonce=<hex>\tsha256=<hex>\tlength=<decimal>\tcopy=receiver-owned-memfd-v1\n
```

Hex is lowercase and 64 characters. Decimal has no leading zero. The record is at most 512 bytes.
The worker then shuts down writes. `EAGAIN` remains `result-pending` and does not consume the
channel. A zero-length datagram carrying credentials or any ancillary data is a packet, never
transport EOF. Only an ancillary-free zero-length receive after peer write shutdown is EOF. A
dequeued malformed packet poisons the channel terminally. Admission requires worker success,
true write EOF, and no second packet or trailing SCM data.

The sender FD must be an owner-matching zero-link shmem/memfd with exact
`WRITE|GROW|SHRINK|SEAL` seals, record-matching bytes, and a valid static output. Sender mode is not
wire authority. The receiver copies with `pread` into its own memfd, canonicalizes mode 0555,
seals, hashes, reparses, and retains only that receiver-owned descriptor.

Actual admitted bytes must be x86-64 ELF64 little-endian ET_EXEC with no PT_INTERP, PT_DYNAMIC,
DT_NEEDED, RPATH/RUNPATH, writable-executable load, or executable stack. This hard policy is
checked independently of the caller's expected profile. The output section subset is NULL,
PROGBITS, SYMTAB, STRTAB, NOTE, NOBITS, and INIT/FINI/PREINIT_ARRAY. Section zero is exact;
file/address alignment, table/body nonoverlap, SHF_ALLOC-to-PT_LOAD mappings, NOTE records, and
symbol entries are validated incrementally. Compressed, CREL, dynamic, extended-index, and unknown
output sections are rejected.

## Non-goals

V1 has no dynamic-loader closure, DSO input, Cargo broker durability, publication authority,
runtime authority, COMGR path, output pathname, or user-namespace dependency. The combined
identity hook checks a separately built static tool's measured contract; full closure/tool/build
integration remains outside this isolated branch.

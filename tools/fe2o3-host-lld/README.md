# fe2o3-host-lld

`fe2o3-host-lld` is the dedicated static GNU ELF host linker for the W0 host
link closure. It embeds pinned upstream LLVM/LLD 22.1.8 and calls
`lld::lldMain` in-process. It never invokes another linker, a shell, COMGR, a
dynamic loader, or a DSO.

This is not the GPU linker. GPU code-object linking remains in the existing
in-process `fe2o3-llvm-link-worker` path.

## Request protocol

The only link protocol is `--fe2o3-host-lld-elf-v2`. The closure passes
resolved, immutable capabilities rather than pathnames:

```text
fe2o3-host-lld --fe2o3-host-lld-elf-v2 [bounded GNU ELF flags] \
  --fe2o3-result-socket-v1=91:DEVICE:INODE \
  --fe2o3-request-v1=PLAN_SHA256:CLOSURE_SHA256:NONCE_SHA256 \
  --fe2o3-input-v1=100:elf-rel:SHA256:SIZE:0644 ...
```

Input descriptors start at 100. A repeated descriptor is allowed at multiple
semantic positions only when its complete canonical record is byte-identical;
the descriptor is measured once and every occurrence remains in LLD's argv.
Conflicting duplicate metadata is rejected.

The three W0 input kinds are:

- `elf-rel`: parsed x86-64 `ET_REL`.
- `archive`: a non-thin regular archive containing only parsed x86-64
  `ET_REL` members.
- `rlib`: the same regular-archive rules, with at most one `lib.rmeta` member.
  `lib.rmeta` must itself parse as x86-64 `ET_REL`, so it cannot enter LLD's
  linker-script content detector.

The tool rejects bare proc-fd inputs, response files, scripts, thin or nested
archives, external archive members, raw bitcode members, malformed members,
plugins, LTO flags, unresolved `-L`/`-l`, and unknown top-level content. Archive
walking uses LLVM's fallible structured `Archive` API.

Every direct `ET_REL` and every regular archive or rlib member is rejected when
its numeric section type is `SHT_LLVM_DEPENDENT_LIBRARIES`, regardless of the
section name or string-table validity. This validation completes before LLD is
called. As defense in depth, the tool also forces `--no-dependent-libraries`
internally and rejects both caller spellings of the dependent-library policy.
The identity record exposes `dependent_libraries=forbidden`.

Current Rust sysroot rlibs contain ordinary `ET_REL` members with embedded
`.llvmbc` sections. Those sections are accepted only after the containing
member parses as x86-64 `ET_REL` and only when `SHF_EXCLUDE` is set while
`SHF_ALLOC`, `SHF_WRITE`, and `SHF_EXECINSTR` are all clear. This permits inert
Rust metadata used by real host links without permitting raw bitcode, an LTO
plugin path, or active bitcode-bearing sections.

Every unique input must be a mode-bound, owner-bound, zero-link memfd with the
exact `WRITE|GROW|SHRINK|SEAL` seal set. Permission write bits do not grant
mutation after those seals are installed. Size, SHA-256, file identity, mode,
timestamps, and seals are checked before LLD and rechecked after it returns.
All inherited descriptors other than stdio, result FD 91, and declared inputs
are rejected.

The empirically sized W0 limits are 4,096 arguments, 4,096 bytes per
argument, 1 MiB of argument text, 2,048 unique inputs, 256 MiB per input,
2 GiB across unique inputs, and 262,144 members cumulatively across all
archives. Archive members are counted while they are parsed; crossing the
cumulative bound fails before LLD starts. `--fe2o3-identity-v1` reports these
limits as machine-readable fields.

## Result protocol

For a link request, the tool first replaces FD 0, 1, and 2 with private
duplicates of a verified `/dev/null` character-device descriptor, before any
operation that can emit diagnostics. This removes stdio aliases to inputs, the
result socket, pipes with blocked readers, and attacker-controlled diagnostic
sinks. In `main()`, it uses the Linux x86-64 kernel `rt_sigaction` ABI to reset
signals 1 through 64 to default, excluding only uncatchable `SIGKILL` and
`SIGSTOP`. This deliberately includes kernel realtime signals 32 and 33, which
the libc `SIGRTMIN` range and `sigaction` wrapper reserve. The tool supplies the
kernel's eight-byte signal-set size rather than libc's larger `sigset_t`, then
uses `rt_sigprocmask` to replace the complete kernel signal mask with empty.
Only then does it become nondumpable and install explicit resource behavior:
`SIGXFSZ` is ignored so oversized writes fail, `SIGXCPU` remains default,
`RLIMIT_FSIZE` is 512 MiB, `RLIMIT_AS` is 4 GiB, and `RLIMIT_CPU` is 60 seconds.
It also installs the canonical locale/time environment, rejects set-id or
capability-bearing execution, and sets `PR_SET_NO_NEW_PRIVS`. Any failed signal
syscall aborts before linking or result publication. The identity record
describes this as
`signal_state=linux-x86_64-kernel-1-64-main-v2`.

The authenticated closure's `execveat` with an empty `envp` is the production
guarantee against pre-main environment influence. The checks in `main()` are
defense-in-depth diagnostics only: static glibc may already have consumed
`GLIBC_TUNABLES`, `MALLOC_*`, or other startup controls before `main()` can
reject them. Likewise, standalone signal normalization does not protect static
libc startup from inherited signal state. The production closure must clear
handlers and reset dispositions and the mask before `execveat`; the tool's
main-time reset is defense in depth. This standalone tool does not claim a
pre-libc entrypoint. A signal pending in inherited state may terminate the tool
when the mask is emptied, which fails closed without a result record.

The tool validates result FD 91 as the exact declared inode of a connected
`AF_UNIX` `SOCK_SEQPACKET` endpoint whose peer is the direct same-UID parent and
whose receive queue is empty. It sets `O_NONBLOCK` itself and then revalidates
the endpoint identity and status flags.

The tool retains `/proc/self/fd`, requires `PROC_SUPER_MAGIC`, retains the
mount-namespace object, and verifies both identities before and after LLD. It
reopens every generated input and output `/proc/self/fd/N` path immediately
before LLD and requires exact device, inode, file type, size, and seal matches.
This proves descriptor-path resolution inside the selected namespace; it does
not decide which initial mount namespace is authorized. The authenticated
launcher owns that decision. A compromised launcher, a privileged process
that can alter the launch namespace, or kernel compromise remains outside this
tool's boundary.

The tool creates its own private `MFD_CLOEXEC|MFD_ALLOW_SEALING` output memfd.
It never accepts an output descriptor or pathname from the caller. LLD receives
only the internally constructed `/proc/self/fd/N` target. The tool forces
`--threads=1`, `--mmap-output-file`, and `--no-dependent-libraries`, rejects
caller spellings of those internal policies, and enforces
`FileOutputBuffer::F_mmap` at the pinned LLVM API boundary. Therefore LLD builds
the output in anonymous mapped memory and commits directly to the exact memfd;
it does not create a sibling temp or rename a pathname. With one linker thread,
LLVM 22's `unlinkAsync` returns without unlinking the proc-fd target.

Only static x86-64 `ET_EXEC` output is admitted. `ET_DYN`, `PT_INTERP`,
`PT_DYNAMIC`, writable executable loads, and executable stacks are rejected.

After LLD returns, the tool parses and hashes the x86-64 output ELF, sets mode
`0555`, adds the exact `WRITE|GROW|SHRINK|SEAL` seals, and revalidates all
measurements. Success sends exactly one bounded packet containing exactly one
`SCM_RIGHTS` descriptor:

```text
fe2o3-host-lld-result-v1\tplan=HEX\tclosure=HEX\tnonce=HEX\tsha256=HEX\tlength=DECIMAL\tcopy=receiver-owned-memfd-v1\n
```

The record is at most 512 bytes. It binds the request identity and the sealed
output and carries `copy=receiver-owned-memfd-v1`. Sender-controlled
mode metadata is not authoritative: after `SCM_RIGHTS`, the closure must copy
the measured bytes with positional reads into a fresh receiver-owned memfd,
set mode `0555`, seal it, and remeasure it before admission. It must then close
the sender's descriptor. Tests mutate the sender-owned descriptor mode before
copying and require the receiver-owned output to remain sealed mode `0555`.
The tool uses
`sendmsg(MSG_DONTWAIT|MSG_NOSIGNAL)` and calls `shutdown(91, SHUT_WR)` after a
successful packet. A full queue, absent reader, short send, or any other
failure sends no authorization packet and cannot block indefinitely.

## Authority boundary

The linker is not an authority source. It cannot authenticate compiler
artifacts, create broker identity, resolve mutable paths, or publish a host
artifact. `HostLinkClosureV1` owns those decisions and must validate the result
record and sealed descriptor before admission. The build artifact manifest is
a measurement, not an authority claim.

The nondumpable boundary excludes same-UID descriptor theft. The tool rejects
all effective, permitted, and inheritable capabilities, including capabilities
gained in a user namespace. A process with kernel-level control or a
compromised closure parent remains outside this tool's threat boundary.

The secure protocol suite extracts a required `#[no_mangle]` symbol from an
auxiliary Rust rlib and executes the linked `no_std` program. It separately
deep-validates the installed real `libcore` rlib without claiming that the
minimal program extracts a `libcore` member.

# fe2o3 static pre-exec launcher

This tool is a Linux x86-64 pre-exec containment mechanism. It has
`AUTHORITY=none`: it does not decide whether an image is trusted and cannot
reserve, publish, load, or launch GPU work. A future authority-bearing
supervisor must make those decisions before supplying this process with fixed
descriptors.

## Fixed descriptor contract

- FD 198 is a fixed-size `fe2o3_preexec_manifest_v1` in a memfd carrying the
  write, grow, shrink, and seal seals.
- FD 199 is the sealed regular ELF image executed with
  `execveat(AT_EMPTY_PATH)`. Its object identity must equal the manifest. It
  also requires Linux `F_SEAL_EXEC`, no set-id mode bits, and no
  `security.capability` xattr.
- FDs 200 through 215 are ordered source slots. Only the first
  `descriptor_count` slots may be open.
- Destination FDs are unique values from 0 through 127. FDs 0, 1, and 2 must
  each occur exactly once. Source objects, the manifest, and the executable
  may not alias one another.
- The parent PID and Linux process start time must match before containment is
  armed and again after `PR_SET_NO_NEW_PRIVS` and `PR_SET_PDEATHSIG(SIGKILL)`.

`fe2o3-static-preexec-manifest` is the canonical safe Rust encoder and
structural validator for this exact 704-byte V1 ABI. Its tests compile this C
header as an independent layout oracle and require byte-for-byte agreement.
The launcher remains responsible for live descriptor snapshots, seals, access
modes, closure, process controls, and target execution.

Alias decisions use only the immutable live-object key `(st_dev, st_ino)`.
File type, size, mode, seals, access mode, and close-on-exec policy are separate
state checks and cannot make two handles for one live object appear distinct.
The launcher validates the manifest, executable, and every source before
installation, retains the manifest through descriptor closure, and revalidates
the manifest, executable, and every installed destination before closing the
manifest and executing the target. Seals stabilize manifest bytes and
executable contents and size, while `F_SEAL_EXEC` prevents changes to executable
mode bits. The repeated state checks fail closed on an observed metadata
transition. They do not lock every mutable inode attribute against an external
holder after the final check.

The launcher installs destinations with `dup3`, marks only the executable FD
close-on-exec, closes every other FD with `close_range`, and uses a bounded
`/proc/self/fd` enumeration when `close_range` is unavailable. The fallback is
independent of inherited `RLIMIT_NOFILE` and fails closed if procfs is
unavailable or malformed. It emits no diagnostics to inherited descriptors.

The production image enters at a repository-owned `_start` and uses Linux
x86-64 syscalls only. It links no CRT or libc, has no undefined symbols, and
never examines the inherited environment. Before reading arguments,
descriptors, or manifest bytes, it makes the launcher nondumpable, sets
`PR_SET_NO_NEW_PRIVS`, sets both `RLIMIT_CORE` values to zero, resets Linux
signals 1 through 64 to default except `SIGKILL` and `SIGSTOP`, and replaces the
kernel signal mask with empty. It revalidates the process controls and parent
identity immediately before `execveat`. A pending inherited signal may
terminate the process when the mask is emptied; that is a fail-closed outcome
with no target execution. `execveat` receives an empty environment and a fixed
one-element argument vector.

## Build and test

```sh
cmake -S tools/fe2o3-static-preexec-launcher -B build/static-preexec \
  -DCMAKE_BUILD_TYPE=Release -DCMAKE_C_COMPILER=/usr/bin/cc
cmake --build build/static-preexec --parallel
ctest --test-dir build/static-preexec --output-on-failure
cargo test -p cargo-fe2o3 --test static_preexec_launcher
```

The production target is compiled freestanding and linked with
`-nostdlib -nostartfiles -nodefaultlibs -static -no-pie`. Its CTest rejects
`PT_INTERP`, `PT_DYNAMIC`, `DT_NEEDED`, RPATH, RUNPATH, executable stack,
undefined symbols, and libc symbols, and requires `_start`. This executable
contract requires a Linux 6.3-or-newer kernel implementing `F_SEAL_EXEC`. The
suite includes hostile inherited environment and signal state,
mutable-metadata alias records,
manifest/executable aliasing, both descriptor-closure implementations, and a
target that successfully crosses `execveat`, reports readiness, then is killed
and reaped after its direct parent exits.

## Deliberate limits

This foundation does not authenticate the supervisor, hash image contents,
apply seccomp or namespaces, normalize credentials or resource limits other
than `RLIMIT_CORE`, inspect ELF policy beyond the sealed regular-file contract,
supervise the target after exec, or provide a publication/replay registry.
Remaining inherited limits can make the launcher fail or change target
behavior. An authority-bearing supervisor must establish and bind their exact
policy before launch.

`PR_SET_DUMPABLE=0` protects the launcher validation window against a new
unprivileged same-UID attach after that syscall succeeds. It cannot undo a
tracer already attached before that operation, defend against
`CAP_SYS_PTRACE`, or establish same-UID isolation for the target: ordinary
`execveat` resets target dumpability. A protected deployment therefore needs a
trusted supervisor, a distinct service identity or equivalent kernel policy,
and target-side pre-libc hardening before target secrets or authority are
reachable. The protected compiler issuer now supplies that target-side secure
entry; the still-pending supervisor must supply the other properties. None of
them is claimed by this generic launcher.

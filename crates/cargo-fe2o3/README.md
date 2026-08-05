# cargo-fe2o3

`cargo-fe2o3` coordinates the current fe2o3 build and smoke-test workflows.
The adjacent `fe2o3-rustc-wrapper` is fail closed for compile invocations while
its trusted execution boundary is built incrementally.

## Pinned rustc executable

The wrapper now contains a private pinned-executable primitive for a future
native `rustc` execution path. On Linux it:

- opens the selected path read-only with `O_NOFOLLOW`, `O_NONBLOCK`, and
  `O_CLOEXEC` so a FIFO or device cannot stall validation;
- requires a non-empty regular file with execute permission and a size no
  larger than 512 MiB;
- hashes exactly the opened object's reported length with SHA-256, rejects
  short reads, growth, and metadata changes during hashing, and rewinds it;
- retains the opened descriptor; and
- constructs commands through a validated `/proc/self/fd/<fd>` reference whose
  lifetime is tied to the retained descriptor.

Compile execution is still disabled. The primitive is not used by bootstrap
passthrough or compile plans yet.

## Pinned codegen-backend object

The wrapper also contains a private Linux primitive for the codegen-backend
dynamic-library object. It applies the same final-component `O_NOFOLLOW` and
nonblocking open policy, requires a non-empty regular file no larger than 512
MiB, hashes exactly the opened object's bytes, and records device, inode, size,
mtime, and ctime before and after hashing. It retains that opened descriptor
and exposes only a validated `/proc/self/fd/<fd>` path, borrowed for no longer
than the retained object.

The backend descriptor is opened with `O_CLOEXEC`. Its descriptor reference
explicitly reports that rustc-child inheritance is blocked by close-on-exec.
This increment does not clear that flag, spawn a compile, or load the library.
Compile activation remains blocked until a later strategy can arrange and
verify descriptor inheritance without reopening the original backend path.

## Platform and trust limits

Linux with a trustworthy, mounted procfs is the only supported execution
strategy. Other Unix systems and Windows return an unsupported-platform error;
they must not fall back to reopening the selected pathname. The current
strategy is intended for native `rustc` binaries. Interpreter scripts can fail
when the descriptor is close-on-exec and are outside this boundary.

The descriptor prevents a later pathname replacement from redirecting
execution. It does not make a writable inode immutable. The toolchain location
must therefore be controlled against in-place writers. Parent-directory
symlinks are resolved during the initial open, and a race before that open can
choose which object is pinned. The SHA-256 becomes authentication evidence only
after a future orchestration layer compares it with a trusted expected digest.

The backend pin has the same mutable-inode limitation: descriptor retention
prevents pathname substitution, but another writer can still alter the opened
inode. Snapshot revalidation detects observable size, mode, mtime, or ctime
changes; it does not make the object immutable or eliminate the interval
between a future final check and dynamic loading.

ELF interpreters, transitive shared libraries of either rustc or the codegen
backend, dynamic-loader search and loading behavior, procfs mount/identity
semantics, and the kernel remain outside these primitives' boundary. In
particular, pinning the backend object does not pin its shared dependencies.

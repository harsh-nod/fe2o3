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
passthrough or compile plans yet, and codegen-backend dynamic-library pinning is
not implemented.

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

ELF interpreters, transitive shared libraries, the codegen backend, procfs, the
kernel, and the dynamic loader remain outside this primitive's trust boundary.

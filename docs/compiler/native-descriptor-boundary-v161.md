# Native Compiler Descriptor Checkpoints

This is a policy implementation and source audit, not evidence that the native
production controller is activated. Plain policy results do not authorize
execution, publication or proof admission.

## Original Custody

`native_runtime_guard::descriptor` prepares a bounded observation from an actual
selected syscall, then consumes it against the original kernel syscall result.
The owning controller must bind both calls to the same task, entry and stop
generation and keep every memory/file-table sharer stopped throughout. It must
reserve `PendingDescriptorCheck::STORAGE` until consumption/drop and use the
complete exported work/scratch quotes on the original account. Failed checks
require foreground retirement before any application instruction resumes.

An open's returned FD is inspected through the original retained task view.
This does **not** make the pathname or pre-open effects safe: the controller's
source/output policy must approve the path, creation, truncation and device-open
effects before stepping the syscall. `open_requirement` exposes inert original
scalars for that policy, including creation/truncation with read-only access.

`recvmsg` accepts zero control capacity or a bounded credential-only Unix
SEQPACKET shape. Credential mode allows at most 64 vectors and 1 MiB of payload,
no name writes, and no payload/header/control overlap. The actual returned
control must contain only one complete `SCM_CREDENTIALS` record, with no rights,
unknown records or truncation. Credentials here are data, not sender authority.
The kernel copies final header fields after the receive operation, so a late
user-copy fault does not establish that no FDs were installed; errors other
than no-progress EINTR/EAGAIN fail closed. See the
[native recvmsg implementation](https://github.com/torvalds/linux/blob/v6.8/net/socket.c).
`recvmmsg` and imported-right
roles are not admitted by this implementation. The original filters still
kill `pidfd_getfd`, `creat` and `openat2`.

The ioctl policy checks actual device kinds before permitting three read-only
queries. Known Linux terminal majors are established before `isatty` itself can
issue TCGETS. FIFO/null terminal probes may return ENOTTY; FIONREAD on sockets
requires AF_UNIX. Unknown devices, other socket protocol drivers, setters and
FD-producing ioctl operations refuse. The underlying Linux contracts are
documented by [pipe_ioctl](https://github.com/torvalds/linux/blob/v6.8/fs/pipe.c),
[null_fops](https://github.com/torvalds/linux/blob/v6.8/drivers/char/mem.c) and
[unix_ioctl](https://github.com/torvalds/linux/blob/v6.8/net/unix/af_unix.c).

## Selected IPC Audit

The source audit at `46279e477` found these distinct roles:

| Path | Receive contract | Relation to traced compiler |
| --- | --- | --- |
| `rustc-codegen-fe2o3/protected_compiler_execution_native_v3.rs` and compiler-execution-client `receive_packet_with_io` | Inherited issuer channel; zero ancillary capacity | Selected backend response path |
| verifier `functional_refinement_executor_channel_v1.rs` and `functional_refinement_executor_socket_v1.rs` | Exact 32-byte credentials-only buffer, PASSCRED, reject rights/truncation | Transport-only prototype; constructor is currently used only by tests, not production authority |
| cargo `binding_wrapper.rs` and `capability_broker.rs` | Authenticated capability rights | Wrapper receives before protected root launch, not rustc runtime import |
| compiler-execution-client `child_channel.rs` | One child channel FD plus credentials | Parent-side bootstrap transfer before compiler execution |
| host-link-closure `result.rs` and broker-authority-service `session.rs` | Typed result FD plus credentials | Broker-side host linker result, not a rustc import |
| worker-v3-verification-client `client.rs` and `client_v2.rs` | Sends payload rights; incoming rights rejected | Worker/application transport, not selected rustc backend receive |

These roles are not interchangeable. A future compiler-side FD import needs its
own supported retained role contract and actual returned-object validation;
accepting a generic ancillary buffer or a pathname is insufficient. Adding
credential-only support does not activate the prototype proof executor.

## Tests And Remaining Qualification

Pure wire tests cover exact native layouts, every iovec, total bounds, overflow,
destination aliasing, rights/unknown/truncated control and stable header fields.
Descriptor component tests cover actual socket/terminal/pipe kinds and open flag
classification. Full coordinator compilation, original trace integration,
pre-open source/output protection and isolated end-to-end execution remain
separate qualification requirements.

# Original Application Process and Input Observation

Source: `b57ca8807385eb16eefdedde071e3d4a472d0513`.
Parent: `f71c44720ed96e7c1d492ad8b35415a233309b1e`.

The multi-GPU admission path now has a retained process/input observation
prerequisite for the external proof custodian. Authenticated application
registration, that custodian's deployment, and the ordinary two-GPU application
campaign remain open. This checkpoint does not complete A3.

## Implementation

`RetainedWorkerV3ApplicationObservationV1::observe_pre_ack` consumes two original
pidfd identity tokens, three descriptor coordinates and descriptive expected
application/envelope identities. It independently checks live PID/start identities,
the direct-parent relationship, and all real/effective/saved/filesystem UID/GID
fields. Procfs observations are bracketed by original-pidfd checks.

The executable is opened through the retained process directory, checked against
Cargo's exact immutable kernel-seal profile and the static ELF byte policy, and
matched to the expected application identity. Input descriptors are duplicated
only with `pidfd_getfd`. Source CLOEXEC flags are separately read from procfs;
the duplicate's automatic CLOEXEC flag is not evidence about the original slot.
Type, access, owner UID, private mode, links and bounded sizes are checked. The
ACK writer must be a nonblocking FIFO. PATH/ASYNC/DIRECT/APPEND status is rejected.
Private artifact files may retain a different inherited GID, consistent with the
existing handoff contract; subsequent metadata changes still reject.

The three independently observed objects reconstruct the exact expected occurrence.
Copied envelope bytes match their expected domain-separated identity. Both
temporary ACK writer duplicates close before success, and RAII closes them on
failure. The returned move-only object retains original process identities,
proc directories, executable, envelope and artifact directory, but no ACK writer.
Post-ACK revalidation ignores the historical ACK slot, including legitimate reuse.
It reopens the running executable to detect replacement, rechecks the immutable
object/seals, and re-reads mutable envelope bytes. Immutable executable bytes need
not be hashed again. Admission rejects wrong expected lengths before reading and
uses fallible allocation for bounded input buffers.

This value does not decode a canonical envelope, verify directory membership,
acquire publication currentness, recover artifacts, authenticate a registration
channel or grant launch authority. Separate deployment admission must establish
protected service isolation and authenticate Cargo's original spawn/sandbox
supervision. Equal image bytes and PID/start time alone cannot rule out a re-exec
before observation. The caller retains a separate containment owner; observation
failure never kills or reaps a process. Process exit is not GPU settlement and
does not authorize proof/native-custody release.

## Qualification

| Check | Result |
| --- | --- |
| Broker library | 206 passed, 12 ignored |
| Broker compile-fail doctests | 55 passed |
| Explicit isolated root campaign | 1 passed, 25 reported scenario groups |
| Broker all-target strict Clippy | Passed with `-D warnings` |
| Coordinator all-target check | Passed |
| Changed-source formatting and whitespace | Passed |
| Frozen source and test-binary hashes | Unchanged |

The root campaign executes a static syscall-only application under UID/GID1000,
spawned and retained by a separate UID/GID1000 parent. The root observer receives
the live unreaped child's original pidfd from that parent. The application remains
at a pre-ACK barrier while its original executable/descriptors are inspected.
The test requires canonical ACK bytes and EOF while the observation remains alive,
then revalidates after the former ACK slot is reused for `/dev/null`.

Controls reject foreign parent/occurrence/envelope bindings, unsealed executable,
same-byte different-inode descriptors, changed access/CLOEXEC/status flags, blocking
ACK, same-length envelope mutation, asserted foreign credentials, pre-observation
exit and post-observation executable replacement with identical sealed bytes.
Application-only death is rejected while the original parent remains live; the
parent subsequently reaps its own child. Failure after ACK duplication still permits
EOF. A positive inherited-foreign-GID case preserves existing handoff compatibility.
Fixture cleanup contains the owned process group before reaping its leader.

The fixture deliberately supplies noncanonical envelope bytes to demonstrate the
observation-only boundary. It does not install Cargo's no-fork profile, execute
Worker/Verus, authenticate a deployed compiler issuer, or launch a GPU. No new
formal theorem, GPU performance or HIP/HSA parity claim follows. The separate
registration protocol still needs an occurrence-bound proof endpoint and fresh
session, leaving FD195 exclusive to compiler-currentness.

Two development failures were static fixture-linker incompatibilities: GNU
property headers and a static-libc segment layout were outside the existing ELF
policy. The final fixture uses no libc or startup objects and passes the unchanged
validator. Both failure logs are retained; production validation was not relaxed.

All tests used four Cargo jobs, disabled incremental/debug information and HIP
discovery, test optimization 1, enabled debug assertions/overflow checks, and four
ordinary test threads. Root qualification used one thread inside a private
PID/IPC/UTS/network namespace with private proc/dev/tmp and scoped inspection and
credential capabilities. No MI300X resources or host runtime installation were used.

## Evidence

[Evidence archive](evidence.tar.gz) SHA-256:
`d8291f48f6042790b2b956f46f7c11788aa656683497d9e03b9890d5cf031022`.
Source patch SHA-256:
`fe9c7b3bb3ec9562c02a66924f1049200abc585e86d82e19f49e8f20b9f1a518`.

The archive contains the signed source commit/patch, exact command scripts,
source/test-binary hash manifests, final regression/root logs, compiler versions,
initial regression captures and both development fixture failures. Every file in
its manifest was checked, and archive comparison against that manifest's source
tree passed. Executables are excluded. All private namespaces exited; this turn's
owned scratch directory and its exploratory fixture binaries were removed.
Existing pinned build-input caches and unrelated user files were preserved.

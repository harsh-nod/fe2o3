# Native Compiler Exec and Capture Checkpoint

Date: 2026-09-28 (Pacific). Continues the
[exact invocation and runtime checkpoint](native-invocation-runtime-20260928.md).

**M0-M7 and the 47/47 production-to-safe-GPU-launch gate remain incomplete.**
No tutorial classification changes. These are reusable launch/capture mechanics,
not a real rustc invocation, proof request, published artifact or GPU launch.
The production per-compiler attempt does not yet call this native compiler mode.

## Implemented

- Native compiler staging owns bounded copies of exact argv and environment,
  including argv[0], repetitions, empty and non-UTF-8 arguments. Frozen pointer
  tables survive source drops and stage moves. Nothing inherits the root
  coordinator's environment. The existing clone/profile/gate/exec and cleanup
  path now accepts these inputs; the fixed service ABI remains unchanged.
- Cwd staging duplicates the actual pinned directory and applies `fchdir` before
  exec. Standard streams preserve their open-file descriptions, status flags and
  offsets. Absent destinations remain closed. Capture distinguishes absence,
  open-CLOEXEC and open-inherited slots; adapters must map original-CLOEXEC to
  post-exec absence rather than blindly clearing its flag. No cwd path is reopened.
- Root-task tracing consumes the existing child, pidfd, artifact lease, cleanup
  slot and retained backing. It is thread-bound and checks the original Budget
  address and work ledger. Closed observations preserve actual signal/group-stop
  semantics and terminal exit status. Only its consuming terminal wait notifies
  cleanup; PID/status input cannot substitute. It does not trace descendants.
- Exec confirmation requires the held exec stop plus the unsafe caller's native
  exec and inherited-lock-closure obligations. An exec observation alone cannot
  discharge the artifact lease. Deferred cleanup keeps the original backing.
- Fixed codegen-backend and fe2o3 proc-macro transfers now join rustc/interpreter
  custody in `CompilerInvocationBacking`, from the same retained runtime. Each
  transfer carries a separate full overlapping charge and checks its exact role,
  mode, object, approval and original account. A retained load image is not proof
  that rustc loaded it or that its ELF dependencies resolved correctly.

## Validation

Code commit: `26e25596854bef86413ae45b8225e824aa6abf57`. The final local test,
source-inventory and all-target check runs share this unchanged 9,250-file
snapshot, with pinned `nightly-2026-04-03`, locked/offline dependencies, one build
job, serial tests and bounded resources:

```text
ba485bdab79f73453cecaa77e47fa50cb2c6148e1af860acb4b5882e0388f483
```

| Check | Result |
| --- | --- |
| Compiler closure unit / deployment integration tests | 267 / 38 passed |
| Coordinator unit tests | 217 passed |
| Process identity unit / integration tests | 14 / 6 passed |
| Native spawn unit tests | 193 passed; 3 privileged tests ignored locally |
| Doctests across these four crates | 251 passed |
| Unsafe source inventory | 5 passed; maintenance command ignored |
| Cargo driver, codegen backend and coordinator all-target checks | Passed |
| Scoped formatting / whitespace / delta hygiene | Passed |
| Hygiene-policy tests | 8 passed |

Nested fixture subprocesses are not counted again. The unit/integration total
is 735, separate from doctests. Expected failing receiver subprocesses are
negative controls inside passing parent tests. This is scoped validation, not
a workspace-wide or warning-free build. Final evidence labels are
`compiler-native-final-tests-rtwo`, `compiler-native-final-unsafe-rone` and
`compiler-native-final-check-rtwo`.

## Isolated Native Exec

On `mi350-2`, the explicit root diagnostic
`native_spawn::compiler_spawn::exec_tests::exact_compiler_inputs_reach_native_exec_and_owned_terminal_wait`
passed: **1 passed, 0 failed, 0 ignored**. It runs the actual native
stage/clone/profile/gate/trace/exec path with a small static C fixture, not rustc.
Its successful-exec case checks exact argument/environment bytes, relative cwd
access, stdin, closed stderr, FD 198, closed staging FDs and the shared stdout
offset. The owning wait preserves the fixture's deliberate exit code 7. Its
negative case checks inaccessible cwd refusal byte `0xca`, exit 126, no exec
event and no fixture output. Both finish through the same cleanup owner.

The private container uses a pinned read-only image, no network, one CPU, 2 GiB
memory, 32 PIDs and private tmpfs. It grants only the root credential-transition
and ptrace capabilities needed by this diagnostic; its seccomp filter is disabled
to exercise clone3. This is not an admitted compiler deployment or isolation proof.
Shared compiler/Verus installations were not mounted or modified.

| Identity | SHA-256 |
| --- | --- |
| Container image | `fd5370f370708f6a02cec6d44818a4295609e5bc68aa42455e53f141168a9d5f` |
| Executed test binary | `a09cc770e735a9a58a32315709a0933b435f27281eb16cd245b20dcb59feca61` |
| Static C fixture | `094476ec62936f01a079cb243685d20c6bec0c5233cb87672af15447e1485442` |
| Successful probe driver | `99b8c85ea3cb1cfad142ac78206b9e88b174e7743804634df8e05c28e944fef8` |

The C source is `crates/fe2o3-protected-service-spawn/tests/fixtures/native_compiler_exec.c`,
built with `cc -static -O2 -Wall -Wextra -Werror`. Earlier attempts exposed test
ordering and expected-length errors; a stale-binary retry and two transport
setup failures are excluded from success. No safety gate was relaxed. The
successful record is `compiler-native-probe-rf`; independent cleanup checks
cover all six attempted container/scratch names, including attempts before launch.

## Remaining Integration

The next production change must connect the authenticated wrapper capture to a
root-owned per-attempt receiver, not call native staging with root stdio or
reopened paths. It must establish exact input, artifact, loader-directory,
backend and proc-macro mappings; bind compiler/helper identity and proof channels;
and replace spawn, readiness and completion together with the same child owner.
The root-only tracer does not establish descendant monitoring or proof enforcement.

The existing same-UID parent channel cannot simply accept a root-parent socket
substitution. Likewise, the untraced service profile intentionally rejects a
tracer; compiler tracing needs its own authenticated process contract, not deletion
of that check. Helper shutdown, one inventory/account across the attempt,
protected deployment provisioning, outside-domain cleanup custody, bounded proof
RPC, finalization and safe GPU launch still need end-to-end composition. Existing
production refusals remain in place until those obligations are met.

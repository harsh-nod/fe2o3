# Launch-owned no-queue debugger object observation

This engineering example is a **qualification candidate**, not a qualified
hardware capture route. It launches one pinned ROCgDB ELF and the reviewed
sibling observe_gfx950_debug_acceptance_noqueue_v1 executable. It never takes
an attach PID, accepts arbitrary MI/CLI expressions, creates a queue, dispatches
a kernel, reads GPU registers, or produces a native-stop capability.

The current successful status means only:

1. The owned debugger's actual child was joined to a pidfd and stable
   parent/start identity. At the observer's first host rendezvous, before its
   KFD/VM work, executable identity, the exact environment, its sole main
   thread, absent children, and its three standard FDs were checked.
2. The expected memory-backed GPU code object was absent at the cold host stop.
3. At the post-publication host stop, the debugger's actual code-object list
   named the same owned PID and bounded original ELF extent. Reading that exact
   extent without continuing the inferior produced the pinned artifact digest.
4. The sibling's exact registration record matched. Inferior normal exit and
   pidfd exit, debugger direct-child reaping, stream completion, and reader
   joins were observed separately.

**This is not an observation of RUNTIME_STATE_LOADED_SUCCESS.** No reviewed MI
producer for that state has been implemented. Therefore
runtime_loaded_success_observed, debugger_acceptance_observed, and
physical_register_capture remain false, including on the successful status.
Metadata version 11, symbols-loaded, an exit code, CPU host registers, and
static resource counts cannot substitute for the missing evidence. No launch,
source, resume, queue, or physical-state authority is returned.

## Required external supervisor contract

This is a standalone reviewed-process experiment, not a general safety API.
Before invoking it, the supervisor must:

- Pin the **reviewed sibling observer build** and its full startup/dependency
  closure, the controller build, selected ROCgDB/ROCm debugger dependency
  closure, and the loader-admitted **gfx950** original artifact. A caller's
  arbitrary executable digest is not proof that it implements this observer.
  Do not project a gfx942 native capability into gfx950.
- Exclude executable/path replacement, in-place mutation, preloads, audit/tool
  injection, foreign ROCr/HIP or other runtime initialization, unreviewed
  constructors, fork/exec/thread creation, and outside process-control agents
  throughout the inferior's lifetime. Retain sole supervisor control over
  those actions. This permits only this reviewed debugger's fixed host
  breakpoint handling; it does not permit arbitrary eval or injected helpers.
- Supply an isolated launch-family owner with an independent wall-clock bound,
  terminal observation, descendant cleanup and reaping policy. The debugger
  is the inferior's parent; this controller is not. Do not infer inferior
  reaping from debugger exit. If this controller reports unproven family
  cleanup, the supervisor must resolve it before another activation attempt.
- Retain the sibling's native resources until actual process exit, including
  on errors, interruption, timeout and activation refusal. No in-process retry,
  rollback, Drop cleanup certificate, or trap-sampling safety is provided.
- Keep queues, dispatch and wave execution absent. The registered zero-TMA
  trap profile is **not** qualified for a GPU wave to execute its handler.

/proc snapshots are refusal fences, not a proof of past or future exclusion.
The entry rendezvous occurs after the reviewed host startup closure; its
absence of prior GPU effects depends on that closure, not on the function name.
Regular-file read bounds and MI deadlines are not an OS-wide cancellation
mechanism for blocked filesystem operations. The outer deadline remains needed.

## Fixed transport

The CLI grammar is printed on argument refusal. It takes one explicit
supervision acknowledgment, the sibling executable/path digest, and exactly
the sibling's twelve arguments. There is no debugger-path override. The
selected ELF is /opt/rocm-7.2.1/bin/rocgdb-py_3.12, with fixed size/digest in
config.rs; the shell wrapper is not used.

The debugger is launched with a cleared environment, no init files, early
auto-load/debuginfod disabling, and shell startup disabled. Only its own
environment additionally contains PYTHONNOUSERSITE=1, PYTHONSAFEPATH=1 and
PYTHONDONTWRITEBYTECODE=1. These flags disable user-site loading, request
Python's safe-path behavior and avoid startup bytecode writes; they do not
disable system sitecustomize, system .pth execution, installed gdb Python
packages, native constructors or dynamic loading. The supervisor must still
pin and review that remaining startup/dependency closure. Do not combine this
profile with Python ignore-environment=true, which would ignore these flags.
The CPU construction test does not attest actual Python startup. Fixed MI commands
clear the inferior environment and set only LANG=C, LC_ALL=C. A differing
actual entry environment is refused, not silently accepted. Breakpoint and
thread IDs come from actual bounded MI replies, not fixture constants.

The byte limits are: artifact 64 KiB, observer executable 64 MiB, debugger
256 MiB with an exact installed size, MI line 192 KiB, decoded MI string
128 KiB, 512 fields/depth 12, 8 MiB total captured output, 8,192 received
records, 64 commands of at most 2,048 bytes, and one 60-second session deadline.
Successful or refused controller JSON retains exact stdout, stderr and
attempted command bytes as hex plus hashes. The JSON bound is 18 MiB.
These are logical/retained byte-domain limits, not whole-process RSS claims.

Only one request is outstanding. No arbitrary memory read API is exposed:
the single memory request comes from the selected same-owned-stop URI and
must return the complete original ELF with the expected digest. Unknown
protocol transitions, extra inferiors/threads, a mismatched token/stop,
stale identity, malformed producer record, partial stream, missing exit or
missing join poison the run. Activation is never retried.

On refusal, known inferior cleanup uses only its owned pidfd and the debugger's
direct Child handle. No numeric-PID signal is sent. Once -exec-run may have
been issued, failure conservatively reports unproven launch-family cleanup,
even when the known pidfd reached exit. Drop is best-effort only and emits no
success receipt. Normal completion records pidfd exit, not an invented
controller waitpid for the debugger's child.

## Qualification status

The controller foundation includes eighteen CPU-only fake-peer tests and reuses the three
MI-parser controls. The depth correction adds two CPU controls for the exact
required library-range nesting and malformed nested/unknown/duplicate range fields.
They exercise the **same closed protocol** as the native
transport but create no subprocess, pidfd or device capability. Their fake
positive result is not native evidence. The startup delta adds one CPU-only
Command-construction test of the debugger environment; it does not execute
Python, start a process or prove startup isolation.

At donor freeze, only pinned rustfmt parsing/formatting and read-only schema
inspection were performed. Compilation, all tests, actual native MI record
compatibility, the entry environment/FD shape, debugger runtime handling,
same-stop code-object/content observation, and cleanup behavior are **unrun**.
Do not mark debugger acceptance, V4 physical capture, or a milestone complete
from these sources or fixture tests.

## Exact startup-setting notifications

The installed debugger's fixed `-iex` commands emit five optional
`cmd-param-changed` notifications: `auto-load gdb-scripts`,
`auto-load libthread-db`, `auto-load local-gdbinit`,
`auto-load python-scripts`, and `startup-with-shell`.
Only exact `param`/`value` fields and value `off` are admitted, once per
parameter and only during setup before `-exec-run`. These are inert startup
compatibility records, not a loader-isolation, runtime-acceptance, or
process-ownership predicate. Unknown parameters, duplicates, changed values,
extra fields, and notifications after setup refuse.

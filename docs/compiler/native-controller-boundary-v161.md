# Original Native Controller Boundary

The private `native_runtime_controller` module composes the original retained
runtime trace, actual stopped-task observations, retained compiler inventory,
kernel image checks, and descriptor policy. It creates no gate-release token,
publication authority, runtime-enforcement guard, or caller-provided success
provider. It is not yet selected by the production compiler execution path.

## Ownership And Transitions

Construction requires the held first exec of the original root task. The
controller records the original resource account and budget address. Its fixed
32-task table holds only metadata obtained through the original trace; PID and
generation scalars never acquire a process independently.

Event generations are the original owner's global observation epoch, not a
per-task stop identity. Registry birth generations are recorded only at an
actual acquired-and-held child transition. Other owned observations may advance
the epoch, but cannot substitute another task or bypass original syscall-result
authentication. The private joined controller must retain exclusive runtime
mutation between its policy transitions.

Each sensitive entry parks the complete original census before policy checks.
The controller retains that task's acquisition generation and entry observation.
Only that task advances to its kernel syscall exit. The original trace verifies
the saved syscall number, exit ABI, and register result. Descriptor results and
every held task's executable mappings are checked before application execution
or a parked sharer resumes. No generic unknown-stop resume path exists.

Actual birth events acquire the child through the original owner. The child
inherits its parent's kernel-image record and remains parked until the parent's
matching syscall exit. An actual exec replaces only that task's image; shared
inherited records survive until the last task reference is gone. A consuming
terminal acknowledgement removes exactly the original task generation before
its slot can be reused. Root exit codes and terminating signals remain distinct.

Any policy or resource refusal marks cancellation. The caller must still fund
foreground retirement of the original complete trace. Trace retirement does
not establish aggregate domain cleanup or artifact publication.

## Bounds

The new bounded parking entry point uses the existing stable-stop algorithm and
original absolute deadline with an additional finite observation allowance.
Exhausting the allowance refuses without resuming a task. The legacy entry point
and proof-controller semantics are unchanged. The controller composes a checked
worst-case work quote and full inline/scratch storage requirements; callbacks
remain separately charged by the original account.

## Remaining Admission Work

Open syscalls currently refuse before kernel execution. Returned-FD inspection
cannot protect creation, truncation, or device-driver open effects retroactively.
Concrete child-installed output confinement and device restrictions must be
joined before opens become eligible. Inherited and imported descriptor checks
remain necessary after such confinement.

Vfork dependencies, exit-group with live siblings, unsupported task-stop kinds,
and unknown clone flags refuse explicitly. These are not general Rust lifecycle
support claims. The native x86-64 mapping check accepts the fixed kernel-only
vsyscall gate only at `ffffffffff600000..ffffffffff601000`, with private
nonwritable `r-xp` or `--xp` permissions and zero offset/device/inode. Duplicates
refuse. Its name grants no authority; arbitrary anonymous executable mappings
still refuse. The vDSO remains separately captured and compared at actual exec.

This is an explicit trusted Linux x86-64 kernel ABI contract. The kernel defines
the read-only-after-init pseudo-VMA at this architecture address and implements
the emulated entry through its seccomp-aware handler. The existing proof mapping
entry point is unchanged. See the primary
[Linux gate implementation](https://github.com/torvalds/linux/blob/v6.8/arch/x86/entry/vsyscall/vsyscall_64.c)
and [architecture address definition](https://github.com/torvalds/linux/blob/v6.8/arch/x86/include/uapi/asm/vsyscall.h).

Production selection, the actual joined controller execution, complete proof,
publication, cleanup, and tutorial-kernel end-to-end qualification remain
separate requirements. Component tests do not establish any of those results.

## Tests

Spawn tests cover the bounded allowance's exact and one-short cases, closed stop
classification, real original-owner takeover, repeated acquired thread births
and consumed terminal waits, a later sensitive syscall, and actual held SIGCHLD
observation before redelivery. Controller state
tests cover inherited image lifetimes across exec/exit, exact PID-generation
reuse, the 32-task bound, entry/exit association, closed dispatch, and clone
flags. They do not replace a full coordinator build or actual protected kernel.

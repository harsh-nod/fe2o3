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
support claims. Existing strict kernel-map policy also continues to refuse a
legacy vsyscall mapping rather than grant executable authority from its label.

Production selection, the actual joined controller execution, complete proof,
publication, cleanup, and tutorial-kernel end-to-end qualification remain
separate requirements. Component tests do not establish any of those results.

## Tests

Spawn tests cover the bounded allowance's exact and one-short cases, closed stop
classification, real original-owner takeover, repeated acquired thread births
and consumed terminal waits, and a later sensitive syscall. Controller state
tests cover inherited image lifetimes across exec/exit, exact PID-generation
reuse, the 32-task bound, entry/exit association, closed dispatch, and clone
flags. They do not replace a full coordinator build or actual protected kernel.

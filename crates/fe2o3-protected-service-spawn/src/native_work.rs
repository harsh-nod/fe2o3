//! Logical child-side work prepaid by the parent before clone, on its original ledger.
//! This does not fund parent staging, readiness, cleanup, or the executed program, and
//! does not bound the duration of a blocking syscall.

use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;

const OPERATION_WORK: usize = 1024 + 64;
const CONTROL_WORK: usize = 256;

/// Optional pre-profile namespace gate: bounded reads and both child-side closes.
pub(crate) const MAPPING_GATE_WORK: usize =
    (crate::pre_exec::MAX_CHILD_GATE_ATTEMPTS_V2 + 2) * OPERATION_WORK + CONTROL_WORK;

/// Covers every bounded attempt in `syscall::child_exec`, including its failure suffix.
/// The caller must charge the returned work before clone; this function spends nothing.
/// Unsupported descriptor counts or capability ceilings fail before arithmetic.
pub(crate) fn child_work(descriptors: usize, cap_last_cap: u32) -> Result<usize, Resource> {
    if !(1..=crate::MAX_PROTECTED_SERVICE_DESCRIPTOR_BINDINGS_V1).contains(&descriptors)
        || cap_last_cap > 63
    {
        return Err(Resource::Arithmetic);
    }
    let capabilities = usize::try_from(cap_last_cap)
        .map_err(|_| Resource::Arithmetic)?
        .checked_add(1)
        .ok_or(Resource::Arithmetic)?;
    // One CAPBSET_DROP, one CAPBSET_READ and one AMBIENT_IS_SET per capability.
    let capability_operations = capabilities.checked_mul(3).ok_or(Resource::Arithmetic)?;
    let operations = [
        64, // normalize_signal_state: all signal slots, even the two skipped slots.
        1,  // rt_sigprocmask.
        8,  // Two arm_parent_death calls, each getppid/set/getppid/get (four calls).
        10, // establish_profile: four setup, three ID/group, three final setup calls.
        11, // validate_profile: six ID/group/capability reads plus five final reads.
        capability_operations,
        crate::pre_exec::MAX_CHILD_GATE_ATTEMPTS_V2,
        1,           // Profile-ready write.
        1,           // close_range(CLOEXEC).
        descriptors, // One dup3 per destination.
        3,           // Close stdin, stdout and stderr.
        1,           // execveat, including a failed attempt.
        2,           // child_fail: failure send and _exit, even after all preceding work.
    ]
    .into_iter()
    .try_fold(0_usize, |sum, count| {
        sum.checked_add(count).ok_or(Resource::Arithmetic)
    })?;
    // With the current 64-attempt gate: (166 + descriptors + 3 * capabilities)
    // operations, each with syscall and scalar allowance, plus fixed control work.
    operations
        .checked_mul(OPERATION_WORK)
        .and_then(|work| work.checked_add(CONTROL_WORK))
        .ok_or(Resource::Arithmetic)
}

#[cfg(test)]
#[path = "native_work_tests.rs"]
mod tests;

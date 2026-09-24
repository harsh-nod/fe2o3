//! Native entrypoint over the production launcher's existing fixed descriptor ABI.
use crate::{
    COMPILER_EXECUTION_ISSUER_CLIENT_PIDFD_V1 as CLIENT,
    COMPILER_EXECUTION_ISSUER_EXTERNAL_ANCHOR_PEER_FD_V1 as ANCHOR,
    COMPILER_EXECUTION_ISSUER_EXTERNAL_ANCHOR_PIDFD_V1 as ANCHOR_PID,
    COMPILER_EXECUTION_ISSUER_LAUNCH_MANIFEST_FD_V1 as MANIFEST,
    COMPILER_EXECUTION_ISSUER_PEER_FD_V1 as PEER, COMPILER_EXECUTION_ISSUER_POLICY_FD_V1 as POLICY,
    COMPILER_EXECUTION_ISSUER_READY_FD_V1 as READY, COMPILER_EXECUTION_ISSUER_ROOT_FD_V1 as ROOT,
    COMPILER_EXECUTION_ISSUER_SIGNING_KEY_FD_V1 as KEY,
    CompilerExecutionIssuerLaunchInputsV2 as Inputs, close_inherited, take_inherited,
};
use fe2o3_broker_authority_service::{
    ExpectedClientProcessIdentityV1 as Expected, LiveClientPidfdIdentityV2 as Client,
    ProtectedCompilerExecutionIssuerAdmissionV2 as Admission,
    ProtectedExternalAnchorServiceAdmissionV2 as Anchor, ProtectedIssuerProcessV1 as Process,
    ProtectedServiceAdmissionV2 as Service,
};
use fe2o3_compiler_closure_capability::CompilerExecutionSigningKeyCapabilityV2 as Key;
use fe2o3_compiler_execution_protocol::CompilerExecutionIssuerPolicyV2 as Policy;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{fmt, fs::File};

#[path = "native_error.rs"]
mod error;
pub use error::CompilerExecutionIssuerEntrypointErrorV2;
type Error = CompilerExecutionIssuerEntrypointErrorV2;
type Result<T> = std::result::Result<T, Error>;

// Prepay the common process-hardening and fixed FD take/close mechanics. Each
// native capability, image, transport and durable operation charges separately.
const IO_WORK: usize = 8 + 64 * 1024;
const FRAME: usize = 16 * 1024
    + Admission::PROCESS_STORAGE
    + Service::FD_PAIR_STORAGE
    + Client::FD_STORAGE
    + Anchor::PAIR_STORAGE
    + Key::FILE_STORAGE
    + Inputs::INPUT_STORAGE
    + Admission::READINESS_WRITER_STORAGE;

/// Runs one native protected issuer from the launcher's fixed slots 3..=11.
///
/// Reads no argv/environment and never dispatches or retries through V1. The
/// existing descriptor ABI and process-hardening mechanics are shared, not
/// admitted policy/key/service owners. The separately installed native binary
/// must be pinned by the native supervisor's program/policy capabilities.
///
/// One caller-owned budget covers admission, recovery, readiness and the entire
/// session. Scopes preserve inherited storage, cumulative work and denial
/// history. Call only in an isolated launched child: process hardening is
/// irreversible, and inherited descriptors are consumed. Only acknowledged
/// cancellation succeeds. Permission denial during compiler inspection is
/// terminal; readiness does not certify that permission or a compiler result.
pub fn run_inherited_compiler_execution_issuer_v2(b: &mut Budget<'_>) -> Result<()> {
    let floor = b.storage();
    b.with_prepaid_scope(floor, 8, IO_WORK, FRAME, run)
}

fn run(b: &mut Budget<'_>) -> Result<()> {
    let process = Process::harden().map_err(Error::Process)?;
    b.reserve_storage(Admission::PROCESS_STORAGE + Inputs::INPUT_STORAGE)?;
    let (inputs, charge) = Inputs::from_inherited(b)?;
    b.reserve_storage(charge.additional_storage())?;
    close_inherited(POLICY)?;
    close_inherited(MANIFEST)?;
    b.release_storage(Inputs::INPUT_STORAGE)?;

    // Independently decoded native ownership for Admission; the sealed input
    // capabilities remain live for exact pre/post-session revalidation.
    let (policy, charge) = Policy::decode(inputs.policy().canonical_bytes(), b)?;
    b.reserve_storage(charge.additional_storage())?;
    b.reserve_storage(
        Service::FD_PAIR_STORAGE
            + Client::FD_STORAGE
            + Anchor::PAIR_STORAGE
            + Key::FILE_STORAGE
            + Admission::READINESS_WRITER_STORAGE,
    )?;
    let root = take_inherited(ROOT)?;
    let peer = take_inherited(PEER)?;
    let client = take_inherited(CLIENT)?;
    let key = File::from(take_inherited(KEY)?);
    // Close original fd9 before any readiness can be emitted; supervisor waits
    // for both the exact frame and EOF, never a surviving inherited writer.
    let writer = take_inherited(READY)?;
    let anchor = take_inherited(ANCHOR)?;
    let anchor_pid = take_inherited(ANCHOR_PID)?;

    let expected = inputs.manifest().client();
    let expected = Expected::new(expected.pid(), expected.uid(), expected.gid())
        .map_err(Error::ExpectedClient)?;
    let (client, charge) = Client::admit(client, expected, b)?;
    b.reserve_storage(charge.additional_storage())?;
    let (service, charge) = Service::admit(root, peer, client, b)?;
    b.reserve_storage(charge.additional_storage())?;
    let (anchor, charge) = Anchor::admit(
        anchor,
        anchor_pid,
        inputs.manifest().external_anchor_service(),
        b,
    )?;
    b.reserve_storage(charge.additional_storage())?;
    let (key, charge) = Key::from_file(key, &policy, b)?;
    b.reserve_storage(charge.additional_storage())?;
    inputs.revalidate(b)?;
    let (admission, charge) = Admission::admit(process, service, policy, key, anchor, b)?;
    b.reserve_storage(charge.additional_storage())?;
    inputs.revalidate(b)?;
    admission.serve_native_with_readiness(inputs.manifest(), writer, b)?;
    inputs.revalidate(b)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

    #[test]
    fn native_entrypoint_denies_work_before_hardening_or_inherited_io() {
        let mut work = Work::new(0);
        let mut b = Budget::new(&mut work, 1_000_000);
        b.reserve_storage(19).unwrap();
        let e = run_inherited_compiler_execution_issuer_v2(&mut b).unwrap_err();
        assert!(matches!(e, Error::Resource(_)));
        assert_eq!(b.storage(), 19);
        assert_eq!(b.work(), 0);
    }

    #[test]
    fn native_entrypoint_denies_scratch_before_hardening_or_inherited_io() {
        let mut work = Work::new(IO_WORK);
        let mut b = Budget::new(&mut work, FRAME - 1);
        let e = run_inherited_compiler_execution_issuer_v2(&mut b).unwrap_err();
        assert!(matches!(e, Error::Resource(_)));
        assert_eq!(b.storage(), 0);
        assert_eq!(b.work(), IO_WORK);
    }
}

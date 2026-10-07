//! Exact artifact composition retaining native registration/deployment provenance.
use super::*;
use fe2o3_compiler_execution_client::NativeApplicationCurrentRecordV1;

/// Native source/F-to-machine relation and independently admitted native response provenance.
///
/// The actual registered endpoint/pidfd, installed V3 policy and original account
/// remain borrowed. A cryptographic response alone cannot enter this wrapper.
/// Durable publication custody, protected proof-custodian readiness, runtime
/// invocation premises and native memory remain separate mandatory gates.
///
/// ```compile_fail
/// use fe2o3_host::{CheckedRegisteredNativeConditionalFillArtifactV1 as Bound,
///     CheckedNativeConditionalFillArtifactV1 as Bare};
/// fn promote<'a, 'work>(value: Bare<'a>) -> Bound<'a, 'work> { value.into() }
/// ```
/// ```compile_fail
/// use fe2o3_host::CheckedRegisteredNativeConditionalFillArtifactV1 as Bound;
/// fn escape<'a, 'work>(value: Bound<'a, 'work>) -> Bound<'static, 'work> { value }
/// ```
pub struct CheckedRegisteredNativeConditionalFillArtifactV1<'a, 'work> {
    artifact: CheckedNativeConditionalFillArtifactV1<'a>,
    currentness: &'a NativeApplicationCurrentRecordV1<'work>,
}
impl<'a, 'work> CheckedRegisteredNativeConditionalFillArtifactV1<'a, 'work> {
    pub const fn artifact(&self) -> &CheckedNativeConditionalFillArtifactV1<'a> {
        &self.artifact
    }
    pub const fn native_current_record(&self) -> &'a NativeApplicationCurrentRecordV1<'work> {
        self.currentness
    }
    pub const fn grants_load_authority(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Requires the actual native-only currentness owner, not its transferable bytes.
/// Original publication, carriage and native currentness storage must be prepaid.
/// Proof execution and any failed postcheck remain terminal on the same ledger.
pub fn check_registered_native_conditional_fill_artifact_v1<'a, 'work>(
    runtime: &FunctionalRefinementVerusRuntimeLeaseV1,
    publication: &'a RecoveredConditionalWorkerHsacoPublicationV5,
    carriage: &'a CompilerExecutionReceiptCarriageV3,
    currentness: &'a NativeApplicationCurrentRecordV1<'work>,
    analysis: &'a AuthenticatedPhysicalMachineAnalysisExecutionV1,
    timeout_seconds: u32,
    budget: &mut Budget<'work>,
) -> Result<
    (
        CheckedRegisteredNativeConditionalFillArtifactV1<'a, 'work>,
        NativeConditionalFillArtifactStorageV1,
    ),
    Error,
> {
    budget.charge_work(8)?;
    let floor = publication
        .required_retained_storage()
        .checked_add(carriage.retained_storage())
        .and_then(|bytes| bytes.checked_add(currentness.retained_storage().ok()?))
        .ok_or(Resource::Arithmetic)?;
    if budget.storage() < floor {
        return Err(Resource::Accounting.into());
    }
    currentness
        .revalidate_provenance(budget)
        .map_err(Error::Provenance)?;
    let (artifact, storage) = check_native_conditional_fill_artifact_v1(
        runtime,
        publication,
        carriage,
        currentness.verified(),
        analysis,
        timeout_seconds,
        budget,
    )?;
    budget.reserve_storage(storage.retained_storage())?;
    currentness
        .revalidate_provenance(budget)
        .map_err(Error::Provenance)?;
    let retained = storage
        .retained_storage()
        .checked_sub(size_of::<CheckedNativeConditionalFillArtifactV1>())
        .and_then(|bytes| {
            bytes.checked_add(size_of::<CheckedRegisteredNativeConditionalFillArtifactV1>())
        })
        .ok_or(Resource::Arithmetic)?;
    budget.release_storage(storage.retained_storage())?;
    Ok((
        CheckedRegisteredNativeConditionalFillArtifactV1 {
            artifact,
            currentness,
        },
        NativeConditionalFillArtifactStorageV1(retained),
    ))
}

//! Metered preparation through the existing production parser and worker pinning.
//!
//! The Rust schedule counts logical byte/row visits and buffer storage, not CPU
//! instructions, allocator capacity or RSS. Worker image capture, OS I/O retries
//! and process supervision retain their existing separate bounded domain.
use super::{
    Budget, BuildConfigError, LinkOptionV1, PinnedWorkerV1, PreparedProductionBuildConfig,
    ProductionBuildConfigVersion, Resource, WorkerExecutionLimitsV1, WorkerInputV1,
    WorkerOutputConstraintsV1, prepare_production_manifest,
};
use crate::{
    compiler_execution_boundary::native::{
        ParentCompilerExecutionReadinessCustodyV3,
        pipeline::{
            ConditionalRecoveryPolicy, ContinuationError, ParentPreparedConditionalArtifact,
        },
    },
    protected_compiler_handoff_v3::ParentRustcInvocationCustody,
};
use fe2o3_artifact_transaction::{BuildAttempt, ProducerIdentity};
use fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1;
use std::{mem::size_of, path::Path};

// Covers bounded paths, error payloads, recipe/measurement headers and fixed
// traversal state. Dynamic JSON and provider payloads are charged separately.
const FRAME: usize = 64 * 1024
    + 2 * size_of::<PreparedProductionBuildConfig>()
    + size_of::<PreparedNativeProductionBuildConfig>();
const ENTRY_WORK: usize = 8192;

/// The exact parsed recipe plus its original prepaid account identity.
/// There is no conversion from an unmetered prepared configuration.
pub(crate) struct PreparedNativeProductionBuildConfig {
    config: PreparedProductionBuildConfig,
    account_address: usize,
    ledger: CanonicalKernelIrWorkLedgerIdentityV1,
    input_floor: usize,
    retained_storage: usize,
}

type WorkerParts = (
    PinnedWorkerV1,
    Vec<WorkerInputV1>,
    Vec<LinkOptionV1>,
    WorkerOutputConstraintsV1,
    WorkerExecutionLimitsV1,
    usize,
);

impl PreparedNativeProductionBuildConfig {
    /// The broker must independently admit the schema and expected identity.
    /// Equality here is not protected configuration provenance or authority.
    ///
    /// Keep the account at its original address and retain its incoming floor
    /// throughout the attempt. Reservations remain on failure/unwind; this is
    /// terminal preparation, not a refundable
    /// scope. On success even temporary charges stay with the recipe and its
    /// downstream artifact, so no returned owner or diagnostic is underpaid.
    pub(crate) fn from_manifest(
        path: &Path,
        version: ProductionBuildConfigVersion,
        expected_identity: &[u8; 32],
        b: &mut Budget<'_>,
    ) -> Result<Self, BuildConfigError> {
        let entry = b.storage();
        prepay_preparation(b)?;
        let config = prepare_production_manifest(path, version, Some(&mut *b))?;
        if config.identity().as_bytes() != expected_identity {
            return Err(BuildConfigError::Invalid(
                "native configuration differs from the expected identity".to_owned(),
            ));
        }
        Ok(Self {
            config,
            account_address: b as *const Budget<'_> as usize,
            ledger: b.work_ledger_identity_v1(),
            input_floor: b.storage(),
            retained_storage: b.storage().checked_sub(entry).ok_or(Resource::Accounting)?,
        })
    }

    pub(crate) fn identity(&self) -> super::BuildConfigIdentity {
        self.config.identity()
    }

    pub(crate) const fn retained_storage(&self) -> usize {
        self.retained_storage
    }

    pub(crate) fn check_account(&self, b: &mut Budget<'_>) -> Result<(), Resource> {
        b.charge_work(8)?;
        if b as *const Budget<'_> as usize != self.account_address
            || b.work_ledger_identity_v1() != self.ledger
            || b.storage() < self.input_floor
        {
            return Err(Resource::Accounting);
        }
        Ok(())
    }

    pub(crate) fn into_worker_parts(self, b: &mut Budget<'_>) -> Result<WorkerParts, Resource> {
        self.check_account(b)?;
        let link = self.config.link;
        Ok((
            link.worker,
            link.providers,
            link.link_options,
            link.candidate_output,
            link.limits,
            self.retained_storage,
        ))
    }

    #[allow(clippy::too_many_arguments, clippy::result_large_err)]
    pub(crate) fn finalize_conditional_current<'a, 'b, 'w>(
        self,
        readiness: ParentCompilerExecutionReadinessCustodyV3<'b, 'w>,
        approval: fe2o3_compiler_closure_capability::RetainedCompilerRuntimeV1,
        output_dir: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        invocation: &'a ParentRustcInvocationCustody,
        policy: &ConditionalRecoveryPolicy<'_>,
    ) -> Result<ParentPreparedConditionalArtifact<'a, 'b, 'w>, ContinuationError> {
        readiness.finalize_current_publication(
            output_dir, producer, attempt, invocation, policy, self, approval,
        )
    }
}

fn scaled(bytes: usize, factor: usize, extra: usize) -> Result<usize, Resource> {
    bytes
        .checked_mul(factor)
        .and_then(|n| n.checked_add(extra))
        .ok_or(Resource::Arithmetic)
}

pub(super) fn prepay_preparation(b: &mut Budget<'_>) -> Result<(), Resource> {
    b.charge_work(ENTRY_WORK)?;
    b.reserve_storage(FRAME)
}

/// Closed quote for one configuration/provider preparation under the existing
/// parser limits, not environment acquisition/diagnostics. Worker image capture
/// retains its separate bounded OS domain.
pub(crate) fn maximum_preparation_quota() -> Result<(usize, usize), Resource> {
    let bytes = super::MAX_CONFIG_BYTES;
    let providers = fe2o3_hsaco_finalize::MAX_WORKER_TOTAL_INPUT_BYTES;
    let count = super::MAX_LINK_INPUTS
        .checked_sub(1)
        .ok_or(Resource::Arithmetic)?;
    let work = [
        ENTRY_WORK,
        scaled(bytes, 4, 4)?,
        scaled(bytes, 1024, 0)?,
        scaled(providers, 8, scaled(count, 256, 0)?)?,
    ]
    .into_iter()
    .try_fold(0_usize, usize::checked_add)
    .ok_or(Resource::Arithmetic)?;
    let storage = [
        FRAME,
        scaled(bytes, 2, 2 + size_of::<Vec<u8>>())?,
        scaled(bytes, 256, 0)?,
        scaled(
            providers,
            2,
            scaled(count, 2 + size_of::<WorkerInputV1>(), 0)?,
        )?,
    ]
    .into_iter()
    .try_fold(0_usize, usize::checked_add)
    .ok_or(Resource::Arithmetic)?;
    Ok((work, storage))
}

pub(super) fn prepay_read(bytes: usize, b: &mut Budget<'_>) -> Result<(), Resource> {
    // The shared reader stops at initial length + 1, then checks the same inode
    // metadata. Growth cannot cause an unquoted read up to the global maximum.
    b.charge_work(scaled(bytes, 4, 4)?)?;
    b.reserve_storage(scaled(bytes, 2, 2 + size_of::<Vec<u8>>())?)
}

pub(super) fn prepay_manifest(bytes: usize, b: &mut Budget<'_>) -> Result<(), Resource> {
    // Pinned serde_json/default recursion bound (128) and nightly BTreeMap:
    // 1024 visits/byte covers parsing, canonical encoding, key comparisons,
    // schema/selector walks, text copies, diagnostics and configuration hashing.
    // 256 bytes/byte covers Value/tree/vector headers, growth, canonical bytes,
    // decoded strings and retained recipe text. Reaudit if these codecs change.
    b.charge_work(scaled(bytes, 1024, 0)?)?;
    b.reserve_storage(scaled(bytes, 256, 0)?)
}

pub(super) fn prepay_providers(
    bytes: usize,
    count: usize,
    b: &mut Budget<'_>,
) -> Result<(), Resource> {
    // Read/growth (+1 per file), content hashing and transitive recipe hashing.
    b.charge_work(scaled(bytes, 8, scaled(count, 256, 0)?)?)?;
    b.reserve_storage(scaled(
        bytes,
        2,
        scaled(count, 2 + size_of::<WorkerInputV1>(), 0)?,
    )?)
}

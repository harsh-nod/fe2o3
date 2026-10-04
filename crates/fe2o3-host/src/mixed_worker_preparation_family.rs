// Shared generated argument preparation; each wrapper selects one concrete contract family.
use super::*;
use crate::{
    AqlDispatchGeometryV1, CompilerGeneratedKernelExpectationV1, CompilerGeneratedKfdArguments,
    generated_kfd_arguments::GeneratedKfdCompletion,
};
use fe2o3_kernel_descriptor::{
    DESCRIPTOR_TABLE_VIEW_STORAGE_V3, decode_device_descriptor_table_v3,
};
use fe2o3_runtime::{
    Gfx942RuntimeInvocationBindingV1, Gfx942RuntimePreparationErrorV1,
    PreparedGfx942RuntimeDispatchV1, prepare_gfx942_runtime_dispatch_v1,
};

#[derive(Debug)]
pub enum MixedWorkerPreparationError {
    Admission(AdmissionError),
    Arguments(Box<dyn std::error::Error>),
    Runtime(Gfx942RuntimePreparationErrorV1),
    Binding(&'static str),
}
impl MixedWorkerPreparationError {
    pub(crate) fn arguments(error: impl std::error::Error + 'static) -> Self {
        Self::Arguments(Box::new(error))
    }
}
impl fmt::Display for MixedWorkerPreparationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Admission(e) => e.fmt(f),
            Self::Arguments(e) => e.fmt(f),
            Self::Runtime(e) => e.fmt(f),
            Self::Binding(detail) => write!(f, "{PREPARATION_LABEL}: {detail}"),
        }
    }
}
impl std::error::Error for MixedWorkerPreparationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Admission(e) => Some(e),
            Self::Arguments(e) => Some(e.as_ref()),
            Self::Runtime(e) => Some(e),
            Self::Binding(_) => None,
        }
    }
}
impl From<fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1>
    for MixedWorkerPreparationError
{
    fn from(error: fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1) -> Self {
        Self::arguments(error)
    }
}

/// Current descriptor custody, initialized buffers, exact runtime premises and all
/// generated Rust output borrows move together. No raw prepared request escapes.
/// A receiving protected compiler/native proof join remains mandatory before
/// this can gain a dispatch method. No unsafe backend placeholder is installed.
/// The crate-private consuming adapter requires a separate reviewed protected
/// provider; this ordinary public owner continues to grant no authority.
///
#[must_use]
pub struct PreparedMixedWorkerInvocation<'allocation, R, K> {
    owner: RecoveredMixedWorkerPinnedRoster<R>,
    current: DurableCurrentLinkPublicationTokenV1,
    prepared: PreparedGfx942RuntimeDispatchV1,
    _completion: GeneratedKfdCompletion<'allocation>,
    _marker: PhantomData<fn() -> K>,
}
impl<R, K> PreparedMixedWorkerInvocation<'_, R, K> {
    pub fn invocation_binding(&self) -> Gfx942RuntimeInvocationBindingV1 {
        self.prepared.invocation_binding()
    }
    pub fn dispatch_contract_sha256(&self) -> [u8; 32] {
        self.prepared.dispatch_contract_sha256()
    }
    pub fn revalidate_currentness(&self) -> Result<()> {
        self.owner.check_current(&self.current)
    }
    pub fn with_verification_request<T, E>(
        &self,
        receive: impl for<'a> FnOnce(
            MixedWorkerVerificationRequest<'a, R>,
            Gfx942RuntimeInvocationBindingV1,
            [u8; 32],
        ) -> std::result::Result<T, E>,
    ) -> Result<std::result::Result<T, E>> {
        self.owner.check_current(&self.current)?;
        let result = receive(
            MixedWorkerVerificationRequest {
                owner: &self.owner,
                current: &self.current,
            },
            self.prepared.invocation_binding(),
            self.prepared.dispatch_contract_sha256(),
        );
        self.owner.check_current(&self.current)?;
        Ok(result)
    }
    pub const fn grants_load_authority(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

impl<R: CompilerGeneratedKernelExpectationRosterV1> RecoveredMixedWorkerPinnedRoster<R> {
    /// Takes real generated arguments through complete versioned premise binding and
    /// ordinary physical COV6 materialization. This is not a V1 authorization
    /// conversion. Returned storage is an already-reserved addition for the typed
    /// premise construction; artifact/ordinary generated-packing allocations
    /// retain their existing bounded host policy rather than claiming full RSS
    /// accounting. Keep this addition charged until the prepared owner drops.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_gfx942<'allocation, K, Arguments>(
        self,
        arguments: Arguments,
        geometry: AqlDispatchGeometryV1,
        dynamic_group_segment_bytes: u32,
        timeout_milliseconds: u32,
        budget: &mut Budget<'_>,
    ) -> std::result::Result<
        (PreparedMixedWorkerInvocation<'allocation, R, K>, usize),
        MixedWorkerPreparationError,
    >
    where
        K: CompilerGeneratedKernelExpectationV1,
        Arguments: CompilerGeneratedKfdArguments<'allocation, K>,
    {
        use MixedWorkerPreparationError as Error;
        let current = self
            .envelope
            .current_publication_lease()
            .acquire_current_token()
            .map_err(AdmissionError::CurrentPublication)
            .map_err(Error::Admission)?;
        self.check_current(&current).map_err(Error::Admission)?;
        let marker = CompilerGeneratedKernelExpectationRosterEntryV1::for_marker::<K>();
        let ordinal =
            R::ENTRIES
                .iter()
                .position(|entry| *entry == marker)
                .ok_or(Error::Binding(
                    "generated marker absent from exact admitted roster",
                ))?;
        let table = self.descriptor_table().map_err(Error::Admission)?;
        let contract = codec_on_budget(budget, CONTRACT_CODEC_STORAGE, |budget| {
            table
                .contract(ordinal, &mut |n| budget.charge_work(n))
                .map_err(codec_error)
        })
        .map_err(Error::Admission)?;
        let contract_identity = *contract.identity();
        let floor = budget.storage();
        let backing = table
            .nominal_canonical_bytes()
            .len()
            .checked_add(DESCRIPTOR_TABLE_VIEW_STORAGE_V3)
            .ok_or(Error::Binding("descriptor working extent"))?;
        let (prepared, completion, retained) =
            budget.with_prepaid_scope(floor, 0, 0, backing, |budget| {
                let nominal =
                    decode_device_descriptor_table_v3(table.nominal_canonical_bytes(), &mut |n| {
                        budget.charge_work(n)
                    })
                    .map_err(Error::arguments)?;
                let start = budget.storage();
                let (inputs, completion) = prepare_generated::<K, Arguments>(
                    &nominal,
                    &contract,
                    arguments,
                    geometry,
                    dynamic_group_segment_bytes,
                    timeout_milliseconds,
                    budget,
                )
                .map_err(Error::from_generated)?;
                let expected = inputs.invocation_binding();
                require_mixed_contract(expected, contract_identity)?;
                let prepared = prepare_gfx942_runtime_dispatch_v1(
                    current.exact_artifact_bytes(),
                    K::EXPORT_NAME,
                    inputs,
                )
                .map_err(Error::Runtime)?;
                if prepared.invocation_binding() != expected
                    || prepared.finalized_hsaco_length()
                        != current.exact_artifact_bytes().len() as u64
                    || prepared.identity().object_sha256()
                        != <[u8; 32]>::from(Sha256::digest(current.exact_artifact_bytes()))
                {
                    return Err(Error::Binding(
                        "prepared mixed artifact or premise substitution",
                    ));
                }
                let retained = budget
                    .storage()
                    .checked_sub(start)
                    .ok_or(Error::Binding("premise storage floor"))?;
                Ok::<_, Error>((prepared, completion, retained))
            })?;
        // The scoped constructor returns unreserved owners. Transfer their
        // retained charge onto this same caller ledger before keeping them.
        budget.reserve_storage(retained)?;
        if let Err(error) = self.check_current(&current) {
            drop((prepared, completion));
            budget.release_storage(retained)?;
            return Err(Error::Admission(error));
        }
        drop(contract);
        drop(table);
        Ok((
            PreparedMixedWorkerInvocation {
                owner: self,
                current,
                prepared,
                _completion: completion,
                _marker: PhantomData,
            },
            retained,
        ))
    }
}

fn require_mixed_contract(
    binding: Gfx942RuntimeInvocationBindingV1,
    expected_contract: [u8; 32],
) -> std::result::Result<(), MixedWorkerPreparationError> {
    // Both concrete packers retain the entire contract in the shared inert
    // transport. The versioned decoder and exact contract identity remain bound.
    match binding {
        Gfx942RuntimeInvocationBindingV1::ConditionalMixedV26 {
            contract_identity, ..
        } if contract_identity == expected_contract => Ok(()),
        _ => Err(MixedWorkerPreparationError::Binding(
            "exact mixed invocation family and contract",
        )),
    }
}

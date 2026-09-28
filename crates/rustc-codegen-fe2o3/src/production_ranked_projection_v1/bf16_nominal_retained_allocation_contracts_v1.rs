//! Retained allocation-contract DATA only; no authentic source or capability token.
use super::*;
use crate::production_ranked_projection_v1::bf16_nominal_preparation_resources_v1::resource;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, CanonicalKernelIrWorkLedgerIdentityV1,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAbiArgumentV1, SemanticAbiPointeeInfoV1, SemanticAbiRegularAttributesV1,
    SemanticAbiValueAttributesV1, SemanticAbiValueV1, SemanticBackendScalarV1,
    SemanticFunctionAbiV1, SemanticTypeAbiPropertiesV1, SemanticTypeDeclV1, SemanticTypeLayoutV1,
};
use std::mem::size_of;
type Error = ProductionRankedProjectionErrorV1;
type Result<T> = std::result::Result<T, Error>;
type Resources<'b, 'w> = PreparationResourcesV1<'b, 'w>;
type Ledger = (usize, CanonicalKernelIrWorkLedgerIdentityV1);
type SourceKey = [usize; 5];
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Fresh,
    Terminal,
    Complete,
}

/// Retains both original vectors, including accepted partial argument contracts
/// and partial result rows. Input loans must outlive the owner's actual use.
pub(in crate::production_ranked_projection_v1) struct RetainedAllocationContractsV1 {
    phase: Phase,
    ledger: Option<Ledger>,
    source: Option<SourceKey>,
    arguments: Vec<Option<AllocationContractV1>>,
    result: Vec<Option<AllocationContractV1>>,
}
impl RetainedAllocationContractsV1 {
    pub(in crate::production_ranked_projection_v1) fn new() -> Self {
        Self {
            phase: Phase::Fresh,
            ledger: None,
            source: None,
            arguments: Vec::new(),
            result: Vec::new(),
        }
    }
    pub(in crate::production_ranked_projection_v1) fn prepare_into(
        &mut self,
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
        origins: &[Option<u32>],
        resources: &mut Resources<'_, '_>,
    ) -> Result<()> {
        let fresh = self.phase == Phase::Fresh && self.ledger.is_none() && self.source.is_none();
        self.phase = Phase::Terminal;
        if !fresh || !resources.is_metered() || resources.has_denial() {
            return Err(resource(Resource::Accounting));
        }
        let ledger = resources
            .original_ledger_v1()
            .ok_or_else(|| resource(Resource::Accounting))?;
        resources.work(32)?;
        resources.reserve_storage(retained_allocation_frame_v1()?)?;
        self.ledger = Some(ledger);
        self.source = Some(source_key(types, function, origins));
        self.prepare_attached(types, function, origins, resources)?;
        if resources.has_denial() || resources.original_ledger_v1() != Some(ledger) {
            return Err(resource(Resource::Accounting));
        }
        self.phase = Phase::Complete;
        Ok(())
    }
    /// Lexical input/ledger consistency, not authenticated source ownership.
    pub(in crate::production_ranked_projection_v1) fn completed_for<'a>(
        &'a self,
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
        origins: &[Option<u32>],
        resources: &Resources<'_, '_>,
    ) -> Result<&'a [Option<AllocationContractV1>]> {
        if self.phase != Phase::Complete
            || self.ledger.is_none()
            || self.ledger != resources.original_ledger_v1()
            || resources.has_denial()
            || self.source != Some(source_key(types, function, origins))
        {
            return Err(resource(Resource::Accounting));
        }
        Ok(&self.result)
    }

    fn prepare_attached(
        &mut self,
        types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
        origins: &[Option<u32>],
        resources: &mut PreparationResourcesV1<'_, '_>,
    ) -> Result<()> {
        resources
            .reserve_storage(2 * std::mem::size_of::<Vec<Option<AllocationContractV1>>>() + 4096)?;
        if origins.len() != function.locals().len() {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "allocation-origin and semantic-local tables have different lengths",
            ));
        }
        let source_types = function.abi().source_input_types();
        let source_ownership = function.abi().source_argument_ownership();
        let abi_arguments = function.abi().adjusted_arguments();
        if source_ownership.len() != source_types.len() {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "source ownership and semantic argument tables have different lengths",
            ));
        }
        fill_attached(&mut self.arguments, source_types.len(), resources)?;
        for (argument_index, &ty) in source_types.iter().enumerate() {
            resources.work(24)?;
            let type_decl = types.get(ty.index() as usize).ok_or(
                ProductionRankedProjectionErrorV1::Unsupported(
                    "a kernel argument type is outside the semantic type table",
                ),
            )?;
            let abi_argument = abi_arguments.get(argument_index).ok_or(
                ProductionRankedProjectionErrorV1::Unsupported(
                    "a kernel source argument is missing its authenticated FnAbi record",
                ),
            )?;
            let pointee = abi_argument
                .value()
                .pointee_override()
                .or(type_decl.abi_properties().first_pointee());
            let allocation_origin = u64::try_from(argument_index)
                .ok()
                .and_then(|index| index.checked_add(1))
                .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                    "a kernel argument index does not fit the allocation identity space",
                ))?;
            let first_pointer_noalias = match abi_argument.mode() {
                SemanticAbiPassModeV1::Direct(attributes) => attributes.regular().no_alias(),
                SemanticAbiPassModeV1::Pair { first, .. } => first.regular().no_alias(),
                SemanticAbiPassModeV1::Ignore
                | SemanticAbiPassModeV1::Cast { .. }
                | SemanticAbiPassModeV1::Indirect { .. } => false,
            };
            let Some(pointee) = pointee else {
                continue;
            };
            let abi_contract = allocation_contract_from_pointee(
                pointee.kind(),
                first_pointer_noalias,
                allocation_origin,
            );
            let singleton_object = matches!(
                source_ownership[argument_index],
                SemanticSourceArgumentOwnershipV1::ExclusiveOwner
            ) && matches!(type_decl.layout().backend_repr(), SemanticBackendReprV1::Scalar(scalar) if matches!(scalar.primitive(), SemanticBackendPrimitiveV1::Pointer { .. }));
            self.arguments[argument_index] = Some(authenticated_source_allocation_contract_v1(
                source_ownership[argument_index],
                pointee.kind(),
                AllocationContractV1 {
                    singleton_object,
                    ..abi_contract
                },
            )?);
        }
        resources.reserve(&mut self.result, origins.len())?;
        for origin in origins {
            resources.work(1)?;
            resources.push(
                &mut self.result,
                origin.and_then(|origin| self.arguments.get(origin as usize).copied().flatten()),
            )?;
        }
        Ok(())
    }
}

fn source_key(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    origins: &[Option<u32>],
) -> SourceKey {
    [
        function as *const SemanticFunctionDeclV1 as usize,
        types.as_ptr() as usize,
        types.len(),
        origins.as_ptr() as usize,
        origins.len(),
    ]
}
fn fill_attached(
    values: &mut Vec<Option<AllocationContractV1>>,
    count: usize,
    resources: &mut Resources<'_, '_>,
) -> Result<()> {
    // Exact original filled(None) charge/reserve/resize order.
    resources.work(count)?;
    resources.reserve(values, count)?;
    values.resize(count, None);
    Ok(())
}
const FRAME_ROWS: usize = 23;
fn typed_rows() -> Result<[usize; FRAME_ROWS]> {
    Ok([
        size_of::<RetainedAllocationContractsV1>(),
        size_of::<(
            Phase,
            Option<Ledger>,
            Option<SourceKey>,
            Vec<Option<AllocationContractV1>>,
            Vec<Option<AllocationContractV1>>,
        )>(),
        size_of::<(
            &mut RetainedAllocationContractsV1,
            &[SemanticTypeDeclV1],
            &SemanticFunctionDeclV1,
            &[Option<u32>],
            &mut Resources<'static, 'static>,
            bool,
            Ledger,
            Option<Ledger>,
            SourceKey,
            Option<SourceKey>,
            Result<()>,
        )>(),
        size_of::<(
            &RetainedAllocationContractsV1,
            &[SemanticTypeDeclV1],
            &SemanticFunctionDeclV1,
            &[Option<u32>],
            &Resources<'static, 'static>,
            Option<Ledger>,
            SourceKey,
            Option<SourceKey>,
            &Vec<Option<AllocationContractV1>>,
            &[Option<AllocationContractV1>],
            Result<&[Option<AllocationContractV1>]>,
        )>(),
        size_of::<(
            &[SemanticTypeDeclV1],
            &SemanticFunctionDeclV1,
            &[Option<u32>],
            SourceKey,
            usize,
        )>(),
        size_of::<(
            &mut RetainedAllocationContractsV1,
            &[SemanticTypeDeclV1],
            &SemanticFunctionDeclV1,
            &[Option<u32>],
            &mut Resources<'static, 'static>,
            &[SemanticTypeIdV1],
            &[SemanticSourceArgumentOwnershipV1],
            &[SemanticAbiArgumentV1],
            Result<()>,
        )>(),
        size_of::<(
            std::iter::Enumerate<std::slice::Iter<'static, SemanticTypeIdV1>>,
            usize,
            SemanticTypeIdV1,
            &SemanticTypeIdV1,
            &SemanticTypeDeclV1,
            Option<&SemanticTypeDeclV1>,
            &SemanticAbiArgumentV1,
            Option<&SemanticAbiArgumentV1>,
        )>(),
        size_of::<(
            Option<SemanticAbiPointeeInfoV1>,
            SemanticAbiPointeeInfoV1,
            u64,
            u64,
            Option<u64>,
            std::result::Result<u64, std::num::TryFromIntError>,
        )>(),
        size_of::<(
            &SemanticAbiPassModeV1,
            &SemanticAbiValueAttributesV1,
            &SemanticAbiValueAttributesV1,
            SemanticAbiValueAttributesV1,
            SemanticAbiRegularAttributesV1,
            bool,
        )>(),
        size_of::<(
            AllocationContractV1,
            AllocationContractV1,
            AllocationContractV1,
            SemanticSourceArgumentOwnershipV1,
            SemanticAbiPointeeKindV1,
            bool,
        )>(),
        size_of::<(
            SemanticAbiPointeeKindV1,
            bool,
            u64,
            AllocationContractV1,
            (u64, bool),
            u64,
            bool,
        )>(),
        size_of::<(
            SemanticSourceArgumentOwnershipV1,
            SemanticAbiPointeeKindV1,
            AllocationContractV1,
            Result<AllocationContractV1>,
            Error,
        )>(),
        size_of::<(
            &mut Vec<Option<AllocationContractV1>>,
            usize,
            &mut Resources<'static, 'static>,
            Option<AllocationContractV1>,
            Result<()>,
        )>(),
        size_of::<(
            &mut Vec<Option<AllocationContractV1>>,
            usize,
            &mut Resources<'static, 'static>,
            Result<()>,
        )>(),
        size_of::<(
            std::slice::Iter<'static, Option<u32>>,
            &Option<u32>,
            Option<u32>,
            u32,
            usize,
            Option<&Option<AllocationContractV1>>,
            Option<Option<AllocationContractV1>>,
            Option<AllocationContractV1>,
        )>(),
        size_of::<(
            &mut Vec<Option<AllocationContractV1>>,
            Option<AllocationContractV1>,
            &mut Resources<'static, 'static>,
            Result<()>,
        )>(),
        size_of::<(
            &Vec<Option<AllocationContractV1>>,
            &[Option<AllocationContractV1>],
            usize,
            &Option<AllocationContractV1>,
            Option<AllocationContractV1>,
        )>(),
        size_of::<(
            SemanticBackendReprV1,
            &SemanticBackendReprV1,
            &SemanticBackendScalarV1,
            SemanticBackendScalarV1,
            SemanticBackendPrimitiveV1,
            bool,
        )>(),
        size_of::<(Error, Resource, Result<()>, Option<Ledger>, bool)>(),
        size_of::<(
            [usize; FRAME_ROWS],
            Result<[usize; FRAME_ROWS]>,
            std::array::IntoIter<usize, FRAME_ROWS>,
            usize,
            usize,
            Option<usize>,
            Result<usize>,
        )>(),
        size_of::<(Result<usize>, usize, Option<usize>, Resource, Error)>(),
        size_of::<(
            &SemanticFunctionAbiV1,
            &SemanticAbiArgumentV1,
            &SemanticAbiValueV1,
            &SemanticTypeDeclV1,
            &SemanticTypeLayoutV1,
            SemanticTypeAbiPropertiesV1,
            SemanticAbiPointeeInfoV1,
            SemanticAbiPointeeKindV1,
        )>(),
        size_of::<(
            &mut Option<AllocationContractV1>,
            Option<AllocationContractV1>,
            Result<AllocationContractV1>,
        )>(),
    ])
}
pub(in crate::production_ranked_projection_v1) fn retained_allocation_frame_v1() -> Result<usize> {
    typed_rows()?.into_iter().try_fold(0usize, |sum, row| {
        sum.checked_add(row)
            .ok_or_else(|| resource(Resource::Arithmetic))
    })
}
#[cfg(test)]
#[path = "bf16_nominal_retained_allocation_contracts_v1_tests.rs"]
mod tests;

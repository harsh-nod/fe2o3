//! Retained provenance DATA dependency, not a source-authorized checkpoint.
use super::*;
use crate::production_ranked_projection_v1::bf16_nominal_preparation_resources_v1::resource;
use crate::production_ranked_projection_v1::exclusive_owner_carrier_v1::RetainedExclusiveCarrierV1;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, CanonicalKernelIrWorkLedgerIdentityV1,
};
use fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1;
use std::mem::size_of;
type Error = ProductionRankedProjectionErrorV1;
type Result<T> = std::result::Result<T, Error>;
type Resources<'b, 'w> = PreparationResourcesV1<'b, 'w>;
type Ledger = (usize, CanonicalKernelIrWorkLedgerIdentityV1);
type SourceKey = [usize; 9];
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Fresh,
    Terminal,
    Complete,
}

pub(in crate::production_ranked_projection_v1) struct RetainedLocalProvenanceV1 {
    phase: Phase,
    ledger: Option<Ledger>,
    source: Option<SourceKey>,
    carrier: RetainedExclusiveCarrierV1,
    carrier_invoked: bool,
    data: LocalProvenanceV1,
    stable_edges: Vec<Vec<usize>>,
    allocation_edges: Vec<Vec<usize>>,
    allocation_contract_edges: Vec<Vec<usize>>,
    edge_count: usize,
    stable_queue: RetainedExactOriginWorklistV1,
    allocation_queue: RetainedExactOriginWorklistV1,
    contract_queue: RetainedExactOriginWorklistV1,
}
impl RetainedLocalProvenanceV1 {
    pub(in crate::production_ranked_projection_v1) fn new() -> Self {
        Self {
            phase: Phase::Fresh,
            ledger: None,
            source: None,
            carrier: RetainedExclusiveCarrierV1::new(),
            carrier_invoked: false,
            data: LocalProvenanceV1 {
                stable_argument_origins: Vec::new(),
                allocation_origins: Vec::new(),
                allocation_provenance: Vec::new(),
            },
            stable_edges: Vec::new(),
            allocation_edges: Vec::new(),
            allocation_contract_edges: Vec::new(),
            edge_count: 0,
            stable_queue: RetainedExactOriginWorklistV1::new(),
            allocation_queue: RetainedExactOriginWorklistV1::new(),
            contract_queue: RetainedExactOriginWorklistV1::new(),
        }
    }
    pub(in crate::production_ranked_projection_v1) fn prepare_into(
        &mut self,
        callables: &[SemanticCallableDeclV1],
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
        definitions: &[u8],
        address_escaped: &[bool],
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
        resources.reserve_storage(retained_provenance_frame_v1()?)?;
        self.ledger = Some(ledger);
        self.source = Some(source_key(
            callables,
            types,
            function,
            definitions,
            address_escaped,
        ));
        self.prepare_attached(
            callables,
            types,
            function,
            definitions,
            address_escaped,
            resources,
        )?;
        if resources.has_denial()
            || resources.original_ledger_v1() != Some(ledger)
            || !self.carrier_invoked
            || !self.stable_queue.completed(resources)
            || !self.allocation_queue.completed(resources)
            || !self.contract_queue.completed(resources)
        {
            return Err(resource(Resource::Accounting));
        }
        self.phase = Phase::Complete;
        Ok(())
    }
    /// Exact lexical input/ledger consistency only; all source loans must remain
    /// alive. No globally unique identity, authenticated source or readiness token.
    pub(in crate::production_ranked_projection_v1) fn completed_for<'a>(
        &'a self,
        callables: &[SemanticCallableDeclV1],
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
        definitions: &[u8],
        address_escaped: &[bool],
        resources: &Resources<'_, '_>,
    ) -> Result<&'a LocalProvenanceV1> {
        if self.phase != Phase::Complete
            || self.ledger.is_none()
            || self.ledger != resources.original_ledger_v1()
            || resources.has_denial()
            || self.source
                != Some(source_key(
                    callables,
                    types,
                    function,
                    definitions,
                    address_escaped,
                ))
        {
            return Err(resource(Resource::Accounting));
        }
        Ok(&self.data)
    }

    fn prepare_attached(
        &mut self,
        callables: &[SemanticCallableDeclV1],
        types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
        definitions: &[u8],
        address_escaped: &[bool],
        resources: &mut Resources<'_, '_>,
    ) -> Result<()> {
        resources.reserve_storage(
            std::mem::size_of::<LocalProvenanceV1>()
                + 3 * std::mem::size_of::<Vec<Vec<usize>>>()
                + std::mem::size_of::<Vec<Option<u32>>>()
                + 4096,
        )?;
        let local_count = function.locals().len();
        if definitions.len() != local_count || address_escaped.len() != local_count {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "local provenance scalar custody tables do not match the semantic local table",
            ));
        }
        self.carrier_invoked = true;
        self.carrier.prepare_into(
            callables,
            function,
            definitions,
            MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1,
            resources,
        )?;
        let exclusive_owner_origins = self
            .carrier
            .completed_for(callables, function, definitions, resources)?
            .0;
        fill_attached(
            &mut self.data.stable_argument_origins,
            local_count,
            None,
            resources,
        )?;
        fill_attached(
            &mut self.data.allocation_origins,
            local_count,
            None,
            resources,
        )?;
        fill_attached(
            &mut self.data.allocation_provenance,
            local_count,
            None,
            resources,
        )?;
        nested_attached(&mut self.stable_edges, local_count, resources)?;
        nested_attached(&mut self.allocation_edges, local_count, resources)?;
        nested_attached(&mut self.allocation_contract_edges, local_count, resources)?;
        for (local_index, local) in function.locals().iter().enumerate() {
            resources.work(1)?;
            if let SemanticLocalRoleV1::Argument(argument) = local.role() {
                if definitions[local_index] == 0 && !address_escaped[local_index] {
                    self.data.stable_argument_origins[local_index] = Some(argument);
                }
                self.data.allocation_origins[local_index] = Some(argument);
                self.data.allocation_provenance[local_index] =
                    Some(LocalAllocationProvenanceV1::Argument(argument));
            }
        }
        for block in function.blocks() {
            resources.work(1)?;
            for statement in block.statements() {
                resources.work(32)?;
                let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                    continue;
                };
                let destination = assignment.destination();
                if !destination.projections().is_empty()
                    || definitions
                        .get(destination.local().index() as usize)
                        .copied()
                        != Some(1)
                {
                    continue;
                }
                let destination = destination.local().index() as usize;
                prepay_provenance_spines_v1(assignment.value().kind(), resources)?;
                let stable_source = match assignment.value().kind() {
                    SemanticRvalueKindV1::Use(operand)
                    | SemanticRvalueKindV1::Cast { operand, .. } => simple_operand_local(operand),
                    _ => None,
                };
                if let Some(source) = stable_source {
                    let source = source.index() as usize;
                    if address_escaped.get(source).copied() == Some(false)
                        && !address_escaped[destination]
                    {
                        push_local_provenance_edge_with_resources_v1(
                            &mut self.stable_edges,
                            source,
                            destination,
                            &mut self.edge_count,
                            resources,
                        )?;
                    }
                };

                let allocation_source = match assignment.value().kind() {
                    SemanticRvalueKindV1::Use(operand) => {
                        allocation_operand_local_v1(types, function, operand)
                    }
                    SemanticRvalueKindV1::Cast {
                        kind: SemanticCastKindV1::Pointer,
                        operand,
                    } => simple_operand_local(operand),
                    SemanticRvalueKindV1::Borrow { place, .. } => {
                        borrowed_allocation_local_v1(function, place, exclusive_owner_origins)
                    }
                    SemanticRvalueKindV1::AddressOf { place, .. } => {
                        reborrowed_allocation_local_v1(place)
                    }
                    _ => None,
                };
                if let Some(source) = allocation_source {
                    push_local_provenance_edge_with_resources_v1(
                        &mut self.allocation_edges,
                        source.index() as usize,
                        destination,
                        &mut self.edge_count,
                        resources,
                    )?;
                    push_local_provenance_edge_with_resources_v1(
                        &mut self.allocation_contract_edges,
                        source.index() as usize,
                        destination,
                        &mut self.edge_count,
                        resources,
                    )?;
                } else if let SemanticRvalueKindV1::Borrow { place, .. }
                | SemanticRvalueKindV1::AddressOf { place, .. } =
                    assignment.value().kind()
                    && place.projections().is_empty()
                    && function
                        .locals()
                        .get(place.local().index() as usize)
                        .is_some_and(|local| {
                            !matches!(local.role(), SemanticLocalRoleV1::Argument(_))
                        })
                {
                    let origin = LocalAllocationProvenanceV1::Private(place.local());
                    match self.data.allocation_provenance.get_mut(destination) {
                        Some(slot @ None) => *slot = Some(origin),
                        Some(Some(existing)) if *existing == origin => {}
                        Some(Some(_)) => {
                            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                                "a local may alias multiple kernel allocation origins",
                            ));
                        }
                        None => {
                            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                                "a private allocation root is outside the semantic local table",
                            ));
                        }
                    }
                }

                if let SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::Offset,
                    left,
                    ..
                } = assignment.value().kind()
                    && let Some(source) = allocation_operand_local_v1(types, function, left)
                {
                    // Pointer offsets retain only an authenticated external allocation
                    // contract. Private roots have no size/range contract and must not
                    // gain address-formation authority through raw pointer arithmetic.
                    push_local_provenance_edge_with_resources_v1(
                        &mut self.allocation_contract_edges,
                        source.index() as usize,
                        destination,
                        &mut self.edge_count,
                        resources,
                    )?;
                }
            }
        }

        self.stable_queue.prepare_into(
            &mut self.data.stable_argument_origins,
            &self.stable_edges,
            "a runtime index may derive from multiple kernel arguments",
            resources,
        )?;
        self.allocation_queue.prepare_into(
            &mut self.data.allocation_provenance,
            &self.allocation_edges,
            "a local may alias multiple kernel allocation origins",
            resources,
        )?;
        self.contract_queue.prepare_into(
            &mut self.data.allocation_origins,
            &self.allocation_contract_edges,
            "a local may alias multiple kernel allocation origins",
            resources,
        )?;
        Ok(())
    }
}
fn source_key(
    callables: &[SemanticCallableDeclV1],
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    definitions: &[u8],
    address_escaped: &[bool],
) -> SourceKey {
    [
        function as *const SemanticFunctionDeclV1 as usize,
        callables.as_ptr() as usize,
        callables.len(),
        types.as_ptr() as usize,
        types.len(),
        definitions.as_ptr() as usize,
        definitions.len(),
        address_escaped.as_ptr() as usize,
        address_escaped.len(),
    ]
}
fn fill_attached<T: Clone>(
    values: &mut Vec<T>,
    count: usize,
    value: T,
    resources: &mut Resources<'_, '_>,
) -> Result<()> {
    resources.work(count)?;
    resources.reserve(values, count)?;
    values.resize(count, value);
    Ok(())
}
fn nested_attached(
    values: &mut Vec<Vec<usize>>,
    count: usize,
    resources: &mut Resources<'_, '_>,
) -> Result<()> {
    resources.work(count)?;
    resources.reserve(values, count)?;
    values.resize_with(count, Vec::new);
    Ok(())
}
const FRAME_ROWS: usize = 25;
fn typed_rows() -> Result<[usize; FRAME_ROWS]> {
    Ok([
        size_of::<RetainedLocalProvenanceV1>(),
        size_of::<(
            RetainedExclusiveCarrierV1,
            LocalProvenanceV1,
            [Vec<Vec<usize>>; 3],
            [RetainedExactOriginWorklistV1; 3],
            Phase,
            Option<Ledger>,
            Option<SourceKey>,
            bool,
            usize,
        )>(),
        size_of::<(
            &mut RetainedLocalProvenanceV1,
            &[SemanticCallableDeclV1],
            &[SemanticTypeDeclV1],
            &SemanticFunctionDeclV1,
            &[u8],
            &[bool],
            &mut Resources<'static, 'static>,
            bool,
            Ledger,
            Option<Ledger>,
            SourceKey,
            Option<SourceKey>,
            Result<()>,
        )>(),
        size_of::<(
            &RetainedLocalProvenanceV1,
            &[SemanticCallableDeclV1],
            &[SemanticTypeDeclV1],
            &SemanticFunctionDeclV1,
            &[u8],
            &[bool],
            &Resources<'static, 'static>,
            Option<Ledger>,
            SourceKey,
            Option<SourceKey>,
            Result<&LocalProvenanceV1>,
        )>(),
        size_of::<(
            &[SemanticCallableDeclV1],
            &[SemanticTypeDeclV1],
            &SemanticFunctionDeclV1,
            &[u8],
            &[bool],
            SourceKey,
            usize,
        )>(),
        size_of::<(
            &mut RetainedLocalProvenanceV1,
            &[SemanticCallableDeclV1],
            &[SemanticTypeDeclV1],
            &SemanticFunctionDeclV1,
            &[u8],
            &[bool],
            &mut Resources<'static, 'static>,
            usize,
            &[Option<u32>],
            Result<()>,
        )>(),
        size_of::<(
            &mut RetainedExclusiveCarrierV1,
            &[SemanticCallableDeclV1],
            &SemanticFunctionDeclV1,
            &[u8],
            usize,
            &mut Resources<'static, 'static>,
            Result<()>,
        )>(),
        size_of::<(
            &RetainedExclusiveCarrierV1,
            &[SemanticCallableDeclV1],
            &SemanticFunctionDeclV1,
            &[u8],
            &Resources<'static, 'static>,
            (&[Option<u32>], usize),
            Result<(&[Option<u32>], usize)>,
        )>(),
        fill_frame::<Option<u32>>(),
        fill_frame::<Option<u32>>(),
        fill_frame::<Option<LocalAllocationProvenanceV1>>(),
        nested_frame(),
        nested_frame(),
        nested_frame(),
        size_of::<(
            &mut Vec<Vec<usize>>,
            &mut [Vec<usize>],
            usize,
            usize,
            &mut usize,
            &mut Resources<'static, 'static>,
            Result<()>,
        )>(),
        size_of::<(
            &SemanticFunctionDeclV1,
            &SemanticPlaceV1,
            &[Option<u32>],
            Option<SemanticLocalIdV1>,
        )>(),
        size_of::<(
            LocalAllocationProvenanceV1,
            Option<&mut Option<LocalAllocationProvenanceV1>>,
            &mut Option<LocalAllocationProvenanceV1>,
            &mut LocalAllocationProvenanceV1,
        )>(),
        queue_frame::<u32>(),
        queue_frame::<LocalAllocationProvenanceV1>(),
        queue_frame::<u32>(),
        size_of::<(
            &RetainedExactOriginWorklistV1,
            &Resources<'static, 'static>,
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
            &SemanticRvalueKindV1,
            &mut Resources<'static, 'static>,
            Result<()>,
        )>(),
    ])
}
fn fill_frame<T>() -> usize {
    size_of::<(
        &mut Vec<T>,
        usize,
        T,
        &mut Resources<'static, 'static>,
        Result<()>,
        Result<()>,
    )>()
}
fn nested_frame() -> usize {
    size_of::<(
        &mut Vec<Vec<usize>>,
        usize,
        &mut Resources<'static, 'static>,
        Result<()>,
        Result<()>,
    )>()
}
fn queue_frame<T: Copy + Eq>() -> usize {
    size_of::<(
        &mut RetainedExactOriginWorklistV1,
        &mut Vec<Option<T>>,
        &Vec<Vec<usize>>,
        &mut [Option<T>],
        &[Vec<usize>],
        &'static str,
        &mut Resources<'static, 'static>,
        Result<()>,
    )>()
}
pub(in crate::production_ranked_projection_v1) fn retained_provenance_frame_v1() -> Result<usize> {
    typed_rows()?.into_iter().try_fold(0usize, |sum, row| {
        sum.checked_add(row)
            .ok_or_else(|| resource(Resource::Arithmetic))
    })
}
#[cfg(test)]
#[path = "bf16_nominal_retained_local_provenance_v1_tests.rs"]
mod tests;

//! Genuine same-source Option/enum/scalar/provenance/allocation observation, separate from historical modes.
use super::*;
use crate::production_ranked_projection_v1::bf16_nominal_source_algorithms_v1::assertion_definition_inventory_with_resources_v1;
use crate::production_ranked_projection_v1::bf16_nominal_source_algorithms_v1::local_allocation_contracts_with_resources_v1;
use crate::production_ranked_projection_v1::bf16_nominal_source_algorithms_v1::local_provenance_with_resources_v1;
use fe2o3_lower_mir_kernel::CheckedBf16CallInstanceV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Compare,
    CallbackError,
    CallbackPanic,
}
#[derive(Clone, Copy, Debug)]
pub(super) struct Observation {
    option_producers: usize,
    option_work: usize,
    enum_work: usize,
    enum_invoked: bool,
    scalar_locals: usize,
    scalar_blocks: usize,
    scalar_definitions: usize,
    scalar_assignments: usize,
    scalar_invoked: bool,
    provenance_locals: usize,
    stable_origins: usize,
    allocation_origins: usize,
    private_origins: usize,
    provenance_invoked: bool,
    allocation_locals: usize,
    allocation_contracts: usize,
    writable_contracts: usize,
    singleton_contracts: usize,
    allocation_invoked: bool,
}
const CALLBACK_ERROR: &str = "Option/enum/scalar/provenance/allocation genuine callback control";

fn comparison_frame() -> usize {
    size_of::<(
        Vec<Option<AllocationContractV1>>,
        BResult<Vec<Option<AllocationContractV1>>>,
        &Vec<Option<AllocationContractV1>>,
        &[Option<AllocationContractV1>],
        &[Option<AllocationContractV1>],
        &Vec<Option<u32>>,
        &[Option<u32>],
        usize,
        usize,
        usize,
        (usize, usize, usize),
        BResult<(usize, usize, usize)>,
        LocalProvenanceV1,
        BResult<LocalProvenanceV1>,
        &LocalProvenanceV1,
        &Vec<u8>,
        &[u8],
        &Vec<bool>,
        &[bool],
        usize,
        usize,
        usize,
        (usize, usize, usize),
        BResult<(usize, usize, usize)>,
        AssertionDefinitionInventoryV1,
        BResult<AssertionDefinitionInventoryV1>,
        &AssertionDefinitionInventoryV1,
        usize,
        usize,
        (usize, usize),
        BResult<(usize, usize)>,
        Ledger,
        Option<Ledger>,
        Vec<SemanticOptionProducerV1>,
        SemanticOptionDominanceV1,
        SemanticEnumPayloadDominanceV1,
        BResult<Vec<SemanticOptionProducerV1>>,
        BResult<SemanticOptionDominanceV1>,
        BResult<SemanticEnumPayloadDominanceV1>,
        SemanticEnumPayloadMeteredErrorV1<Backend>,
        BeforeCapabilitiesV1<'static>,
        &mut Prep<'static, 'static>,
        ModelMeter<'static, 'static, 'static>,
        &SemanticFunctionDeclV1,
        &[SemanticCallableDeclV1],
        &[SemanticTypeDeclV1],
        &[SemanticOptionProducerV1],
        &SemanticOptionDominanceV1,
        &SemanticEnumPayloadDominanceV1,
        Observation,
        Option<Observation>,
        BResult<Observation>,
        usize,
        usize,
        usize,
        Option<usize>,
        bool,
        Backend,
        &Backend,
    )>()
}
pub(super) fn compare(
    view: BeforeCapabilitiesV1<'_>,
    resources: &mut Prep<'_, '_>,
) -> BResult<Observation> {
    resources.reserve_storage(comparison_frame())?;
    if resources.original_ledger_v1() != Some(view.earlier.earlier.earlier.options.ledger)
        || resources.has_denial()
        || !view.enum_api_invoked()
        || !view.scalar_api_invoked()
        || !view.provenance_api_invoked()
        || !view.allocation_api_invoked()
    {
        return Err(accounting());
    }
    // These unchanged original APIs construct expectations from the actual
    // source, never from the retained candidate arrays or phase flags.
    let expected_producers = fe2o3_mir_model::semantic_option_producers_with_meter_v1(
        view.function(),
        view.earlier.earlier.earlier.options.callables,
        &mut ModelMeter(resources),
    )
    .map_err(model_error)?;
    let expected_options = SemanticOptionDominanceV1::analyze_with_meter_v1(
        view.function(),
        &expected_producers,
        &mut ModelMeter(resources),
    )
    .map_err(model_error)?;
    let expected_enum = SemanticEnumPayloadDominanceV1::analyze_with_meter_v1(
        view.function(),
        view.earlier.earlier.earlier.options.types,
        &mut ModelMeter(resources),
    )
    .map_err(model_error)?;
    let expected_scalar =
        assertion_definition_inventory_with_resources_v1(view.function(), resources)?;
    let (scalar_definitions, scalar_assignments) =
        compare_scalar(&expected_scalar, view.scalar_inventory(), resources)?;
    let expected_provenance = local_provenance_with_resources_v1(
        view.earlier.earlier.earlier.options.callables,
        view.earlier.earlier.earlier.options.types,
        view.function(),
        &expected_scalar.counts,
        &expected_scalar.address_escaped,
        resources,
    )?;
    let (stable_origins, allocation_origins, private_origins) =
        compare_provenance(&expected_provenance, view.provenance(), resources)?;
    // The unchanged allocation oracle uses independently recomputed original
    // provenance origins, never the candidate origin table.
    let expected_allocation = local_allocation_contracts_with_resources_v1(
        view.earlier.earlier.earlier.options.types,
        view.function(),
        &expected_provenance.allocation_origins,
        resources,
    )?;
    let (allocation_contracts, writable_contracts, singleton_contracts) =
        compare_allocation(&expected_allocation, view.allocation_contracts(), resources)?;
    let work = expected_options
        .work_units()
        .checked_add(expected_enum.work_units())
        .and_then(|n| n.checked_add(view.option_dominance().work_units()))
        .and_then(|n| n.checked_add(view.enum_dominance().work_units()))
        .and_then(|n| n.checked_mul(4))
        .and_then(|n| {
            view.function()
                .locals()
                .len()
                .checked_mul(8)
                .and_then(|x| n.checked_add(x))
        })
        .and_then(|n| {
            expected_producers
                .len()
                .checked_mul(8)
                .and_then(|x| n.checked_add(x))
        })
        .and_then(|n| n.checked_add(64))
        .ok_or_else(arithmetic)?;
    resources.work(work)?;
    if expected_producers.as_slice() != view.option_producers()
        || &expected_options != view.option_dominance()
        || &expected_enum != view.enum_dominance()
        || expected_producers.is_empty()
        || expected_options.work_units() == 0
        || expected_enum.work_units() == 0
        || view.enum_dominance().grants_authority()
        || expected_scalar.counts.is_empty()
        || expected_scalar.blocks.is_empty()
        || scalar_definitions == 0
        || scalar_assignments == 0
        || expected_provenance.stable_argument_origins.len() != expected_scalar.counts.len()
        || expected_provenance.allocation_origins.len() != expected_scalar.counts.len()
        || expected_provenance.allocation_provenance.len() != expected_scalar.counts.len()
        || expected_allocation.len() != expected_scalar.counts.len()
    {
        return Err(Backend::Incomplete(
            "Option/enum/scalar/provenance/allocation genuine original/candidate DATA differs",
        ));
    }
    let observed = Observation {
        option_producers: expected_producers.len(),
        option_work: expected_options.work_units(),
        enum_work: expected_enum.work_units(),
        enum_invoked: view.enum_api_invoked(),
        scalar_locals: expected_scalar.counts.len(),
        scalar_blocks: expected_scalar.blocks.len(),
        scalar_definitions,
        scalar_assignments,
        scalar_invoked: view.scalar_api_invoked(),
        provenance_locals: expected_provenance.stable_argument_origins.len(),
        stable_origins,
        allocation_origins,
        private_origins,
        provenance_invoked: view.provenance_api_invoked(),
        allocation_locals: expected_allocation.len(),
        allocation_contracts,
        writable_contracts,
        singleton_contracts,
        allocation_invoked: view.allocation_api_invoked(),
    };
    // Enum availability may legitimately be empty. Successful actual retained
    // invocation/completion plus complete original DATA equality is required;
    // a prefilled empty result or skipped analyzer cannot satisfy this entry.
    // All-None allocation rows are legitimate, but actual invocation/completion
    // and equality of every original row remain required.
    drop(expected_allocation);
    drop(expected_provenance);
    drop(expected_scalar);
    drop(expected_enum);
    drop(expected_options);
    drop(expected_producers);
    Ok(observed)
}

fn scalar_comparison_frame() -> usize {
    size_of::<(
        &AssertionDefinitionInventoryV1,
        &AssertionDefinitionInventoryV1,
        &mut Prep<'static, 'static>,
        usize,
        usize,
        usize,
        usize,
        usize,
        [&AssertionDefinitionInventoryV1; 2],
        std::array::IntoIter<&'static AssertionDefinitionInventoryV1, 2>,
        &AssertionDefinitionInventoryV1,
        &[Vec<usize>],
        std::slice::Iter<'static, Vec<usize>>,
        &Vec<usize>,
        std::slice::Iter<'static, u8>,
        &u8,
        std::slice::Iter<'static, Option<ScalarAssignmentSiteV1>>,
        &Option<ScalarAssignmentSiteV1>,
        &Vec<u8>,
        &Vec<u8>,
        &[u8],
        &[u8],
        &Vec<Vec<usize>>,
        &Vec<Vec<usize>>,
        &[Vec<usize>],
        &[Vec<usize>],
        &Vec<Option<ScalarAssignmentSiteV1>>,
        &Vec<Option<ScalarAssignmentSiteV1>>,
        &[Option<ScalarAssignmentSiteV1>],
        &[Option<ScalarAssignmentSiteV1>],
        &Vec<bool>,
        &Vec<bool>,
        &[bool],
        &[bool],
        BResult<()>,
        BResult<usize>,
        Backend,
        Option<usize>,
        BResult<(usize, usize)>,
        (usize, usize),
        bool,
    )>()
}
fn compare_scalar(
    expected: &AssertionDefinitionInventoryV1,
    actual: &AssertionDefinitionInventoryV1,
    resources: &mut Prep<'_, '_>,
) -> BResult<(usize, usize)> {
    resources.reserve_storage(scalar_comparison_frame())?;
    resources.work(64)?;
    let mut work = 0usize;
    for data in [expected, actual] {
        // Prepay length and block-header scans before traversing either table.
        resources.work(data.blocks.len())?;
        work = work
            .checked_add(data.counts.len())
            .and_then(|n| n.checked_add(data.assignments.len()))
            .and_then(|n| n.checked_add(data.address_escaped.len()))
            .and_then(|n| n.checked_add(data.blocks.len()))
            .ok_or_else(arithmetic)?;
        for block in &data.blocks {
            work = work.checked_add(block.len()).ok_or_else(arithmetic)?;
        }
    }
    resources.work(work.checked_mul(4).ok_or_else(arithmetic)?)?;
    if expected.counts != actual.counts
        || expected.blocks != actual.blocks
        || expected.assignments != actual.assignments
        || expected.address_escaped != actual.address_escaped
    {
        return Err(Backend::Incomplete(
            "Option/enum/scalar/provenance/allocation original scalar DATA differs",
        ));
    }
    let mut definitions = 0usize;
    for count in &expected.counts {
        definitions = definitions
            .checked_add(usize::from(*count))
            .ok_or_else(arithmetic)?;
    }
    let mut assignments = 0usize;
    for site in &expected.assignments {
        if site.is_some() {
            assignments = assignments.checked_add(1).ok_or_else(arithmetic)?;
        }
    }
    Ok((definitions, assignments))
}

fn provenance_comparison_frame() -> usize {
    size_of::<(
        &LocalProvenanceV1,
        &LocalProvenanceV1,
        &mut Prep<'static, 'static>,
        usize,
        usize,
        usize,
        usize,
        usize,
        Option<usize>,
        &Vec<Option<u32>>,
        &Vec<Option<u32>>,
        &Vec<Option<u32>>,
        &Vec<Option<u32>>,
        &[Option<u32>],
        &[Option<u32>],
        &[Option<u32>],
        &[Option<u32>],
        &Vec<Option<LocalAllocationProvenanceV1>>,
        &Vec<Option<LocalAllocationProvenanceV1>>,
        &[Option<LocalAllocationProvenanceV1>],
        &[Option<LocalAllocationProvenanceV1>],
        std::slice::Iter<'static, Option<u32>>,
        std::slice::Iter<'static, Option<u32>>,
        &Option<u32>,
        &Option<u32>,
        std::slice::Iter<'static, Option<LocalAllocationProvenanceV1>>,
        &Option<LocalAllocationProvenanceV1>,
        (usize, usize, usize),
        BResult<(usize, usize, usize)>,
        BResult<()>,
        Backend,
        bool,
    )>()
}
fn compare_provenance(
    expected: &LocalProvenanceV1,
    actual: &LocalProvenanceV1,
    resources: &mut Prep<'_, '_>,
) -> BResult<(usize, usize, usize)> {
    resources.reserve_storage(provenance_comparison_frame())?;
    resources.work(64)?;
    let work = expected
        .stable_argument_origins
        .len()
        .checked_add(expected.allocation_origins.len())
        .and_then(|n| n.checked_add(expected.allocation_provenance.len()))
        .and_then(|n| n.checked_add(actual.stable_argument_origins.len()))
        .and_then(|n| n.checked_add(actual.allocation_origins.len()))
        .and_then(|n| n.checked_add(actual.allocation_provenance.len()))
        .and_then(|n| n.checked_mul(6))
        .ok_or_else(arithmetic)?;
    resources.work(work)?;
    if expected.stable_argument_origins != actual.stable_argument_origins
        || expected.allocation_origins != actual.allocation_origins
        || expected.allocation_provenance != actual.allocation_provenance
    {
        return Err(Backend::Incomplete(
            "pre-allocation original provenance DATA differs",
        ));
    }
    let mut stable = 0usize;
    for origin in &expected.stable_argument_origins {
        if origin.is_some() {
            stable = stable.checked_add(1).ok_or_else(arithmetic)?;
        }
    }
    let mut allocations = 0usize;
    for origin in &expected.allocation_origins {
        if origin.is_some() {
            allocations = allocations.checked_add(1).ok_or_else(arithmetic)?;
        }
    }
    let mut private = 0usize;
    for origin in &expected.allocation_provenance {
        if matches!(origin, Some(LocalAllocationProvenanceV1::Private(_))) {
            private = private.checked_add(1).ok_or_else(arithmetic)?;
        }
    }
    Ok((stable, allocations, private))
}

fn allocation_comparison_frame() -> usize {
    size_of::<(
        &[Option<AllocationContractV1>],
        &[Option<AllocationContractV1>],
        &mut Prep<'static, 'static>,
        usize,
        usize,
        usize,
        usize,
        usize,
        Option<usize>,
        bool,
        Backend,
        BResult<()>,
        std::slice::Iter<'static, Option<AllocationContractV1>>,
        &Option<AllocationContractV1>,
        &AllocationContractV1,
        (usize, usize, usize),
        BResult<(usize, usize, usize)>,
    )>()
}
fn compare_allocation(
    expected: &[Option<AllocationContractV1>],
    actual: &[Option<AllocationContractV1>],
    resources: &mut Prep<'_, '_>,
) -> BResult<(usize, usize, usize)> {
    resources.reserve_storage(allocation_comparison_frame())?;
    resources.work(64)?;
    let work = expected
        .len()
        .checked_add(actual.len())
        .and_then(|n| n.checked_mul(8))
        .ok_or_else(arithmetic)?;
    resources.work(work)?;
    if expected != actual {
        return Err(Backend::Incomplete(
            "pre-capability original allocation DATA differs",
        ));
    }
    let mut contracts = 0usize;
    let mut writable = 0usize;
    let mut singleton = 0usize;
    for contract in expected {
        if let Some(contract) = contract {
            contracts = contracts.checked_add(1).ok_or_else(arithmetic)?;
            if contract.writable {
                writable = writable.checked_add(1).ok_or_else(arithmetic)?;
            }
            if contract.singleton_object {
                singleton = singleton.checked_add(1).ok_or_else(arithmetic)?;
            }
        }
    }
    Ok((contracts, writable, singleton))
}

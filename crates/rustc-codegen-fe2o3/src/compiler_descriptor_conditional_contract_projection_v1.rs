//! Bounded producer projection, not receipt, CPU-source, or launch authority.
//! The enclosing production owner must keep the source/reference join and the
//! original account live. Copied bytes, views, and identities remain inert.
#![allow(dead_code, reason = "integration retains the existing finalizer gate")]
#![allow(
    clippy::result_large_err,
    reason = "preserve replay errors without allocation"
)]

use super::conditional_generated_fields_v1::ConditionalGeneratedFieldsV1;
use fe2o3_functional_proof::{
    FunctionalRefinementBindingV2, FunctionalRefinementBoundaryV2, FunctionalRefinementSubjectsV2,
    SafeReferenceKindV2,
};
use fe2o3_kernel_descriptor::{
    CONDITIONAL_INVOCATION_CODEC_STORAGE_V1, ConditionalAddressDomainV1 as Domain,
    ConditionalArgumentBindingV1 as Argument, ConditionalArgumentRoleV1 as Role,
    ConditionalCanonicalLocationV1 as CanonicalLocation,
    ConditionalInvocationContractInputV1 as Input, ConditionalInvocationContractV1 as View,
    ConditionalInvocationWireErrorV1, ConditionalNumericalDomainV1, ConditionalOutputV1 as Output,
    ConditionalRankedLocationV1 as RankedLocation, ConditionalRankedValueV1 as RankedValue,
    ConditionalReadOccurrenceV1 as Read, ConditionalReferenceKindV1,
    ConditionalRuntimePremiseV1 as Premise, ConditionalSubjectsV1 as Subjects,
    ConditionalTheoremV1 as Theorem, MAX_CONDITIONAL_ARGUMENTS_V1,
    MAX_CONDITIONAL_INVOCATION_BYTES_V1, MAX_CONDITIONAL_PREMISES_V1, MAX_CONDITIONAL_READS_V1,
    MAX_CONDITIONAL_ROOTS_V1, decode_conditional_invocation_contract_v1,
    encode_conditional_invocation_contract_v1, encoded_conditional_invocation_contract_v1_len,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, ConditionalTotalViewAddressDomainV1,
    FunctionOperationLocation,
};
use fe2o3_pliron::{
    ProductionConditionalAggregateErrorV1, ProductionConditionalRuntimePremiseV1,
    ProductionPolicyCheckedRefinementStagingV2, ProductionRankedValueV1,
};
use fe2o3_verifier::ProductionConditionalFormulaExecutionV1;
use std::{
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
};

#[derive(Debug)]
pub(crate) enum ConditionalContractProjectionErrorV1 {
    Resource(Resource),
    Codec(ConditionalInvocationWireErrorV1<Resource>),
    Replay(ProductionConditionalAggregateErrorV1),
    Mismatch(&'static str),
}
type Error = ConditionalContractProjectionErrorV1;
impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl From<ConditionalInvocationWireErrorV1<Resource>> for Error {
    fn from(error: ConditionalInvocationWireErrorV1<Resource>) -> Self {
        Self::Codec(error)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "conditional contract projection: {self:?}")
    }
}
impl std::error::Error for Error {}

const EMPTY_ARGUMENT: Argument = Argument {
    canonical_parameter: 0,
    source_argument: 0,
    adjusted_argument: 0,
    semantic_local: 0,
    semantic_type: 0,
    generated_field: 0,
    role: Role::Input,
    source_type_identity: [0; 32],
    device_layout_identity: [0; 32],
};
const EMPTY_READ: Read = Read {
    argument: 0,
    canonical: CanonicalLocation {
        block: 0,
        operation: 0,
    },
    slice: 0,
    pointer: 0,
    index: 0,
    value: 0,
    ranked: RankedLocation {
        block: 0,
        operation: 0,
    },
    ranked_view: RankedValue::Argument(0),
    ranked_index: RankedValue::Argument(0),
    access_domain: Domain::GuardedOutput,
    address_domain: Domain::GuardedOutput,
    element_bytes: 0,
    alignment: 0,
};
struct Scratch {
    arguments: [Argument; MAX_CONDITIONAL_ARGUMENTS_V1],
    reads: [Read; MAX_CONDITIONAL_READS_V1],
    premises: [Premise; MAX_CONDITIONAL_PREMISES_V1],
}
// Include coexisting getter copies, binding/subject comparisons, and row
// temporaries. Roots are borrowed in their original order, never reconstructed.
const PROJECTION_STORAGE: usize = size_of::<Scratch>()
    + size_of::<Input<'static>>()
    + 4 * size_of::<FunctionalRefinementBindingV2>()
    + 4 * size_of::<Subjects>()
    + 4 * size_of::<Theorem>()
    + 8 * size_of::<Read>()
    + 8 * size_of::<Argument>()
    + 1024;
const ENCODING_STORAGE: usize =
    MAX_CONDITIONAL_INVOCATION_BYTES_V1 + CONDITIONAL_INVOCATION_CODEC_STORAGE_V1;

/// Runs only inside the checked generated-fields and executed-formula scopes.
/// There is no caller-authored Input, receipt wire, kernel id, or root roster at
/// this entry. The codec crosschecks the exact existing theorem preimage; it
/// does not reclassify the shared-IEEE implication as machine refinement.
///
/// `consume` borrows `view.canonical_bytes()` and may charge and copy them on
/// this same budget. This function releases only its own scratch. However,
/// `with_generated_fields_v1` releases ALL storage back to its entry floor:
/// returned owners are then unreserved. The integrating owner must reserve the
/// copied payload on the SAME account immediately after that scope returns,
/// and install it only after all enclosing CPU/verifier/lower postchecks pass.
/// R cannot borrow the local wire. Neither an owned copy nor this borrowed view
/// replaces the enclosing source, signed proof, runtime, and account custody.
pub(crate) fn with_conditional_contract_projection_v1<'w, R>(
    fields: &ConditionalGeneratedFieldsV1<'_>,
    execution: &ProductionConditionalFormulaExecutionV1,
    budget: &mut Budget<'w>,
    consume: impl for<'wire> FnOnce(View<'wire>, &mut Budget<'w>) -> R,
) -> Result<R, Error> {
    let report = execution.report();
    with_projected_rows(
        fields,
        report.binding(),
        report.statement_identity().as_bytes(),
        PROJECTION_STORAGE,
        budget,
        |rows, staging, budget| {
            let theorem = Theorem {
                statement_identity: *report.statement_identity().as_bytes(),
                generated_source_identity: *report.generated_source_identity().as_bytes(),
                execution_identity: *report.execution_identity().as_bytes(),
                receipt_identity: *report.receipt_identity().as_bytes(),
                staging_receipt_identity: *staging.receipt_identity().digest().as_bytes(),
                staging_obligation_identity: *staging
                    .binding()
                    .normalized_obligation_effect_ir_hash()
                    .as_bytes(),
                staging_signer_identity: *staging.signer_identity().as_bytes(),
                staging_execution_identity: *staging.execution_identity().as_bytes(),
            };
            let projected = Input {
                numerical_domain: ConditionalNumericalDomainV1::LittleEndianSharedIeeeV1,
                subjects: rows.subjects,
                theorem,
                typed_roots: rows.typed_roots,
                arguments: rows.arguments,
                output: rows.output,
                reads: rows.reads,
                premises: rows.premises,
            };
            with_encoded(&projected, budget, consume)
        },
    )
}

struct ProjectedRows<'a> {
    subjects: Subjects,
    typed_roots: &'a [[u64; 4]],
    arguments: &'a [Argument],
    output: Output,
    reads: &'a [Read],
    premises: &'a [Premise],
}

// Both versioned entries derive the same rows from the live checked views.
// The private binding/statement arguments do not construct proof custody.
fn with_projected_rows<'w, R>(
    fields: &ConditionalGeneratedFieldsV1<'_>,
    binding: FunctionalRefinementBindingV2,
    statement: &[u8; 32],
    storage: usize,
    budget: &mut Budget<'w>,
    consume: impl FnOnce(
        ProjectedRows<'_>,
        &ProductionPolicyCheckedRefinementStagingV2,
        &mut Budget<'w>,
    ) -> Result<R, Error>,
) -> Result<R, Error> {
    with_scratch(budget, storage, |budget| {
        let input = fields.request().pliron_input();
        input
            .require_current_graph_v1(budget)
            .map_err(Error::Replay)?;
        budget.charge_work(storage)?;
        let [output] = input.outputs() else {
            return Err(Error::Mismatch("one checked output"));
        };
        let [staging] = input.retained_policy_checked_refinement_staging() else {
            return Err(Error::Mismatch("one retained staging receipt"));
        };
        require_counts(
            input.typed_root_commitments().len(),
            fields.arguments().len(),
            input.reads().len(),
            input.premises().len(),
            fields.read_arguments().len(),
        )?;
        require_subjects(
            input.reference_subjects(),
            binding,
            staging.binding(),
            staging.boundary(),
            budget,
        )?;
        if statement != binding.normalized_obligation_effect_ir_hash().as_bytes() {
            return Err(Error::Mismatch("executed statement binding"));
        }
        let reference = input.reference_subjects();
        let subjects = Subjects {
            kernel_id: fields.typed_root().kernel_binding_bytes(),
            exact_graph_identity: input.exact_graph_identity().digest(),
            aggregate_statement_identity: *input.identity().as_bytes(),
            source_semantic_identity: *input.source_semantic_identity().as_bytes(),
            reference_kind: ConditionalReferenceKindV1::Mir,
            safe_reference_identity: *reference.safe_reference_identity().as_bytes(),
            safe_reference_source_hash: *reference.safe_reference_source_hash().as_bytes(),
            safe_reference_mir_hash: *reference.safe_reference_mir_hash().as_bytes(),
            kernel_subject_identity: *reference.kernel_subject_identity().as_bytes(),
            kernel_mir_hash: *reference.kernel_mir_hash().as_bytes(),
        };
        let mut scratch = Scratch {
            arguments: [EMPTY_ARGUMENT; MAX_CONDITIONAL_ARGUMENTS_V1],
            reads: [EMPTY_READ; MAX_CONDITIONAL_READS_V1],
            premises: [Premise::D1Launch; MAX_CONDITIONAL_PREMISES_V1],
        };
        for (destination, argument) in scratch.arguments.iter_mut().zip(fields.arguments()) {
            budget.charge_work(size_of::<Argument>() + 1)?;
            *destination = argument.projection();
        }
        for (destination, premise) in scratch.premises.iter_mut().zip(input.premises()) {
            budget.charge_work(2 * size_of::<Premise>() + 1)?;
            *destination = project_premise(*premise);
        }
        let arguments = &scratch.arguments[..fields.arguments().len()];
        require_argument(
            arguments,
            fields.output_argument(),
            output.canonical_parameter(),
            Role::Output,
        )?;
        // The checked aggregate's fourth premise retains the canonical output
        // layout/domain. Read it exactly; do not infer layout from a source type.
        let (address_domain, element_bytes, alignment) = output_layout(
            &scratch.premises[..input.premises().len()],
            output.canonical_parameter(),
        )?;
        let (block, operation) = output.effect_site();
        let output = Output {
            argument: fields.output_argument(),
            canonical_store: canonical_location(input.canonical_output_store_location_v1())?,
            ranked_store: RankedLocation {
                block: output.write().block(),
                operation: output.write().operation(),
            },
            ranked_effect: RankedLocation { block, operation },
            element_bytes,
            alignment,
            address_domain,
        };
        for (i, read) in input.reads().iter().enumerate() {
            budget.charge_work(4 * size_of::<Read>() + 32)?;
            let canonical = read.canonical();
            let argument = fields.read_arguments()[i];
            require_argument(arguments, argument, canonical.parameter(), Role::Input)?;
            if canonical.location() != read.site().canonical
                || canonical.slice() != fields.arguments()[usize::from(argument)].canonical_value()
            {
                return Err(Error::Mismatch("checked canonical read occurrence"));
            }
            scratch.reads[i] = Read {
                argument,
                canonical: canonical_location(canonical.location())?,
                slice: canonical.slice().0,
                pointer: canonical.pointer().0,
                index: canonical.index().0,
                value: canonical.value().0,
                ranked: RankedLocation {
                    block: read.site().block,
                    operation: read.site().operation,
                },
                ranked_view: ranked_value(read.view()),
                ranked_index: ranked_value(read.index()),
                access_domain: domain(canonical.access_domain()),
                address_domain: domain(canonical.address_domain()),
                element_bytes: canonical.element_bytes(),
                alignment: canonical.alignment(),
            };
        }
        let rows = ProjectedRows {
            subjects,
            typed_roots: input.typed_root_commitments(),
            arguments,
            output,
            reads: &scratch.reads[..input.reads().len()],
            premises: &scratch.premises[..input.premises().len()],
        };
        let result = consume(rows, staging, budget)?;
        input
            .require_current_graph_v1(budget)
            .map_err(Error::Replay)?;
        Ok(result)
    })
}

#[path = "compiler_descriptor_conditional_contract_projection_v2.rs"]
mod v2;
pub(crate) use v2::with_conditional_contract_projection_v2;

fn require_counts(
    roots: usize,
    arguments: usize,
    reads: usize,
    premises: usize,
    mappings: usize,
) -> Result<(), Error> {
    if roots == 0
        || roots > MAX_CONDITIONAL_ROOTS_V1
        || arguments == 0
        || arguments > MAX_CONDITIONAL_ARGUMENTS_V1
        || reads > MAX_CONDITIONAL_READS_V1
        || mappings != reads
        || premises != 4 + 3 * reads
        || premises > MAX_CONDITIONAL_PREMISES_V1
    {
        return Err(Error::Mismatch("bounded complete projection rosters"));
    }
    Ok(())
}

fn require_subjects(
    reference: FunctionalRefinementSubjectsV2,
    executed: FunctionalRefinementBindingV2,
    staging: FunctionalRefinementBindingV2,
    boundary: FunctionalRefinementBoundaryV2,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    budget.charge_work(4 * size_of::<FunctionalRefinementBindingV2>())?;
    if reference.safe_reference_kind() != SafeReferenceKindV2::Mir
        || executed.subjects() != reference
        || staging.subjects() != reference
        || boundary != FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir
    {
        return Err(Error::Mismatch("executed/staging reference subjects"));
    }
    Ok(())
}

fn require_argument(
    arguments: &[Argument],
    index: u16,
    parameter: u32,
    role: Role,
) -> Result<(), Error> {
    match arguments.get(usize::from(index)) {
        Some(row) if row.canonical_parameter == parameter && row.role == role => Ok(()),
        _ => Err(Error::Mismatch("checked argument table reference")),
    }
}

fn output_layout(premises: &[Premise], output: u32) -> Result<(Domain, u64, u32), Error> {
    match premises.get(3) {
        Some(Premise::RepresentableAddress {
            parameter,
            domain,
            element_bytes,
            alignment,
        }) if *parameter == output => Ok((*domain, *element_bytes, *alignment)),
        _ => Err(Error::Mismatch("checked output address premise")),
    }
}

fn canonical_location(location: FunctionOperationLocation) -> Result<CanonicalLocation, Error> {
    Ok(CanonicalLocation {
        block: location.block.0,
        operation: u64::try_from(location.operation_index).map_err(|_| Resource::Arithmetic)?,
    })
}
fn ranked_value(value: ProductionRankedValueV1) -> RankedValue {
    match value {
        ProductionRankedValueV1::Argument(argument) => RankedValue::Argument(argument),
        ProductionRankedValueV1::BlockArgument { block, argument } => {
            RankedValue::BlockArgument { block, argument }
        }
        ProductionRankedValueV1::Local(value) => RankedValue::Local(value.get()),
    }
}
fn domain(value: ConditionalTotalViewAddressDomainV1) -> Domain {
    match value {
        ConditionalTotalViewAddressDomainV1::GuardedOutput => Domain::GuardedOutput,
        ConditionalTotalViewAddressDomainV1::GlobalLaunch => Domain::GlobalLaunch,
    }
}
fn project_premise(value: ProductionConditionalRuntimePremiseV1) -> Premise {
    use ProductionConditionalRuntimePremiseV1 as P;
    match value {
        P::D1Launch => Premise::D1Launch,
        P::OutputWithinGlobalX { parameter } => Premise::OutputWithinGlobalX { parameter },
        P::WritableOutput { parameter } => Premise::WritableOutput { parameter },
        P::ReadableInput {
            parameter,
            domain: d,
        } => Premise::ReadableInput {
            parameter,
            domain: domain(d),
        },
        P::SeparateInputOutput { input, output } => Premise::SeparateInputOutput { input, output },
        P::RepresentableAddress {
            parameter,
            domain: d,
            element_bytes,
            alignment,
        } => Premise::RepresentableAddress {
            parameter,
            domain: domain(d),
            element_bytes,
            alignment,
        },
    }
}

// Private codec plumbing accepts inert fixtures in tests. Production can reach
// it only after projecting the checked views above; this is not another entry.
fn with_encoded<'w, R>(
    input: &Input<'_>,
    budget: &mut Budget<'w>,
    consume: impl for<'wire> FnOnce(View<'wire>, &mut Budget<'w>) -> R,
) -> Result<R, Error> {
    with_scratch(budget, ENCODING_STORAGE, |budget| {
        budget.charge_work(MAX_CONDITIONAL_INVOCATION_BYTES_V1)?;
        let mut wire = [0; MAX_CONDITIONAL_INVOCATION_BYTES_V1];
        let len =
            encoded_conditional_invocation_contract_v1_len(input, &mut |n| budget.charge_work(n))?;
        encode_conditional_invocation_contract_v1(input, &mut wire[..len], &mut |n| {
            budget.charge_work(n)
        })?;
        let view = decode_conditional_invocation_contract_v1(&wire[..len], &mut |n| {
            budget.charge_work(n)
        })?;
        Ok(consume(view, budget))
    })
}

// Identity is observed only for this active borrow. Release exactly this
// reservation, never reset a floor or erase a downstream retained allocation.
fn with_scratch<'w, R>(
    budget: &mut Budget<'w>,
    bytes: usize,
    run: impl FnOnce(&mut Budget<'w>) -> Result<R, Error>,
) -> Result<R, Error> {
    budget.charge_work(1)?;
    let account = budget.work_ledger_identity_v1();
    budget.reserve_storage(bytes)?;
    let protected = budget.storage();
    let result = catch_unwind(AssertUnwindSafe(|| run(budget)));
    let cleanup = if budget.work_ledger_identity_v1() == account && budget.storage() >= protected {
        budget.release_storage(bytes)
    } else {
        Err(Resource::Accounting)
    };
    match result {
        Ok(result) => {
            cleanup?;
            result
        }
        Err(payload) => resume_unwind(payload),
    }
}

#[cfg(test)]
#[path = "compiler_descriptor_conditional_contract_projection_v1_tests.rs"]
mod tests;

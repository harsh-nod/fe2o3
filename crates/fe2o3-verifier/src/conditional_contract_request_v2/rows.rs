use super::*;
use fe2o3_functional_proof::{FunctionalRefinementBoundaryV2, SafeReferenceKindV2};
use fe2o3_kernel_descriptor::{
    ConditionalAddressDomainV1 as Domain, ConditionalArgumentBindingV1 as Argument,
    ConditionalArgumentRoleV1 as Role, ConditionalCanonicalLocationV1 as Location,
    ConditionalNumericalDomainV1, ConditionalOutputV1 as Output,
    ConditionalRankedLocationV1 as RankedLocation, ConditionalRankedValueV1 as Value,
    ConditionalReadOccurrenceV1 as Read, ConditionalReferenceKindV1,
    ConditionalRuntimePremiseV1 as Premise, ConditionalSubjectsV1 as Subjects,
    ConditionalTheoremV2 as Theorem, MAX_CONDITIONAL_ARGUMENTS_V1, MAX_CONDITIONAL_PREMISES_V1,
    MAX_CONDITIONAL_READS_V1, MAX_CONDITIONAL_ROOTS_V1,
};
use fe2o3_kernel_ir::{ConditionalTotalViewAddressDomainV1, FunctionOperationLocation};
use fe2o3_pliron::{ProductionConditionalRuntimePremiseV1 as LivePremise, ProductionRankedValueV1};

pub(super) const STORAGE: usize = 2 * size_of::<Subjects>()
    + 2 * size_of::<Theorem>()
    + 3 * size_of::<Read>()
    + 2 * size_of::<Output>()
    + 4 * size_of::<Argument>()
    + 3 * size_of::<Premise>()
    + 4 * size_of::<fe2o3_functional_proof::FunctionalRefinementBindingV2>()
    + size_of::<crate::ProductionConditionalFormulaReportV2>()
    + size_of::<fe2o3_kernel_descriptor::ConditionalInvocationCursorV1<'static, Read>>()
    + size_of::<fe2o3_kernel_descriptor::ConditionalInvocationCursorV1<'static, Premise>>()
    + size_of::<fe2o3_kernel_descriptor::ConditionalInvocationCursorV1<'static, [u64; 4]>>()
    + size_of::<fe2o3_kernel_descriptor::ConditionalInvocationCursorV1<'static, Argument>>();

pub(super) fn check(
    request: &Request<'_>,
    execution: &Execution,
    kernel: KernelId,
    contract: &Contract<'_>,
    budget: &mut Budget<'_>,
) -> R<()> {
    let input = request.pliron_input();
    budget.charge_work(16)?;
    counts(
        contract.typed_root_count(),
        contract.argument_count(),
        contract.read_count(),
        contract.premise_count(),
    )?;
    require(
        input.outputs().len() == 1
            && input.retained_policy_checked_refinement_staging().len() == 1
            && input.reads().len() == contract.read_count()
            && input.premises().len() == contract.premise_count()
            && input.typed_root_commitments().len() == contract.typed_root_count()
            && request.arguments().len() == input.reads().len() + 1,
        "complete aggregate counts",
    )?;
    let report = execution.report();
    let reference = input.reference_subjects();
    let staging = &input.retained_policy_checked_refinement_staging()[0];
    budget.charge_work(4 * size_of::<fe2o3_functional_proof::FunctionalRefinementBindingV2>())?;
    require(
        reference.safe_reference_kind() == SafeReferenceKindV2::Mir
            && report.binding().subjects() == reference
            && staging.binding().subjects() == reference
            && staging.boundary() == FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir
            && report.statement_identity()
                == report.binding().normalized_obligation_effect_ir_hash(),
        "same executed/staging MIR subjects",
    )?;
    let expected = Subjects {
        kernel_id: *kernel.as_bytes(),
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
        cpu_input_commitment: *report.cpu_input_commitment().as_bytes(),
    };
    compare_header(contract, &expected, &theorem, budget)?;
    ordered_roots(contract, input.typed_root_commitments(), budget)?;
    ordered_premises(contract, input.premises(), budget)?;
    let output = input.outputs()[0];
    let actual = contract.output();
    argument(
        contract,
        actual.argument,
        output.canonical_parameter(),
        Role::Output,
        budget,
    )?;
    let (domain, element_bytes, alignment) = match input.premises().get(3) {
        Some(LivePremise::RepresentableAddress {
            parameter,
            domain,
            element_bytes,
            alignment,
        }) if *parameter == output.canonical_parameter() => (domain, element_bytes, alignment),
        _ => return Err(E::Mismatch("actual output layout premise")),
    };
    let (block, operation) = output.effect_site();
    let expected = Output {
        argument: actual.argument,
        canonical_store: location(input.canonical_output_store_location_v1())?,
        ranked_store: RankedLocation {
            block: output.write().block(),
            operation: output.write().operation(),
        },
        ranked_effect: RankedLocation { block, operation },
        element_bytes: *element_bytes,
        alignment: *alignment,
        address_domain: address(*domain),
    };
    budget.charge_work(2 * size_of::<Output>())?;
    require(actual == expected, "exact output occurrence")?;
    let mut reads = contract.reads();
    for expected in input.reads() {
        let actual = reads
            .next(&mut |w| budget.charge_work(w))?
            .ok_or(E::Mismatch("missing read"))?;
        let canonical = expected.canonical();
        argument(
            contract,
            actual.argument,
            canonical.parameter(),
            Role::Input,
            budget,
        )?;
        require(
            canonical.location() == expected.site().canonical,
            "actual read location",
        )?;
        let expected = Read {
            argument: actual.argument,
            canonical: location(canonical.location())?,
            slice: canonical.slice().0,
            pointer: canonical.pointer().0,
            index: canonical.index().0,
            value: canonical.value().0,
            ranked: RankedLocation {
                block: expected.site().block,
                operation: expected.site().operation,
            },
            ranked_view: value(expected.view()),
            ranked_index: value(expected.index()),
            access_domain: address(canonical.access_domain()),
            address_domain: address(canonical.address_domain()),
            element_bytes: canonical.element_bytes(),
            alignment: canonical.alignment(),
        };
        same_read(actual, expected, budget)?;
    }
    require(
        reads.next(&mut |w| budget.charge_work(w))?.is_none(),
        "extra read",
    )
}

pub(super) fn compare_header(
    contract: &Contract<'_>,
    subjects: &Subjects,
    theorem: &Theorem,
    budget: &mut Budget<'_>,
) -> R<()> {
    budget.charge_work(2 * size_of::<Subjects>() + 2 * size_of::<Theorem>() + 1)?;
    require(
        contract.numerical_domain() == ConditionalNumericalDomainV1::LittleEndianSharedIeeeV1
            && contract.subjects() == subjects
            && contract.theorem() == theorem,
        "exact V2 subjects/theorem",
    )
}

pub(super) fn ordered_roots(
    contract: &Contract<'_>,
    expected: &[[u64; 4]],
    budget: &mut Budget<'_>,
) -> R<()> {
    budget.charge_work(1)?;
    require(
        expected.len() == contract.typed_root_count() && expected.len() <= MAX_CONDITIONAL_ROOTS_V1,
        "typed expression root count",
    )?;
    let mut roots = contract.typed_roots();
    for expected in expected {
        budget.charge_work(32)?;
        require(
            roots.next(&mut |w| budget.charge_work(w))?.as_ref() == Some(expected),
            "ordered typed expression roots",
        )?;
    }
    require(
        roots.next(&mut |w| budget.charge_work(w))?.is_none(),
        "extra typed root",
    )
}
pub(super) fn ordered_premises(
    contract: &Contract<'_>,
    expected: &[LivePremise],
    budget: &mut Budget<'_>,
) -> R<()> {
    budget.charge_work(1)?;
    require(
        expected.len() == contract.premise_count() && expected.len() <= MAX_CONDITIONAL_PREMISES_V1,
        "premise count",
    )?;
    let mut cursor = contract.premises();
    for expected in expected {
        budget.charge_work(2 * size_of::<Premise>())?;
        require(
            cursor.next(&mut |w| budget.charge_work(w))? == Some(premise(*expected)),
            "ordered runtime premise",
        )?;
    }
    require(
        cursor.next(&mut |w| budget.charge_work(w))?.is_none(),
        "extra runtime premise",
    )
}
pub(super) fn same_read(actual: Read, expected: Read, budget: &mut Budget<'_>) -> R<()> {
    budget.charge_work(2 * size_of::<Read>())?;
    require(actual == expected, "exact ordered read occurrence")
}

pub(super) fn counts(roots: usize, arguments: usize, reads: usize, premises: usize) -> R<()> {
    require(
        (1..=MAX_CONDITIONAL_ROOTS_V1).contains(&roots)
            && (1..=MAX_CONDITIONAL_ARGUMENTS_V1).contains(&arguments)
            && reads <= MAX_CONDITIONAL_READS_V1
            && premises <= MAX_CONDITIONAL_PREMISES_V1
            && reads.checked_mul(3).and_then(|n| n.checked_add(4)) == Some(premises),
        "bounded complete counts",
    )
}
pub(super) fn argument(
    contract: &Contract<'_>,
    index: u16,
    parameter: u32,
    role: Role,
    budget: &mut Budget<'_>,
) -> R<Argument> {
    let row = contract.argument(usize::from(index), &mut |w| budget.charge_work(w))?;
    budget.charge_work(2)?;
    require(
        row.canonical_parameter == parameter && row.role == role,
        "occurrence parameter/role",
    )?;
    Ok(row)
}
fn location(value: FunctionOperationLocation) -> R<Location> {
    Ok(Location {
        block: value.block.0,
        operation: u64::try_from(value.operation_index).map_err(|_| Resource::Arithmetic)?,
    })
}
pub(super) fn address(value: ConditionalTotalViewAddressDomainV1) -> Domain {
    match value {
        ConditionalTotalViewAddressDomainV1::GuardedOutput => Domain::GuardedOutput,
        ConditionalTotalViewAddressDomainV1::GlobalLaunch => Domain::GlobalLaunch,
    }
}
pub(super) fn value(value: ProductionRankedValueV1) -> Value {
    match value {
        ProductionRankedValueV1::Argument(n) => Value::Argument(n),
        ProductionRankedValueV1::BlockArgument { block, argument } => {
            Value::BlockArgument { block, argument }
        }
        ProductionRankedValueV1::Local(n) => Value::Local(n.get()),
    }
}
pub(super) fn premise(value: LivePremise) -> Premise {
    match value {
        LivePremise::D1Launch => Premise::D1Launch,
        LivePremise::OutputWithinGlobalX { parameter } => {
            Premise::OutputWithinGlobalX { parameter }
        }
        LivePremise::WritableOutput { parameter } => Premise::WritableOutput { parameter },
        LivePremise::ReadableInput { parameter, domain } => Premise::ReadableInput {
            parameter,
            domain: address(domain),
        },
        LivePremise::SeparateInputOutput { input, output } => {
            Premise::SeparateInputOutput { input, output }
        }
        LivePremise::RepresentableAddress {
            parameter,
            domain,
            element_bytes,
            alignment,
        } => Premise::RepresentableAddress {
            parameter,
            domain: address(domain),
            element_bytes,
            alignment,
        },
    }
}

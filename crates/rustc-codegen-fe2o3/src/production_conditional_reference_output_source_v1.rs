//! Source-authenticated CPU correspondence composed with scoped formula execution.
//! Neither the correspondence nor the formula proof is an ordinary lowering owner.

use super::*;
use crate::reference_effect_v1::AuthenticatedReferenceEffectBindingV1 as Binding;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_lower_mir_kernel::{
    ProductionConditionalSourceArgumentV1 as Argument,
    ProductionSourceBoundConditionalAggregateRequestV1 as Request,
};
use fe2o3_mir_model::semantic_mir_v1::{SemanticFunctionIdV1, SemanticFunctionRoleV1};
use fe2o3_pliron::{ProductionConditionalReadBindingV1 as Read, ProductionSemanticLoadV2};

type Error = ProductionReferenceEffectJoinErrorV2;
type Expr = ProductionSemanticExpressionV2;
type Op = ProductionRankedOperationV1;
type Value = ProductionRankedValueV1;

#[path = "production_conditional_cpu_read_premises_v1.rs"]
pub(crate) mod read_premises_v1;

fn reject(why: &'static str) -> Error {
    Error::UnsupportedReference(why)
}

fn charge(budget: &mut Budget<'_>, work: usize) -> Result<(), Error> {
    budget.charge_work(work).map_err(resource)
}

fn resource(error: Resource) -> Error {
    Error::ProofExecution(format!("conditional CPU correspondence resource: {error}"))
}

fn require(condition: bool, why: &'static str) -> Result<(), Error> {
    if condition { Ok(()) } else { Err(reject(why)) }
}

/// The fields are private even to the consuming parent. Its only constructor
/// replays actual CPU MIR and joins it to this very source-bound live graph.
struct SourceBoundCpuCorrespondenceV1<'a> {
    request: &'a Request<'a>,
    binding: &'a Binding,
}

pub(crate) fn retain_source_bound_cpu_formula_v1(
    runtime: &FunctionalRefinementVerusRuntimeLeaseV1,
    request: &Request<'_>,
    references: &AuthenticatedReferenceEffectBindingsV1,
    semantic_root: u32,
    budget: &mut Budget<'_>,
    timeout_seconds: u32,
) -> Result<fe2o3_verifier::RetainedProductionConditionalFormulaV1, Error> {
    with_cpu_correspondence(request, references, semantic_root, budget, |cpu, budget| {
        cpu.require_subjects(budget)?;
        fe2o3_verifier::execute_and_retain_conditional_ranked_formula_v1(
            runtime,
            cpu.request,
            budget,
            timeout_seconds,
        )
        .map_err(|error| Error::ProofExecution(error.to_string()))
    })
}

pub(crate) fn replay_source_bound_cpu_formula_v1<R>(
    retained: &fe2o3_verifier::RetainedProductionConditionalFormulaV1,
    request: &Request<'_>,
    binding: &Binding,
    semantic_root: u32,
    budget: &mut Budget<'_>,
    consume: impl for<'proof> FnOnce(
        &'proof fe2o3_verifier::ProductionConditionalFormulaExecutionV1,
        &mut Budget<'_>,
    ) -> R,
) -> Result<R, Error> {
    with_cpu_binding(request, binding, semantic_root, budget, |cpu, budget| {
        cpu.require_subjects(budget)?;
        retained
            .with_replayed_request_v1(cpu.request, budget, consume)
            .map_err(|error| Error::ProofExecution(error.to_string()))
    })
}

fn with_cpu_correspondence<R>(
    request: &Request<'_>,
    references: &AuthenticatedReferenceEffectBindingsV1,
    semantic_root: u32,
    budget: &mut Budget<'_>,
    consume: impl FnOnce(&SourceBoundCpuCorrespondenceV1<'_>, &mut Budget<'_>) -> Result<R, Error>,
) -> Result<R, Error> {
    let [binding] = references.as_slice() else {
        return Err(reject(
            "conditional CPU join requires one authenticated binding",
        ));
    };
    with_cpu_binding(request, binding, semantic_root, budget, consume)
}

fn with_cpu_binding<R>(
    request: &Request<'_>,
    binding: &Binding,
    semantic_root: u32,
    budget: &mut Budget<'_>,
    consume: impl FnOnce(&SourceBoundCpuCorrespondenceV1<'_>, &mut Budget<'_>) -> Result<R, Error>,
) -> Result<R, Error> {
    check_source_identity(request, binding, semantic_root, budget)?;
    // This bridge uses the existing MIR resolver and the caller's original meter.
    binding
        .with_replayed_output_writes_v1(budget, |replay, budget| {
            check_outputs(request, binding, &replay.writes, budget)?;
            read_premises_v1::check_source_bound_reads_v1(request, binding, replay, budget)?;
            let cpu = SourceBoundCpuCorrespondenceV1 { request, binding };
            consume(&cpu, budget)
        })
        .map_err(|error| Error::ProofExecution(error.to_string()))?
}

impl SourceBoundCpuCorrespondenceV1<'_> {
    fn require_subjects(&self, budget: &mut Budget<'_>) -> Result<(), Error> {
        charge(budget, 256)?;
        require(
            self.request.pliron_input().reference_subjects() == subjects(self.binding)?,
            "CPU subjects changed after correspondence",
        )
    }
}

pub(crate) fn subjects(binding: &Binding) -> Result<FunctionalRefinementSubjectsV2, Error> {
    FunctionalRefinementSubjectsV2::new(
        SafeReferenceKindV2::Mir,
        DigestV1::from_untrusted_bytes(binding.reference.function_sha256),
        DigestV1::ZERO,
        DigestV1::from_untrusted_bytes(binding.reference.rustc_mir_body_sha256),
        DigestV1::from_untrusted_bytes(binding.kernel.function_sha256),
        DigestV1::from_untrusted_bytes(binding.kernel.rustc_mir_body_sha256),
    )
    .map_err(|error| Error::Subjects(error.to_string()))
}

fn check_source_identity(
    request: &Request<'_>,
    binding: &Binding,
    semantic_root: u32,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    charge(budget, 512)?;
    let source = request.source().semantic_ssa().source_semantic();
    let root = source
        .functions()
        .get(semantic_root as usize)
        .ok_or_else(|| reject("conditional CPU semantic root"))?;
    let k = &binding.kernel;
    require(
        root.role() == SemanticFunctionRoleV1::KernelRoot
            && root.identity().as_bytes() == &k.function_sha256
            && root.item_definition_identity().as_bytes() == &k.item_definition_sha256
            && root.monomorphization_identity().as_bytes() == &k.monomorphization_sha256
            && root.generic_type_arguments_identity().as_bytes()
                == &k.generic_type_arguments_sha256
            && root.const_generic_arguments_identity().as_bytes()
                == &k.const_generic_arguments_sha256,
        "conditional CPU kernel identity",
    )?;
    let entry = root
        .kernel_entry()
        .ok_or_else(|| reject("conditional CPU kernel entry"))?;
    let kernel = request.pliron_input().kernel();
    charge(
        budget,
        entry
            .export_symbol()
            .as_bytes()
            .len()
            .checked_add(kernel.function_name().len())
            .ok_or_else(|| resource(Resource::Arithmetic))?,
    )?;
    require(
        entry.export_symbol().as_bytes() == kernel.function_name().as_bytes(),
        "conditional CPU root is not this source-bound graph",
    )?;
    require(
        source
            .select_kernel_body_for_root_v1(SemanticFunctionIdV1::from_index(semantic_root))
            .is_some(),
        "conditional CPU body selection",
    )?;
    let signature = &binding.signature_preimage;
    charge(
        budget,
        signature
            .reference_inputs()
            .len()
            .checked_mul(8)
            .ok_or_else(|| resource(Resource::Arithmetic))?,
    )?;
    let relations = signature
        .derive_relations_v1()
        .map_err(|_| reject("conditional CPU signature relations"))?;
    require(
        relations.len() == binding.effect_ir.relations.len()
            && signature.reference_inputs().len() == binding.effect_ir.argument_count as usize,
        "conditional CPU signature arity",
    )?;
    for (raw, relation) in binding.effect_ir.relations.iter().enumerate() {
        require(
            relations.relation_at_raw_argument_v1(raw as u32) == Some(*relation),
            "conditional CPU raw ABI relation",
        )?;
    }
    require(
        request.pliron_input().reference_subjects() == subjects(binding)?,
        "conditional CPU reference subject substitution",
    )
}

fn argument(
    request: &Request<'_>,
    canonical: u32,
    budget: &mut Budget<'_>,
) -> Result<Argument, Error> {
    let mut selected = None;
    for row in request.arguments() {
        charge(budget, 8)?;
        if row.canonical_parameter() == canonical {
            require(
                selected.is_none_or(|previous| previous == *row),
                "conditional CPU conflicting source argument",
            )?;
            selected = Some(*row);
        }
    }
    selected.ok_or_else(|| reject("conditional CPU missing source argument"))
}

fn check_outputs(
    request: &Request<'_>,
    binding: &Binding,
    writes: &[ReferenceOutputWriteV1],
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    let input = request.pliron_input();
    let [output] = input.outputs() else {
        return Err(reject("conditional CPU output roster"));
    };
    let [write] = writes else {
        return Err(reject("conditional CPU observable write roster"));
    };
    let output_argument = argument(request, output.canonical_parameter(), budget)?;
    let contract = input
        .effect_contract(output)
        .ok_or_else(|| reject("conditional CPU effect contract"))?;
    let source_argument = output_argument.source_argument();
    let relations = binding
        .signature_preimage
        .derive_relations_v1()
        .map_err(|_| reject("conditional CPU signature relations"))?;
    let raw = relations
        .reference_argument_for_kernel_argument_v1(source_argument)
        .ok_or_else(|| reject("conditional CPU raw output argument"))?;
    let Some(ReferenceArgumentRelationV1::DisjointOutputCoordinate { argument, element }) =
        relations.relation_at_raw_argument_v1(raw)
    else {
        return Err(reject("conditional CPU output role"));
    };
    require(
        argument == source_argument && write.argument == source_argument,
        "conditional CPU output source argument",
    )?;
    let site = contract.reference_output_site();
    require(
        site.argument() == source_argument
            && site.block() == write.block
            && site.statement() == write.statement,
        "conditional CPU output occurrence",
    )?;
    let kernel = input.kernel();
    let gpu = semantic_expression(kernel, contract.gpu_value(), budget)?;
    check_read_bindings(request, binding, gpu, budget)?;
    check_contract_roles(
        kernel,
        contract,
        binding,
        write,
        element,
        gpu,
        request.arguments(),
        budget,
    )?;
    Ok(())
}

fn semantic_expression<'a>(
    kernel: &'a ProductionRankedKernelV1,
    value: Value,
    budget: &mut Budget<'_>,
) -> Result<&'a Expr, Error> {
    let mut found = None;
    for block in kernel.blocks() {
        charge(budget, 1)?;
        for op in block.operations() {
            charge(budget, 4)?;
            if let Op::SemanticExpression {
                result,
                expression,
                numerical_contract,
            } = op
                && value == Value::Local(*result)
            {
                require(
                    found.is_none(),
                    "conditional CPU ambiguous expression definition",
                )?;
                expression_work(expression, budget, 0)?;
                require(
                    *numerical_contract
                        == ProductionNumericalContractV2::exact_for_expression(expression),
                    "conditional CPU numerical contract",
                )?;
                found = Some(expression);
            }
        }
    }
    found.ok_or_else(|| reject("conditional CPU missing expression definition"))
}

fn check_contract_roles(
    kernel: &ProductionRankedKernelV1,
    contract: &ProductionEffectRefinementContractV2,
    binding: &Binding,
    write: &ReferenceOutputWriteV1,
    element: ReferenceScalarTypeV1,
    gpu: &Expr,
    arguments: &[Argument],
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    require(
        matches!(&write.coordinate, ReferenceOutputCoordinateV1::LogicalPoint(axes)
        if matches!(axes.as_ref(), [ReferenceEffectExpressionV1::PointCoordinate { axis: 0 }])),
        "conditional CPU point coordinate",
    )?;
    require(
        matches!(write.guard.clauses.as_ref(), [clause] if clause.atoms.is_empty()),
        "conditional CPU reference guard is not unconditional",
    )?;
    let site = contract.gpu_write_site();
    let op = kernel
        .blocks()
        .get(site.block() as usize)
        .and_then(|block| block.operations().get(site.operation() as usize));
    require(
        matches!(op, Some(Op::ValueAccess { kind: dialect_kernel::AccessKindAttr::Write, view, indices, value })
        if *view == contract.view() && indices == contract.indices() && *value == contract.gpu_value()),
        "conditional CPU exact value-bearing GPU write",
    )?;
    let [gpu_coordinate] = contract.gpu_coordinates() else {
        return Err(reject("conditional CPU GPU rank"));
    };
    let [cpu_coordinate] = contract.reference_coordinates() else {
        return Err(reject("conditional CPU reference rank"));
    };
    let coordinate = Expr::Symbol {
        symbol: 0,
        scalar: ProductionSemanticScalarTypeV2::Integer {
            signed: false,
            bits: 64,
        },
    };
    for operand in [*gpu_coordinate, *cpu_coordinate] {
        require(
            semantic_expression(kernel, operand, budget)? == &coordinate,
            "conditional CPU coordinate expression",
        )?;
    }
    let truth = Expr::Constant {
        scalar: ProductionSemanticScalarTypeV2::Bool,
        bits: 1,
    };
    for operand in [
        contract.gpu_domain(),
        contract.reference_domain(),
        contract.gpu_precondition(),
        contract.reference_precondition(),
    ] {
        require(
            semantic_expression(kernel, operand, budget)? == &truth,
            "conditional CPU domain or precondition expression",
        )?;
    }
    let cpu = semantic_expression(kernel, contract.reference_value(), budget)?;
    with_reference_expression(
        binding,
        write,
        element,
        kernel,
        gpu,
        arguments,
        budget,
        |expected| {
            require(
                cpu == expected,
                "conditional CPU MIR value expression substitution",
            )
        },
    )?;
    let mut prefix = [0; 8];
    prefix.copy_from_slice(&binding.effect_ir_sha256[..8]);
    require(
        contract.contract_identity()
            == (u64::from_le_bytes(prefix)
                ^ u64::from(site.block()).rotate_left(17)
                ^ u64::from(site.operation()).rotate_left(41))
            .max(1),
        "conditional CPU contract identity",
    )
}

fn check_read_bindings(
    request: &Request<'_>,
    binding: &Binding,
    expression: &Expr,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    visit_loads(expression, budget, &mut |load, budget| {
        let mut selected: Option<Read> = None;
        for read in request.pliron_input().reads() {
            charge(budget, 16)?;
            if read.site().block == load.block && read.site().operation == load.operation {
                require(
                    selected.replace(*read).is_none(),
                    "conditional CPU duplicate read occurrence",
                )?;
            }
        }
        let read =
            selected.ok_or_else(|| reject("conditional CPU load lacks checked source read"))?;
        let row = argument(request, read.canonical().parameter(), budget)?;
        require(
            load.view == read.view() && load.indices.as_ref() == [read.index()],
            "conditional CPU read origin/index substitution",
        )?;
        require_read_origins(
            request.pliron_input().kernel().blocks(),
            load,
            row.source_argument(),
            row.adjusted_argument(),
            budget,
        )?;
        let relations = binding
            .signature_preimage
            .derive_relations_v1()
            .map_err(|_| reject("conditional CPU read signature"))?;
        let raw = relations
            .reference_argument_for_kernel_argument_v1(row.source_argument())
            .ok_or_else(|| reject("conditional CPU read raw argument"))?;
        require(
            matches!(relations.relation_at_raw_argument_v1(raw),
            Some(ReferenceArgumentRelationV1::SharedSliceInput { argument, element })
            if argument == row.source_argument() && reference_scalar_v2(element) == Some(load.scalar)),
            "conditional CPU read ABI/type substitution",
        )
    })
}

// The source/adjusted pair must come from the selected authenticated request
// argument. Do not apply the recipe's source ordinal to CPU/effect expressions.
fn require_read_origins(
    blocks: &[fe2o3_pliron::ProductionRankedBlockV1],
    load: &ProductionSemanticLoadV2,
    source_argument: u32,
    adjusted_argument: u32,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    charge(budget, 3)?;
    require(
        load.allocation_origin == u64::from(adjusted_argument) + 1,
        "conditional CPU read expression origin substitution",
    )?;
    let mut found = false;
    for block in blocks {
        charge(budget, 1)?;
        for operation in block.operations() {
            charge(budget, 4)?;
            if let Op::View {
                result,
                allocation_origin,
                ..
            }
            | Op::ViewInSpace {
                result,
                allocation_origin,
                ..
            } = operation
                && Value::Local(*result) == load.view
            {
                require(
                    !found && *allocation_origin == u64::from(source_argument) + 1,
                    "conditional CPU read recipe origin substitution",
                )?;
                found = true;
            }
        }
    }
    require(found, "conditional CPU read missing recipe view")
}

#[cfg(test)]
#[path = "production_conditional_read_origins_v1_tests.rs"]
mod read_origin_tests;

fn visit_loads(
    expression: &Expr,
    budget: &mut Budget<'_>,
    visit: &mut impl FnMut(&ProductionSemanticLoadV2, &mut Budget<'_>) -> Result<(), Error>,
) -> Result<(), Error> {
    charge(budget, 1)?;
    match expression {
        Expr::Load(load) => visit(load, budget),
        Expr::Unary { operand, .. } | Expr::Cast { operand, .. } => {
            visit_loads(operand, budget, visit)
        }
        Expr::Binary { lhs, rhs, .. } | Expr::Compare { lhs, rhs, .. } => {
            visit_loads(lhs, budget, visit)?;
            visit_loads(rhs, budget, visit)
        }
        Expr::Select {
            condition,
            when_true,
            when_false,
            ..
        } => {
            visit_loads(condition, budget, visit)?;
            visit_loads(when_true, budget, visit)?;
            visit_loads(when_false, budget, visit)
        }
        Expr::Constant { .. } | Expr::Symbol { .. } => Ok(()),
    }
}

fn expression_work(
    expression: &Expr,
    budget: &mut Budget<'_>,
    depth: usize,
) -> Result<usize, Error> {
    charge(budget, 4)?;
    require(
        depth < fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2,
        "conditional CPU expression depth",
    )?;
    let mut nodes = 1usize;
    match expression {
        Expr::Unary { operand, .. } | Expr::Cast { operand, .. } => {
            nodes += expression_work(operand, budget, depth + 1)?;
        }
        Expr::Binary { lhs, rhs, .. } | Expr::Compare { lhs, rhs, .. } => {
            nodes += expression_work(lhs, budget, depth + 1)?;
            nodes += expression_work(rhs, budget, depth + 1)?;
        }
        Expr::Select {
            condition,
            when_true,
            when_false,
            ..
        } => {
            nodes += expression_work(condition, budget, depth + 1)?;
            nodes += expression_work(when_true, budget, depth + 1)?;
            nodes += expression_work(when_false, budget, depth + 1)?;
        }
        Expr::Load(load) => {
            charge(budget, load.indices.len())?;
        }
        _ => {}
    }
    Ok(nodes)
}

fn with_reference_expression<R>(
    binding: &Binding,
    write: &ReferenceOutputWriteV1,
    element: ReferenceScalarTypeV1,
    kernel: &ProductionRankedKernelV1,
    gpu: &Expr,
    arguments: &[Argument],
    budget: &mut Budget<'_>,
    consume: impl FnOnce(&Expr) -> Result<R, Error>,
) -> Result<R, Error> {
    // Existing typed resolver has bounded depth. Prepay its repeated read/index
    // scans and temporary expression payload before entering that allocator domain.
    let nodes = expression_work(gpu, budget, 0)?;
    let operations = kernel.blocks().iter().try_fold(0usize, |sum, block| {
        charge(budget, 1)?;
        sum.checked_add(block.operations().len())
            .ok_or_else(|| resource(Resource::Arithmetic))
    })?;
    let cpu_nodes = reference_nodes(&write.rhs, budget, 0)?;
    let roles = binding
        .effect_ir
        .relations
        .len()
        .checked_mul(binding.effect_ir.relations.len())
        .and_then(|n| n.checked_add(arguments.len()))
        .ok_or_else(|| resource(Resource::Arithmetic))?;
    let work = cpu_nodes
        .checked_mul(nodes)
        .and_then(|n| n.checked_mul(operations.max(1)))
        .and_then(|n| roles.checked_mul(cpu_nodes).and_then(|m| n.checked_add(m)))
        .and_then(|n| n.checked_mul(128))
        .ok_or_else(|| resource(Resource::Arithmetic))?;
    charge(budget, work)?;
    let storage = cpu_nodes
        .checked_mul(std::mem::size_of::<Expr>() + 64)
        .and_then(|n| {
            nodes
                .checked_mul(8 * std::mem::size_of::<&ProductionSemanticLoadV2>())
                .and_then(|m| n.checked_add(m))
        })
        .ok_or_else(|| resource(Resource::Arithmetic))?;
    budget.reserve_storage(storage).map_err(resource)?;
    let account = budget.work_ledger_identity_v1();
    let floor = budget.storage();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        reference_expression_inner_checked_v2(
            &binding.effect_ir,
            &write.rhs,
            element,
            Some(ReferenceGpuLoadsV2 {
                kernel,
                expression: gpu,
                arguments: Some(arguments),
            }),
        )
        .and_then(|expression| consume(&expression))
    }));
    let cleanup = if budget.work_ledger_identity_v1() == account && budget.storage() >= floor {
        budget.release_storage(storage).map_err(resource)
    } else {
        Err(resource(Resource::Accounting))
    };
    match result {
        Ok(result) => {
            cleanup?;
            result
        }
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

fn reference_nodes(
    expression: &ReferenceEffectExpressionV1,
    budget: &mut Budget<'_>,
    depth: usize,
) -> Result<usize, Error> {
    charge(budget, 4)?;
    require(
        depth < fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2,
        "conditional CPU MIR expression depth",
    )?;
    Ok(match expression {
        ReferenceEffectExpressionV1::Binary { lhs, rhs, .. } => {
            1 + reference_nodes(lhs, budget, depth + 1)? + reference_nodes(rhs, budget, depth + 1)?
        }
        ReferenceEffectExpressionV1::Unary { operand, .. }
        | ReferenceEffectExpressionV1::Cast { operand, .. }
        | ReferenceEffectExpressionV1::InputLoad { index: operand, .. } => {
            1 + reference_nodes(operand, budget, depth + 1)?
        }
        _ => 1,
    })
}

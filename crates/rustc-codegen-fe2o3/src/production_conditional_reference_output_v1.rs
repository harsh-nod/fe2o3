//! Scoped conditional output correspondence, never a clean stage or proof receipt.
//!
//! The current caller is the explicitly selected post-bind observer. Normal
//! compilation retains every existing gate until conditional discharge exists.
//! The initial reference fragment is one flat u32 output, one usize point axis,
//! and an unconditional constant store. Unsupported shapes fail closed. Runtime
//! N <= G, inactive-axis and address-representation premises remain external,
//! including when N is zero; this join does not grant unconditional TotalView.
#![cfg_attr(
    not(test),
    expect(dead_code, reason = "conditional discharge is not activated")
)]

use crate::reference_effect_v1::{
    AuthenticatedReferenceEffectBindingV1, AuthenticatedReferenceEffectBindingsV1,
    ReferenceArgumentRelationV1 as Rel, ReferenceConstantV1 as Const,
    ReferenceEffectExpressionV1 as CpuExpr, ReferenceOperandV1 as Operand,
    ReferenceOutputCoordinateV1 as Coord, ReferenceOutputWriteV1,
    ReferencePlaceProjectionV1 as Projection, ReferenceScalarTypeV1 as Scalar,
    ReferenceTerminatorV1 as Term, ReferenceValueV1 as Rvalue,
    reference_signature_preimage_v1::{
        ReferenceCarrierV1 as Carrier, ReferenceSignatureInputV1 as Input,
    },
};
use dialect_kernel::{OwnershipCoverageAttr, OwnershipPartitionAttr};
use fe2o3_functional_proof::{FunctionalRefinementSubjectsV2, SafeReferenceKindV2};
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, ConditionalTotalViewAnalysisV1,
    ConditionalTotalViewErrorV1, ConditionalTotalViewUnsupportedV1, ScalarType, Type,
    derive_conditional_total_view_from_verified_v1,
};
use fe2o3_lower_mir_kernel::{
    NativeRankedSourceCandidateV1, ProductionConditionalOutputBindingErrorV1,
    ProductionConditionalRankedCoverageErrorV1, ProductionConditionalRankedCoverageV1,
    ProductionConditionalRankedOutputErrorV1, ProductionConditionalRankedOutputV1,
    ProductionConditionalSourceTranslationErrorV1, ProductionConditionalSourceTranslationV1,
    ProductionPreRankedKirOwnerV1,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAbiArgumentRoleV1 as Role, SemanticAbiPassModeV1 as Pass, SemanticFunctionRoleV1,
    SemanticRustTypeKindV1, SemanticTypeShapeV1,
};
use fe2o3_pliron::{
    ProductionConditionalOwnershipSiteV1, ProductionConditionalRankedAnalysisV1,
    ProductionEffectRefinementContractV2 as Contract, ProductionNumericalContractV2 as Numerical,
    ProductionRankedBlockV1 as Block, ProductionRankedOperationV1 as Op,
    ProductionRankedValueV1 as Value, ProductionSemanticExpressionV2 as Expr,
    ProductionSemanticScalarTypeV2 as Ty,
};
use fe2o3_proof_contracts::DigestV1;
use std::fmt;

#[derive(Debug)]
pub(crate) enum Error {
    Resource(Resource),
    Source(Box<ProductionConditionalSourceTranslationErrorV1>),
    Canonical(Box<ConditionalTotalViewErrorV1>),
    Unsupported(ConditionalTotalViewUnsupportedV1),
    Binding(Box<ProductionConditionalOutputBindingErrorV1>),
    Output(ProductionConditionalRankedOutputErrorV1),
    Coverage(ProductionConditionalRankedCoverageErrorV1),
    Root,
    Ownership,
    Reference(&'static str),
}

impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("conditional reference output: ")?;
        match self {
            Self::Resource(error) => write!(out, "{error}"),
            Self::Source(error) => write!(out, "source translation: {error}"),
            Self::Canonical(error) => write!(out, "canonical coverage: {error}"),
            Self::Unsupported(reason) => write!(out, "unsupported canonical coverage: {reason:?}"),
            Self::Binding(error) => write!(out, "source output: {error}"),
            Self::Output(error) => write!(out, "ranked output: {error}"),
            Self::Coverage(error) => write!(out, "ranked coverage: {error}"),
            Self::Root => out.write_str("source root association changed"),
            Self::Ownership => out.write_str("selected ownership occurrence changed"),
            Self::Reference(reason) => write!(out, "CPU reference: {reason}"),
        }
    }
}

struct CpuReferenceOutputV1<'a> {
    binding: &'a AuthenticatedReferenceEffectBindingV1,
    write: &'a ReferenceOutputWriteV1,
    raw_argument: u32,
}

pub(crate) struct ConditionalReferenceOutputV1<'a> {
    translation: &'a ProductionConditionalSourceTranslationV1<'a>,
    coverage: &'a ProductionConditionalRankedCoverageV1<'a>,
    ownership: &'a ProductionConditionalOwnershipSiteV1,
    reference: CpuReferenceOutputV1<'a>,
}

impl<'a> ConditionalReferenceOutputV1<'a> {
    pub(crate) fn translation(&self) -> &'a ProductionConditionalSourceTranslationV1<'a> {
        self.translation
    }

    pub(crate) fn coverage(&self) -> &'a ProductionConditionalRankedCoverageV1<'a> {
        self.coverage
    }

    pub(crate) fn ownership(&self) -> &'a ProductionConditionalOwnershipSiteV1 {
        self.ownership
    }

    pub(crate) fn reference_binding(&self) -> &'a AuthenticatedReferenceEffectBindingV1 {
        self.reference.binding
    }

    pub(crate) fn reference_write(&self) -> &'a ReferenceOutputWriteV1 {
        self.reference.write
    }

    pub(crate) fn raw_reference_argument(&self) -> u32 {
        self.reference.raw_argument
    }
}

/// The callback cannot retain local checked facts. Runtime premises and every
/// pending pipeline obligation remain external. Existing checker allocation
/// boundaries apply; this wrapper adds no retained graph or correspondence map.
pub(crate) fn with_conditional_reference_output_v1<R>(
    owner: &ProductionPreRankedKirOwnerV1,
    pending: &ProductionConditionalRankedAnalysisV1,
    candidate: NativeRankedSourceCandidateV1<'_>,
    references: &AuthenticatedReferenceEffectBindingsV1,
    logical_name: &str,
    budget: &mut Budget<'_>,
    use_join: impl for<'q> FnOnce(ConditionalReferenceOutputV1<'q>) -> Result<R, Error>,
) -> Result<R, Error> {
    let translation = owner
        .check_conditional_source_translation_v1(pending, candidate, budget)
        .map_err(|error| Error::Source(Box::new(error)))?;
    let mut selected = None;
    for kernel in &owner.executable().module().kernels {
        budget.charge_work(3)?;
        budget.charge_work(
            kernel
                .entry
                .as_str()
                .len()
                .checked_add(candidate.kernel().function_name().len())
                .ok_or(Resource::Accounting)?,
        )?;
        // The symbol only locates a candidate; the checked source-root binding
        // below, not this lookup, establishes its association with the source.
        if kernel.entry.as_str() == candidate.kernel().function_name()
            && selected.replace(kernel).is_some()
        {
            return Err(Error::Root);
        }
    }
    let kernel = selected.ok_or(Error::Root)?;
    let facts = match derive_conditional_total_view_from_verified_v1(
        owner.executable().verified_module_ref_v1(),
        &kernel.id,
        budget,
    )
    .map_err(|error| Error::Canonical(Box::new(error)))?
    {
        ConditionalTotalViewAnalysisV1::Established(facts) => facts,
        ConditionalTotalViewAnalysisV1::Unsupported(reason) => {
            return Err(Error::Unsupported(reason));
        }
    };
    let binding = owner
        .bind_conditional_output_v1(facts, budget)
        .map_err(|error| Error::Binding(Box::new(error)))?;
    budget.charge_work(3)?;
    if binding.association().correspondence_owner().index() != candidate.semantic_root()
        || binding.source_launch().source_rank() != candidate.launch_rank()
    {
        return Err(Error::Root);
    }
    let coverage = binding
        .inspect_ranked_output_v1(candidate, budget)
        .map_err(Error::Output)?
        .check_ranked_coverage_v1(budget)
        .map_err(Error::Coverage)?;
    let ownership = checked_ownership_site(pending, coverage.output(), budget)?;
    let reference = check_cpu_reference(references, coverage.output(), logical_name, budget)?;
    // Replay establishes the value relation; these counts restrict its cardinality.
    budget.charge_work(4)?;
    cpu_require(
        translation.memory_effects() == 1 && translation.value_expressions() == 1,
        "source effect/value count",
    )?;
    use_join(ConditionalReferenceOutputV1 {
        translation: &translation,
        coverage: &coverage,
        ownership,
        reference,
    })
}

fn checked_ownership_site<'a>(
    pending: &'a ProductionConditionalRankedAnalysisV1,
    output: &ProductionConditionalRankedOutputV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<&'a ProductionConditionalOwnershipSiteV1, Error> {
    budget.charge_work(8)?;
    let [site] = pending.selections() else {
        return Err(Error::Ownership);
    };
    let operation = output
        .candidate()
        .kernel()
        .blocks()
        .get(site.block as usize)
        .and_then(|block| block.operations().get(site.operation as usize));
    match operation {
        Some(Op::OwnershipContract {
            view,
            coverage: OwnershipCoverageAttr::TotalView,
            partition: OwnershipPartitionAttr::ExactSets,
        }) if *view == site.view && *view == output.view() => Ok(site),
        _ => Err(Error::Ownership),
    }
}

fn cpu_require(ok: bool, why: &'static str) -> Result<(), Error> {
    if ok {
        Ok(())
    } else {
        Err(Error::Reference(why))
    }
}

fn cpu_u32(value: &Rvalue) -> Option<u128> {
    match value {
        Rvalue::Use(Operand::Constant(Const::Scalar {
            scalar: Scalar::U32,
            bits,
        })) => Some(*bits),
        _ => None,
    }
}

fn check_cpu_reference<'a>(
    references: &'a AuthenticatedReferenceEffectBindingsV1,
    output: &ProductionConditionalRankedOutputV1<'_>,
    logical_name: &str,
    budget: &mut Budget<'_>,
) -> Result<CpuReferenceOutputV1<'a>, Error> {
    // Root identities and at most two complete ABI arguments are fixed-size.
    budget.charge_work(1024)?;
    let [binding] = references.as_slice() else {
        return Err(Error::Reference("binding count"));
    };
    let bound = output.binding();
    if budget.storage() < bound.owner().retained_analysis_storage_v1() {
        return Err(Resource::Accounting.into());
    }
    budget.charge_work(
        logical_name
            .len()
            .checked_add(binding.logical_kernel_name.len())
            .ok_or(Resource::Arithmetic)?,
    )?;
    cpu_require(binding.logical_kernel_name == logical_name, "logical name")?;
    let semantic = bound.owner().semantic_ssa().source_semantic();
    let association = bound.association();
    let root = semantic
        .functions()
        .get(association.correspondence_owner().index() as usize)
        .ok_or(Error::Reference("root"))?;
    let body = semantic
        .functions()
        .get(association.semantic_function().index() as usize)
        .ok_or(Error::Reference("body"))?;
    let k = &binding.kernel;
    cpu_require(
        root.role() == SemanticFunctionRoleV1::KernelRoot
            && root.identity().as_bytes() == &k.function_sha256
            && root.item_definition_identity().as_bytes() == &k.item_definition_sha256
            && root.monomorphization_identity().as_bytes() == &k.monomorphization_sha256
            && root.generic_type_arguments_identity().as_bytes()
                == &k.generic_type_arguments_sha256
            && root.const_generic_arguments_identity().as_bytes()
                == &k.const_generic_arguments_sha256,
        "kernel identity",
    )?;
    cpu_require(
        bound.source_argument() == 0 && bound.adjusted_argument() == 0,
        "output ordinal",
    )?;
    for function in [root, body] {
        let abi = function.abi();
        let [arg] = abi.arguments() else {
            return Err(Error::Reference("flat ABI"));
        };
        cpu_require(
            !abi.c_variadic()
                && abi.source_input_types() == [bound.semantic_type()]
                && arg.ty() == bound.semantic_type()
                && arg.role() == Role::Source
                && matches!(arg.mode(), Pass::Direct(_) | Pass::Pair { .. }),
            "flat ABI",
        )?;
    }
    let ty = semantic
        .types()
        .get(bound.semantic_type().index() as usize)
        .ok_or(Error::Reference("output type"))?;
    cpu_require(
        !matches!(ty.shape(), SemanticTypeShapeV1::Tuple(_))
            && !matches!(ty.rust_type_kind(), SemanticRustTypeKindV1::Execution(_)),
        "output type",
    )?;
    let physical = bound
        .coverage()
        .function()
        .signature
        .parameters
        .get(bound.coverage().output_parameter_index() as usize);
    cpu_require(
        matches!(physical, Some(Type::Slice(slice))
        if slice.address_space == AddressSpace::Global && slice.access == AccessMode::ReadWrite
        && matches!(slice.element.as_ref(), Type::Scalar(ScalarType::U32))),
        "physical output",
    )?;
    let (reference, bits) = check_constant_point_effect(binding, budget)?;
    let write = reference.write;
    let contract = output.contract();
    let site = contract.reference_output_site();
    cpu_require(
        site.argument() == bound.source_argument()
            && site.block() == write.block
            && site.statement() == write.statement,
        "reference site",
    )?;
    let gpu = contract.gpu_write_site();
    let mut prefix = [0; 8];
    prefix.copy_from_slice(&binding.effect_ir_sha256[..8]);
    let identity = (u64::from_le_bytes(prefix)
        ^ u64::from(gpu.block()).rotate_left(17)
        ^ u64::from(gpu.operation()).rotate_left(41))
    .max(1);
    cpu_require(
        contract.contract_identity() == identity,
        "contract identity",
    )?;
    let subjects = FunctionalRefinementSubjectsV2::new(
        SafeReferenceKindV2::Mir,
        DigestV1::from_untrusted_bytes(binding.reference.function_sha256),
        DigestV1::ZERO,
        DigestV1::from_untrusted_bytes(binding.reference.rustc_mir_body_sha256),
        DigestV1::from_untrusted_bytes(k.function_sha256),
        DigestV1::from_untrusted_bytes(k.rustc_mir_body_sha256),
    )
    .map_err(|_| Error::Reference("subjects"))?;
    check_constant_u32_operands_v1(
        output.candidate().kernel().blocks(),
        contract,
        subjects,
        bits as u32,
        budget,
    )?;
    Ok(reference)
}

fn check_constant_u32_operands_v1(
    blocks: &[Block],
    contract: &Contract,
    subjects: FunctionalRefinementSubjectsV2,
    bits: u32,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    let [coordinate] = contract.reference_coordinates() else {
        return Err(Error::Reference("reference rank"));
    };
    budget.charge_work(16)?;
    let [gpu_coordinate] = contract.gpu_coordinates() else {
        return Err(Error::Reference("GPU rank"));
    };
    let gpu = contract.gpu_write_site();
    let operation = blocks
        .get(gpu.block() as usize)
        .and_then(|block| block.operations().get(gpu.operation() as usize));
    cpu_require(
        matches!(operation, Some(Op::ValueAccess {
            kind: dialect_kernel::AccessKindAttr::Write, view, indices, value,
        }) if *view == contract.view()
            && matches!(indices.as_slice(), [index] if contract.indices() == [*index])
            && *value == contract.gpu_value()),
        "GPU write",
    )?;
    let expected = [
        (
            [*coordinate, *gpu_coordinate],
            Expr::Symbol {
                symbol: 0,
                scalar: Ty::Integer {
                    signed: false,
                    bits: 64,
                },
            },
        ),
        (
            [contract.reference_domain(), contract.gpu_domain()],
            Expr::Constant {
                scalar: Ty::Bool,
                bits: 1,
            },
        ),
        (
            [
                contract.reference_precondition(),
                contract.gpu_precondition(),
            ],
            Expr::Constant {
                scalar: Ty::Bool,
                bits: 1,
            },
        ),
        (
            [contract.reference_value(), contract.gpu_value()],
            Expr::Constant {
                scalar: Ty::Integer {
                    signed: false,
                    bits: 32,
                },
                bits: u64::from(bits),
            },
        ),
    ];
    let (mut selected, mut seen) = (false, [false; 8]);
    for block in blocks {
        budget.charge_work(1)?;
        for op in block.operations() {
            // Covers eight fixed operand checks and the fixed-size subject comparison.
            budget.charge_work(1024)?;
            let actual_subjects = match op {
                Op::RequireEffectRefinement {
                    contract: row,
                    proof,
                } if std::ptr::eq(row, contract) => Some(proof.binding().subjects()),
                _ => None,
            };
            if let Some(actual) = actual_subjects {
                cpu_require(!selected && actual == subjects, "contract subjects")?;
                selected = true;
            }
            if let Op::SemanticExpression {
                result,
                expression,
                numerical_contract,
            } = op
            {
                for (i, (operand, wanted)) in expected
                    .iter()
                    .flat_map(|(operands, wanted)| {
                        operands.iter().map(move |operand| (operand, wanted))
                    })
                    .enumerate()
                {
                    if *operand != Value::Local(*result) {
                        continue;
                    }
                    cpu_require(
                        !seen[i]
                            && matches!(expression, Expr::Constant { .. } | Expr::Symbol { .. }),
                        ["reference definition", "GPU definition"][i % 2],
                    )?;
                    cpu_require(
                        *numerical_contract == Numerical::ExactBitVectorOperatorCongruence
                            && expression == wanted,
                        ["reference expression", "GPU expression"][i % 2],
                    )?;
                    seen[i] = true;
                }
            }
        }
    }
    // Terminal debit makes a one-short successful-path budget fail with Resource::Work.
    budget.charge_work(16)?;
    cpu_require(
        seen.into_iter().all(|found| found),
        "missing contract operand",
    )?;
    cpu_require(selected, "missing required proof")
}

fn check_constant_point_effect<'a>(
    binding: &'a AuthenticatedReferenceEffectBindingV1,
    budget: &mut Budget<'_>,
) -> Result<(CpuReferenceOutputV1<'a>, u128), Error> {
    // Shape checks bound all nested data before equality or canonical hashing.
    // At most two inputs, assignments and output observations are visited.
    budget.charge_work(1024)?;
    let sig = &binding.signature_preimage;
    cpu_require(
        matches!(
            sig.kernel_inputs(),
            [Input::NominalOutput {
                carrier: Carrier::DisjointSlice,
                element: Scalar::U32
            }]
        ) && sig.reference_inputs().len() == 2,
        "signature fragment",
    )?;
    // Derivation borrows without allocating; the fixed debit covers both slots.
    let derived = sig
        .derive_relations_v1()
        .map_err(|_| Error::Reference("signature relation"))?;
    let ir = &binding.effect_ir;
    cpu_require(
        ir.argument_count == 2
            && ir.local_count == 3
            && ir.relations.len() == 2
            && ir.loop_summaries.is_empty(),
        "IR header",
    )?;
    for (raw, actual) in ir.relations.iter().enumerate() {
        cpu_require(
            derived.relation_at_raw_argument_v1(raw as u32) == Some(*actual),
            "IR relation",
        )?;
    }
    let raw_argument = derived
        .reference_argument_for_kernel_argument_v1(0)
        .ok_or(Error::Reference("raw argument"))?;
    cpu_require(
        derived.relation_at_raw_argument_v1(raw_argument)
            == Some(Rel::DisjointOutputCoordinate {
                argument: 0,
                element: Scalar::U32,
            }),
        "output role",
    )?;
    let local = raw_argument.checked_add(1).ok_or(Resource::Arithmetic)?;
    let [block] = ir.blocks.as_ref() else {
        return Err(Error::Reference("CPU blocks"));
    };
    cpu_require(
        block.block == 0
            && matches!(&block.terminator, Term::Return)
            && (1..=2).contains(&block.assignments.len()),
        "CPU block",
    )?;
    let (mut store, mut previous) = (None, None);
    for assignment in &block.assignments {
        cpu_require(
            previous.is_none_or(|s| s < assignment.statement),
            "statement order",
        )?;
        previous = Some(assignment.statement);
        let place = &assignment.destination;
        if place.local == local && matches!(place.projection.as_ref(), [Projection::Dereference]) {
            cpu_require(store.replace(assignment).is_none(), "duplicate CPU store")?;
        } else {
            cpu_require(
                place.local == 0
                    && place.projection.is_empty()
                    && matches!(
                        &assignment.value,
                        Rvalue::Use(Operand::Constant(Const::ZeroSized))
                    ),
                "CPU assignment",
            )?;
        }
    }
    let store = store.ok_or(Error::Reference("missing CPU store"))?;
    let bits = cpu_u32(&store.value).ok_or(Error::Reference("CPU value"))?;
    cpu_require(bits <= u128::from(u32::MAX), "CPU constant width")?;
    let [write] = binding.observable_output_writes.as_ref() else {
        return Err(Error::Reference("output count"));
    };
    let [effect] = ir.observable_output_effects.as_ref() else {
        return Err(Error::Reference("effect count"));
    };
    for w in [write, effect] {
        cpu_require(
            w.argument == 0
                && w.block == 0
                && w.statement == store.statement
                && cpu_u32(&w.value) == Some(bits),
            "CPU write identity/value",
        )?;
        cpu_require(
            matches!(&w.coordinate, Coord::LogicalPoint(axes)
            if matches!(axes.as_ref(), [CpuExpr::PointCoordinate { axis: 0 }])),
            "CPU coordinate",
        )?;
        cpu_require(
            matches!(w.guard.clauses.as_ref(), [clause] if clause.atoms.is_empty()),
            "CPU guard",
        )?;
        cpu_require(
            matches!(&w.rhs, CpuExpr::Constant(Const::Scalar { scalar: Scalar::U32, bits: b })
            if *b == bits),
            "CPU RHS",
        )?;
    }
    // The two admitted assignment shapes encode 194 or 212 transcript bytes.
    // Prepay hashing and its bounded traversal only after all shape checks.
    budget.charge_work(1024)?;
    cpu_require(
        ir.canonical_sha256_v1() == binding.effect_ir_sha256,
        "effect digest",
    )?;
    Ok((
        CpuReferenceOutputV1 {
            binding,
            write,
            raw_argument,
        },
        bits,
    ))
}

#[cfg(test)]
#[path = "production_conditional_reference_output_v1_tests.rs"]
mod tests;

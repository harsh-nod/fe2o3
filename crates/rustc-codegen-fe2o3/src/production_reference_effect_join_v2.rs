//! Compiler-private join from authenticated Rust reference MIR to bounded ranked GPU writes.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use dialect_kernel::{
    DYNAMIC_EXTENT, IndexBinaryKindAttr, OwnershipCoverageAttr, OwnershipPartitionAttr,
};
use fe2o3_functional_proof::{FunctionalRefinementSubjectsV2, SafeReferenceKindV2};
use fe2o3_pliron::{
    ProductionConstructionV1, ProductionEffectRefinementContractV2, ProductionGpuWriteSiteV2,
    ProductionNumericalContractV2, ProductionOverflowContractV2, ProductionRankedBlockV1,
    ProductionRankedCompileErrorV2, ProductionRankedKernelErrorV1,
    ProductionRankedKernelLoweringInputV1, ProductionRankedKernelV1, ProductionRankedOperationV1,
    ProductionRankedTerminatorV1, ProductionRankedValueIdV1, ProductionRankedValueV1,
    ProductionReferenceOutputSiteV2, ProductionReferenceProofV2,
    ProductionRefinementStagingPolicyV2, ProductionSemanticBinaryOpV2, ProductionSemanticCastV2,
    ProductionSemanticComparisonV2, ProductionSemanticExpressionV2, ProductionSemanticScalarTypeV2,
    ProductionSemanticUnaryOpV2, ProductionSessionLimitsV1,
    compile_ranked_kernel_with_policy_checked_refinement_staging_v2,
};
use fe2o3_proof_contracts::DigestV1;

use crate::reference_effect_bijection_v1::{
    CompilerExtractedGpuOutputEffectV1, ReferenceEffectBijectionErrorV1,
    establish_reference_effect_bijection_v1,
};
use crate::reference_effect_v1::{
    AuthenticatedReferenceEffectBindingsV1, ReferenceArgumentRelationV1, ReferenceBinaryOpV1,
    ReferenceCastKindV1, ReferenceConstantV1, ReferenceEffectExpressionV1, ReferenceEffectIrV1,
    ReferenceOutputCoordinateV1, ReferenceOutputWriteV1, ReferencePathPredicateV1,
    ReferenceScalarTypeV1, ReferenceUnaryOpV1,
};

const ROOT_NAME_V2: &str = "semantic_safety_module";
const LOCAL_PROOF_TIMEOUT_SECONDS_V2: u32 = 60;
const WHOLE_COMPILE_PROOF_TIMEOUT_SECONDS_V2: u32 = 120;
const RETAINED_FUNCTIONAL_REFINEMENT_RUNTIME_ROOT_V1: &str =
    "/opt/fe2o3/verus-runtime-v2/functional-refinement-0.2026.08.02-b677dd5";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RankedGpuWriteV2 {
    pub(crate) block: usize,
    pub(crate) operation: usize,
    pub(crate) allocation_origin: u64,
    pub(crate) view: ProductionRankedValueV1,
    pub(crate) indices: Vec<ProductionRankedValueV1>,
    pub(crate) value: Result<ProductionSemanticExpressionV2, &'static str>,
}

pub(crate) fn reserved_reference_value_count_v2(
    bindings: &AuthenticatedReferenceEffectBindingsV1,
) -> Result<usize, crate::production_ranked_projection_v1::ProductionRankedProjectionErrorV1> {
    reserved_reference_output_ranks_v2(bindings)?
        .into_iter()
        .try_fold(0_usize, |total, coordinate_count| {
            total.checked_add(3)?.checked_add(coordinate_count)
        })
        .ok_or(
            crate::production_ranked_projection_v1::ProductionRankedProjectionErrorV1::Unsupported(
                "reference-effect scalar reservation count overflowed",
            ),
        )
}

pub(crate) fn reserved_reference_output_ranks_v2(
    bindings: &AuthenticatedReferenceEffectBindingsV1,
) -> Result<Vec<usize>, crate::production_ranked_projection_v1::ProductionRankedProjectionErrorV1> {
    let [binding] = bindings.as_slice() else {
        return Err(
            crate::production_ranked_projection_v1::ProductionRankedProjectionErrorV1::Unsupported(
                "reference-effect projection requires exactly one authenticated kernel/reference binding",
            ),
        );
    };
    if binding.observable_output_writes.is_empty() {
        return Err(
            crate::production_ranked_projection_v1::ProductionRankedProjectionErrorV1::Unsupported(
                "reference-effect projection found no observable reference output",
            ),
        );
    }
    if binding.observable_output_writes.len()
        > fe2o3_verifier::MAX_PRODUCTION_AGGREGATE_EFFECT_FORMULA_OUTPUTS_V1
    {
        return Err(
            crate::production_ranked_projection_v1::ProductionRankedProjectionErrorV1::Unsupported(
                "reference-effect output count exceeds the production aggregate proof limit",
            ),
        );
    }
    binding
        .observable_output_writes
        .iter()
        .map(|write| match &write.coordinate {
            ReferenceOutputCoordinateV1::LogicalPoint(axes) => Ok(axes.len()),
            _ => Err(
                crate::production_ranked_projection_v1::ProductionRankedProjectionErrorV1::Unsupported(
                    "reference-effect projection requires independently indexed logical point outputs",
                ),
            ),
        })
        .collect()
}

/// Move-only compiler custody over a request derived from exact collector and
/// ranked-projection state. Generic receipt APIs cannot construct this type.
pub(crate) struct CompilerOwnedReferenceEffectRequestV2 {
    kernel: ProductionRankedKernelV1,
    requests: Vec<CompilerOwnedReferenceEffectSiteV2>,
    proof_timeout_seconds: u32,
}

struct CompilerOwnedReferenceEffectSiteV2 {
    block: usize,
    operation: usize,
    subjects: FunctionalRefinementSubjectsV2,
}

struct PreparedReferenceOutputV2 {
    write: RankedGpuWriteV2,
    reference_write: ReferenceOutputWriteV1,
    output_argument: u32,
    gpu_expression: ProductionSemanticExpressionV2,
    reference_expression: ProductionSemanticExpressionV2,
    numerical_contract: ProductionNumericalContractV2,
    reserved_values: Vec<ProductionRankedValueIdV1>,
}

impl CompilerOwnedReferenceEffectRequestV2 {
    pub(crate) fn prove_and_compile(
        self,
    ) -> Result<ProductionRankedKernelLoweringInputV1, ProductionReferenceEffectJoinErrorV2> {
        let runtime = fe2o3_verifier::FunctionalRefinementVerusRuntimeLeaseV1::open(
            RETAINED_FUNCTIONAL_REFINEMENT_RUNTIME_ROOT_V1,
        )
        .map_err(|error| {
            ProductionReferenceEffectJoinErrorV2::ProofRuntimeUnavailable {
                root: RETAINED_FUNCTIONAL_REFINEMENT_RUNTIME_ROOT_V1,
                detail: error.to_string(),
            }
        })?;
        let mut imported_proofs = Vec::with_capacity(self.requests.len());
        let mut bindings = Vec::with_capacity(self.requests.len());
        let mut signers = Vec::with_capacity(self.requests.len());
        let mut toolchain = None;
        for request in &self.requests {
            let (binding, imported, _single_receipt_policy) =
                fe2o3_verifier::execute_and_import_ranked_functional_refinement_locally_v2(
                    &runtime,
                    &self.kernel,
                    request.block,
                    request.operation,
                    request.subjects,
                    self.proof_timeout_seconds,
                )
                .map_err(|error| {
                    ProductionReferenceEffectJoinErrorV2::ProofExecution(error.to_string())
                })?;
            if toolchain.is_some_and(|expected| expected != imported.toolchain()) {
                return Err(ProductionReferenceEffectJoinErrorV2::ProofExecution(
                    "per-output receipts were imported under different Verus toolchains".to_owned(),
                ));
            }
            toolchain = Some(imported.toolchain());
            signers.push(imported.signer_identity());
            bindings.push((
                request.block,
                request.operation,
                ProductionReferenceProofV2::request_exact(imported.receipt_identity(), binding),
            ));
            imported_proofs.push(imported);
        }
        let toolchain = toolchain.ok_or_else(|| {
            ProductionReferenceEffectJoinErrorV2::ProofExecution(
                "compiler-owned reference request contains no output roles".to_owned(),
            )
        })?;
        let policy = ProductionRefinementStagingPolicyV2::new(signers, toolchain)
            .map_err(ProductionReferenceEffectJoinErrorV2::Recipe)?;
        let mut bound = self.kernel;
        for (block, operation, request) in bindings {
            bound = bound
                .bind_functional_refinement_request_v2(block, operation, request)
                .map_err(ProductionReferenceEffectJoinErrorV2::Recipe)?;
        }
        let construction =
            ProductionConstructionV1::ranked_kernel(ROOT_NAME_V2, bound).map_err(|error| {
                ProductionReferenceEffectJoinErrorV2::Construction(format!("{error:?}"))
            })?;
        compile_ranked_kernel_with_policy_checked_refinement_staging_v2(
            construction,
            ProductionSessionLimitsV1::default(),
            imported_proofs,
            policy,
        )
        .map_err(|error| ProductionReferenceEffectJoinErrorV2::Compile(Box::new(error)))
    }
}

fn per_output_proof_timeout_v2(
    output_count: usize,
) -> Result<u32, ProductionReferenceEffectJoinErrorV2> {
    let output_count_u32 = u32::try_from(output_count).map_err(|_| {
        ProductionReferenceEffectJoinErrorV2::ProofOutputLimit {
            actual: output_count,
            limit: fe2o3_verifier::MAX_PRODUCTION_AGGREGATE_EFFECT_FORMULA_OUTPUTS_V1,
        }
    })?;
    if output_count == 0
        || output_count > fe2o3_verifier::MAX_PRODUCTION_AGGREGATE_EFFECT_FORMULA_OUTPUTS_V1
    {
        return Err(ProductionReferenceEffectJoinErrorV2::ProofOutputLimit {
            actual: output_count,
            limit: fe2o3_verifier::MAX_PRODUCTION_AGGREGATE_EFFECT_FORMULA_OUTPUTS_V1,
        });
    }
    Ok((WHOLE_COMPILE_PROOF_TIMEOUT_SECONDS_V2 / output_count_u32)
        .min(LOCAL_PROOF_TIMEOUT_SECONDS_V2))
}

pub(crate) fn prepare_reference_effect_request_v2(
    kernel: ProductionRankedKernelV1,
    bindings: &AuthenticatedReferenceEffectBindingsV1,
    writes: &[RankedGpuWriteV2],
    reserved_values: Vec<ProductionRankedValueIdV1>,
) -> Result<CompilerOwnedReferenceEffectRequestV2, ProductionReferenceEffectJoinErrorV2> {
    let [binding] = bindings.as_slice() else {
        return Err(ProductionReferenceEffectJoinErrorV2::BindingCount(
            bindings.as_slice().len(),
        ));
    };
    if binding.observable_output_writes.is_empty() {
        return Err(ProductionReferenceEffectJoinErrorV2::ReferenceWriteCount(
            binding.observable_output_writes.len(),
        ));
    }
    let proof_timeout_seconds =
        per_output_proof_timeout_v2(binding.observable_output_writes.len())?;
    if binding.observable_output_writes.iter().any(|write| {
        !matches!(
            write.coordinate,
            ReferenceOutputCoordinateV1::LogicalPoint(_)
        )
    }) {
        return Err(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
            "V2 source join accepts independently indexed logical point outputs",
        ));
    }
    let gpu_effects = writes
        .iter()
        .map(|write| compiler_extracted_gpu_effect_v1(&kernel, binding, write))
        .collect::<Result<Vec<_>, _>>()?;
    let pairs = establish_reference_effect_bijection_v1(
        binding.observable_output_writes.as_ref(),
        &gpu_effects,
    )
    .map_err(ProductionReferenceEffectJoinErrorV2::EffectBijection)?;
    if pairs.len() != binding.observable_output_writes.len() {
        return Err(ProductionReferenceEffectJoinErrorV2::ReferenceWriteCount(
            pairs.len(),
        ));
    }

    let mut writes_by_location = BTreeMap::new();
    for write in writes {
        if writes_by_location
            .insert((write.block, write.operation), write)
            .is_some()
        {
            return Err(ProductionReferenceEffectJoinErrorV2::WriteLocation);
        }
    }
    let mut output_relations = BTreeMap::new();
    for relation in &binding.effect_ir.relations {
        if let ReferenceArgumentRelationV1::DisjointOutputCoordinate { argument, element } =
            relation
            && output_relations.insert(*argument, *element).is_some()
        {
            return Err(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
                "logical output argument has multiple per-coordinate ABI relations",
            ));
        }
    }

    let mut prepared = Vec::with_capacity(pairs.len());
    let mut reserved_cursor = 0_usize;
    for (reference_write, pair) in binding.observable_output_writes.iter().zip(pairs.iter()) {
        let element = output_relations
            .get(&reference_write.argument)
            .copied()
            .ok_or(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
                "observable reference write has no per-coordinate logical ABI relation",
            ))?;
        let output_argument = reference_write.argument;
        let write = writes_by_location
            .get(&(pair.gpu_block as usize, pair.gpu_operation as usize))
            .copied()
            .cloned()
            .ok_or(ProductionReferenceEffectJoinErrorV2::WriteLocation)?;
        let gpu_expression = write.value.clone().map_err(|detail| {
            ProductionReferenceEffectJoinErrorV2::UnmodeledGpuValue {
                block: write.block,
                operation: write.operation,
                detail,
            }
        })?;
        let reference_expression = reference_expression_with_gpu_loads_v2(
            &binding.effect_ir,
            &reference_write.rhs,
            element,
            &kernel,
            &gpu_expression,
        )?;
        if gpu_expression.scalar() != reference_expression.scalar() {
            return Err(ProductionReferenceEffectJoinErrorV2::UnsupportedGpuEffect {
                block: write.block,
                operation: write.operation,
                detail: "GPU and reference RHS scalar types disagree",
            });
        }
        let numerical_contract =
            ProductionNumericalContractV2::exact_for_expression(&reference_expression);
        let reference_rank = reference_logical_point_rank_v2(&reference_write.coordinate)?;
        let reserved_count = 3_usize
            .checked_add(reference_rank)
            .ok_or(ProductionReferenceEffectJoinErrorV2::ReservedValueCountOverflow)?;
        let reserved_end = reserved_cursor
            .checked_add(reserved_count)
            .ok_or(ProductionReferenceEffectJoinErrorV2::ReservedValueCountOverflow)?;
        let output_reserved_values = reserved_values
            .get(reserved_cursor..reserved_end)
            .ok_or(
                ProductionReferenceEffectJoinErrorV2::InvalidReservedValueCount {
                    expected: reserved_end,
                    actual: reserved_values.len(),
                },
            )?
            .to_vec();
        reserved_cursor = reserved_end;
        prepared.push(PreparedReferenceOutputV2 {
            write,
            reference_write: reference_write.clone(),
            output_argument,
            gpu_expression,
            reference_expression,
            numerical_contract,
            reserved_values: output_reserved_values,
        });
    }
    if reserved_cursor != reserved_values.len() {
        return Err(
            ProductionReferenceEffectJoinErrorV2::InvalidReservedValueCount {
                expected: reserved_cursor,
                actual: reserved_values.len(),
            },
        );
    }
    crate::production_reference_bounds_v2::discharge_reference_bounds_over_ranked_domains_v2(
        &kernel,
        &binding.effect_ir,
        &prepared
            .iter()
            .map(
                |output| crate::production_reference_bounds_v2::CompilerOwnedOutputDomainV2 {
                    reference: &output.reference_write,
                    ranked_view: output.write.view,
                },
            )
            .collect::<Vec<_>>(),
    )
    .map_err(
        |error| ProductionReferenceEffectJoinErrorV2::ReferenceBoundsCheck {
            block: error.block(),
            detail: error.detail().to_owned(),
        },
    )?;

    let subjects = FunctionalRefinementSubjectsV2::new(
        SafeReferenceKindV2::Mir,
        DigestV1::from_untrusted_bytes(binding.reference.function_sha256),
        DigestV1::ZERO,
        DigestV1::from_untrusted_bytes(binding.reference.rustc_mir_body_sha256),
        DigestV1::from_untrusted_bytes(binding.kernel.function_sha256),
        DigestV1::from_untrusted_bytes(binding.kernel.rustc_mir_body_sha256),
    )
    .map_err(|error| ProductionReferenceEffectJoinErrorV2::Subjects(error.to_string()))?;

    let mut blocks = kernel.blocks().to_vec();
    let mut owned_views = BTreeSet::new();
    let existing_ownership = blocks
        .iter()
        .flat_map(|block| block.operations())
        .filter_map(|operation| match operation {
            ProductionRankedOperationV1::OwnershipContract { view, .. } => Some(*view),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    for output in &prepared {
        if !owned_views.insert(output.write.view) || existing_ownership.contains(&output.write.view)
        {
            return Err(ProductionReferenceEffectJoinErrorV2::AmbiguousOwnership);
        }
    }
    let entry = blocks
        .first_mut()
        .ok_or(ProductionReferenceEffectJoinErrorV2::WriteLocation)?;
    let mut entry_operations = entry.operations().to_vec();
    for output in &prepared {
        let [
            true_value,
            gpu_value_id,
            reference_value_id,
            coordinate_values @ ..,
        ] = output.reserved_values.as_slice()
        else {
            return Err(
                ProductionReferenceEffectJoinErrorV2::InvalidReservedValueCount {
                    expected: 3,
                    actual: output.reserved_values.len(),
                },
            );
        };
        replace_reserved_semantic_expression_v2(
            &mut entry_operations,
            *true_value,
            ProductionSemanticExpressionV2::Constant {
                scalar: ProductionSemanticScalarTypeV2::Bool,
                bits: 1,
            },
            ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
        )?;
        replace_reserved_semantic_expression_v2(
            &mut entry_operations,
            *gpu_value_id,
            output.gpu_expression.clone(),
            output.numerical_contract,
        )?;
        replace_reserved_semantic_expression_v2(
            &mut entry_operations,
            *reference_value_id,
            output.reference_expression.clone(),
            output.numerical_contract,
        )?;
        for (axis, identity) in coordinate_values.iter().copied().enumerate() {
            let expected_symbol = u32::try_from(axis).map_err(|_| {
                ProductionReferenceEffectJoinErrorV2::InvalidReservedValue(identity.get())
            })?;
            replace_reserved_semantic_symbol_v2(&mut entry_operations, identity, expected_symbol)?;
        }
        entry_operations.push(ProductionRankedOperationV1::OwnershipContract {
            view: output.write.view,
            coverage: OwnershipCoverageAttr::TotalView,
            partition: OwnershipPartitionAttr::ExactSets,
        });
    }
    *entry = ProductionRankedBlockV1::with_index_arguments(
        entry.index_argument_count(),
        entry_operations,
        entry.terminator().clone(),
    );
    let mut requests = Vec::with_capacity(prepared.len());
    for output in prepared {
        let [
            true_value,
            gpu_value_id,
            reference_value_id,
            coordinate_identities @ ..,
        ] = output.reserved_values.as_slice()
        else {
            unreachable!("validated per-output reservation")
        };
        let coordinate_values = coordinate_identities
            .iter()
            .copied()
            .map(ProductionRankedValueV1::Local)
            .collect::<Vec<_>>();
        let target = blocks
            .get(output.write.block)
            .ok_or(ProductionReferenceEffectJoinErrorV2::WriteLocation)?;
        let mut operations = target.operations().to_vec();
        let terminator = target.terminator().clone();
        let index_arguments = target.index_argument_count();
        let projected_write = operations
            .get(output.write.operation)
            .cloned()
            .ok_or(ProductionReferenceEffectJoinErrorV2::WriteLocation)?;
        match projected_write {
            ProductionRankedOperationV1::Access {
                kind,
                view,
                indices,
            } if kind.writes_memory()
                && view == output.write.view
                && indices == output.write.indices =>
            {
                operations[output.write.operation] = ProductionRankedOperationV1::ValueAccess {
                    kind,
                    view,
                    indices,
                    value: ProductionRankedValueV1::Local(*gpu_value_id),
                };
            }
            _ => {
                return Err(ProductionReferenceEffectJoinErrorV2::UnsupportedGpuEffect {
                    block: output.write.block,
                    operation: output.write.operation,
                    detail: "functional refinement requires the exact projected non-atomic write with matching view and indices",
                });
            }
        }
        let request_operation = operations.len();
        let contract = ProductionEffectRefinementContractV2::new(
            contract_identity(binding.effect_ir_sha256, &output.write),
            ProductionGpuWriteSiteV2::new(
                u32::try_from(output.write.block)
                    .map_err(|_| ProductionReferenceEffectJoinErrorV2::WriteLocation)?,
                u32::try_from(output.write.operation)
                    .map_err(|_| ProductionReferenceEffectJoinErrorV2::WriteLocation)?,
            ),
            ProductionReferenceOutputSiteV2::new(
                output.output_argument,
                output.reference_write.block,
                output.reference_write.statement,
            ),
            output.write.view,
            output.write.indices.clone(),
            coordinate_values.clone(),
            coordinate_values,
            ProductionRankedValueV1::Local(*true_value),
            ProductionRankedValueV1::Local(*true_value),
            ProductionRankedValueV1::Local(*true_value),
            ProductionRankedValueV1::Local(*true_value),
            ProductionRankedValueV1::Local(*gpu_value_id),
            ProductionRankedValueV1::Local(*reference_value_id),
        )
        .map_err(ProductionReferenceEffectJoinErrorV2::Recipe)?;
        operations
            .push(ProductionRankedOperationV1::RequestEffectRefinement { contract, subjects });
        blocks[output.write.block] =
            ProductionRankedBlockV1::with_index_arguments(index_arguments, operations, terminator);
        requests.push(CompilerOwnedReferenceEffectSiteV2 {
            block: output.write.block,
            operation: request_operation,
            subjects,
        });
    }
    let kernel =
        ProductionRankedKernelV1::new(kernel.function_name(), kernel.argument_count(), blocks)
            .map_err(ProductionReferenceEffectJoinErrorV2::Recipe)?;
    Ok(CompilerOwnedReferenceEffectRequestV2 {
        kernel,
        requests,
        proof_timeout_seconds,
    })
}

fn supported_ranked_scalar_v2(scalar: ReferenceScalarTypeV1) -> bool {
    reference_scalar_v2(scalar).is_some()
}

fn reference_scalar_v2(scalar: ReferenceScalarTypeV1) -> Option<ProductionSemanticScalarTypeV2> {
    Some(match scalar {
        ReferenceScalarTypeV1::Bool => ProductionSemanticScalarTypeV2::Bool,
        ReferenceScalarTypeV1::U8 => ProductionSemanticScalarTypeV2::Integer {
            signed: false,
            bits: 8,
        },
        ReferenceScalarTypeV1::U16 => ProductionSemanticScalarTypeV2::Integer {
            signed: false,
            bits: 16,
        },
        ReferenceScalarTypeV1::U32 => ProductionSemanticScalarTypeV2::Integer {
            signed: false,
            bits: 32,
        },
        ReferenceScalarTypeV1::U64 | ReferenceScalarTypeV1::Usize => {
            ProductionSemanticScalarTypeV2::Integer {
                signed: false,
                bits: 64,
            }
        }
        ReferenceScalarTypeV1::I8 => ProductionSemanticScalarTypeV2::Integer {
            signed: true,
            bits: 8,
        },
        ReferenceScalarTypeV1::I16 => ProductionSemanticScalarTypeV2::Integer {
            signed: true,
            bits: 16,
        },
        ReferenceScalarTypeV1::I32 => ProductionSemanticScalarTypeV2::Integer {
            signed: true,
            bits: 32,
        },
        ReferenceScalarTypeV1::I64 | ReferenceScalarTypeV1::Isize => {
            ProductionSemanticScalarTypeV2::Integer {
                signed: true,
                bits: 64,
            }
        }
        ReferenceScalarTypeV1::F32 => ProductionSemanticScalarTypeV2::Float { bits: 32 },
        ReferenceScalarTypeV1::F64 => ProductionSemanticScalarTypeV2::Float { bits: 64 },
    })
}

#[cfg(test)]
fn reference_expression_v2(
    effect_ir: &ReferenceEffectIrV1,
    expression: &ReferenceEffectExpressionV1,
    expected: ReferenceScalarTypeV1,
) -> Result<ProductionSemanticExpressionV2, ProductionReferenceEffectJoinErrorV2> {
    reference_expression_inner_checked_v2(effect_ir, expression, expected, None)
}

fn reference_expression_with_gpu_loads_v2(
    effect_ir: &ReferenceEffectIrV1,
    expression: &ReferenceEffectExpressionV1,
    expected: ReferenceScalarTypeV1,
    kernel: &ProductionRankedKernelV1,
    gpu_expression: &ProductionSemanticExpressionV2,
) -> Result<ProductionSemanticExpressionV2, ProductionReferenceEffectJoinErrorV2> {
    reference_expression_inner_checked_v2(
        effect_ir,
        expression,
        expected,
        Some((kernel, gpu_expression)),
    )
}

fn reference_expression_inner_checked_v2(
    effect_ir: &ReferenceEffectIrV1,
    expression: &ReferenceEffectExpressionV1,
    expected: ReferenceScalarTypeV1,
    gpu_loads: Option<(&ProductionRankedKernelV1, &ProductionSemanticExpressionV2)>,
) -> Result<ProductionSemanticExpressionV2, ProductionReferenceEffectJoinErrorV2> {
    let expression = reference_expression_inner_v2(effect_ir, expression, gpu_loads, 0)?;
    let expected = reference_scalar_v2(expected).ok_or(
        ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
            "reference output scalar is outside typed semantic refinement V2",
        ),
    )?;
    if expression.scalar() != expected {
        return Err(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
            "reference output RHS type disagrees with its logical ABI",
        ));
    }
    expression
        .validate()
        .map_err(ProductionReferenceEffectJoinErrorV2::SemanticExpression)?;
    Ok(expression)
}

fn reference_expression_inner_v2(
    effect_ir: &ReferenceEffectIrV1,
    expression: &ReferenceEffectExpressionV1,
    gpu_loads: Option<(&ProductionRankedKernelV1, &ProductionSemanticExpressionV2)>,
    depth: usize,
) -> Result<ProductionSemanticExpressionV2, ProductionReferenceEffectJoinErrorV2> {
    if depth >= fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 {
        return Err(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
            "reference RHS exceeds the typed semantic expression depth bound",
        ));
    }
    match expression {
        ReferenceEffectExpressionV1::PointCoordinate { axis } => {
            Ok(ProductionSemanticExpressionV2::Symbol {
                symbol: *axis,
                scalar: ProductionSemanticScalarTypeV2::Integer {
                    signed: false,
                    bits: 64,
                },
            })
        }
        ReferenceEffectExpressionV1::KernelScalarArgument { argument } => {
            let scalar = effect_ir
                .relations
                .iter()
                .find_map(|relation| match relation {
                    ReferenceArgumentRelationV1::ScalarInput {
                        argument: actual,
                        scalar,
                    } if actual == argument => reference_scalar_v2(*scalar),
                    _ => None,
                })
                .ok_or(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
                    "reference RHS scalar argument has no exact logical ABI type",
                ))?;
            let symbol = crate::reference_effect_v1::kernel_scalar_symbol_v2(*argument).ok_or(
                ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
                    "reference scalar argument exceeds the reserved semantic symbol namespace",
                ),
            )?;
            Ok(ProductionSemanticExpressionV2::Symbol { symbol, scalar })
        }
        ReferenceEffectExpressionV1::InputLength { .. } => {
            Err(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
                "reference slice length cannot be used as an opaque semantic value",
            ))
        }
        ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::Scalar { scalar, bits }) => {
            let scalar = reference_scalar_v2(*scalar).ok_or(
                ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
                    "reference RHS constant type is unsupported",
                ),
            )?;
            let bits = u64::try_from(*bits).map_err(|_| {
                ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
                    "reference RHS constant exceeds 64 bits",
                )
            })?;
            Ok(ProductionSemanticExpressionV2::Constant { scalar, bits })
        }
        ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::ZeroSized) => {
            Err(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
                "reference RHS is a zero-sized value rather than a scalar",
            ))
        }
        ReferenceEffectExpressionV1::InputLoad {
            reference_argument,
            index,
        } => {
            let (kernel, gpu_expression) =
                gpu_loads.ok_or(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
                    "safe reference load requires an independently projected GPU expression",
                ))?;
            let (argument, element) = effect_ir
                .relations
                .iter()
                .find_map(|relation| match relation {
                    ReferenceArgumentRelationV1::SharedSliceInput { argument, element }
                        if effect_ir
                            .reference_argument_for_kernel_argument_v1(*argument)
                            .is_ok_and(|exact| exact == *reference_argument) =>
                    {
                        Some((*argument, *element))
                    }
                    _ => None,
                })
                .ok_or(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
                    "safe reference load base is not one exact shared-slice input",
                ))?;
            let scalar = reference_scalar_v2(element).ok_or(
                ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
                    "safe reference load element type is unsupported",
                ),
            )?;
            let allocation_origin = u64::from(argument).checked_add(1).ok_or(
                ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
                    "safe reference load argument origin overflowed",
                ),
            )?;
            let mut loads = Vec::new();
            collect_semantic_loads_v2(gpu_expression, &mut loads);
            let mut matches = loads.into_iter().filter(|load| {
                load.scalar == scalar
                    && load.allocation_origin == allocation_origin
                    && load.indices.len() == 1
                    && gpu_index_expression_v2(kernel, load.indices[0], 0)
                        .is_ok_and(|gpu_index| gpu_index == **index)
            });
            let Some(load) = matches.next() else {
                return Err(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
                    "safe reference load has no exact ranked GPU read with matching input, type, and index",
                ));
            };
            if matches.next().is_some() {
                return Err(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
                    "safe reference load matches multiple ranked GPU reads",
                ));
            }
            Ok(ProductionSemanticExpressionV2::Load(load.clone()))
        }
        ReferenceEffectExpressionV1::Binary {
            operation,
            lhs,
            rhs,
            checked,
        } => {
            let lhs = reference_expression_inner_v2(effect_ir, lhs, gpu_loads, depth + 1)?;
            let rhs = reference_expression_inner_v2(effect_ir, rhs, gpu_loads, depth + 1)?;
            let comparison = match operation {
                ReferenceBinaryOpV1::Equal => Some(ProductionSemanticComparisonV2::Equal),
                ReferenceBinaryOpV1::LessThan => Some(ProductionSemanticComparisonV2::LessThan),
                ReferenceBinaryOpV1::LessEqual => Some(ProductionSemanticComparisonV2::LessOrEqual),
                ReferenceBinaryOpV1::NotEqual => Some(ProductionSemanticComparisonV2::NotEqual),
                ReferenceBinaryOpV1::GreaterEqual => {
                    Some(ProductionSemanticComparisonV2::GreaterOrEqual)
                }
                ReferenceBinaryOpV1::GreaterThan => {
                    Some(ProductionSemanticComparisonV2::GreaterThan)
                }
                _ => None,
            };
            if let Some(operation) = comparison {
                return Ok(ProductionSemanticExpressionV2::Compare {
                    operation,
                    operand_scalar: lhs.scalar(),
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                });
            }
            let operation = match operation {
                ReferenceBinaryOpV1::Add => ProductionSemanticBinaryOpV2::Add,
                ReferenceBinaryOpV1::Subtract => ProductionSemanticBinaryOpV2::Subtract,
                ReferenceBinaryOpV1::Multiply => ProductionSemanticBinaryOpV2::Multiply,
                ReferenceBinaryOpV1::Divide => ProductionSemanticBinaryOpV2::Divide,
                ReferenceBinaryOpV1::Remainder => ProductionSemanticBinaryOpV2::Remainder,
                ReferenceBinaryOpV1::BitXor => ProductionSemanticBinaryOpV2::BitXor,
                ReferenceBinaryOpV1::BitAnd => ProductionSemanticBinaryOpV2::BitAnd,
                ReferenceBinaryOpV1::BitOr => ProductionSemanticBinaryOpV2::BitOr,
                ReferenceBinaryOpV1::ShiftLeft => ProductionSemanticBinaryOpV2::ShiftLeft,
                ReferenceBinaryOpV1::ShiftRight => ProductionSemanticBinaryOpV2::ShiftRight,
                ReferenceBinaryOpV1::Equal
                | ReferenceBinaryOpV1::LessThan
                | ReferenceBinaryOpV1::LessEqual
                | ReferenceBinaryOpV1::NotEqual
                | ReferenceBinaryOpV1::GreaterEqual
                | ReferenceBinaryOpV1::GreaterThan => unreachable!(),
            };
            Ok(ProductionSemanticExpressionV2::Binary {
                operation,
                scalar: lhs.scalar(),
                overflow: if *checked {
                    ProductionOverflowContractV2::Checked
                } else {
                    ProductionOverflowContractV2::Wrapping
                },
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            })
        }
        ReferenceEffectExpressionV1::Unary { operation, operand } => {
            let operand = reference_expression_inner_v2(effect_ir, operand, gpu_loads, depth + 1)?;
            Ok(ProductionSemanticExpressionV2::Unary {
                operation: match operation {
                    ReferenceUnaryOpV1::Not => ProductionSemanticUnaryOpV2::Not,
                    ReferenceUnaryOpV1::Negate => ProductionSemanticUnaryOpV2::Negate,
                },
                scalar: operand.scalar(),
                operand: Box::new(operand),
            })
        }
        ReferenceEffectExpressionV1::Cast {
            kind,
            source,
            target,
            operand,
        } => {
            let source = reference_scalar_v2(*source).ok_or(
                ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
                    "reference cast source type is unsupported",
                ),
            )?;
            let target = reference_scalar_v2(*target).ok_or(
                ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
                    "reference cast target type is unsupported",
                ),
            )?;
            let kind = match kind {
                ReferenceCastKindV1::Integer => ProductionSemanticCastV2::Integer,
                ReferenceCastKindV1::IntegerToFloat => ProductionSemanticCastV2::IntegerToFloat,
                ReferenceCastKindV1::FloatToFloat => ProductionSemanticCastV2::FloatToFloat,
                ReferenceCastKindV1::FloatToIntegerSaturating => {
                    ProductionSemanticCastV2::FloatToIntegerSaturating
                }
            };
            Ok(ProductionSemanticExpressionV2::Cast {
                kind,
                source,
                target,
                operand: Box::new(reference_expression_inner_v2(
                    effect_ir,
                    operand,
                    gpu_loads,
                    depth + 1,
                )?),
            })
        }
    }
}

fn collect_semantic_loads_v2<'a>(
    expression: &'a ProductionSemanticExpressionV2,
    loads: &mut Vec<&'a fe2o3_pliron::ProductionSemanticLoadV2>,
) {
    match expression {
        ProductionSemanticExpressionV2::Load(load) => loads.push(load),
        ProductionSemanticExpressionV2::Symbol { .. }
        | ProductionSemanticExpressionV2::Constant { .. } => {}
        ProductionSemanticExpressionV2::Unary { operand, .. }
        | ProductionSemanticExpressionV2::Cast { operand, .. } => {
            collect_semantic_loads_v2(operand, loads);
        }
        ProductionSemanticExpressionV2::Binary { lhs, rhs, .. }
        | ProductionSemanticExpressionV2::Compare { lhs, rhs, .. } => {
            collect_semantic_loads_v2(lhs, loads);
            collect_semantic_loads_v2(rhs, loads);
        }
        ProductionSemanticExpressionV2::Select {
            condition,
            when_true,
            when_false,
            ..
        } => {
            collect_semantic_loads_v2(condition, loads);
            collect_semantic_loads_v2(when_true, loads);
            collect_semantic_loads_v2(when_false, loads);
        }
    }
}

fn compiler_extracted_gpu_effect_v1(
    kernel: &ProductionRankedKernelV1,
    binding: &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingV1,
    write: &RankedGpuWriteV2,
) -> Result<CompilerExtractedGpuOutputEffectV1, ProductionReferenceEffectJoinErrorV2> {
    let output_argument = write
        .allocation_origin
        .checked_sub(1)
        .and_then(|argument| u32::try_from(argument).ok())
        .ok_or(ProductionReferenceEffectJoinErrorV2::UnmodeledGlobalWrite {
            allocation_origin: write.allocation_origin,
        })?;
    let scalar = binding
        .effect_ir
        .relations
        .iter()
        .find_map(|relation| match relation {
            ReferenceArgumentRelationV1::DisjointOutputCoordinate { argument, element }
                if *argument == output_argument =>
            {
                Some(*element)
            }
            _ => None,
        })
        .ok_or(ProductionReferenceEffectJoinErrorV2::UnmodeledGlobalWrite {
            allocation_origin: write.allocation_origin,
        })?;
    if !supported_ranked_scalar_v2(scalar) {
        return Err(ProductionReferenceEffectJoinErrorV2::UnsupportedGpuEffect {
            block: write.block,
            operation: write.operation,
            detail: "GPU output scalar is outside the bounded reference-join subset",
        });
    }
    validate_bounds_only_gpu_guard_v2(kernel, write)?;
    let coordinate = ReferenceOutputCoordinateV1::LogicalPoint(
        write
            .indices
            .iter()
            .copied()
            .map(|value| gpu_index_expression_v2(kernel, value, 0))
            .collect::<Result<Vec<_>, _>>()?
            .into_boxed_slice(),
    );
    Ok(CompilerExtractedGpuOutputEffectV1 {
        output_argument,
        block: u32::try_from(write.block).map_err(|_| {
            ProductionReferenceEffectJoinErrorV2::UnsupportedGpuEffect {
                block: write.block,
                operation: write.operation,
                detail: "GPU write block does not fit the canonical effect identity",
            }
        })?,
        operation: u32::try_from(write.operation).map_err(|_| {
            ProductionReferenceEffectJoinErrorV2::UnsupportedGpuEffect {
                block: write.block,
                operation: write.operation,
                detail: "GPU write operation does not fit the canonical effect identity",
            }
        })?,
        coordinate,
        guard: ReferencePathPredicateV1::unconditional_v1(),
    })
}

fn gpu_index_expression_v2(
    kernel: &ProductionRankedKernelV1,
    value: ProductionRankedValueV1,
    depth: usize,
) -> Result<ReferenceEffectExpressionV1, ProductionReferenceEffectJoinErrorV2> {
    if depth >= 64 {
        return Err(ProductionReferenceEffectJoinErrorV2::UnsupportedGpuIndex(
            "GPU index expression exceeds the bounded normalization depth",
        ));
    }
    match value {
        ProductionRankedValueV1::Argument(argument) => {
            Ok(ReferenceEffectExpressionV1::KernelScalarArgument { argument })
        }
        ProductionRankedValueV1::BlockArgument { .. } => {
            Err(ProductionReferenceEffectJoinErrorV2::UnsupportedGpuIndex(
                "GPU output coordinate depends on a block argument",
            ))
        }
        ProductionRankedValueV1::Local(identity) => {
            let mut definitions = kernel
                .blocks()
                .iter()
                .flat_map(|block| block.operations())
                .filter(|operation| operation_result_v2(operation) == Some(identity));
            let definition = definitions.next().ok_or(
                ProductionReferenceEffectJoinErrorV2::UnsupportedGpuIndex(
                    "GPU output coordinate has no ranked definition",
                ),
            )?;
            if definitions.next().is_some() {
                return Err(ProductionReferenceEffectJoinErrorV2::UnsupportedGpuIndex(
                    "GPU output coordinate has multiple ranked definitions",
                ));
            }
            match definition {
                ProductionRankedOperationV1::InvocationIndex { dimension, .. } => {
                    Ok(ReferenceEffectExpressionV1::PointCoordinate { axis: *dimension })
                }
                ProductionRankedOperationV1::IndexConstant { value, .. } => Ok(
                    ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::Scalar {
                        scalar: ReferenceScalarTypeV1::Usize,
                        bits: u128::from(*value),
                    }),
                ),
                ProductionRankedOperationV1::IndexBinary { kind, lhs, rhs, .. } => {
                    let operation = match kind {
                        IndexBinaryKindAttr::Add => ReferenceBinaryOpV1::Add,
                        IndexBinaryKindAttr::Multiply => ReferenceBinaryOpV1::Multiply,
                        IndexBinaryKindAttr::Remainder => ReferenceBinaryOpV1::Remainder,
                        IndexBinaryKindAttr::Divide => ReferenceBinaryOpV1::Divide,
                    };
                    Ok(ReferenceEffectExpressionV1::Binary {
                        operation,
                        lhs: Box::new(gpu_index_expression_v2(kernel, *lhs, depth + 1)?),
                        rhs: Box::new(gpu_index_expression_v2(kernel, *rhs, depth + 1)?),
                        checked: false,
                    })
                }
                _ => Err(ProductionReferenceEffectJoinErrorV2::UnsupportedGpuIndex(
                    "GPU output coordinate uses an unsupported ranked definition",
                )),
            }
        }
    }
}

fn operation_result_v2(
    operation: &ProductionRankedOperationV1,
) -> Option<ProductionRankedValueIdV1> {
    match operation {
        ProductionRankedOperationV1::IndexConstant { result, .. }
        | ProductionRankedOperationV1::IndexUnknown { result }
        | ProductionRankedOperationV1::InvocationIndex { result, .. }
        | ProductionRankedOperationV1::IndexBinary { result, .. }
        | ProductionRankedOperationV1::DeterministicJoin { result, .. }
        | ProductionRankedOperationV1::CheckedTiledIndex2D { result, .. }
        | ProductionRankedOperationV1::CheckedRowStripedIndex2D { result, .. }
        | ProductionRankedOperationV1::Dimension { result, .. }
        | ProductionRankedOperationV1::SemanticConstant { result, .. }
        | ProductionRankedOperationV1::SemanticSymbol { result, .. }
        | ProductionRankedOperationV1::SemanticExpression { result, .. } => Some(*result),
        _ => None,
    }
}

fn reference_logical_point_rank_v2(
    coordinate: &ReferenceOutputCoordinateV1,
) -> Result<usize, ProductionReferenceEffectJoinErrorV2> {
    let ReferenceOutputCoordinateV1::LogicalPoint(axes) = coordinate else {
        return Err(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
            "reference output coordinate is not a logical point",
        ));
    };
    if axes
        .iter()
        .any(|axis| !matches!(axis, ReferenceEffectExpressionV1::PointCoordinate { .. }))
    {
        return Err(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
            "reference logical point is not a direct coordinate argument",
        ));
    }
    Ok(axes.len())
}

fn validate_bounds_only_gpu_guard_v2(
    kernel: &ProductionRankedKernelV1,
    write: &RankedGpuWriteV2,
) -> Result<(), ProductionReferenceEffectJoinErrorV2> {
    let blocks = kernel.blocks();
    if write.block >= blocks.len() || !can_reach_block_v2(blocks, 0, write.block) {
        return Err(ProductionReferenceEffectJoinErrorV2::WriteLocation);
    }
    if terminator_successors_v2(blocks[write.block].terminator())
        .into_iter()
        .any(|successor| can_reach_block_v2(blocks, successor, write.block))
    {
        return Err(ProductionReferenceEffectJoinErrorV2::UnsupportedGpuEffect {
            block: write.block,
            operation: write.operation,
            detail: "GPU output write lies on a CFG cycle",
        });
    }
    for (block_index, block) in blocks.iter().enumerate() {
        if block_index == write.block
            || !can_reach_block_v2(blocks, 0, block_index)
            || !can_reach_block_v2(blocks, block_index, write.block)
        {
            continue;
        }
        let controlled_successors = match block.terminator() {
            ProductionRankedTerminatorV1::IndexLessThan {
                lhs,
                rhs,
                true_block,
                false_block,
            }
            | ProductionRankedTerminatorV1::IndexLessThanArgs {
                lhs,
                rhs,
                true_block,
                false_block,
                ..
            } => Some((*lhs, *rhs, *true_block as usize, *false_block as usize)),
            ProductionRankedTerminatorV1::IndexEqual {
                true_block,
                false_block,
                ..
            }
            | ProductionRankedTerminatorV1::IndexEqualArgs {
                true_block,
                false_block,
                ..
            }
            | ProductionRankedTerminatorV1::AnalysisSplit {
                first_block: true_block,
                second_block: false_block,
                ..
            }
            | ProductionRankedTerminatorV1::AnalysisSplitArgs {
                first_block: true_block,
                second_block: false_block,
                ..
            } => {
                let true_reaches = can_reach_block_v2(blocks, *true_block as usize, write.block);
                let false_reaches = can_reach_block_v2(blocks, *false_block as usize, write.block);
                if true_reaches != false_reaches {
                    return Err(ProductionReferenceEffectJoinErrorV2::UnsupportedGpuEffect {
                        block: write.block,
                        operation: write.operation,
                        detail: "GPU write has a logical path guard outside the exact memory-bounds selection",
                    });
                }
                None
            }
            _ => None,
        };
        let Some((lhs, rhs, true_block, false_block)) = controlled_successors else {
            continue;
        };
        let true_reaches = can_reach_block_v2(blocks, true_block, write.block);
        let false_reaches = can_reach_block_v2(blocks, false_block, write.block);
        if true_reaches == false_reaches {
            continue;
        }
        if !true_reaches || !exact_effect_bounds_pair_v2(kernel, write, lhs, rhs) {
            return Err(ProductionReferenceEffectJoinErrorV2::UnsupportedGpuEffect {
                block: write.block,
                operation: write.operation,
                detail: "GPU write has a logical path guard outside the exact memory-bounds selection",
            });
        }
    }
    Ok(())
}

fn exact_effect_bounds_pair_v2(
    kernel: &ProductionRankedKernelV1,
    write: &RankedGpuWriteV2,
    lhs: ProductionRankedValueV1,
    rhs: ProductionRankedValueV1,
) -> bool {
    if exact_view_bounds_pair_v2(kernel, write.view, &write.indices, lhs, rhs) {
        return true;
    }
    let Ok(expression) = &write.value else {
        return false;
    };
    let mut loads = Vec::new();
    collect_semantic_loads_v2(expression, &mut loads);
    loads
        .into_iter()
        .any(|load| exact_view_bounds_pair_v2(kernel, load.view, &load.indices, lhs, rhs))
}

fn exact_view_bounds_pair_v2(
    kernel: &ProductionRankedKernelV1,
    view_value: ProductionRankedValueV1,
    indices: &[ProductionRankedValueV1],
    lhs: ProductionRankedValueV1,
    rhs: ProductionRankedValueV1,
) -> bool {
    let view = kernel
        .blocks()
        .iter()
        .flat_map(|block| block.operations())
        .find_map(|operation| match operation {
            ProductionRankedOperationV1::View {
                result,
                shape,
                dynamic_extents,
                ..
            }
            | ProductionRankedOperationV1::ViewInSpace {
                result,
                shape,
                dynamic_extents,
                ..
            } if view_value == ProductionRankedValueV1::Local(*result) => {
                Some((shape.as_slice(), dynamic_extents.as_slice()))
            }
            _ => None,
        });
    let Some((shape, dynamic_extents)) = view else {
        return false;
    };
    indices.iter().enumerate().any(|(axis, index)| {
        if !same_exact_ranked_index_v2(kernel, *index, lhs) || axis >= shape.len() {
            return false;
        }
        if shape[axis] != DYNAMIC_EXTENT {
            return ranked_constant_v2(kernel, rhs) == Some(shape[axis]);
        }
        let dynamic_index = shape[..axis]
            .iter()
            .filter(|extent| **extent == DYNAMIC_EXTENT)
            .count();
        dynamic_extents.get(dynamic_index).copied() == Some(rhs)
    })
}

fn same_exact_ranked_index_v2(
    kernel: &ProductionRankedKernelV1,
    lhs: ProductionRankedValueV1,
    rhs: ProductionRankedValueV1,
) -> bool {
    if lhs == rhs {
        return true;
    }
    let invocation_dimension = |value| {
        let ProductionRankedValueV1::Local(identity) = value else {
            return None;
        };
        let mut definitions = kernel
            .blocks()
            .iter()
            .flat_map(|block| block.operations())
            .filter(|operation| operation_result_v2(operation) == Some(identity));
        let definition = definitions.next()?;
        if definitions.next().is_some() {
            return None;
        }
        match definition {
            ProductionRankedOperationV1::InvocationIndex { dimension, .. } => Some(*dimension),
            _ => None,
        }
    };
    invocation_dimension(lhs)
        .zip(invocation_dimension(rhs))
        .is_some_and(|(lhs, rhs)| lhs == rhs)
}

fn ranked_constant_v2(
    kernel: &ProductionRankedKernelV1,
    value: ProductionRankedValueV1,
) -> Option<u64> {
    let ProductionRankedValueV1::Local(identity) = value else {
        return None;
    };
    kernel
        .blocks()
        .iter()
        .flat_map(|block| block.operations())
        .find_map(|operation| match operation {
            ProductionRankedOperationV1::IndexConstant { result, value } if *result == identity => {
                Some(*value)
            }
            _ => None,
        })
}

fn can_reach_block_v2(blocks: &[ProductionRankedBlockV1], start: usize, target: usize) -> bool {
    if start >= blocks.len() || target >= blocks.len() {
        return false;
    }
    let mut visited = vec![false; blocks.len()];
    let mut pending = vec![start];
    while let Some(block) = pending.pop() {
        if block == target {
            return true;
        }
        if visited[block] {
            continue;
        }
        visited[block] = true;
        pending.extend(
            terminator_successors_v2(blocks[block].terminator())
                .into_iter()
                .filter(|successor| *successor < blocks.len()),
        );
    }
    false
}

fn terminator_successors_v2(terminator: &ProductionRankedTerminatorV1) -> Vec<usize> {
    match terminator {
        ProductionRankedTerminatorV1::IndexLessThan {
            true_block,
            false_block,
            ..
        }
        | ProductionRankedTerminatorV1::IndexLessThanArgs {
            true_block,
            false_block,
            ..
        }
        | ProductionRankedTerminatorV1::IndexEqual {
            true_block,
            false_block,
            ..
        }
        | ProductionRankedTerminatorV1::IndexEqualArgs {
            true_block,
            false_block,
            ..
        } => vec![*true_block as usize, *false_block as usize],
        ProductionRankedTerminatorV1::AnalysisSplit {
            first_block,
            second_block,
            ..
        }
        | ProductionRankedTerminatorV1::AnalysisSplitArgs {
            first_block,
            second_block,
            ..
        } => vec![*first_block as usize, *second_block as usize],
        ProductionRankedTerminatorV1::Branch { target }
        | ProductionRankedTerminatorV1::BranchArgs { target, .. }
        | ProductionRankedTerminatorV1::BranchArgsAdd { target, .. }
        | ProductionRankedTerminatorV1::BranchArgsAddAt { target, .. } => {
            vec![*target as usize]
        }
        ProductionRankedTerminatorV1::Return | ProductionRankedTerminatorV1::Trap => Vec::new(),
    }
}

fn replace_reserved_semantic_expression_v2(
    operations: &mut [ProductionRankedOperationV1],
    identity: ProductionRankedValueIdV1,
    expression: ProductionSemanticExpressionV2,
    numerical_contract: ProductionNumericalContractV2,
) -> Result<(), ProductionReferenceEffectJoinErrorV2> {
    let mut found = false;
    for operation in operations {
        if operation_result_v2(operation) != Some(identity) {
            continue;
        }
        if !matches!(
            operation,
            ProductionRankedOperationV1::SemanticConstant { .. }
        ) || found
        {
            return Err(ProductionReferenceEffectJoinErrorV2::InvalidReservedValue(
                identity.get(),
            ));
        }
        *operation = ProductionRankedOperationV1::SemanticExpression {
            result: identity,
            expression: expression.clone(),
            numerical_contract,
        };
        found = true;
    }
    if !found {
        return Err(ProductionReferenceEffectJoinErrorV2::InvalidReservedValue(
            identity.get(),
        ));
    }
    Ok(())
}

fn replace_reserved_semantic_symbol_v2(
    operations: &mut [ProductionRankedOperationV1],
    identity: ProductionRankedValueIdV1,
    expected_symbol: u32,
) -> Result<(), ProductionReferenceEffectJoinErrorV2> {
    let mut found = false;
    for operation in operations {
        if operation_result_v2(operation) != Some(identity) {
            continue;
        }
        if !matches!(
            operation,
            ProductionRankedOperationV1::SemanticSymbol { symbol, .. }
                if *symbol == expected_symbol
        ) || found
        {
            return Err(ProductionReferenceEffectJoinErrorV2::InvalidReservedValue(
                identity.get(),
            ));
        }
        *operation = ProductionRankedOperationV1::SemanticExpression {
            result: identity,
            expression: ProductionSemanticExpressionV2::Symbol {
                symbol: expected_symbol,
                scalar: ProductionSemanticScalarTypeV2::Integer {
                    signed: false,
                    bits: 64,
                },
            },
            numerical_contract: ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
        };
        found = true;
    }
    if !found {
        return Err(ProductionReferenceEffectJoinErrorV2::InvalidReservedValue(
            identity.get(),
        ));
    }
    Ok(())
}

fn contract_identity(effect_ir: [u8; 32], write: &RankedGpuWriteV2) -> u64 {
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(&effect_ir[..8]);
    let identity = u64::from_le_bytes(bytes)
        ^ (write.block as u64).rotate_left(17)
        ^ (write.operation as u64).rotate_left(41);
    identity.max(1)
}

#[derive(Debug)]
pub(crate) enum ProductionReferenceEffectJoinErrorV2 {
    BindingCount(usize),
    ReferenceWriteCount(usize),
    UnsupportedReference(&'static str),
    EffectBijection(ReferenceEffectBijectionErrorV1),
    UnmodeledGlobalWrite {
        allocation_origin: u64,
    },
    UnsupportedGpuEffect {
        block: usize,
        operation: usize,
        detail: &'static str,
    },
    UnsupportedGpuIndex(&'static str),
    UnmodeledGpuValue {
        block: usize,
        operation: usize,
        detail: &'static str,
    },
    ReferenceBoundsCheck {
        block: u32,
        detail: String,
    },
    AmbiguousOwnership,
    WriteLocation,
    InvalidReservedValueCount {
        expected: usize,
        actual: usize,
    },
    ReservedValueCountOverflow,
    ProofOutputLimit {
        actual: usize,
        limit: usize,
    },
    InvalidReservedValue(u32),
    SemanticExpression(fe2o3_pliron::ProductionSemanticExpressionErrorV2),
    Subjects(String),
    Recipe(ProductionRankedKernelErrorV1),
    Construction(String),
    ProofRuntimeUnavailable {
        root: &'static str,
        detail: String,
    },
    ProofExecution(String),
    Compile(Box<ProductionRankedCompileErrorV2>),
}

impl fmt::Display for ProductionReferenceEffectJoinErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BindingCount(actual) => write!(
                formatter,
                "source-to-proof V2 requires exactly one authenticated reference binding; found {actual}"
            ),
            Self::ReferenceWriteCount(actual) => write!(
                formatter,
                "source-to-proof V2 found {actual} matched observable reference output writes; at least one and exactly one match per retained output are required"
            ),
            Self::UnsupportedReference(detail) => {
                write!(formatter, "source-to-proof V2 reference is unsupported: {detail}")
            }
            Self::EffectBijection(error) => {
                write!(formatter, "source-to-proof V2 effect mismatch: {error}")
            }
            Self::UnmodeledGlobalWrite { allocation_origin } => write!(
                formatter,
                "source-to-proof V2 found a global GPU write at allocation origin {allocation_origin} with no logical output ABI relation"
            ),
            Self::UnsupportedGpuEffect {
                block,
                operation,
                detail,
            } => write!(
                formatter,
                "source-to-proof V2 cannot normalize the GPU effect at ranked block {block} op {operation}: {detail}"
            ),
            Self::UnsupportedGpuIndex(detail) => {
                write!(formatter, "source-to-proof V2 cannot normalize the GPU coordinate: {detail}")
            }
            Self::UnmodeledGpuValue {
                block,
                operation,
                detail,
            } => write!(
                formatter,
                "source-to-proof V2 cannot normalize the GPU store value at ranked block {block} op {operation}: {detail}"
            ),
            Self::ReferenceBoundsCheck { block, detail } => write!(
                formatter,
                "source-to-proof V2 cannot establish safe Rust slice bounds authority in reference block {block}: {detail}"
            ),
            Self::AmbiguousOwnership => formatter.write_str(
                "source-to-proof V2 output view already has an ownership contract; one compiler-owned contract is required",
            ),
            Self::WriteLocation => {
                formatter.write_str("source-to-proof V2 GPU write location is outside the ranked CFG")
            }
            Self::InvalidReservedValueCount { expected, actual } => write!(
                formatter,
                "source-to-proof V2 reserved {actual} semantic values but the exact effect requires {expected}"
            ),
            Self::ReservedValueCountOverflow => formatter.write_str(
                "source-to-proof V2 logical point rank overflows the reserved semantic value domain",
            ),
            Self::ProofOutputLimit { actual, limit } => write!(
                formatter,
                "source-to-proof V2 has {actual} output proofs; the production limit is {limit} under the fixed whole-compilation proof-time budget",
            ),
            Self::InvalidReservedValue(identity) => write!(
                formatter,
                "source-to-proof V2 reserved semantic value %{identity} is missing, duplicated, or has the wrong ranked type"
            ),
            Self::SemanticExpression(error) => {
                write!(formatter, "source-to-proof V2 semantic expression is invalid: {error}")
            }
            Self::Subjects(detail) => {
                write!(formatter, "source-to-proof V2 subject identity is invalid: {detail}")
            }
            Self::Recipe(error) => write!(formatter, "source-to-proof V2 recipe is invalid: {error}"),
            Self::Construction(detail) => {
                write!(formatter, "source-to-proof V2 construction failed: {detail}")
            }
            Self::ProofRuntimeUnavailable { root, detail } => write!(
                formatter,
                "functional-refinement proof runtime unavailable at {root}: {detail}; compilation stopped before proof admission or artifact emission"
            ),
            Self::ProofExecution(detail) => write!(
                formatter,
                "functional-refinement proof execution failed: {detail}; compilation stopped before artifact emission"
            ),
            Self::Compile(error) => write!(
                formatter,
                "source-to-proof V2 ranked admission failed: {error}"
            ),
        }
    }
}

impl std::error::Error for ProductionReferenceEffectJoinErrorV2 {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference_effect_v1::{
        AuthenticatedReferenceEffectBindingV1, ReferenceAssignmentV1, ReferenceBlockV1,
        ReferenceBoundsCheckV1, ReferenceFunctionIdentityV1, ReferenceOperandV1, ReferencePlaceV1,
        ReferenceTerminatorV1, ReferenceValueV1,
    };
    use dialect_kernel::{AccessKindAttr, MemorySpaceAttr};

    #[test]
    fn output_proofs_share_one_fixed_compilation_timeout_budget() {
        assert_eq!(per_output_proof_timeout_v2(1).unwrap(), 60);
        assert_eq!(per_output_proof_timeout_v2(2).unwrap(), 60);
        assert_eq!(per_output_proof_timeout_v2(3).unwrap(), 40);
        assert_eq!(per_output_proof_timeout_v2(64).unwrap(), 1);
        assert!(matches!(
            per_output_proof_timeout_v2(0),
            Err(ProductionReferenceEffectJoinErrorV2::ProofOutputLimit { .. })
        ));
        assert!(matches!(
            per_output_proof_timeout_v2(65),
            Err(ProductionReferenceEffectJoinErrorV2::ProofOutputLimit { .. })
        ));
    }

    #[test]
    fn prepare_rejects_output_limit_before_gpu_effect_extraction() {
        let (effect_ir, outputs) = bounds_discharge_fixture();
        let output_count = fe2o3_verifier::MAX_PRODUCTION_AGGREGATE_EFFECT_FORMULA_OUTPUTS_V1 + 1;
        let identity = ReferenceFunctionIdentityV1 {
            def_path_hash: [1; 16],
            function_sha256: [2; 32],
            item_definition_sha256: [3; 32],
            monomorphization_sha256: [4; 32],
            generic_type_arguments_sha256: [5; 32],
            const_generic_arguments_sha256: [6; 32],
            rustc_mir_body_sha256: [7; 32],
        };
        let bindings = AuthenticatedReferenceEffectBindingsV1::new(vec![
            AuthenticatedReferenceEffectBindingV1 {
                registration_path: "test".to_owned(),
                logical_kernel_name: "test".to_owned(),
                kernel: identity,
                reference: identity,
                effect_ir_sha256: [8; 32],
                effect_ir,
                observable_output_writes: vec![outputs[0].clone(); output_count].into_boxed_slice(),
            },
        ]);
        let (kernel, _) = dynamic_point_kernel(false);
        let error = match prepare_reference_effect_request_v2(kernel, &bindings, &[], Vec::new()) {
            Ok(_) => panic!("output limit unexpectedly reached GPU effect extraction"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            ProductionReferenceEffectJoinErrorV2::ProofOutputLimit {
                actual,
                limit
            } if actual == output_count
                && limit == fe2o3_verifier::MAX_PRODUCTION_AGGREGATE_EFFECT_FORMULA_OUTPUTS_V1
        ));
    }

    fn dynamic_point_kernel(logical_guard: bool) -> (ProductionRankedKernelV1, RankedGpuWriteV2) {
        let invocation = ProductionRankedValueIdV1::new(0);
        let view = ProductionRankedValueIdV1::new(1);
        let zero = ProductionRankedValueIdV1::new(2);
        let mut entry_operations = vec![
            ProductionRankedOperationV1::InvocationIndex {
                result: invocation,
                dimension: 0,
                launch_extent: 64,
            },
            ProductionRankedOperationV1::ViewInSpace {
                result: view,
                element_width: 32,
                writable: true,
                shape: vec![DYNAMIC_EXTENT],
                dynamic_extents: vec![ProductionRankedValueV1::Argument(0)],
                memory_space: MemorySpaceAttr::Global,
                allocation_origin: 1,
                noalias_class: 1,
            },
        ];
        if logical_guard {
            entry_operations.push(ProductionRankedOperationV1::IndexConstant {
                result: zero,
                value: 0,
            });
        }
        let entry_terminator = if logical_guard {
            ProductionRankedTerminatorV1::IndexEqual {
                lhs: ProductionRankedValueV1::Local(invocation),
                rhs: ProductionRankedValueV1::Local(zero),
                true_block: 1,
                false_block: 4,
            }
        } else {
            ProductionRankedTerminatorV1::Branch { target: 1 }
        };
        let blocks = vec![
            ProductionRankedBlockV1::new(entry_operations, entry_terminator),
            ProductionRankedBlockV1::new(
                Vec::new(),
                ProductionRankedTerminatorV1::IndexLessThan {
                    lhs: ProductionRankedValueV1::Local(invocation),
                    rhs: ProductionRankedValueV1::Argument(0),
                    true_block: 2,
                    false_block: 3,
                },
            ),
            ProductionRankedBlockV1::new(
                vec![ProductionRankedOperationV1::Access {
                    kind: AccessKindAttr::Write,
                    view: ProductionRankedValueV1::Local(view),
                    indices: vec![ProductionRankedValueV1::Local(invocation)],
                }],
                ProductionRankedTerminatorV1::Return,
            ),
            ProductionRankedBlockV1::new(Vec::new(), ProductionRankedTerminatorV1::Trap),
            ProductionRankedBlockV1::new(Vec::new(), ProductionRankedTerminatorV1::Return),
        ];
        let kernel = ProductionRankedKernelV1::new("dynamic_point", 1, blocks).unwrap();
        let write = RankedGpuWriteV2 {
            block: 2,
            operation: 0,
            allocation_origin: 1,
            view: ProductionRankedValueV1::Local(view),
            indices: vec![ProductionRankedValueV1::Local(invocation)],
            value: Ok(ProductionSemanticExpressionV2::Constant {
                scalar: ProductionSemanticScalarTypeV2::Integer {
                    signed: false,
                    bits: 32,
                },
                bits: 17,
            }),
        };
        (kernel, write)
    }

    #[test]
    fn dynamic_point_coordinate_excludes_only_the_exact_bounds_selection() {
        let (kernel, write) = dynamic_point_kernel(false);
        validate_bounds_only_gpu_guard_v2(&kernel, &write).unwrap();
        assert_eq!(
            gpu_index_expression_v2(&kernel, write.indices[0], 0).unwrap(),
            ReferenceEffectExpressionV1::PointCoordinate { axis: 0 },
        );
        assert_eq!(
            reference_logical_point_rank_v2(&ReferenceOutputCoordinateV1::LogicalPoint(
                vec![ReferenceEffectExpressionV1::PointCoordinate { axis: 0 }].into_boxed_slice(),
            ))
            .unwrap(),
            write.indices.len(),
        );
    }

    #[test]
    fn repeated_builtin_coordinate_roots_match_only_on_the_same_axis() {
        let (kernel, write) = dynamic_point_kernel(false);
        let repeated = ProductionRankedValueIdV1::new(2);
        let conflicting = ProductionRankedValueIdV1::new(3);
        let mut entry_operations = kernel.blocks()[0].operations().to_vec();
        entry_operations.extend([
            ProductionRankedOperationV1::InvocationIndex {
                result: repeated,
                dimension: 0,
                launch_extent: 64,
            },
            ProductionRankedOperationV1::InvocationIndex {
                result: conflicting,
                dimension: 1,
                launch_extent: 1,
            },
        ]);
        let mut blocks = kernel.blocks().to_vec();
        blocks[0] =
            ProductionRankedBlockV1::new(entry_operations, kernel.blocks()[0].terminator().clone());
        let kernel = ProductionRankedKernelV1::new("repeated_dynamic_point", 1, blocks).unwrap();
        assert!(same_exact_ranked_index_v2(
            &kernel,
            write.indices[0],
            ProductionRankedValueV1::Local(repeated),
        ));
        assert!(!same_exact_ranked_index_v2(
            &kernel,
            write.indices[0],
            ProductionRankedValueV1::Local(conflicting),
        ));
    }

    #[test]
    fn non_bounds_logical_guard_is_not_erased() {
        let (kernel, write) = dynamic_point_kernel(true);
        assert!(matches!(
            validate_bounds_only_gpu_guard_v2(&kernel, &write),
            Err(ProductionReferenceEffectJoinErrorV2::UnsupportedGpuEffect { .. })
        ));
    }

    fn scalar_reference_ir(scalar: ReferenceScalarTypeV1) -> ReferenceEffectIrV1 {
        ReferenceEffectIrV1 {
            argument_count: 1,
            local_count: 0,
            relations: vec![ReferenceArgumentRelationV1::ScalarInput {
                argument: 0,
                scalar,
            }]
            .into_boxed_slice(),
            blocks: Box::default(),
            loop_summaries: Box::default(),
            observable_output_effects: Box::default(),
        }
    }

    #[test]
    fn independently_translates_reference_integer_expression() {
        let effect_ir = scalar_reference_ir(ReferenceScalarTypeV1::U32);
        let expression = ReferenceEffectExpressionV1::Binary {
            operation: ReferenceBinaryOpV1::Add,
            lhs: Box::new(ReferenceEffectExpressionV1::KernelScalarArgument { argument: 0 }),
            rhs: Box::new(ReferenceEffectExpressionV1::Constant(
                ReferenceConstantV1::Scalar {
                    scalar: ReferenceScalarTypeV1::U32,
                    bits: 7,
                },
            )),
            checked: false,
        };
        let translated =
            reference_expression_v2(&effect_ir, &expression, ReferenceScalarTypeV1::U32).unwrap();
        assert!(matches!(
            translated,
            ProductionSemanticExpressionV2::Binary {
                operation: ProductionSemanticBinaryOpV2::Add,
                overflow: ProductionOverflowContractV2::Wrapping,
                ..
            }
        ));
    }

    #[test]
    fn reference_float_cast_retains_saturating_policy_and_ieee_contract() {
        let effect_ir = scalar_reference_ir(ReferenceScalarTypeV1::F32);
        let expression = ReferenceEffectExpressionV1::Cast {
            kind: ReferenceCastKindV1::FloatToIntegerSaturating,
            source: ReferenceScalarTypeV1::F32,
            target: ReferenceScalarTypeV1::I32,
            operand: Box::new(ReferenceEffectExpressionV1::KernelScalarArgument { argument: 0 }),
        };
        let translated =
            reference_expression_v2(&effect_ir, &expression, ReferenceScalarTypeV1::I32).unwrap();
        assert!(matches!(
            translated,
            ProductionSemanticExpressionV2::Cast {
                kind: ProductionSemanticCastV2::FloatToIntegerSaturating,
                ..
            }
        ));
        assert!(matches!(
            ProductionNumericalContractV2::exact_for_expression(&translated),
            ProductionNumericalContractV2::ExactIeee754OperatorCongruence { .. }
        ));
    }

    #[test]
    fn reserved_effect_coordinates_and_preconditions_become_typed_roots() {
        let constant = ProductionRankedValueIdV1::new(0);
        let coordinate = ProductionRankedValueIdV1::new(1);
        let mut operations = vec![
            ProductionRankedOperationV1::SemanticConstant {
                result: constant,
                value: 0,
            },
            ProductionRankedOperationV1::SemanticSymbol {
                result: coordinate,
                symbol: 3,
            },
        ];
        replace_reserved_semantic_expression_v2(
            &mut operations,
            constant,
            ProductionSemanticExpressionV2::Constant {
                scalar: ProductionSemanticScalarTypeV2::Bool,
                bits: 1,
            },
            ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
        )
        .unwrap();
        replace_reserved_semantic_symbol_v2(&mut operations, coordinate, 3).unwrap();
        assert!(matches!(
            &operations[0],
            ProductionRankedOperationV1::SemanticExpression {
                expression: ProductionSemanticExpressionV2::Constant {
                    scalar: ProductionSemanticScalarTypeV2::Bool,
                    bits: 1,
                },
                ..
            }
        ));
        assert!(matches!(
            &operations[1],
            ProductionRankedOperationV1::SemanticExpression {
                expression: ProductionSemanticExpressionV2::Symbol {
                    symbol: 3,
                    scalar: ProductionSemanticScalarTypeV2::Integer {
                        signed: false,
                        bits: 64,
                    },
                },
                ..
            }
        ));
    }

    fn usize_constant(bits: u128) -> ReferenceConstantV1 {
        ReferenceConstantV1::Scalar {
            scalar: ReferenceScalarTypeV1::Usize,
            bits,
        }
    }

    fn bounds_discharge_fixture() -> (ReferenceEffectIrV1, Vec<ReferenceOutputWriteV1>) {
        let index = ReferenceOperandV1::Constant(usize_constant(0));
        let length = ReferenceOperandV1::Copy(ReferencePlaceV1 {
            local: 3,
            projection: Box::default(),
        });
        let condition = ReferenceOperandV1::Copy(ReferencePlaceV1 {
            local: 4,
            projection: Box::default(),
        });
        let blocks = vec![
            ReferenceBlockV1 {
                block: 0,
                assignments: vec![
                    ReferenceAssignmentV1 {
                        statement: 0,
                        destination: ReferencePlaceV1 {
                            local: 3,
                            projection: Box::default(),
                        },
                        value: ReferenceValueV1::InputLength {
                            reference_argument: 0,
                        },
                    },
                    ReferenceAssignmentV1 {
                        statement: 1,
                        destination: ReferencePlaceV1 {
                            local: 4,
                            projection: Box::default(),
                        },
                        value: ReferenceValueV1::Binary {
                            operation: ReferenceBinaryOpV1::LessThan,
                            lhs: index.clone(),
                            rhs: length.clone(),
                            checked: false,
                        },
                    },
                ]
                .into_boxed_slice(),
                terminator: ReferenceTerminatorV1::Assert {
                    condition,
                    expected: true,
                    success: 1,
                    bounds_check: Some(ReferenceBoundsCheckV1 {
                        index: index.clone(),
                        length,
                    }),
                },
            },
            ReferenceBlockV1 {
                block: 1,
                assignments: Box::default(),
                terminator: ReferenceTerminatorV1::Return,
            },
        ];
        let load = ReferenceEffectExpressionV1::InputLoad {
            reference_argument: 0,
            index: Box::new(ReferenceEffectExpressionV1::Constant(usize_constant(0))),
        };
        let output = ReferenceOutputWriteV1 {
            argument: 1,
            block: 1,
            statement: 0,
            coordinate: ReferenceOutputCoordinateV1::LogicalPoint(Box::default()),
            guard: ReferencePathPredicateV1::unconditional_v1(),
            rhs: load,
            value: ReferenceValueV1::Use(ReferenceOperandV1::Constant(
                ReferenceConstantV1::Scalar {
                    scalar: ReferenceScalarTypeV1::U32,
                    bits: 0,
                },
            )),
        };
        (
            ReferenceEffectIrV1 {
                argument_count: 2,
                local_count: 5,
                relations: vec![
                    ReferenceArgumentRelationV1::SharedSliceInput {
                        argument: 0,
                        element: ReferenceScalarTypeV1::U32,
                    },
                    ReferenceArgumentRelationV1::DisjointOutputCoordinate {
                        argument: 1,
                        element: ReferenceScalarTypeV1::U32,
                    },
                ]
                .into_boxed_slice(),
                blocks: blocks.into_boxed_slice(),
                loop_summaries: Box::default(),
                observable_output_effects: vec![output.clone()].into_boxed_slice(),
            },
            vec![output],
        )
    }
}

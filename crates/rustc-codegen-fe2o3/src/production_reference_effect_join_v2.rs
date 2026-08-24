//! Compiler-private join from authenticated Rust reference MIR to one ranked GPU write.

use std::fmt;

use dialect_kernel::{
    DYNAMIC_EXTENT, IndexBinaryKindAttr, OwnershipCoverageAttr, OwnershipPartitionAttr,
};
use fe2o3_functional_proof::{FunctionalRefinementSubjectsV2, SafeReferenceKindV2};
use fe2o3_pliron::{
    ProductionConstructionV1, ProductionEffectRefinementContractV2, ProductionGpuWriteSiteV2,
    ProductionRankedBlockV1, ProductionRankedCompileErrorV2, ProductionRankedKernelErrorV1,
    ProductionRankedKernelLoweringInputV1, ProductionRankedKernelV1, ProductionRankedOperationV1,
    ProductionRankedTerminatorV1, ProductionRankedValueIdV1, ProductionRankedValueV1,
    ProductionReferenceOutputSiteV2, ProductionReferenceProofV2, ProductionSessionLimitsV1,
    compile_ranked_kernel_for_lowering_v2,
};
use fe2o3_proof_contracts::DigestV1;

use crate::reference_effect_bijection_v1::{
    CompilerExtractedGpuOutputEffectV1, ReferenceEffectBijectionErrorV1,
    establish_reference_effect_bijection_v1,
};
use crate::reference_effect_v1::{
    AuthenticatedReferenceEffectBindingsV1, ReferenceArgumentRelationV1, ReferenceBinaryOpV1,
    ReferenceConstantV1, ReferenceEffectExpressionV1, ReferenceOutputCoordinateV1,
    ReferencePathPredicateV1, ReferenceScalarTypeV1,
};

const ROOT_NAME_V2: &str = "semantic_safety_module";
const LOCAL_PROOF_TIMEOUT_SECONDS_V2: u32 = 60;
const RETAINED_FUNCTIONAL_REFINEMENT_RUNTIME_ROOT_V1: &str =
    "/opt/fe2o3/verus-runtime-v2/functional-refinement-0.2026.08.02-b677dd5";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RankedGpuWriteV2 {
    pub(crate) block: usize,
    pub(crate) operation: usize,
    pub(crate) allocation_origin: u64,
    pub(crate) view: ProductionRankedValueV1,
    pub(crate) indices: Vec<ProductionRankedValueV1>,
    pub(crate) value: Option<u64>,
}

pub(crate) fn reserved_reference_value_count_v2(
    bindings: &AuthenticatedReferenceEffectBindingsV1,
) -> Result<usize, crate::production_ranked_projection_v1::ProductionRankedProjectionErrorV1> {
    let coordinate_count = bindings
        .as_slice()
        .first()
        .and_then(|binding| binding.observable_output_writes.first())
        .and_then(|write| match &write.coordinate {
            ReferenceOutputCoordinateV1::LogicalPoint(axes) => Some(axes.len()),
            _ => None,
        })
        .unwrap_or(0);
    3_usize.checked_add(coordinate_count).ok_or(
        crate::production_ranked_projection_v1::ProductionRankedProjectionErrorV1::Unsupported(
            "reference-effect scalar reservation count overflowed",
        ),
    )
}

/// Move-only compiler custody over a request derived from exact collector and
/// ranked-projection state. Generic receipt APIs cannot construct this type.
pub(crate) struct CompilerOwnedReferenceEffectRequestV2 {
    kernel: ProductionRankedKernelV1,
    block: usize,
    operation: usize,
    subjects: FunctionalRefinementSubjectsV2,
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
        let (binding, imported, policy) =
            fe2o3_verifier::execute_and_import_ranked_functional_refinement_locally_v2(
                &runtime,
                &self.kernel,
                self.block,
                self.operation,
                self.subjects,
                LOCAL_PROOF_TIMEOUT_SECONDS_V2,
            )
            .map_err(|error| {
                ProductionReferenceEffectJoinErrorV2::ProofExecution(error.to_string())
            })?;
        let request =
            ProductionReferenceProofV2::request_exact(imported.receipt_identity(), binding);
        let bound = self
            .kernel
            .bind_functional_refinement_request_v2(self.block, self.operation, request)
            .map_err(ProductionReferenceEffectJoinErrorV2::Recipe)?;
        let construction =
            ProductionConstructionV1::ranked_kernel(ROOT_NAME_V2, bound).map_err(|error| {
                ProductionReferenceEffectJoinErrorV2::Construction(format!("{error:?}"))
            })?;
        compile_ranked_kernel_for_lowering_v2(
            construction,
            ProductionSessionLimitsV1::default(),
            vec![imported],
            policy,
        )
        .map_err(ProductionReferenceEffectJoinErrorV2::Compile)
    }
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
    let [reference_write] = binding.observable_output_writes.as_ref() else {
        return Err(ProductionReferenceEffectJoinErrorV2::ReferenceWriteCount(
            binding.observable_output_writes.len(),
        ));
    };
    if !matches!(
        reference_write.coordinate,
        ReferenceOutputCoordinateV1::LogicalPoint(_)
    ) {
        return Err(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
            "V2 source join currently accepts one independently indexed logical point output",
        ));
    }
    let relation = binding
        .effect_ir
        .relations
        .iter()
        .find(|relation| match relation {
            ReferenceArgumentRelationV1::DisjointOutputCoordinate { argument, .. } => {
                *argument == reference_write.argument
            }
            _ => false,
        })
        .ok_or(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
            "observable reference write has no per-coordinate logical ABI relation",
        ))?;
    let ReferenceArgumentRelationV1::DisjointOutputCoordinate { argument, element } = relation
    else {
        unreachable!()
    };
    let gpu_effects = writes
        .iter()
        .map(|write| compiler_extracted_gpu_effect_v1(&kernel, binding, write))
        .collect::<Result<Vec<_>, _>>()?;
    let pairs = establish_reference_effect_bijection_v1(
        binding.observable_output_writes.as_ref(),
        &gpu_effects,
    )
    .map_err(ProductionReferenceEffectJoinErrorV2::EffectBijection)?;
    let [pair] = pairs.as_ref() else {
        return Err(ProductionReferenceEffectJoinErrorV2::ReferenceWriteCount(
            pairs.len(),
        ));
    };
    let write = writes
        .iter()
        .find(|write| {
            write.block == pair.gpu_block as usize && write.operation == pair.gpu_operation as usize
        })
        .ok_or(ProductionReferenceEffectJoinErrorV2::WriteLocation)?;
    let gpu_scalar =
        write
            .value
            .ok_or(ProductionReferenceEffectJoinErrorV2::UnmodeledGpuValue {
                block: write.block,
                operation: write.operation,
            })?;
    let reference_value =
        reference_constant_expression_value(reference_write.rhs.clone(), *element)?;
    let reference_indices = reference_ranked_indices_v2(&kernel, &reference_write.coordinate)?;
    if reserved_values.len() != 3 + reference_indices.len() {
        return Err(
            ProductionReferenceEffectJoinErrorV2::InvalidReservedValueCount {
                expected: 3 + reference_indices.len(),
                actual: reserved_values.len(),
            },
        );
    }
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
    if blocks.iter().flat_map(|block| block.operations()).any(
        |operation| matches!(operation, ProductionRankedOperationV1::OwnershipContract { view, .. } if *view == write.view),
    ) {
        return Err(ProductionReferenceEffectJoinErrorV2::AmbiguousOwnership);
    }
    let true_value = reserved_values[0];
    let gpu_value_id = reserved_values[1];
    let reference_value_id = reserved_values[2];
    let coordinate_values = reserved_values[3..]
        .iter()
        .copied()
        .map(ProductionRankedValueV1::Local)
        .collect::<Vec<_>>();
    let entry = blocks
        .first_mut()
        .ok_or(ProductionReferenceEffectJoinErrorV2::WriteLocation)?;
    let mut entry_operations = entry.operations().to_vec();
    replace_reserved_semantic_constant_v2(&mut entry_operations, true_value, 1)?;
    replace_reserved_semantic_constant_v2(&mut entry_operations, gpu_value_id, gpu_scalar)?;
    replace_reserved_semantic_constant_v2(
        &mut entry_operations,
        reference_value_id,
        reference_value,
    )?;
    for (axis, identity) in reserved_values[3..].iter().copied().enumerate() {
        validate_reserved_semantic_symbol_v2(&entry_operations, identity, axis as u32)?;
    }
    entry_operations.push(ProductionRankedOperationV1::OwnershipContract {
        view: write.view,
        coverage: OwnershipCoverageAttr::ExactEffectDomain,
        partition: OwnershipPartitionAttr::ExactSets,
    });
    *entry = ProductionRankedBlockV1::with_index_arguments(
        entry.index_argument_count(),
        entry_operations,
        entry.terminator().clone(),
    );
    let target = blocks
        .get(write.block)
        .ok_or(ProductionReferenceEffectJoinErrorV2::WriteLocation)?;
    let mut operations = target.operations().to_vec();
    let terminator = target.terminator().clone();
    let index_arguments = target.index_argument_count();

    let projected_write = operations
        .get(write.operation)
        .cloned()
        .ok_or(ProductionReferenceEffectJoinErrorV2::WriteLocation)?;
    match projected_write {
        ProductionRankedOperationV1::Access {
            kind,
            view,
            indices,
        } if kind.writes_memory() && view == write.view && indices == write.indices => {
            operations[write.operation] = ProductionRankedOperationV1::ValueAccess {
                kind,
                view,
                indices,
                value: ProductionRankedValueV1::Local(gpu_value_id),
            };
        }
        _ => {
            return Err(ProductionReferenceEffectJoinErrorV2::UnsupportedGpuEffect {
                block: write.block,
                operation: write.operation,
                detail: "functional refinement requires the exact projected non-atomic write with matching view and indices",
            });
        }
    }

    let request_operation = operations.len();
    let contract_identity = contract_identity(binding.effect_ir_sha256, write);
    let contract = ProductionEffectRefinementContractV2::new(
        contract_identity,
        ProductionGpuWriteSiteV2::new(
            u32::try_from(write.block)
                .map_err(|_| ProductionReferenceEffectJoinErrorV2::WriteLocation)?,
            u32::try_from(write.operation)
                .map_err(|_| ProductionReferenceEffectJoinErrorV2::WriteLocation)?,
        ),
        ProductionReferenceOutputSiteV2::new(
            *argument,
            reference_write.block,
            reference_write.statement,
        ),
        write.view,
        write.indices.clone(),
        coordinate_values.clone(),
        coordinate_values,
        ProductionRankedValueV1::Local(true_value),
        ProductionRankedValueV1::Local(true_value),
        ProductionRankedValueV1::Local(true_value),
        ProductionRankedValueV1::Local(true_value),
        ProductionRankedValueV1::Local(gpu_value_id),
        ProductionRankedValueV1::Local(reference_value_id),
    )
    .map_err(ProductionReferenceEffectJoinErrorV2::Recipe)?;
    operations.push(ProductionRankedOperationV1::RequestEffectRefinement { contract, subjects });
    blocks[write.block] =
        ProductionRankedBlockV1::with_index_arguments(index_arguments, operations, terminator);
    let kernel =
        ProductionRankedKernelV1::new(kernel.function_name(), kernel.argument_count(), blocks)
            .map_err(ProductionReferenceEffectJoinErrorV2::Recipe)?;
    Ok(CompilerOwnedReferenceEffectRequestV2 {
        kernel,
        block: write.block,
        operation: request_operation,
        subjects,
    })
}

fn reference_constant_expression_value(
    value: ReferenceEffectExpressionV1,
    scalar: ReferenceScalarTypeV1,
) -> Result<u64, ProductionReferenceEffectJoinErrorV2> {
    if !supported_ranked_scalar_v2(scalar) {
        return Err(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
            "per-coordinate source join currently accepts unsigned integer or bool output constants",
        ));
    }
    let ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::Scalar {
        scalar: actual,
        bits,
    }) = value
    else {
        return Err(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
            "reference output RHS is not one normalized scalar constant",
        ));
    };
    if actual != scalar {
        return Err(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
            "reference output constant type disagrees with its logical ABI",
        ));
    }
    u64::try_from(bits).map_err(|_| {
        ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
            "reference output constant does not fit the ranked semantic scalar domain",
        )
    })
}

fn supported_ranked_scalar_v2(scalar: ReferenceScalarTypeV1) -> bool {
    matches!(
        scalar,
        ReferenceScalarTypeV1::Bool
            | ReferenceScalarTypeV1::U8
            | ReferenceScalarTypeV1::U16
            | ReferenceScalarTypeV1::U32
            | ReferenceScalarTypeV1::U64
            | ReferenceScalarTypeV1::Usize
    )
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
    let value = write
        .value
        .ok_or(ProductionReferenceEffectJoinErrorV2::UnmodeledGpuValue {
            block: write.block,
            operation: write.operation,
        })?;
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
        rhs: ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::Scalar {
            scalar,
            bits: u128::from(value),
        }),
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
        | ProductionRankedOperationV1::SemanticSymbol { result, .. } => Some(*result),
        _ => None,
    }
}

fn reference_ranked_indices_v2(
    kernel: &ProductionRankedKernelV1,
    coordinate: &ReferenceOutputCoordinateV1,
) -> Result<Vec<ProductionRankedValueV1>, ProductionReferenceEffectJoinErrorV2> {
    let ReferenceOutputCoordinateV1::LogicalPoint(axes) = coordinate else {
        return Err(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
            "reference output coordinate is not a logical point",
        ));
    };
    axes.iter()
        .map(|axis| {
            let ReferenceEffectExpressionV1::PointCoordinate { axis } = axis else {
                return Err(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
                    "reference logical point is not a direct coordinate argument",
                ));
            };
            unique_invocation_index_v2(kernel, *axis)
        })
        .collect()
}

fn unique_invocation_index_v2(
    kernel: &ProductionRankedKernelV1,
    axis: u32,
) -> Result<ProductionRankedValueV1, ProductionReferenceEffectJoinErrorV2> {
    let mut values = kernel
        .blocks()
        .iter()
        .flat_map(|block| block.operations())
        .filter_map(|operation| match operation {
            ProductionRankedOperationV1::InvocationIndex {
                result, dimension, ..
            } if *dimension == axis => Some(ProductionRankedValueV1::Local(*result)),
            _ => None,
        });
    let value = values
        .next()
        .ok_or(ProductionReferenceEffectJoinErrorV2::UnsupportedGpuIndex(
            "reference point axis has no compiler-derived GPU invocation coordinate",
        ))?;
    if values.next().is_some() {
        return Err(ProductionReferenceEffectJoinErrorV2::UnsupportedGpuIndex(
            "reference point axis has multiple GPU invocation coordinates",
        ));
    }
    Ok(value)
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
        if !true_reaches || !exact_bounds_pair_v2(kernel, write, lhs, rhs) {
            return Err(ProductionReferenceEffectJoinErrorV2::UnsupportedGpuEffect {
                block: write.block,
                operation: write.operation,
                detail: "GPU write has a logical path guard outside the exact memory-bounds selection",
            });
        }
    }
    Ok(())
}

fn exact_bounds_pair_v2(
    kernel: &ProductionRankedKernelV1,
    write: &RankedGpuWriteV2,
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
            } if write.view == ProductionRankedValueV1::Local(*result) => {
                Some((shape.as_slice(), dynamic_extents.as_slice()))
            }
            _ => None,
        });
    let Some((shape, dynamic_extents)) = view else {
        return false;
    };
    write.indices.iter().enumerate().any(|(axis, index)| {
        if *index != lhs || axis >= shape.len() {
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

fn replace_reserved_semantic_constant_v2(
    operations: &mut [ProductionRankedOperationV1],
    identity: ProductionRankedValueIdV1,
    replacement: u64,
) -> Result<(), ProductionReferenceEffectJoinErrorV2> {
    let mut found = false;
    for operation in operations {
        if operation_result_v2(operation) != Some(identity) {
            continue;
        }
        let ProductionRankedOperationV1::SemanticConstant { value, .. } = operation else {
            return Err(ProductionReferenceEffectJoinErrorV2::InvalidReservedValue(
                identity.get(),
            ));
        };
        if found {
            return Err(ProductionReferenceEffectJoinErrorV2::InvalidReservedValue(
                identity.get(),
            ));
        }
        *value = replacement;
        found = true;
    }
    if !found {
        return Err(ProductionReferenceEffectJoinErrorV2::InvalidReservedValue(
            identity.get(),
        ));
    }
    Ok(())
}

fn validate_reserved_semantic_symbol_v2(
    operations: &[ProductionRankedOperationV1],
    identity: ProductionRankedValueIdV1,
    expected_symbol: u32,
) -> Result<(), ProductionReferenceEffectJoinErrorV2> {
    let mut definitions = operations
        .iter()
        .filter(|operation| operation_result_v2(operation) == Some(identity));
    let valid = matches!(
        definitions.next(),
        Some(ProductionRankedOperationV1::SemanticSymbol { symbol, .. })
            if *symbol == expected_symbol
    ) && definitions.next().is_none();
    if !valid {
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
    },
    AmbiguousOwnership,
    WriteLocation,
    InvalidReservedValueCount {
        expected: usize,
        actual: usize,
    },
    InvalidReservedValue(u32),
    Subjects(String),
    Recipe(ProductionRankedKernelErrorV1),
    Construction(String),
    ProofRuntimeUnavailable {
        root: &'static str,
        detail: String,
    },
    ProofExecution(String),
    Compile(ProductionRankedCompileErrorV2),
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
                "source-to-proof V2 requires exactly one observable reference output write; found {actual}"
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
            Self::UnmodeledGpuValue { block, operation } => write!(
                formatter,
                "source-to-proof V2 cannot normalize the GPU store value at ranked block {block} op {operation}; only a compiler-derived exact scalar expression is accepted"
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
            Self::InvalidReservedValue(identity) => write!(
                formatter,
                "source-to-proof V2 reserved semantic value %{identity} is missing, duplicated, or has the wrong ranked type"
            ),
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
    use dialect_kernel::{AccessKindAttr, MemorySpaceAttr};

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
            value: Some(17),
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
            reference_ranked_indices_v2(
                &kernel,
                &ReferenceOutputCoordinateV1::LogicalPoint(
                    vec![ReferenceEffectExpressionV1::PointCoordinate { axis: 0 }]
                        .into_boxed_slice(),
                ),
            )
            .unwrap(),
            write.indices,
        );
    }

    #[test]
    fn non_bounds_logical_guard_is_not_erased() {
        let (kernel, write) = dynamic_point_kernel(true);
        assert!(matches!(
            validate_bounds_only_gpu_guard_v2(&kernel, &write),
            Err(ProductionReferenceEffectJoinErrorV2::UnsupportedGpuEffect { .. })
        ));
    }
}

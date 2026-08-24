//! Compiler-private join from authenticated Rust reference MIR to one ranked GPU write.

use std::fmt;

use dialect_kernel::{OwnershipCoverageAttr, OwnershipPartitionAttr};
use fe2o3_functional_proof::{FunctionalRefinementSubjectsV2, SafeReferenceKindV2};
use fe2o3_pliron::{
    ProductionConstructionV1, ProductionEffectRefinementContractV2, ProductionGpuWriteSiteV2,
    ProductionRankedBlockV1, ProductionRankedCompileErrorV2, ProductionRankedKernelErrorV1,
    ProductionRankedKernelLoweringInputV1, ProductionRankedKernelV1, ProductionRankedOperationV1,
    ProductionRankedValueIdV1, ProductionRankedValueV1, ProductionReferenceOutputSiteV2,
    ProductionReferenceProofV2, ProductionSessionLimitsV1, compile_ranked_kernel_for_lowering_v2,
};
use fe2o3_proof_contracts::DigestV1;

use crate::reference_effect_v1::{
    AuthenticatedReferenceEffectBindingsV1, ReferenceArgumentRelationV1, ReferenceConstantV1,
    ReferenceOperandV1, ReferenceOutputCoordinateV1, ReferenceScalarTypeV1, ReferenceValueV1,
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
    next_value: &mut u32,
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
        ReferenceOutputCoordinateV1::SingleCoordinate
    ) {
        return Err(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
            "V2 source join currently accepts one per-coordinate mutable output; indexed slice normalization is pending",
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
    let reference_value = reference_constant_value(reference_write.value.clone(), *element)?;
    let allocation_origin = u64::from(*argument)
        .checked_add(1)
        .ok_or(ProductionReferenceEffectJoinErrorV2::InvalidAllocationOrigin)?;
    let mut candidates = writes
        .iter()
        .filter(|write| write.allocation_origin == allocation_origin);
    let write =
        candidates
            .next()
            .ok_or(ProductionReferenceEffectJoinErrorV2::UnmodeledGpuWrite {
                argument: *argument,
                allocation_origin,
            })?;
    if candidates.next().is_some() {
        return Err(ProductionReferenceEffectJoinErrorV2::AmbiguousGpuWrite {
            argument: *argument,
            allocation_origin,
        });
    }
    let gpu_scalar =
        write
            .value
            .ok_or(ProductionReferenceEffectJoinErrorV2::UnmodeledGpuValue {
                block: write.block,
                operation: write.operation,
            })?;
    if gpu_scalar != reference_value {
        return Err(ProductionReferenceEffectJoinErrorV2::ValueMismatch {
            gpu: gpu_scalar,
            reference: reference_value,
        });
    }
    if binding.effect_ir.relations.iter().any(|relation| {
        matches!(
            relation,
            ReferenceArgumentRelationV1::DisjointOutputCoordinate { .. }
        )
    }) {
        return Err(ProductionReferenceEffectJoinErrorV2::IndependentReferenceSemanticsUnavailable);
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
    let true_value = allocate_value(next_value)?;
    let gpu_value_id = allocate_value(next_value)?;
    let reference_value_id = allocate_value(next_value)?;
    let entry = blocks
        .first_mut()
        .ok_or(ProductionReferenceEffectJoinErrorV2::WriteLocation)?;
    let mut entry_operations = entry.operations().to_vec();
    entry_operations.push(ProductionRankedOperationV1::SemanticConstant {
        result: true_value,
        value: 1,
    });
    entry_operations.push(ProductionRankedOperationV1::SemanticConstant {
        result: gpu_value_id,
        value: gpu_scalar,
    });
    entry_operations.push(ProductionRankedOperationV1::SemanticConstant {
        result: reference_value_id,
        value: reference_value,
    });
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
        write.indices.clone(),
        write.indices.clone(),
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

fn reference_constant_value(
    value: ReferenceValueV1,
    scalar: ReferenceScalarTypeV1,
) -> Result<u64, ProductionReferenceEffectJoinErrorV2> {
    if !matches!(
        scalar,
        ReferenceScalarTypeV1::Bool
            | ReferenceScalarTypeV1::U8
            | ReferenceScalarTypeV1::U16
            | ReferenceScalarTypeV1::U32
            | ReferenceScalarTypeV1::U64
            | ReferenceScalarTypeV1::Usize
    ) {
        return Err(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
            "per-coordinate source join currently accepts unsigned integer or bool output constants",
        ));
    }
    let ReferenceValueV1::Use(ReferenceOperandV1::Constant(ReferenceConstantV1::Scalar {
        scalar: actual,
        bits,
    })) = value
    else {
        return Err(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
            "reference output value is not one direct scalar constant",
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

fn allocate_value(
    next_value: &mut u32,
) -> Result<ProductionRankedValueIdV1, ProductionReferenceEffectJoinErrorV2> {
    let value = ProductionRankedValueIdV1::new(*next_value);
    *next_value = next_value
        .checked_add(1)
        .ok_or(ProductionReferenceEffectJoinErrorV2::ValueIdentityOverflow)?;
    Ok(value)
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
    InvalidAllocationOrigin,
    UnmodeledGpuWrite {
        argument: u32,
        allocation_origin: u64,
    },
    AmbiguousGpuWrite {
        argument: u32,
        allocation_origin: u64,
    },
    UnmodeledGpuValue {
        block: usize,
        operation: usize,
    },
    ValueMismatch {
        gpu: u64,
        reference: u64,
    },
    IndependentReferenceSemanticsUnavailable,
    AmbiguousOwnership,
    WriteLocation,
    ValueIdentityOverflow,
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
            Self::InvalidAllocationOrigin => {
                formatter.write_str("source-to-proof V2 output allocation origin overflowed")
            }
            Self::UnmodeledGpuWrite {
                argument,
                allocation_origin,
            } => write!(
                formatter,
                "source-to-proof V2 found no GPU write for reference output argument {argument} (allocation origin {allocation_origin})"
            ),
            Self::AmbiguousGpuWrite {
                argument,
                allocation_origin,
            } => write!(
                formatter,
                "source-to-proof V2 found multiple GPU writes for reference output argument {argument} (allocation origin {allocation_origin}); one exact write is required"
            ),
            Self::UnmodeledGpuValue { block, operation } => write!(
                formatter,
                "source-to-proof V2 cannot normalize the GPU store value at ranked block {block} op {operation}; only a compiler-derived exact scalar expression is accepted"
            ),
            Self::ValueMismatch { gpu, reference } => write!(
                formatter,
                "source-to-proof V2 exact value mismatch: GPU store is {gpu}, safe Rust reference output is {reference}"
            ),
            Self::IndependentReferenceSemanticsUnavailable => formatter.write_str(
                "source-to-proof V2 stopped before proof admission: the current reference-effect IR retains only an implicit point coordinate and does not independently normalize the safe Rust reference coordinate and path guard",
            ),
            Self::AmbiguousOwnership => formatter.write_str(
                "source-to-proof V2 output view already has an ownership contract; one compiler-owned contract is required",
            ),
            Self::WriteLocation => {
                formatter.write_str("source-to-proof V2 GPU write location is outside the ranked CFG")
            }
            Self::ValueIdentityOverflow => {
                formatter.write_str("source-to-proof V2 ranked value identity overflowed")
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

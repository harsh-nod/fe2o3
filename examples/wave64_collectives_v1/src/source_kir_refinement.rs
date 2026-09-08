//! Bounded source-model correspondence for compiler-produced canonical KIR V13.
//!
//! This layer admits only an owned, verified V13 graph and observes its generic
//! execution-capability operations. It is intentionally not a compiler proof:
//! unresolved convergence, ownership, and refinement obligations remain owned
//! by the production compiler pipeline.

use core::fmt;

use fe2o3_kernel_ir::{
    ExecutionCapabilityOperationV1, ExecutionCollectiveKindV1, ExecutionMemoryAccessV1,
    ExecutionMemoryAddressSpaceV1, KernelIrDecodeError, Module, OperationKind, ScalarType,
    VerifiedCanonicalKernelIrErrorV13, VerifiedCanonicalKernelIrV13, decode_module_v13,
};
use sha2::{Digest as _, Sha256};

use crate::{OracleErrorV1, WAVE64_LANES_V1, lane_is_active_v1, wave64_collectives_oracle_v1};

const ATTRIBUTED_SOURCE_BYTES_V1: &[u8] = include_bytes!("kernel.rs");

/// SHA-256 of the exact checked-in attributed source admitted by this adapter.
pub const WAVE64_COLLECTIVES_V1_SOURCE_SHA256: [u8; 32] = [
    0xa0, 0x07, 0xfc, 0xe3, 0x3c, 0x6f, 0x61, 0xc8, 0x86, 0x42, 0x7a, 0xf1, 0x16, 0xa5, 0xd8, 0x35,
    0x9c, 0x95, 0x12, 0x4f, 0xec, 0x31, 0x11, 0xb7, 0xd3, 0x1f, 0x6e, 0xe6, 0x11, 0x02, 0xdd, 0xdf,
];

/// Exact non-authority boundary carried by a successful V13 observation.
pub const WAVE64_REFINEMENT_BOUNDARY_V13: &str = "exact source bytes and verified canonical KIR V13 bytes;target-neutral collective-family and disjoint-publication observation;finite integral f32 corpus;active zero sign is abstracted;unresolved proof obligations are not discharged;no source-to-KIR proof;no compiler causality;no LLVM/ISA refinement;no artifact, protected-execution, generalized-safety, or parity authority";

/// Exact source and canonical V13 identities selected by one observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Wave64RefinementIdentitiesV13 {
    /// SHA-256 of `src/kernel.rs`.
    pub attributed_source_sha256: [u8; 32],
    /// Domain-separated identity of the complete canonical KIR V13 bytes.
    pub canonical_kir_v13_identity: [u8; 32],
    /// Exact canonical KIR V13 byte length.
    pub canonical_kir_v13_length: u64,
}

/// Binds the checked-in source identity to one exact canonical V13 owner.
pub fn bind_wave64_refinement_identities_v13(
    canonical: &VerifiedCanonicalKernelIrV13,
) -> Wave64RefinementIdentitiesV13 {
    Wave64RefinementIdentitiesV13 {
        attributed_source_sha256: WAVE64_COLLECTIVES_V1_SOURCE_SHA256,
        canonical_kir_v13_identity: *canonical.identity().digest(),
        canonical_kir_v13_length: canonical.identity().canonical_length(),
    }
}

/// One abstract output family in the semantic relation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Wave64SemanticOutputV1 {
    /// Full masked sum for every active lane.
    Reduction,
    /// Increasing-lane prefix through the active output lane.
    Inclusive,
    /// Increasing-lane prefix before the active output lane.
    Exclusive,
}

impl Wave64SemanticOutputV1 {
    const ALL: [Self; 3] = [Self::Reduction, Self::Inclusive, Self::Exclusive];
}

/// Three exact one-subgroup output arrays produced by either semantic model.
#[derive(Clone, Debug, PartialEq)]
pub struct Wave64SemanticOutputsV1 {
    /// Full masked reduction values.
    pub reduction: [f32; WAVE64_LANES_V1],
    /// Inclusive masked prefixes.
    pub inclusive: [f32; WAVE64_LANES_V1],
    /// Exclusive masked prefixes.
    pub exclusive: [f32; WAVE64_LANES_V1],
}

impl Wave64SemanticOutputsV1 {
    fn values(&self, output: Wave64SemanticOutputV1) -> &[f32; WAVE64_LANES_V1] {
        match output {
            Wave64SemanticOutputV1::Reduction => &self.reduction,
            Wave64SemanticOutputV1::Inclusive => &self.inclusive,
            Wave64SemanticOutputV1::Exclusive => &self.exclusive,
        }
    }
}

/// Structural mismatch in the generic V13 projection used by this example.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Wave64KirShapeErrorV13 {
    /// The exact kernel root was absent or duplicated.
    KernelRoot,
    /// The exact 64-invocation workgroup contract was absent.
    WorkgroupSize,
    /// The exact kernel entry body was absent.
    KernelEntry,
    /// A retired profile-specific wave operation remained in the graph.
    LegacyWaveOperation,
    /// The generic reduction/scan capability families were absent or duplicated.
    CollectiveFamilies,
    /// The graph did not contain exactly three disjoint global publications.
    DisjointPublications,
}

impl fmt::Display for Wave64KirShapeErrorV13 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::KernelRoot => "canonical KIR V13 does not have one exact wave64 kernel root",
            Self::WorkgroupSize => "canonical KIR V13 does not require a 64-invocation workgroup",
            Self::KernelEntry => "canonical KIR V13 does not contain the exact kernel entry body",
            Self::LegacyWaveOperation => {
                "canonical KIR V13 retained a retired profile-specific wave operation"
            }
            Self::CollectiveFamilies => {
                "canonical KIR V13 does not contain the exact generic collective families"
            }
            Self::DisjointPublications => {
                "canonical KIR V13 does not contain three disjoint global publications"
            }
        })
    }
}

/// First fail-closed rejection from the bounded V13 correspondence.
#[derive(Clone, Debug, PartialEq)]
pub enum Wave64RefinementErrorV13 {
    /// The checked-in attributed source no longer has its pinned identity.
    CheckedInSourceIdentity,
    /// The caller selected a different attributed source identity.
    SelectedSourceIdentity,
    /// The canonical V13 owner failed custody revalidation.
    CanonicalKir(VerifiedCanonicalKernelIrErrorV13),
    /// Revalidated V13 bytes could not be decoded.
    Decode(KernelIrDecodeError),
    /// The caller selected a different canonical V13 identity or length.
    SelectedCanonicalKirIdentity,
    /// The canonical V13 graph did not have the required generic shape.
    NonCanonicalKernelIr(Wave64KirShapeErrorV13),
    /// The existing source model rejected the finite-F32 input corpus.
    SourceModel(OracleErrorV1),
    /// Source and V13 symbolic contributor sets differ.
    ContributorSet {
        /// Output family containing the mismatch.
        output: Wave64SemanticOutputV1,
        /// Physical output lane.
        lane: usize,
    },
    /// Source-model and V13 output values differ.
    SemanticValue {
        /// Output family containing the mismatch.
        output: Wave64SemanticOutputV1,
        /// Physical output lane.
        lane: usize,
        /// Source-model binary32 bits.
        source_bits: u32,
        /// V13-model binary32 bits.
        kernel_ir_bits: u32,
    },
}

impl fmt::Display for Wave64RefinementErrorV13 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CheckedInSourceIdentity => {
                formatter.write_str("checked-in attributed source identity drifted")
            }
            Self::SelectedSourceIdentity => {
                formatter.write_str("selected attributed source identity is not exact")
            }
            Self::CanonicalKir(error) => write!(formatter, "{error}"),
            Self::Decode(error) => write!(formatter, "cannot decode revalidated KIR V13: {error}"),
            Self::SelectedCanonicalKirIdentity => {
                formatter.write_str("selected canonical KIR V13 identity is not exact")
            }
            Self::NonCanonicalKernelIr(error) => write!(formatter, "{error}"),
            Self::SourceModel(error) => write!(formatter, "source model rejected input: {error}"),
            Self::ContributorSet { output, lane } => {
                write!(
                    formatter,
                    "{output:?} contributor set differs at lane {lane}"
                )
            }
            Self::SemanticValue { output, lane, .. } => {
                write!(
                    formatter,
                    "{output:?} semantic value differs at lane {lane}"
                )
            }
        }
    }
}

impl std::error::Error for Wave64RefinementErrorV13 {}

/// Inert result of one exact source-model and V13 observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Wave64SourceKirRefinementV13 {
    identities: Wave64RefinementIdentitiesV13,
    active_mask: u64,
    active_lanes: u32,
    checked_symbolic_relations: u32,
}

impl Wave64SourceKirRefinementV13 {
    /// Exact identities checked for this observation.
    pub const fn identities(self) -> Wave64RefinementIdentitiesV13 {
        self.identities
    }

    /// Exact logical activity mask checked for this observation.
    pub const fn active_mask(self) -> u64 {
        self.active_mask
    }

    /// Number of active logical lanes.
    pub const fn active_lanes(self) -> u32 {
        self.active_lanes
    }

    /// Number of output/lane contributor relations checked symbolically.
    pub const fn checked_symbolic_relations(self) -> u32 {
        self.checked_symbolic_relations
    }

    /// This structural observation does not prove source-to-KIR refinement.
    pub const fn proves_source_to_kir_refinement(self) -> bool {
        false
    }

    /// This structural observation does not prove compiler causality.
    pub const fn proves_compiler_causality(self) -> bool {
        false
    }

    /// This structural observation does not discharge V13 proof obligations.
    pub const fn discharges_proof_obligations(self) -> bool {
        false
    }

    /// This structural observation grants no protected execution authority.
    pub const fn grants_protected_execution(self) -> bool {
        false
    }
}

/// Symbolic source-model contributor lanes before applying the active mask.
pub const fn source_contributor_mask_v1(output: Wave64SemanticOutputV1, lane: usize) -> u64 {
    if lane >= WAVE64_LANES_V1 {
        return 0;
    }
    match output {
        Wave64SemanticOutputV1::Reduction => u64::MAX,
        Wave64SemanticOutputV1::Inclusive => prefix_mask(lane + 1),
        Wave64SemanticOutputV1::Exclusive => prefix_mask(lane),
    }
}

const fn prefix_mask(end: usize) -> u64 {
    if end == 0 {
        0
    } else if end >= WAVE64_LANES_V1 {
        u64::MAX
    } else {
        (1_u64 << end) - 1
    }
}

fn verify_identities(
    canonical: &VerifiedCanonicalKernelIrV13,
    identities: Wave64RefinementIdentitiesV13,
) -> Result<(), Wave64RefinementErrorV13> {
    let source_sha256: [u8; 32] = Sha256::digest(ATTRIBUTED_SOURCE_BYTES_V1).into();
    if source_sha256 != WAVE64_COLLECTIVES_V1_SOURCE_SHA256 {
        return Err(Wave64RefinementErrorV13::CheckedInSourceIdentity);
    }
    if identities.attributed_source_sha256 != WAVE64_COLLECTIVES_V1_SOURCE_SHA256 {
        return Err(Wave64RefinementErrorV13::SelectedSourceIdentity);
    }
    if identities.canonical_kir_v13_identity != *canonical.identity().digest()
        || identities.canonical_kir_v13_length != canonical.identity().canonical_length()
    {
        return Err(Wave64RefinementErrorV13::SelectedCanonicalKirIdentity);
    }
    Ok(())
}

fn verify_generic_v13_shape(module: &Module) -> Result<(), Wave64KirShapeErrorV13> {
    let kernels = module
        .kernels
        .iter()
        .filter(|kernel| kernel.id.as_str() == "wave64_collectives_v1")
        .collect::<Vec<_>>();
    let [kernel] = kernels.as_slice() else {
        return Err(Wave64KirShapeErrorV13::KernelRoot);
    };
    if kernel
        .workgroup_size
        .is_none_or(|size| (size.x, size.y, size.z) != (64, 1, 1))
    {
        return Err(Wave64KirShapeErrorV13::WorkgroupSize);
    }
    let function = module
        .functions
        .iter()
        .find(|function| function.id == kernel.entry)
        .ok_or(Wave64KirShapeErrorV13::KernelEntry)?;
    let body = function
        .body
        .as_ref()
        .ok_or(Wave64KirShapeErrorV13::KernelEntry)?;

    let mut subgroup_reductions = 0;
    let mut workgroup_inclusive = 0;
    let mut workgroup_exclusive = 0;
    let mut disjoint_publications = 0;
    for operation in body.blocks.iter().flat_map(|block| &block.operations) {
        match &operation.kind {
            OperationKind::Wave(_) => return Err(Wave64KirShapeErrorV13::LegacyWaveOperation),
            OperationKind::ExecutionCapability(capability) => match &capability.operation {
                ExecutionCapabilityOperationV1::SubgroupCollective {
                    kind: ExecutionCollectiveKindV1::ReduceSum,
                    value_type: ScalarType::F32,
                    width: 64,
                    ..
                } => subgroup_reductions += 1,
                ExecutionCapabilityOperationV1::WorkgroupCollective {
                    kind: ExecutionCollectiveKindV1::InclusiveScanSum,
                    value_type: ScalarType::F32,
                    elements: 64,
                    ..
                } => workgroup_inclusive += 1,
                ExecutionCapabilityOperationV1::WorkgroupCollective {
                    kind: ExecutionCollectiveKindV1::ExclusiveScanSum,
                    value_type: ScalarType::F32,
                    elements: 64,
                    ..
                } => workgroup_exclusive += 1,
                ExecutionCapabilityOperationV1::SubgroupCollective { .. }
                | ExecutionCapabilityOperationV1::WorkgroupCollective { .. } => {
                    return Err(Wave64KirShapeErrorV13::CollectiveFamilies);
                }
                ExecutionCapabilityOperationV1::MemoryStore {
                    space: ExecutionMemoryAddressSpaceV1::Global,
                    access: ExecutionMemoryAccessV1::DisjointWrite,
                    ..
                } => disjoint_publications += 1,
                _ => {}
            },
            _ => {}
        }
    }
    if (
        subgroup_reductions,
        workgroup_inclusive,
        workgroup_exclusive,
    ) != (1, 1, 1)
    {
        return Err(Wave64KirShapeErrorV13::CollectiveFamilies);
    }
    if disjoint_publications != 3 {
        return Err(Wave64KirShapeErrorV13::DisjointPublications);
    }
    Ok(())
}

fn source_model(input: &[f32], active_mask: u64) -> Result<Wave64SemanticOutputsV1, OracleErrorV1> {
    let mut reduction = [f32::NAN; WAVE64_LANES_V1];
    let mut inclusive = [f32::NAN; WAVE64_LANES_V1];
    let mut exclusive = [f32::NAN; WAVE64_LANES_V1];
    wave64_collectives_oracle_v1(
        input,
        active_mask,
        &mut reduction,
        &mut inclusive,
        &mut exclusive,
    )?;
    Ok(Wave64SemanticOutputsV1 {
        reduction,
        inclusive,
        exclusive,
    })
}

fn exact_integer_sum(input: &[f32], selected: u64) -> f32 {
    input
        .iter()
        .copied()
        .enumerate()
        .filter(|(lane, _)| selected & (1_u64 << lane) != 0)
        .map(|(_, value)| value as i64)
        .sum::<i64>() as f32
}

fn kernel_ir_model(input: &[f32], active_mask: u64) -> Wave64SemanticOutputsV1 {
    let evaluate = |output| {
        core::array::from_fn(|lane| {
            if lane_is_active_v1(active_mask, lane) {
                exact_integer_sum(
                    input,
                    active_mask & source_contributor_mask_v1(output, lane),
                )
            } else {
                0.0
            }
        })
    };
    Wave64SemanticOutputsV1 {
        reduction: evaluate(Wave64SemanticOutputV1::Reduction),
        inclusive: evaluate(Wave64SemanticOutputV1::Inclusive),
        exclusive: evaluate(Wave64SemanticOutputV1::Exclusive),
    }
}

fn compare_semantics(
    source: &Wave64SemanticOutputsV1,
    kernel_ir: &Wave64SemanticOutputsV1,
    active_mask: u64,
) -> Result<(), Wave64RefinementErrorV13> {
    for output in Wave64SemanticOutputV1::ALL {
        for lane in 0..WAVE64_LANES_V1 {
            let source_value = source.values(output)[lane];
            let kernel_ir_value = kernel_ir.values(output)[lane];
            let equivalent = if lane_is_active_v1(active_mask, lane) {
                source_value == kernel_ir_value
            } else {
                source_value.to_bits() == 0 && kernel_ir_value.to_bits() == 0
            };
            if !equivalent {
                return Err(Wave64RefinementErrorV13::SemanticValue {
                    output,
                    lane,
                    source_bits: source_value.to_bits(),
                    kernel_ir_bits: kernel_ir_value.to_bits(),
                });
            }
        }
    }
    Ok(())
}

/// Checks one source-model observation against an exact canonical V13 owner.
///
/// The returned receipt records a structural observation only. It neither
/// authenticates how the graph was produced nor discharges its obligation bits.
pub fn verify_wave64_source_model_to_kir_v13(
    input: &[f32],
    active_mask: u64,
    canonical: &VerifiedCanonicalKernelIrV13,
    identities: Wave64RefinementIdentitiesV13,
) -> Result<Wave64SourceKirRefinementV13, Wave64RefinementErrorV13> {
    canonical
        .revalidate()
        .map_err(Wave64RefinementErrorV13::CanonicalKir)?;
    verify_identities(canonical, identities)?;
    let module =
        decode_module_v13(canonical.canonical_bytes()).map_err(Wave64RefinementErrorV13::Decode)?;
    verify_generic_v13_shape(&module).map_err(Wave64RefinementErrorV13::NonCanonicalKernelIr)?;
    let source = source_model(input, active_mask).map_err(Wave64RefinementErrorV13::SourceModel)?;
    let kernel_ir = kernel_ir_model(input, active_mask);
    compare_semantics(&source, &kernel_ir, active_mask)?;
    Ok(Wave64SourceKirRefinementV13 {
        identities,
        active_mask,
        active_lanes: active_mask.count_ones(),
        checked_symbolic_relations: (Wave64SemanticOutputV1::ALL.len() * WAVE64_LANES_V1) as u32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_comparison_rejects_each_hostile_output_family() {
        let source = Wave64SemanticOutputsV1 {
            reduction: [0.0; WAVE64_LANES_V1],
            inclusive: [0.0; WAVE64_LANES_V1],
            exclusive: [0.0; WAVE64_LANES_V1],
        };
        for output in Wave64SemanticOutputV1::ALL {
            let mut hostile = source.clone();
            match output {
                Wave64SemanticOutputV1::Reduction => hostile.reduction[17] = 1.0,
                Wave64SemanticOutputV1::Inclusive => hostile.inclusive[17] = 1.0,
                Wave64SemanticOutputV1::Exclusive => hostile.exclusive[17] = 1.0,
            }
            assert!(matches!(
                compare_semantics(&source, &hostile, 1_u64 << 17),
                Err(Wave64RefinementErrorV13::SemanticValue {
                    output: actual,
                    lane: 17,
                    ..
                }) if actual == output
            ));
        }
    }
}

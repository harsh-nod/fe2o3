//! Executable source-model-to-Kernel-IR refinement for the exact Wave64 V1 profile.
//!
//! This layer compares the checked CPU source model with an independent
//! interpreter of the canonical semantic Kernel IR. It binds the exact
//! checked-in attributed source and Kernel-IR schema bytes. It does not prove
//! that rustc produced the Kernel IR, or that LLVM, ISA, runtime, or hardware
//! behavior refines either model.

use core::fmt;

use fe2o3_kernel_ir::{
    WAVE64_COLLECTIVES_V1_SOURCE_SHA256, Wave64ArgumentRoleV1, Wave64CollectiveKindV1,
    Wave64CollectivesKernelIrV1, Wave64CollectivesProfileV1, Wave64CollectivesV1Error,
    Wave64OutputOwnershipV1, verify_wave64_collectives_v1,
};
use sha2::{Digest as _, Sha256};

use crate::{
    OracleErrorV1, WAVE64_LANES_V1, lane_is_active_v1, lane_outputs_v1,
    wave64_collectives_oracle_v1,
};

const ATTRIBUTED_SOURCE_BYTES_V1: &[u8] = include_bytes!("kernel.rs");
const KERNEL_IR_SCHEMA_BYTES_V1: &[u8] =
    include_bytes!("../../../crates/fe2o3-kernel-ir/src/wave64_collectives_v1.rs");

/// SHA-256 of the exact checked-in semantic Kernel-IR schema source.
pub const WAVE64_COLLECTIVES_V1_KIR_SCHEMA_SHA256: [u8; 32] = [
    0xd7, 0x33, 0xad, 0x4e, 0x53, 0x0c, 0xc7, 0x5a, 0xdd, 0x27, 0xf0, 0x51, 0x62, 0x03, 0x94, 0xbf,
    0xd7, 0xa0, 0x5e, 0x75, 0xb1, 0xd5, 0x77, 0x54, 0x3b, 0x69, 0x3f, 0x42, 0xd9, 0x1b, 0x4b, 0x42,
];

/// The exact non-authority boundary carried by every successful check.
pub const WAVE64_REFINEMENT_BOUNDARY_V1: &str = "exact source-model-to-canonical-semantic-Kernel-IR value/ownership correspondence under the u64 mask and finite integral f32 corpus;active zero sign is abstracted;no source-to-model proof;no compiler causality;no LLVM/ISA refinement;no artifact, protected-execution, generalized-safety, or parity authority";

/// Exact checked-in identities selected by this bounded correspondence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Wave64RefinementIdentitiesV1 {
    /// SHA-256 of `src/kernel.rs`.
    pub attributed_source_sha256: [u8; 32],
    /// SHA-256 of the exact Wave64 semantic Kernel-IR schema source.
    pub kernel_ir_schema_sha256: [u8; 32],
}

/// Returns the only identity pair admitted by the V1 refinement checker.
pub const fn exact_wave64_refinement_identities_v1() -> Wave64RefinementIdentitiesV1 {
    Wave64RefinementIdentitiesV1 {
        attributed_source_sha256: WAVE64_COLLECTIVES_V1_SOURCE_SHA256,
        kernel_ir_schema_sha256: WAVE64_COLLECTIVES_V1_KIR_SCHEMA_SHA256,
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

    const fn argument(self) -> Wave64ArgumentRoleV1 {
        match self {
            Self::Reduction => Wave64ArgumentRoleV1::ReductionOutput,
            Self::Inclusive => Wave64ArgumentRoleV1::InclusiveOutput,
            Self::Exclusive => Wave64ArgumentRoleV1::ExclusiveOutput,
        }
    }
}

/// Three exact one-Wave output arrays produced by either semantic model.
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

/// First fail-closed rejection from the bounded refinement checker.
#[derive(Clone, Debug, PartialEq)]
pub enum Wave64RefinementErrorV1 {
    /// The checked-in attributed source no longer has its pinned identity.
    CheckedInSourceIdentity {
        /// Pinned SHA-256.
        expected: [u8; 32],
        /// Observed SHA-256.
        actual: [u8; 32],
    },
    /// The caller selected a different attributed source identity.
    SelectedSourceIdentity,
    /// The checked-in Kernel-IR schema no longer has its pinned identity.
    CheckedInKernelIrSchemaIdentity {
        /// Pinned SHA-256.
        expected: [u8; 32],
        /// Observed SHA-256.
        actual: [u8; 32],
    },
    /// The caller selected a different Kernel-IR schema identity.
    SelectedKernelIrSchemaIdentity,
    /// The semantic Kernel IR or exact gfx942 profile was not canonical.
    NonCanonicalKernelIr(Wave64CollectivesV1Error),
    /// The existing source model rejected the finite-F32 input corpus.
    SourceModel(OracleErrorV1),
    /// Source and Kernel-IR symbolic contributor sets differ.
    ContributorSet {
        /// Output family containing the mismatch.
        output: Wave64SemanticOutputV1,
        /// Physical output lane.
        lane: usize,
        /// Source-model contributor set before applying the active mask.
        source: u64,
        /// Kernel-IR contributor set before applying the active mask.
        kernel_ir: u64,
    },
    /// Source and Kernel-IR lane ownership differ.
    OutputOwnership {
        /// Output family containing the mismatch.
        output: Wave64SemanticOutputV1,
        /// Physical writing lane.
        lane: usize,
    },
    /// Source-model and Kernel-IR output values differ.
    SemanticValue {
        /// Output family containing the mismatch.
        output: Wave64SemanticOutputV1,
        /// Physical output lane.
        lane: usize,
        /// Source-model binary32 bits.
        source_bits: u32,
        /// Kernel-IR-model binary32 bits.
        kernel_ir_bits: u32,
    },
}

impl fmt::Display for Wave64RefinementErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CheckedInSourceIdentity { .. } => {
                formatter.write_str("checked-in Wave64 attributed source identity drifted")
            }
            Self::SelectedSourceIdentity => {
                formatter.write_str("selected Wave64 attributed source identity is not exact")
            }
            Self::CheckedInKernelIrSchemaIdentity { .. } => {
                formatter.write_str("checked-in Wave64 Kernel-IR schema identity drifted")
            }
            Self::SelectedKernelIrSchemaIdentity => {
                formatter.write_str("selected Wave64 Kernel-IR schema identity is not exact")
            }
            Self::NonCanonicalKernelIr(error) => write!(formatter, "{error}"),
            Self::SourceModel(error) => write!(formatter, "source model rejected input: {error}"),
            Self::ContributorSet { output, lane, .. } => {
                write!(
                    formatter,
                    "{output:?} contributor set differs at lane {lane}"
                )
            }
            Self::OutputOwnership { output, lane } => {
                write!(
                    formatter,
                    "{output:?} output ownership differs at lane {lane}"
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

impl std::error::Error for Wave64RefinementErrorV1 {}

/// Inert evidence that one exact mask/input observation refined successfully.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Wave64SourceKirRefinementV1 {
    identities: Wave64RefinementIdentitiesV1,
    active_mask: u64,
    active_lanes: u32,
    checked_symbolic_relations: u32,
}

impl Wave64SourceKirRefinementV1 {
    /// Exact identities checked for this observation.
    pub const fn identities(self) -> Wave64RefinementIdentitiesV1 {
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

    /// This bounded model comparison does not prove source-to-model refinement.
    pub const fn proves_source_to_model_refinement(self) -> bool {
        false
    }

    /// This bounded model comparison does not prove compiler causality.
    pub const fn proves_compiler_causality(self) -> bool {
        false
    }

    /// This bounded model comparison does not prove LLVM or ISA refinement.
    pub const fn proves_llvm_or_isa_refinement(self) -> bool {
        false
    }

    /// Active exact-zero sign is abstracted to the mathematical integer zero.
    pub const fn proves_active_zero_sign_refinement(self) -> bool {
        false
    }

    /// This inert evidence grants no protected execution authority.
    pub const fn grants_protected_execution(self) -> bool {
        false
    }

    /// This exact profile result is not generalized memory or race safety.
    pub const fn proves_generalized_safety(self) -> bool {
        false
    }

    /// This evidence alone cannot promote a parity row.
    pub const fn grants_parity_promotion(self) -> bool {
        false
    }
}

/// Symbolic source-model contributor lanes before applying the active mask.
///
/// Equality of this set with the KIR set for every output lane proves mask
/// selection for every possible `u64` mask without enumerating `2^64` values.
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

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn verify_identities(
    identities: Wave64RefinementIdentitiesV1,
) -> Result<(), Wave64RefinementErrorV1> {
    let source = sha256(ATTRIBUTED_SOURCE_BYTES_V1);
    if source != WAVE64_COLLECTIVES_V1_SOURCE_SHA256 {
        return Err(Wave64RefinementErrorV1::CheckedInSourceIdentity {
            expected: WAVE64_COLLECTIVES_V1_SOURCE_SHA256,
            actual: source,
        });
    }
    if identities.attributed_source_sha256 != source {
        return Err(Wave64RefinementErrorV1::SelectedSourceIdentity);
    }

    let kernel_ir_schema = sha256(KERNEL_IR_SCHEMA_BYTES_V1);
    if kernel_ir_schema != WAVE64_COLLECTIVES_V1_KIR_SCHEMA_SHA256 {
        return Err(Wave64RefinementErrorV1::CheckedInKernelIrSchemaIdentity {
            expected: WAVE64_COLLECTIVES_V1_KIR_SCHEMA_SHA256,
            actual: kernel_ir_schema,
        });
    }
    if identities.kernel_ir_schema_sha256 != kernel_ir_schema {
        return Err(Wave64RefinementErrorV1::SelectedKernelIrSchemaIdentity);
    }
    Ok(())
}

fn kir_output(
    ir: &Wave64CollectivesKernelIrV1,
    output: Wave64SemanticOutputV1,
) -> Option<&fe2o3_kernel_ir::Wave64OutputV1> {
    ir.outputs
        .iter()
        .find(|candidate| candidate.argument == output.argument())
}

fn kir_contributor_mask_v1(
    ir: &Wave64CollectivesKernelIrV1,
    output: Wave64SemanticOutputV1,
    lane: usize,
) -> u64 {
    let Some(output) = kir_output(ir, output) else {
        return 0;
    };
    match output.source {
        Wave64CollectiveKindV1::ReduceSum => u64::MAX,
        Wave64CollectiveKindV1::InclusiveScanSum => prefix_mask(lane + 1),
        Wave64CollectiveKindV1::ExclusiveScanSum => prefix_mask(lane),
    }
}

fn verify_symbolic_relation(
    ir: &Wave64CollectivesKernelIrV1,
) -> Result<u32, Wave64RefinementErrorV1> {
    let mut checked = 0_u32;
    for output in Wave64SemanticOutputV1::ALL {
        let Some(kir_output) = kir_output(ir, output) else {
            return Err(Wave64RefinementErrorV1::OutputOwnership { output, lane: 0 });
        };
        for lane in 0..WAVE64_LANES_V1 {
            let source = source_contributor_mask_v1(output, lane);
            let kernel_ir = kir_contributor_mask_v1(ir, output, lane);
            if source != kernel_ir {
                return Err(Wave64RefinementErrorV1::ContributorSet {
                    output,
                    lane,
                    source,
                    kernel_ir,
                });
            }
            let source_owner = lane_outputs_v1(lane).is_some_and(|ownership| match output {
                Wave64SemanticOutputV1::Reduction => ownership.reduction_index == lane,
                Wave64SemanticOutputV1::Inclusive => ownership.inclusive_index == lane,
                Wave64SemanticOutputV1::Exclusive => ownership.exclusive_index == lane,
            });
            if !source_owner
                || kir_output.ownership != Wave64OutputOwnershipV1::PhysicalLaneOwnsSameIndex
            {
                return Err(Wave64RefinementErrorV1::OutputOwnership { output, lane });
            }
            checked += 1;
        }
    }
    Ok(checked)
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
    let sum: i64 = input
        .iter()
        .copied()
        .enumerate()
        .filter(|(lane, _)| selected & (1_u64 << lane) != 0)
        .map(|(_, value)| value as i64)
        .sum();
    sum as f32
}

fn kernel_ir_model(
    input: &[f32],
    active_mask: u64,
    ir: &Wave64CollectivesKernelIrV1,
) -> Wave64SemanticOutputsV1 {
    let evaluate = |output| {
        core::array::from_fn(|lane| {
            if lane_is_active_v1(active_mask, lane) {
                exact_integer_sum(
                    input,
                    active_mask & kir_contributor_mask_v1(ir, output, lane),
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
) -> Result<(), Wave64RefinementErrorV1> {
    for output in Wave64SemanticOutputV1::ALL {
        let source_values = source.values(output);
        let kernel_ir_values = kernel_ir.values(output);
        for lane in 0..WAVE64_LANES_V1 {
            // The semantic KIR uses mathematical integer values on the exact
            // finite corpus. Active +0/-0 therefore compare by value. The
            // source contract requires inactive publication to be +0 bits.
            let equivalent = if lane_is_active_v1(active_mask, lane) {
                source_values[lane] == kernel_ir_values[lane]
            } else {
                source_values[lane].to_bits() == 0.0_f32.to_bits()
                    && kernel_ir_values[lane].to_bits() == 0.0_f32.to_bits()
            };
            if !equivalent {
                return Err(Wave64RefinementErrorV1::SemanticValue {
                    output,
                    lane,
                    source_bits: source_values[lane].to_bits(),
                    kernel_ir_bits: kernel_ir_values[lane].to_bits(),
                });
            }
        }
    }
    Ok(())
}

/// Checks one exact finite-F32 source-model observation against canonical KIR.
///
/// The symbolic contributor check covers all possible active masks; the
/// supplied `active_mask` selects the concrete value observation recorded in
/// the returned inert receipt.
pub fn verify_wave64_source_model_to_kir_v1(
    input: &[f32],
    active_mask: u64,
    ir: &Wave64CollectivesKernelIrV1,
    profile: &Wave64CollectivesProfileV1,
    identities: Wave64RefinementIdentitiesV1,
) -> Result<Wave64SourceKirRefinementV1, Wave64RefinementErrorV1> {
    verify_identities(identities)?;
    verify_wave64_collectives_v1(ir, profile)
        .map_err(Wave64RefinementErrorV1::NonCanonicalKernelIr)?;
    let checked_symbolic_relations = verify_symbolic_relation(ir)?;
    let source = source_model(input, active_mask).map_err(Wave64RefinementErrorV1::SourceModel)?;
    let kernel_ir = kernel_ir_model(input, active_mask, ir);
    compare_semantics(&source, &kernel_ir, active_mask)?;
    Ok(Wave64SourceKirRefinementV1 {
        identities,
        active_mask,
        active_lanes: active_mask.count_ones(),
        checked_symbolic_relations,
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
                Err(Wave64RefinementErrorV1::SemanticValue {
                    output: actual,
                    lane: 17,
                    ..
                }) if actual == output
            ));
        }
    }

    #[test]
    fn inactive_negative_zero_is_rejected() {
        let exact = Wave64SemanticOutputsV1 {
            reduction: [0.0; WAVE64_LANES_V1],
            inclusive: [0.0; WAVE64_LANES_V1],
            exclusive: [0.0; WAVE64_LANES_V1],
        };
        let mut hostile = exact.clone();
        hostile.exclusive[63] = -0.0;
        assert!(matches!(
            compare_semantics(&hostile, &exact, 0),
            Err(Wave64RefinementErrorV1::SemanticValue {
                output: Wave64SemanticOutputV1::Exclusive,
                lane: 63,
                ..
            })
        ));
    }
}

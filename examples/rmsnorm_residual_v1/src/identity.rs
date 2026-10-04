//! Canonical inert algorithm and schedule identities.

use sha2::{Digest, Sha256};

use crate::{EffectContractErrorV1, RmsNormResourceContractV1};
use crate::{
    GFX942_PROCESSOR_V1, GFX942_TARGET_FEATURES_V1, NumericalPolicyErrorV1,
    RMSNORM_REDUCTION_STRIDES_V1, RMSNORM_WAVE_LANES_V1, RmsNormEffectContractV1,
    RmsNormNumericalPolicyV1, RmsNormProfileDescriptorV1, RmsNormProfileErrorV1,
    ValidatedRmsNormProfileV1, validate_effect_contract_v1, validate_numerical_policy_v1,
    validate_rmsnorm_profile_v1,
};

/// Algorithm identity domain separator.
pub const RMSNORM_ALGORITHM_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.rmsnorm_residual.algorithm.v1\0";
/// Schedule identity domain separator.
pub const RMSNORM_SCHEDULE_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.rmsnorm_residual.schedule.v1\0";
/// Candidate identity domain separator.
pub const RMSNORM_CANDIDATE_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.rmsnorm_residual.candidate.v1\0";

/// One SHA-256 identity over a canonical structural record.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StructuralIdentityV1([u8; 32]);

impl StructuralIdentityV1 {
    /// Returns the digest bytes.
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Public inert gfx942 Wave64 schedule record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RmsNormScheduleDescriptorV1 {
    /// Canonical schema version.
    pub schema_version: u16,
    /// Exact processor.
    pub processor: &'static str,
    /// Exact target features.
    pub target_features: &'static str,
    /// Workgroups own this many rows.
    pub rows_per_workgroup: u8,
    /// Exact wave size.
    pub lanes_per_wave: u8,
    /// Exact waves per workgroup.
    pub waves_per_workgroup: u8,
    /// Per-lane column stride.
    pub column_stride: u16,
    /// Fixed reduction stages.
    pub reduction_strides: [u8; 6],
    /// Structural LDS use.
    pub lds_bytes: u32,
    /// Live owners per output element.
    pub output_owners_per_element: u8,
}

impl RmsNormScheduleDescriptorV1 {
    /// Returns the sole accepted schedule record.
    pub const fn exact() -> Self {
        Self {
            schema_version: 1,
            processor: GFX942_PROCESSOR_V1,
            target_features: GFX942_TARGET_FEATURES_V1,
            rows_per_workgroup: 1,
            lanes_per_wave: RMSNORM_WAVE_LANES_V1 as u8,
            waves_per_workgroup: 1,
            column_stride: RMSNORM_WAVE_LANES_V1 as u16,
            reduction_strides: RMSNORM_REDUCTION_STRIDES_V1,
            lds_bytes: 0,
            output_owners_per_element: 1,
        }
    }
}

/// Independent schedule mismatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScheduleErrorV1 {
    /// Schema version changed.
    SchemaVersion,
    /// Processor or target features changed.
    Target,
    /// Row/workgroup mapping changed.
    RowMapping,
    /// Wave geometry changed.
    WaveGeometry,
    /// Column ownership stride changed.
    ColumnStride,
    /// Reduction order changed.
    ReductionOrder,
    /// LDS use changed.
    LdsUse,
    /// Output ownership multiplicity changed.
    OutputOwnership,
}

/// Validates every independent structural schedule axis.
pub fn validate_schedule_descriptor_v1(
    schedule: RmsNormScheduleDescriptorV1,
) -> Result<(), ScheduleErrorV1> {
    let exact = RmsNormScheduleDescriptorV1::exact();
    if schedule.schema_version != exact.schema_version {
        return Err(ScheduleErrorV1::SchemaVersion);
    }
    if schedule.processor != exact.processor || schedule.target_features != exact.target_features {
        return Err(ScheduleErrorV1::Target);
    }
    if schedule.rows_per_workgroup != exact.rows_per_workgroup {
        return Err(ScheduleErrorV1::RowMapping);
    }
    if schedule.lanes_per_wave != exact.lanes_per_wave
        || schedule.waves_per_workgroup != exact.waves_per_workgroup
    {
        return Err(ScheduleErrorV1::WaveGeometry);
    }
    if schedule.column_stride != exact.column_stride {
        return Err(ScheduleErrorV1::ColumnStride);
    }
    if schedule.reduction_strides != exact.reduction_strides {
        return Err(ScheduleErrorV1::ReductionOrder);
    }
    if schedule.lds_bytes != exact.lds_bytes {
        return Err(ScheduleErrorV1::LdsUse);
    }
    if schedule.output_owners_per_element != exact.output_owners_per_element {
        return Err(ScheduleErrorV1::OutputOwnership);
    }
    Ok(())
}

/// Complete public record used to request an inert structural candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RmsNormCandidateDescriptorV1 {
    /// Candidate schema version.
    pub schema_version: u16,
    /// Algorithm discriminator.
    pub algorithm: &'static str,
    /// Exact Qwen3/B3 profile.
    pub profile: RmsNormProfileDescriptorV1,
    /// Exact numerical policy.
    pub numerical: RmsNormNumericalPolicyV1,
    /// Exact memory/effect policy.
    pub effects: RmsNormEffectContractV1,
    /// Exact Wave64 structural schedule.
    pub schedule: RmsNormScheduleDescriptorV1,
}

impl RmsNormCandidateDescriptorV1 {
    /// Constructs the canonical record for one exact profile.
    pub const fn canonical(profile: RmsNormProfileDescriptorV1) -> Self {
        Self {
            schema_version: 1,
            algorithm: "qwen3-bf16-rmsnorm-plus-residual",
            profile,
            numerical: RmsNormNumericalPolicyV1::exact(),
            effects: RmsNormEffectContractV1::exact(),
            schedule: RmsNormScheduleDescriptorV1::exact(),
        }
    }
}

/// Fail-closed candidate validation error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CandidateErrorV1 {
    /// Candidate schema version changed.
    SchemaVersion,
    /// Algorithm discriminator changed.
    Algorithm,
    /// Exact B3 profile validation failed.
    Profile(RmsNormProfileErrorV1),
    /// Numerical-policy validation failed.
    Numerical(NumericalPolicyErrorV1),
    /// Effect-contract validation failed.
    Effects(EffectContractErrorV1),
    /// Structural schedule validation failed.
    Schedule(ScheduleErrorV1),
}

/// Validated, copyable, inert structural candidate.
///
/// Copying this record copies no compiler custody, proof evidence, artifact,
/// load handle, or launch capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuralRmsNormCandidateV1 {
    profile: ValidatedRmsNormProfileV1,
    schedule: RmsNormScheduleDescriptorV1,
    algorithm_identity: StructuralIdentityV1,
    schedule_identity: StructuralIdentityV1,
    candidate_identity: StructuralIdentityV1,
}

impl StructuralRmsNormCandidateV1 {
    /// Returns the exact validated profile.
    pub const fn profile(self) -> ValidatedRmsNormProfileV1 {
        self.profile
    }

    /// Returns the checked resource contract.
    pub const fn resources(self) -> RmsNormResourceContractV1 {
        self.profile.resources()
    }

    /// Returns the inert structural schedule.
    pub const fn schedule(self) -> RmsNormScheduleDescriptorV1 {
        self.schedule
    }

    /// Returns the algorithm-record identity.
    pub const fn algorithm_identity(self) -> StructuralIdentityV1 {
        self.algorithm_identity
    }

    /// Returns the schedule-record identity, which binds the algorithm ID.
    pub const fn schedule_identity(self) -> StructuralIdentityV1 {
        self.schedule_identity
    }

    /// Returns the complete candidate-record identity.
    pub const fn candidate_identity(self) -> StructuralIdentityV1 {
        self.candidate_identity
    }

    /// Reports that this inert record carries no production authority.
    pub const fn grants_production_authority(self) -> bool {
        false
    }
}

fn hash_fields(domain: &[u8], fields: &[&[u8]]) -> StructuralIdentityV1 {
    let mut hash = Sha256::new();
    hash.update(domain);
    for field in fields {
        hash.update((field.len() as u64).to_le_bytes());
        hash.update(field);
    }
    StructuralIdentityV1(hash.finalize().into())
}

fn algorithm_identity(descriptor: RmsNormCandidateDescriptorV1) -> StructuralIdentityV1 {
    let profile = descriptor.profile;
    let numerical = descriptor.numerical;
    hash_fields(
        RMSNORM_ALGORITHM_IDENTITY_DOMAIN_V1,
        &[
            descriptor.algorithm.as_bytes(),
            &[profile.role.identity_tag()],
            &[profile.bucket.identity_tag()],
            &(profile.sequences as u64).to_le_bytes(),
            &(profile.active_tokens as u64).to_le_bytes(),
            &(profile.rows as u64).to_le_bytes(),
            &(profile.hidden_size as u64).to_le_bytes(),
            &numerical.epsilon_bits.to_le_bytes(),
            &[numerical.residual_add as u8],
            &[numerical.square_reduction as u8],
            &[numerical.reciprocal_root as u8],
            &[numerical.scale_order as u8],
            &[numerical.output_cast as u8],
            &[u8::from(numerical.reject_non_finite)],
        ],
    )
}

fn schedule_identity(
    algorithm: StructuralIdentityV1,
    descriptor: RmsNormCandidateDescriptorV1,
) -> StructuralIdentityV1 {
    let schedule = descriptor.schedule;
    hash_fields(
        RMSNORM_SCHEDULE_IDENTITY_DOMAIN_V1,
        &[
            &algorithm.bytes(),
            schedule.processor.as_bytes(),
            schedule.target_features.as_bytes(),
            &schedule.schema_version.to_le_bytes(),
            &[schedule.rows_per_workgroup],
            &[schedule.lanes_per_wave],
            &[schedule.waves_per_workgroup],
            &schedule.column_stride.to_le_bytes(),
            &schedule.reduction_strides,
            &schedule.lds_bytes.to_le_bytes(),
            &[schedule.output_owners_per_element],
        ],
    )
}

fn candidate_identity(
    algorithm: StructuralIdentityV1,
    schedule: StructuralIdentityV1,
    descriptor: RmsNormCandidateDescriptorV1,
) -> StructuralIdentityV1 {
    let effects = descriptor.effects;
    hash_fields(
        RMSNORM_CANDIDATE_IDENTITY_DOMAIN_V1,
        &[
            &algorithm.bytes(),
            &schedule.bytes(),
            &descriptor.schema_version.to_le_bytes(),
            &[effects.initialized_read_buffers],
            &[effects.write_buffers],
            &[u8::from(effects.read_only_inputs_may_alias)],
            &[u8::from(effects.writable_outputs_are_disjoint)],
            &[u8::from(effects.output_mapping_is_total_and_injective)],
            &[u8::from(effects.wave_collectives_are_convergent)],
            &[u8::from(effects.output_commit_is_transactional)],
            &[u8::from(effects.accesses_are_bounded)],
        ],
    )
}

/// Validates and identity-binds one inert candidate record.
pub fn validate_structural_candidate_v1(
    descriptor: RmsNormCandidateDescriptorV1,
) -> Result<StructuralRmsNormCandidateV1, CandidateErrorV1> {
    if descriptor.schema_version != 1 {
        return Err(CandidateErrorV1::SchemaVersion);
    }
    if descriptor.algorithm != "qwen3-bf16-rmsnorm-plus-residual" {
        return Err(CandidateErrorV1::Algorithm);
    }
    let profile =
        validate_rmsnorm_profile_v1(descriptor.profile).map_err(CandidateErrorV1::Profile)?;
    validate_numerical_policy_v1(descriptor.numerical).map_err(CandidateErrorV1::Numerical)?;
    validate_effect_contract_v1(descriptor.effects).map_err(CandidateErrorV1::Effects)?;
    validate_schedule_descriptor_v1(descriptor.schedule).map_err(CandidateErrorV1::Schedule)?;

    let algorithm_identity = algorithm_identity(descriptor);
    let schedule_identity = schedule_identity(algorithm_identity, descriptor);
    let candidate_identity = candidate_identity(algorithm_identity, schedule_identity, descriptor);
    Ok(StructuralRmsNormCandidateV1 {
        profile,
        schedule: descriptor.schedule,
        algorithm_identity,
        schedule_identity,
        candidate_identity,
    })
}

//! Two changed-content sharded vecadd authorizations on stable child allocation IDs.
//!
//! This independent hardware-qualification policy composes the two unchanged
//! one-shot recipes. It trusts the finite embedded artifact's semantics, not
//! general production kernels or authenticated output lineage. Phase advancement
//! records authorization only; the caller must observe successful completion,
//! release consumers and refresh every input byte before the second batch.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use sha2::{Digest, Sha256};

use crate::qualification_gfx942_sharded_vecadd_v1 as one_shot;
use crate::{
    KfdRuntimeAuthorityRequestV1, RuntimeAllocationIdV1, RuntimeArgumentsV1, RuntimeBindingV1,
    RuntimeLaunchGeometryV1,
};

pub use one_shot::{
    Gfx942ShardedVecaddQualificationAdmissionErrorV1 as Gfx942ShardedVecaddRoundsQualificationAdmissionErrorV1,
    Gfx942ShardedVecaddQualificationFixtureErrorV1 as Gfx942ShardedVecaddRoundsQualificationFixtureErrorV1,
    Gfx942ShardedVecaddQualificationHostBuffersV1 as Gfx942ShardedVecaddRoundsQualificationHostBuffersV1,
    Gfx942ShardedVecaddQualificationRecipeV1 as Gfx942ShardedVecaddRoundsQualificationRecipeV1,
};

pub const GFX942_SHARDED_VECADD_ROUNDS_QUALIFICATION_PROFILE_ID_V1: &str =
    "fe2o3.runtime.gfx942-sharded-vecadd-rounds-qualification.v1";
pub const GFX942_SHARDED_VECADD_ROUNDS_QUALIFICATION_POLICY_SHA256_V1: [u8; 32] = [
    0xa6, 0xd7, 0x1c, 0xaa, 0x1a, 0xb8, 0x57, 0x39, 0xa3, 0x70, 0x76, 0x76, 0xb1, 0xc5, 0x6e, 0x9c,
    0x82, 0x19, 0x22, 0x01, 0xf1, 0xae, 0x23, 0x86, 0x2d, 0x63, 0x01, 0x00, 0xc7, 0xdd, 0xd6, 0x1d,
];
pub const GFX942_SHARDED_VECADD_ROUNDS_QUALIFICATION_SIGNATURE_V1: [u8; 32] =
    GFX942_SHARDED_VECADD_ROUNDS_QUALIFICATION_POLICY_SHA256_V1;

const POLICY: &[u8] =
    include_bytes!("../fixtures/trusted-gfx942-sharded-vecadd-rounds-v1/policy-v1.txt");
const INNER_POLICY_SHA256: [u8; 32] = [
    0xe1, 0xd3, 0x5c, 0x9a, 0x26, 0x6d, 0x0c, 0x66, 0x34, 0x7d, 0xf1, 0x89, 0x2c, 0xcf, 0x7d, 0xf8,
    0x81, 0x1b, 0x45, 0x73, 0x56, 0xe7, 0xb0, 0x28, 0xfb, 0x40, 0x6d, 0x71, 0xca, 0x06, 0x10, 0x15,
];

pub const fn gfx942_sharded_vecadd_rounds_qualification_policy_v1() -> &'static [u8] {
    POLICY
}

/// The existing checked encoding with a distinct two-round policy signature.
#[derive(Debug)]
pub struct Gfx942ShardedVecaddRoundsQualificationArgumentsV1(
    one_shot::Gfx942ShardedVecaddQualificationArgumentsV1,
);

impl Gfx942ShardedVecaddRoundsQualificationArgumentsV1 {
    pub fn new(
        recipe: Gfx942ShardedVecaddRoundsQualificationRecipeV1,
        left: RuntimeAllocationIdV1,
        right: RuntimeAllocationIdV1,
        output: RuntimeAllocationIdV1,
    ) -> Result<Self, Gfx942ShardedVecaddRoundsQualificationFixtureErrorV1> {
        one_shot::Gfx942ShardedVecaddQualificationArgumentsV1::new(recipe, left, right, output)
            .map(Self)
    }

    pub const fn recipe(&self) -> Gfx942ShardedVecaddRoundsQualificationRecipeV1 {
        self.0.recipe()
    }

    pub const fn allocations(&self) -> [RuntimeAllocationIdV1; 3] {
        self.0.allocations()
    }
}

impl RuntimeArgumentsV1 for Gfx942ShardedVecaddRoundsQualificationArgumentsV1 {
    const SIGNATURE_V1: [u8; 32] = GFX942_SHARDED_VECADD_ROUNDS_QUALIFICATION_SIGNATURE_V1;

    fn encode_explicit_kernarg_v1(&self) -> Vec<u8> {
        self.0.encode_explicit_kernarg_v1()
    }

    fn bindings_v1(&self) -> Vec<RuntimeBindingV1> {
        self.0.bindings_v1()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    First,
    Second { allocations: [u64; 3] },
    Exhausted,
}

#[derive(Debug)]
struct AuthorityState {
    calls: AtomicU64,
    phase: Mutex<Phase>,
}

/// Stored authorization counts, never completion or reusable-storage evidence.
#[derive(Clone, Debug)]
pub struct Gfx942ShardedVecaddRoundsQualificationAuthorityObservationV1 {
    state: Arc<AuthorityState>,
}

impl Gfx942ShardedVecaddRoundsQualificationAuthorityObservationV1 {
    pub fn authorization_calls_v1(&self) -> u64 {
        self.state.calls.load(Ordering::Acquire)
    }

    /// Returns zero, one or two accepted authorizations, or `None` after poison.
    pub fn accepted_rounds_v1(&self) -> Option<usize> {
        self.state.phase.lock().ok().map(|phase| match *phase {
            Phase::First => 0,
            Phase::Second { .. } => 1,
            Phase::Exhausted => 2,
        })
    }
}

/// Non-cloneable, irreversible two-round authority for one indexed child.
#[derive(Debug)]
pub struct AdmittedGfx942ShardedVecaddRoundsQualificationV1 {
    rounds: [one_shot::AdmittedGfx942ShardedVecaddQualificationV1; 2],
    state: Arc<AuthorityState>,
}

impl AdmittedGfx942ShardedVecaddRoundsQualificationV1 {
    pub fn recipe(
        &self,
        round: usize,
    ) -> Result<
        Gfx942ShardedVecaddRoundsQualificationRecipeV1,
        Gfx942ShardedVecaddRoundsQualificationAdmissionErrorV1,
    > {
        self.rounds
            .get(round)
            .map(|admitted| admitted.recipe())
            .ok_or(Gfx942ShardedVecaddRoundsQualificationAdmissionErrorV1::Recipe)
    }

    pub const fn hsaco(&self) -> &'static [u8] {
        self.rounds[0].hsaco()
    }

    pub const fn hsaco_sha256(&self) -> [u8; 32] {
        self.rounds[0].hsaco_sha256()
    }

    pub const fn kernel_name(&self) -> &'static str {
        self.rounds[0].kernel_name()
    }

    pub const fn signature(&self) -> [u8; 32] {
        GFX942_SHARDED_VECADD_ROUNDS_QUALIFICATION_SIGNATURE_V1
    }

    pub const fn geometry(&self) -> RuntimeLaunchGeometryV1 {
        self.rounds[0].geometry()
    }

    pub fn explicit_kernarg(&self) -> [u8; 48] {
        self.rounds[0].explicit_kernarg()
    }

    pub fn host_buffers(
        &self,
        round: usize,
    ) -> Result<
        Gfx942ShardedVecaddRoundsQualificationHostBuffersV1,
        Gfx942ShardedVecaddRoundsQualificationAdmissionErrorV1,
    > {
        self.recipe(round)?
            .host_buffers()
            .map_err(|_| Gfx942ShardedVecaddRoundsQualificationAdmissionErrorV1::Capacity)
    }

    pub fn observation_v1(&self) -> Gfx942ShardedVecaddRoundsQualificationAuthorityObservationV1 {
        Gfx942ShardedVecaddRoundsQualificationAuthorityObservationV1 {
            state: Arc::clone(&self.state),
        }
    }

    pub(crate) fn authorizes_kfd_request_v1(
        &self,
        request: KfdRuntimeAuthorityRequestV1<'_>,
    ) -> bool {
        if self
            .state
            .calls
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |calls| {
                calls.checked_add(1)
            })
            .is_err()
            || request.signature != self.signature()
            || request.bindings.len() != 3
        {
            return false;
        }
        let allocations = core::array::from_fn(|index| request.bindings[index].region.allocation);
        let Ok(mut phase) = self.state.phase.lock() else {
            return false;
        };
        let round = match *phase {
            Phase::First => 0,
            Phase::Second {
                allocations: original,
            } if original == allocations => 1,
            Phase::Second { .. } | Phase::Exhausted => return false,
        };
        // Only the already-authenticated policy signature changes. The selected
        // one-shot gate checks every remaining request field without exceptions.
        if !self.rounds[round].authorizes_kfd_request_v1(KfdRuntimeAuthorityRequestV1 {
            signature: self.rounds[round].signature(),
            ..request
        }) {
            return false;
        }
        // No fallible work follows inner acceptance. An unwind while holding
        // this lock poisons the outer authority instead of reopening a phase.
        *phase = if round == 0 {
            Phase::Second { allocations }
        } else {
            Phase::Exhausted
        };
        true
    }
}

fn validate_policy_v1(
    policy: &[u8],
    inner_policy: &[u8],
) -> Result<(), Gfx942ShardedVecaddRoundsQualificationAdmissionErrorV1> {
    if <[u8; 32]>::from(Sha256::digest(policy))
        != GFX942_SHARDED_VECADD_ROUNDS_QUALIFICATION_POLICY_SHA256_V1
        || <[u8; 32]>::from(Sha256::digest(inner_policy)) != INNER_POLICY_SHA256
    {
        return Err(Gfx942ShardedVecaddRoundsQualificationAdmissionErrorV1::Identity);
    }
    Ok(())
}

/// Admits both unchanged artifact/ABI/recipe closures before any native device
/// opens. The two-round policy is additional, not a relaxation of either gate.
pub fn admit_gfx942_sharded_vecadd_rounds_qualification_v1(
    count: usize,
    index: usize,
) -> Result<
    AdmittedGfx942ShardedVecaddRoundsQualificationV1,
    Gfx942ShardedVecaddRoundsQualificationAdmissionErrorV1,
> {
    validate_policy_v1(
        POLICY,
        one_shot::gfx942_sharded_vecadd_qualification_policy_v1(),
    )?;
    let rounds = [
        one_shot::admit_gfx942_sharded_vecadd_qualification_v1(count, index, 0)?,
        one_shot::admit_gfx942_sharded_vecadd_qualification_v1(count, index, 1)?,
    ];
    Ok(AdmittedGfx942ShardedVecaddRoundsQualificationV1 {
        rounds,
        state: Arc::new(AuthorityState {
            calls: AtomicU64::new(0),
            phase: Mutex::new(Phase::First),
        }),
    })
}

#[cfg(test)]
mod tests;

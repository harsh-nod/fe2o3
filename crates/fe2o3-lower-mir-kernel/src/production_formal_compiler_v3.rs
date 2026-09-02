//! Exact composition custody for the bounded Formal Compiler V3 fragment.
//!
//! This layer joins independently replayed CFG/value and byte-memory evidence
//! only when every retained source, semantic, KIR, call-result, and operation
//! locator agrees. It is optional compiler-correctness evidence and grants no
//! lowering, artifact, publication, load, or launch authority.

use std::{error::Error, fmt};

use sha2::{Digest, Sha256};

use crate::{
    FORMAL_COMPILER_V3_BRANCH_ARMS, FORMAL_COMPILER_V3_BYTE_WIDTH, FORMAL_COMPILER_V3_CLAIM_NAME,
    FORMAL_COMPILER_V3_CONTRACT_SHA256, FORMAL_COMPILER_V3_DISJOINT_WRITES,
    FORMAL_COMPILER_V3_DYNAMIC_EXTENTS, FORMAL_COMPILER_V3_GUARD_PREDICATES,
    FORMAL_COMPILER_V3_HELPER_PARAMETERS, FORMAL_COMPILER_V3_READONLY_ACCESSES,
    FORMAL_COMPILER_V3_WORD_BITS, InertMirKirXorCfgEvidenceV3, MirKirXorCfgBindingsV3,
    MirKirXorCfgErrorV3, MirKirXorCfgStatusV3, ProductionFormalMemoryOwnerV1,
    ProductionMemoryTraceEvidenceV3, ProductionMemoryTraceSelectorV3,
    ProductionMemoryTraceStatusV3,
};

/// Version of the exact lower-compiler composition policy.
pub const FORMAL_LOWER_COMPILER_COMPOSITION_POLICY_VERSION_V3: u16 = 3;

const EVIDENCE_DOMAIN_V3: &[u8] = b"FE2O3/FORMAL-COMPILER/COMPOSED-EVIDENCE/V3\0";

/// Optional coverage status for the exact lower-compiler fragment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionLowerCompilerStatusV3 {
    /// Neither exact classifier recognizes the owner.
    NotApplicable,
    /// Only part of the exact claim is available. This is never a proof.
    Incomplete {
        /// Whether exact helper CFG/value evidence was present.
        cfg_value_verified: bool,
        /// Whether exact guarded byte-memory evidence was present.
        memory_trace_verified: bool,
    },
    /// Both claims were replayed and their complete live bindings agree.
    Proved(ProductionLowerCompilerEvidenceV3),
}

impl ProductionLowerCompilerStatusV3 {
    /// Replays both classifiers against the same move-only production owner.
    pub fn from_live_owner(
        owner: &ProductionFormalMemoryOwnerV1,
    ) -> Result<Self, ProductionLowerCompilerErrorV3> {
        owner
            .verify_equivalence()
            .map_err(|error| ProductionLowerCompilerErrorV3::LiveOwner(error.to_string()))?;
        let cfg = MirKirXorCfgStatusV3::from_live_owner(owner.semantic_kir())
            .map_err(ProductionLowerCompilerErrorV3::CfgValue)?;
        let memory = ProductionMemoryTraceStatusV3::from_live_owner(owner)
            .map_err(ProductionLowerCompilerErrorV3::MemoryTrace)?;
        match (cfg, memory) {
            (MirKirXorCfgStatusV3::NotEligible, ProductionMemoryTraceStatusV3::NotEligible) => {
                Ok(Self::NotApplicable)
            }
            (
                MirKirXorCfgStatusV3::Verified(cfg),
                ProductionMemoryTraceStatusV3::Verified(memory),
            ) => Ok(Self::Proved(ProductionLowerCompilerEvidenceV3::new(
                owner, cfg, memory,
            )?)),
            (MirKirXorCfgStatusV3::NotEligible, ProductionMemoryTraceStatusV3::Verified(_)) => {
                Err(ProductionLowerCompilerErrorV3::ClassifierInconsistency)
            }
            (cfg, memory) => Ok(Self::Incomplete {
                cfg_value_verified: cfg.evidence().is_some(),
                memory_trace_verified: memory.evidence().is_some(),
            }),
        }
    }

    /// Replays the same owner and rejects a changed status or payload.
    pub fn revalidate_against(
        &self,
        owner: &ProductionFormalMemoryOwnerV1,
    ) -> Result<(), ProductionLowerCompilerErrorV3> {
        (Self::from_live_owner(owner)? == *self)
            .then_some(())
            .ok_or(ProductionLowerCompilerErrorV3::NonCanonicalEvidence)
    }

    /// Returns composed evidence only for the fully proved exact fragment.
    pub const fn evidence(&self) -> Option<&ProductionLowerCompilerEvidenceV3> {
        match self {
            Self::Proved(evidence) => Some(evidence),
            Self::NotApplicable | Self::Incomplete { .. } => None,
        }
    }

    /// Optional compiler-correctness evidence grants no authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

/// Authority-free joined evidence for exact CFG/value and memory refinement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionLowerCompilerEvidenceV3 {
    identity: [u8; 32],
    cfg: InertMirKirXorCfgEvidenceV3,
    memory: ProductionMemoryTraceEvidenceV3,
}

impl ProductionLowerCompilerEvidenceV3 {
    fn new(
        owner: &ProductionFormalMemoryOwnerV1,
        cfg: InertMirKirXorCfgEvidenceV3,
        memory: ProductionMemoryTraceEvidenceV3,
    ) -> Result<Self, ProductionLowerCompilerErrorV3> {
        cfg.revalidate()
            .map_err(ProductionLowerCompilerErrorV3::CfgValue)?;
        memory
            .revalidate()
            .map_err(ProductionLowerCompilerErrorV3::MemoryTrace)?;
        validate_cross_track_bindings_v3(owner, &cfg, &memory)?;
        let identity = composed_identity_v3(&cfg, &memory);
        let evidence = Self {
            identity,
            cfg,
            memory,
        };
        evidence.revalidate()?;
        Ok(evidence)
    }

    /// Rechecks both proof payloads and their exact cross-track relation.
    pub fn revalidate(&self) -> Result<(), ProductionLowerCompilerErrorV3> {
        self.cfg
            .revalidate()
            .map_err(ProductionLowerCompilerErrorV3::CfgValue)?;
        self.memory
            .revalidate()
            .map_err(ProductionLowerCompilerErrorV3::MemoryTrace)?;
        if self.identity != composed_identity_v3(&self.cfg, &self.memory) {
            return Err(ProductionLowerCompilerErrorV3::NonCanonicalEvidence);
        }
        Ok(())
    }

    /// Returns the domain-separated identity of the complete joined payload.
    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }

    /// Borrows the exact helper CFG/value evidence.
    pub const fn cfg_value(&self) -> &InertMirKirXorCfgEvidenceV3 {
        &self.cfg
    }

    /// Borrows the exact guarded byte-memory evidence.
    pub const fn memory_trace(&self) -> &ProductionMemoryTraceEvidenceV3 {
        &self.memory
    }

    /// This bounded evidence does not claim whole-compiler correctness.
    pub const fn claims_general_compiler_correctness(&self) -> bool {
        false
    }

    /// Joined evidence grants no lowering, artifact, or launch authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

fn validate_cross_track_bindings_v3(
    owner: &ProductionFormalMemoryOwnerV1,
    cfg: &InertMirKirXorCfgEvidenceV3,
    memory: &ProductionMemoryTraceEvidenceV3,
) -> Result<(), ProductionLowerCompilerErrorV3> {
    let bindings = cfg.bindings();
    let selector = memory.selector();
    let memory_locations = memory.memory_locations();
    let call_location = memory.root_call_location();
    let kir_identity = owner.semantic_kir().canonical_kernel_ir_identity();
    let semantic_sha256 = *owner
        .semantic_kir()
        .semantic()
        .semantic()
        .semantic_sha256()
        .as_bytes();

    if cfg.semantic_mir_sha256() != &semantic_sha256
        || memory.semantic_mir_sha256() != &semantic_sha256
        || cfg.canonical_kernel_ir_identity() != kir_identity
        || memory.canonical_kernel_ir_identity() != kir_identity
        || cfg.grants_authority()
        || memory.grants_artifact_or_launch_authority()
    {
        return Err(ProductionLowerCompilerErrorV3::CrossTrackBinding);
    }
    exact_fragment_locators_match_v3(
        bindings,
        selector,
        memory_locations,
        call_location,
        memory.semantic_root_values(),
        memory.semantic_helper_values(),
        memory.kir_root_values(),
        memory.kir_helper_values(),
        memory.fallback(),
        cfg.fallback(),
    )
}

#[allow(clippy::too_many_arguments)]
fn exact_fragment_locators_match_v3(
    bindings: MirKirXorCfgBindingsV3,
    selector: ProductionMemoryTraceSelectorV3,
    memory_locations: [fe2o3_kernel_ir::FunctionOperationLocation; 3],
    call_location: fe2o3_kernel_ir::FunctionOperationLocation,
    semantic_root_values: [u32; 3],
    semantic_helper_values: [u32; 4],
    kir_root_values: [fe2o3_kernel_ir::ValueId; 3],
    kir_helper_values: [fe2o3_kernel_ir::ValueId; 5],
    memory_fallback: u32,
    cfg_fallback: u32,
) -> Result<(), ProductionLowerCompilerErrorV3> {
    let location = |site: fe2o3_kernel_ir::FunctionOperationLocation| {
        u32::try_from(site.operation_index)
            .map(|ordinal| [site.block.0, ordinal])
            .map_err(|_| ProductionLowerCompilerErrorV3::CrossTrackBinding)
    };
    if bindings.semantic_functions != [selector.root_function, selector.helper_function]
        || bindings.semantic_root_load_sites
            != [
                [selector.first_load.0, selector.first_load.1],
                [selector.second_load.0, selector.second_load.1],
            ]
        || bindings.semantic_root_blocks != [selector.helper_call_block, selector.store.0]
        || semantic_root_values != bindings.semantic_root_values
        || semantic_helper_values != bindings.semantic_helper_values
        || kir_root_values != bindings.kir_root_values
        || kir_helper_values != bindings.kir_helper_values
        || memory_fallback != cfg_fallback
        || [
            location(memory_locations[0])?,
            location(memory_locations[1])?,
        ] != bindings.kir_root_load_sites
        || location(call_location)? != bindings.kir_root_call_site
        || [call_location.block.0, memory_locations[2].block.0] != bindings.kir_root_blocks
    {
        return Err(ProductionLowerCompilerErrorV3::CrossTrackBinding);
    }
    Ok(())
}

fn composed_identity_v3(
    cfg: &InertMirKirXorCfgEvidenceV3,
    memory: &ProductionMemoryTraceEvidenceV3,
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(EVIDENCE_DOMAIN_V3);
    hash.update(FORMAL_LOWER_COMPILER_COMPOSITION_POLICY_VERSION_V3.to_le_bytes());
    hash.update(FORMAL_COMPILER_V3_CLAIM_NAME.as_bytes());
    hash.update(FORMAL_COMPILER_V3_CONTRACT_SHA256);
    hash.update(FORMAL_COMPILER_V3_WORD_BITS.to_le_bytes());
    hash.update(FORMAL_COMPILER_V3_BYTE_WIDTH.to_le_bytes());
    hash.update(FORMAL_COMPILER_V3_HELPER_PARAMETERS.to_le_bytes());
    hash.update(FORMAL_COMPILER_V3_BRANCH_ARMS.to_le_bytes());
    hash.update(FORMAL_COMPILER_V3_READONLY_ACCESSES.to_le_bytes());
    hash.update(FORMAL_COMPILER_V3_DISJOINT_WRITES.to_le_bytes());
    hash.update(FORMAL_COMPILER_V3_DYNAMIC_EXTENTS.to_le_bytes());
    hash.update(FORMAL_COMPILER_V3_GUARD_PREDICATES.to_le_bytes());
    hash.update(cfg.identity());
    hash.update(memory.identity());
    hash.finalize().into()
}

/// Fail-closed exact V3 composition error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionLowerCompilerErrorV3 {
    /// The shared live owner failed replay.
    LiveOwner(String),
    /// Exact helper CFG/value classification failed.
    CfgValue(MirKirXorCfgErrorV3),
    /// Exact guarded byte-memory classification failed.
    MemoryTrace(crate::MemoryTraceRefinementErrorV3),
    /// Two individually valid payloads did not name the same live fragment.
    CrossTrackBinding,
    /// Memory recognized the exact helper relation but the CFG classifier did not.
    ClassifierInconsistency,
    /// A retained identity or status changed during replay.
    NonCanonicalEvidence,
}

impl fmt::Display for ProductionLowerCompilerErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LiveOwner(error) => write!(formatter, "live owner failed: {error}"),
            Self::CfgValue(error) => write!(formatter, "CFG/value evidence failed: {error}"),
            Self::MemoryTrace(error) => write!(formatter, "memory-trace evidence failed: {error}"),
            Self::CrossTrackBinding => {
                formatter.write_str("CFG/value and memory evidence name different live fragments")
            }
            Self::ClassifierInconsistency => formatter.write_str(
                "memory evidence recognized an exact helper that CFG/value custody rejected",
            ),
            Self::NonCanonicalEvidence => {
                formatter.write_str("composed Formal Compiler V3 evidence is noncanonical")
            }
        }
    }
}

impl Error for ProductionLowerCompilerErrorV3 {}

#[cfg(test)]
mod tests {
    use fe2o3_kernel_ir::{BlockId, FunctionOperationLocation, ValueId};

    use super::*;

    fn exact_inputs() -> (
        MirKirXorCfgBindingsV3,
        ProductionMemoryTraceSelectorV3,
        [FunctionOperationLocation; 3],
        FunctionOperationLocation,
    ) {
        let bindings = MirKirXorCfgBindingsV3 {
            semantic_functions: [0, 1],
            semantic_root_values: [10, 11, 12],
            semantic_root_load_sites: [[3, 0], [3, 1]],
            semantic_helper_values: [20, 21, 22, 23],
            semantic_root_blocks: [3, 4],
            semantic_helper_blocks: [0, 1, 2, 3],
            kir_root_values: [ValueId(30), ValueId(31), ValueId(32)],
            kir_root_load_sites: [[13, 0], [13, 1]],
            kir_root_call_site: [13, 2],
            kir_helper_values: [
                ValueId(40),
                ValueId(41),
                ValueId(42),
                ValueId(43),
                ValueId(44),
            ],
            kir_root_blocks: [13, 14],
            kir_helper_blocks: [20, 21, 22, 23],
        };
        let selector = ProductionMemoryTraceSelectorV3 {
            root_function: 0,
            guard_blocks: [0, 1, 2],
            enabled_block: 3,
            first_load: (3, 0),
            second_load: (3, 1),
            helper_call_block: 3,
            store: (4, 0),
            helper_function: 1,
        };
        let memory = [
            FunctionOperationLocation::new(BlockId(13), 0),
            FunctionOperationLocation::new(BlockId(13), 1),
            FunctionOperationLocation::new(BlockId(14), 0),
        ];
        let call = FunctionOperationLocation::new(BlockId(13), 2);
        (bindings, selector, memory, call)
    }

    fn check(
        bindings: MirKirXorCfgBindingsV3,
        selector: ProductionMemoryTraceSelectorV3,
        memory: [FunctionOperationLocation; 3],
        call: FunctionOperationLocation,
    ) -> Result<(), ProductionLowerCompilerErrorV3> {
        exact_fragment_locators_match_v3(
            bindings,
            selector,
            memory,
            call,
            [10, 11, 12],
            [20, 21, 22, 23],
            [ValueId(30), ValueId(31), ValueId(32)],
            [
                ValueId(40),
                ValueId(41),
                ValueId(42),
                ValueId(43),
                ValueId(44),
            ],
            99,
            99,
        )
    }

    #[test]
    fn exact_cfg_and_memory_bindings_compose() {
        let (bindings, selector, memory, call) = exact_inputs();
        check(bindings, selector, memory, call).unwrap();
    }

    #[test]
    fn optional_status_never_grants_artifact_or_launch_authority() {
        assert!(
            !ProductionLowerCompilerStatusV3::NotApplicable.grants_artifact_or_launch_authority()
        );
        assert!(
            !ProductionLowerCompilerStatusV3::Incomplete {
                cfg_value_verified: true,
                memory_trace_verified: false,
            }
            .grants_artifact_or_launch_authority()
        );
    }

    #[test]
    fn substitutions_do_not_compose() {
        let (bindings, mut selector, memory, call) = exact_inputs();
        selector.store = (5, 0);
        assert_eq!(
            check(bindings, selector, memory, call),
            Err(ProductionLowerCompilerErrorV3::CrossTrackBinding)
        );

        let (mut bindings, selector, memory, call) = exact_inputs();
        bindings.kir_root_values[2] = ValueId(99);
        assert_eq!(
            check(bindings, selector, memory, call),
            Err(ProductionLowerCompilerErrorV3::CrossTrackBinding)
        );

        let (bindings, selector, memory, _) = exact_inputs();
        let wrong_call = FunctionOperationLocation::new(BlockId(13), 1);
        assert_eq!(
            check(bindings, selector, memory, wrong_call),
            Err(ProductionLowerCompilerErrorV3::CrossTrackBinding)
        );

        let (mut bindings, selector, memory, call) = exact_inputs();
        bindings.semantic_root_load_sites.swap(0, 1);
        assert_eq!(
            check(bindings, selector, memory, call),
            Err(ProductionLowerCompilerErrorV3::CrossTrackBinding)
        );
    }
}

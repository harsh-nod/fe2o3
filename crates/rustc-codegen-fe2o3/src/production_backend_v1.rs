//! Target-neutral production backend boundary.
//!
//! The production pipeline owns only the records in this module. Concrete
//! target profiles, capability legalization, and object-format lowering stay
//! behind the sealed adapter implementation.

use std::fmt;

use fe2o3_amd_target::{
    PRODUCTION_AMDHSA_LLVM22_WORKER_DATA_LAYOUT_V1, PRODUCTION_AMDHSA_RUSTC_DATA_LAYOUT_V1,
    ProductionAmdTargetProfileV1,
};

pub(crate) const PRODUCTION_RUSTC_DATA_LAYOUT_V1: &str = PRODUCTION_AMDHSA_RUSTC_DATA_LAYOUT_V1;
pub(crate) const PRODUCTION_WORKER_DATA_LAYOUT_V1: &str =
    PRODUCTION_AMDHSA_LLVM22_WORKER_DATA_LAYOUT_V1;

#[cfg(test)]
const SYNTHETIC_TARGET_FEATURES_V1: &[fe2o3_target_spec::TargetFeatureSpecV1] =
    &[fe2o3_target_spec::TargetFeatureSpecV1::new_unchecked(
        "bounded-grid",
        fe2o3_target_spec::TargetFeatureStateV1::Enabled,
    )];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProductionBackendTargetContractV1 {
    backend_family: &'static str,
    canonical_target: &'static str,
    rustc_target: &'static str,
    rustc_data_layout: &'static str,
    worker_data_layout: &'static str,
    pointer_width_bits: u16,
    cpu: &'static str,
    rustc_features: &'static str,
    code_object_version: u16,
    wave_width_bits: u16,
    neutral_profile: fe2o3_target_spec::TargetProfileSpecV1,
}

impl ProductionBackendTargetContractV1 {
    #[cfg(test)]
    pub(crate) fn synthetic_test_contract_v1() -> Self {
        Self {
            backend_family: "synthetic-test-v1",
            canonical_target: "portable64:checked-",
            rustc_target: "portable64-unknown-none",
            rustc_data_layout: "e-p:64:64",
            worker_data_layout: "e-p:64:64",
            pointer_width_bits: 64,
            cpu: "portable64",
            rustc_features: "+bounded-grid",
            code_object_version: 1,
            wave_width_bits: 32,
            neutral_profile: fe2o3_target_spec::TargetProfileSpecV1::from_static_parts(
                fe2o3_target_spec::TargetVendorV1::Other,
                fe2o3_target_spec::TargetArchitectureFamilyV1::Other,
                "portable64",
                Some("portable64-unknown-none"),
                Some("portable64-unknown-none"),
                fe2o3_target_spec::TargetArtifactFormatV1::Unknown,
                fe2o3_target_spec::TargetExecutionModelV1::GpuGrid,
                Some("e-p:64:64"),
                SYNTHETIC_TARGET_FEATURES_V1,
            ),
        }
    }

    pub(crate) const fn backend_family(self) -> &'static str {
        self.backend_family
    }

    pub(crate) const fn canonical_target(self) -> &'static str {
        self.canonical_target
    }

    pub(crate) const fn rustc_target(self) -> &'static str {
        self.rustc_target
    }

    pub(crate) const fn rustc_data_layout(self) -> &'static str {
        self.rustc_data_layout
    }

    pub(crate) const fn worker_data_layout(self) -> &'static str {
        self.worker_data_layout
    }

    pub(crate) const fn pointer_width_bits(self) -> u16 {
        self.pointer_width_bits
    }

    pub(crate) const fn cpu(self) -> &'static str {
        self.cpu
    }

    pub(crate) const fn rustc_features(self) -> &'static str {
        self.rustc_features
    }

    pub(crate) const fn code_object_version(self) -> u16 {
        self.code_object_version
    }

    pub(crate) const fn wave_width_bits(self) -> u16 {
        self.wave_width_bits
    }

    pub(crate) const fn neutral_profile(self) -> fe2o3_target_spec::TargetProfileSpecV1 {
        self.neutral_profile
    }
}

/// Opaque selected backend target. It is descriptive input, not compilation
/// authority; authenticated target custody remains owned by `production_target_v1`.
#[derive(Debug)]
pub(crate) struct ProductionBackendTargetV1 {
    inner: AmdProductionTargetV1,
}

impl ProductionBackendTargetV1 {
    pub(crate) fn from_configured_target(target: &str) -> Option<Self> {
        AmdProductionBackendV1::from_configured_target(target).map(|inner| Self { inner })
    }

    pub(crate) fn from_live_cpu(cpu: &str) -> Option<Self> {
        AmdProductionBackendV1::from_live_cpu(cpu).map(|inner| Self { inner })
    }

    pub(crate) fn contract(&self) -> ProductionBackendTargetContractV1 {
        AmdProductionBackendV1::target_contract(&self.inner)
    }

    pub(crate) fn same_target(&self, other: &Self) -> bool {
        self.contract() == other.contract()
    }

    pub(crate) fn device_target(&self) -> fe2o3_compiler_ffi::DeviceTargetV1 {
        fe2o3_compiler_ffi::DeviceTargetV1::parse(self.contract().canonical_target())
            .expect("the selected production backend target is canonical")
    }

    pub(crate) fn close_semantic_capabilities_v1(
        &self,
        canonical: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrV13,
        epoch: u64,
    ) -> Result<ProductionBackendCapabilityClosureV1, ProductionBackendErrorV1> {
        let inner = AmdProductionBackendV1::close_semantic_capabilities_v1(
            &self.inner,
            ProductionSemanticCapabilityInputV1::V13 { canonical, epoch },
        )?;
        Ok(ProductionBackendCapabilityClosureV1 { inner })
    }

    pub(crate) fn lower_v13_module_v1(
        &self,
        canonical: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrV13,
        epoch: u64,
        closure: &ProductionBackendCapabilityClosureV1,
    ) -> Result<String, ProductionBackendErrorV1> {
        AmdProductionBackendV1::lower_v13_module_v1(&self.inner, canonical, epoch, &closure.inner)
    }

    pub(crate) fn bind_worker_layout_v1(
        &self,
        llvm_ir: &str,
    ) -> Result<String, ProductionBackendErrorV1> {
        AmdProductionBackendV1::bind_worker_layout_v1(&self.inner, llvm_ir)
    }

    pub(crate) fn prepare_lineage_replay_v1(
        &self,
        neutral_kernel_ir: &[u8],
        target_module: &fe2o3_kernel_ir::Module,
        target_optimization: &fe2o3_kernel_opt::KernelIrPlironOptimizationReportV2,
        pre_descriptor_llvm: &str,
    ) -> Result<ProductionBackendLineageReplayV1, ProductionBackendErrorV1> {
        let inner = AmdProductionBackendV1::prepare_lineage_replay_v1(
            &self.inner,
            neutral_kernel_ir,
            target_module,
            target_optimization,
            pre_descriptor_llvm,
        )?;
        Ok(ProductionBackendLineageReplayV1 { inner })
    }
}

/// Move-only backend replay custody. Its representation and target profile are
/// sealed inside the selected adapter.
pub(crate) struct ProductionBackendLineageReplayV1 {
    inner: AmdProductionLineageReplayV1,
}

impl ProductionBackendLineageReplayV1 {
    pub(crate) fn validate_frozen_v3(
        self,
        kernel_ir: &fe2o3_compiler_lineage::InertKernelIrReceiptV3,
        expected_target_bound_kir: fe2o3_compiler_lineage::TargetLineageIdentityV3,
        target: ProductionBackendTargetContractV1,
    ) -> Result<ProductionBackendLineageReceiptV1, ProductionBackendErrorV1> {
        AmdProductionBackendV1::validate_frozen_v3_lineage_replay_v1(
            self.inner,
            kernel_ir,
            expected_target_bound_kir,
            target,
        )
    }
}

/// Replay-validated receipt for the frozen V3 lineage schema. The historical
/// concrete receipt type remains confined to the adapter boundary.
pub(crate) struct ProductionBackendLineageReceiptV1 {
    inner: fe2o3_compiler_lineage::InertAmdgpuLoweringReceiptV3,
}

impl ProductionBackendLineageReceiptV1 {
    pub(crate) fn identity_sha256(&self) -> [u8; 32] {
        *self.inner.identity().sha256()
    }

    pub(crate) fn identity_byte_len(&self) -> u64 {
        self.inner.identity().byte_len()
    }

    pub(crate) fn into_frozen_v3_receipt(
        self,
    ) -> fe2o3_compiler_lineage::InertAmdgpuLoweringReceiptV3 {
        self.inner
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProductionBackendGraphSubjectV1 {
    semantic_operation_version: u16,
    canonical_graph_version: u16,
    digest: [u8; 32],
    canonical_length: u64,
    epoch: u64,
}

impl ProductionBackendGraphSubjectV1 {
    pub(crate) const fn semantic_operation_version(self) -> u16 {
        self.semantic_operation_version
    }

    pub(crate) const fn canonical_graph_version(self) -> u16 {
        self.canonical_graph_version
    }

    pub(crate) const fn digest(self) -> [u8; 32] {
        self.digest
    }

    pub(crate) const fn canonical_length(self) -> u64 {
        self.canonical_length
    }

    pub(crate) const fn epoch(self) -> u64 {
        self.epoch
    }
}

/// Move-only, authority-free closure over one exact backend capability query.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ProductionBackendCapabilityClosureV1 {
    inner: AmdProductionCapabilityClosureV1,
}

impl ProductionBackendCapabilityClosureV1 {
    pub(crate) const fn identity(&self) -> [u8; 32] {
        self.inner.record.identity
    }

    pub(crate) const fn subject(&self) -> ProductionBackendGraphSubjectV1 {
        self.inner.record.subject
    }

    pub(crate) fn launch_subject_matches(&self) -> bool {
        self.inner.record.launch_subject == self.inner.record.subject
    }

    pub(crate) const fn decision_count(&self) -> usize {
        self.inner.record.decision_count
    }

    pub(crate) fn final_graph_target_contract_v1(
        &self,
        final_canonical: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrV13,
        final_epoch: u64,
        neutral_identity: fe2o3_kernel_ir::VerifiedCanonicalKernelIrIdentityV13,
        neutral_epoch: u64,
    ) -> Result<fe2o3_pliron::ProductionFinalGraphTargetContractV1, ProductionBackendErrorV1> {
        let _resource_evidence = self.validated_resource_evidence_v1()?;
        fe2o3_pliron::ProductionFinalGraphTargetContractV1::try_new(
            final_canonical,
            final_epoch,
            neutral_identity,
            neutral_epoch,
            self.inner.closure.identity(),
            self.inner.closure.target_model(),
            self.inner.closure.decisions().iter().copied(),
        )
        .map_err(ProductionBackendErrorV1::FinalGraphTargetContract)
    }

    /// Replays the backend closure and returns a nonconstructible typed view for
    /// W4 resource analysis. Bytes alone cannot create this value.
    pub(crate) fn validated_resource_evidence_v1(
        &self,
    ) -> Result<ProductionBackendResourceEvidenceV1<'_>, ProductionBackendErrorV1> {
        let replay = dialect_amdgcn::ProductionTargetCapabilityClosureV13::decode_canonical(
            self.inner.closure.canonical_bytes(),
            self.inner.closure.launch_evidence(),
        )
        .map_err(ProductionBackendErrorV1::CapabilityCanonical)?;
        let target = AmdProductionBackendV1::target_contract(&AmdProductionTargetV1 {
            profile: self.inner.profile,
        });
        let model = fe2o3_target_spec::TargetCapabilityModelIdentityV1::new(
            target.neutral_profile(),
            fe2o3_amd_target::PRODUCTION_AMD_NEUTRAL_CAPABILITY_MODEL_REVISION_V1,
        )
        .map_err(ProductionBackendErrorV1::CapabilityModel)?;
        let subject = self.inner.closure.subject();
        let observed_subject = ProductionBackendGraphSubjectV1 {
            semantic_operation_version: 1,
            canonical_graph_version: match subject.version() {
                dialect_amdgcn::ProductionCanonicalGraphVersionV1::V12 => 12,
                dialect_amdgcn::ProductionCanonicalGraphVersionV1::V13 => 13,
            },
            digest: subject.digest(),
            canonical_length: subject.canonical_length(),
            epoch: subject.epoch(),
        };
        if replay != self.inner.closure
            || replay.identity() != self.inner.record.identity
            || observed_subject != self.inner.record.subject
            || self.inner.record.launch_subject != self.inner.record.subject
            || replay.target_model() != model
        {
            return Err(ProductionBackendErrorV1::CapabilityClosureChanged);
        }
        for decision in replay.decisions() {
            decision
                .validate()
                .map_err(ProductionBackendErrorV1::CapabilityDecision)?;
            if decision.model() != model {
                return Err(ProductionBackendErrorV1::CapabilityClosureChanged);
            }
        }
        Ok(ProductionBackendResourceEvidenceV1 {
            closure_identity: self.inner.closure.identity(),
            subject: observed_subject,
            target,
            model,
            launch_evidence_identity: replay.launch_evidence().identity(),
            system_atomic_memory_eligible: replay.launch_evidence().values().iter().any(|value| {
                matches!(
                    value,
                    dialect_amdgcn::ProductionTargetLaunchEvidenceValueV1::SystemAtomicMemoryEligibility(
                        true
                    )
                )
            }),
            canonical_closure: self.inner.closure.canonical_bytes(),
            decisions: self.inner.closure.decisions(),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProductionBackendCapabilityRecordV1 {
    identity: [u8; 32],
    subject: ProductionBackendGraphSubjectV1,
    launch_subject: ProductionBackendGraphSubjectV1,
    decision_count: usize,
}

/// Exact typed target evidence borrowed from a replay-validated backend
/// closure. Private fields prevent construction from caller-owned facts.
pub(crate) struct ProductionBackendResourceEvidenceV1<'a> {
    closure_identity: [u8; 32],
    subject: ProductionBackendGraphSubjectV1,
    target: ProductionBackendTargetContractV1,
    model: fe2o3_target_spec::TargetCapabilityModelIdentityV1,
    launch_evidence_identity: [u8; 32],
    system_atomic_memory_eligible: bool,
    canonical_closure: &'a [u8],
    decisions: &'a [fe2o3_target_spec::TargetCapabilityDecisionV1],
}

#[allow(
    dead_code,
    reason = "the W4 resource adapter consumes this typed view in the next integration step"
)]
impl ProductionBackendResourceEvidenceV1<'_> {
    pub(crate) const fn closure_identity(&self) -> [u8; 32] {
        self.closure_identity
    }

    pub(crate) const fn subject(&self) -> ProductionBackendGraphSubjectV1 {
        self.subject
    }

    pub(crate) const fn target(&self) -> ProductionBackendTargetContractV1 {
        self.target
    }

    pub(crate) const fn model(&self) -> fe2o3_target_spec::TargetCapabilityModelIdentityV1 {
        self.model
    }

    pub(crate) const fn launch_evidence_identity(&self) -> [u8; 32] {
        self.launch_evidence_identity
    }

    pub(crate) const fn system_atomic_memory_eligible(&self) -> bool {
        self.system_atomic_memory_eligible
    }

    pub(crate) const fn canonical_closure(&self) -> &[u8] {
        self.canonical_closure
    }

    pub(crate) const fn decisions(&self) -> &[fe2o3_target_spec::TargetCapabilityDecisionV1] {
        self.decisions
    }
}

enum ProductionSemanticCapabilityInputV1<'a> {
    V13 {
        canonical: &'a fe2o3_kernel_ir::VerifiedCanonicalKernelIrV13,
        epoch: u64,
    },
}

trait ProductionBackendAdapterV1 {
    type Target;
    type CapabilityClosure;
    type LineageReplay;

    fn from_configured_target(target: &str) -> Option<Self::Target>;
    fn from_live_cpu(cpu: &str) -> Option<Self::Target>;
    fn target_contract(target: &Self::Target) -> ProductionBackendTargetContractV1;
    fn close_semantic_capabilities_v1(
        target: &Self::Target,
        input: ProductionSemanticCapabilityInputV1<'_>,
    ) -> Result<Self::CapabilityClosure, ProductionBackendErrorV1>;
    fn lower_v13_module_v1(
        target: &Self::Target,
        canonical: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrV13,
        epoch: u64,
        closure: &Self::CapabilityClosure,
    ) -> Result<String, ProductionBackendErrorV1>;
    fn bind_worker_layout_v1(
        target: &Self::Target,
        llvm_ir: &str,
    ) -> Result<String, ProductionBackendErrorV1>;
    fn prepare_lineage_replay_v1(
        target: &Self::Target,
        neutral_kernel_ir: &[u8],
        target_module: &fe2o3_kernel_ir::Module,
        target_optimization: &fe2o3_kernel_opt::KernelIrPlironOptimizationReportV2,
        pre_descriptor_llvm: &str,
    ) -> Result<Self::LineageReplay, ProductionBackendErrorV1>;
    fn validate_frozen_v3_lineage_replay_v1(
        replay: Self::LineageReplay,
        kernel_ir: &fe2o3_compiler_lineage::InertKernelIrReceiptV3,
        expected_target_bound_kir: fe2o3_compiler_lineage::TargetLineageIdentityV3,
        target: ProductionBackendTargetContractV1,
    ) -> Result<ProductionBackendLineageReceiptV1, ProductionBackendErrorV1>;
}

struct AmdProductionBackendV1;

#[derive(Debug)]
struct AmdProductionTargetV1 {
    profile: ProductionAmdTargetProfileV1,
}

#[derive(Debug, Eq, PartialEq)]
struct AmdProductionCapabilityClosureV1 {
    profile: ProductionAmdTargetProfileV1,
    record: ProductionBackendCapabilityRecordV1,
    closure: dialect_amdgcn::ProductionTargetCapabilityClosureV13,
}

struct AmdProductionLineageReplayV1 {
    profile: ProductionAmdTargetProfileV1,
    evidence: dialect_amdgcn::CanonicalProductionKirToLlvmReplayEvidenceV1,
}

impl ProductionBackendAdapterV1 for AmdProductionBackendV1 {
    type Target = AmdProductionTargetV1;
    type CapabilityClosure = AmdProductionCapabilityClosureV1;
    type LineageReplay = AmdProductionLineageReplayV1;

    fn from_configured_target(target: &str) -> Option<Self::Target> {
        ProductionAmdTargetProfileV1::from_device_target(target)
            .map(|profile| Self::Target { profile })
    }

    fn from_live_cpu(cpu: &str) -> Option<Self::Target> {
        ProductionAmdTargetProfileV1::from_cpu(cpu).map(|profile| Self::Target { profile })
    }

    fn target_contract(target: &Self::Target) -> ProductionBackendTargetContractV1 {
        let profile = target.profile;
        ProductionBackendTargetContractV1 {
            backend_family: "amdhsa-v1",
            canonical_target: profile.device_target(),
            rustc_target: profile.rustc_target(),
            rustc_data_layout: PRODUCTION_RUSTC_DATA_LAYOUT_V1,
            worker_data_layout: PRODUCTION_WORKER_DATA_LAYOUT_V1,
            pointer_width_bits: 64,
            cpu: profile.cpu(),
            rustc_features: profile.rustc_features(),
            code_object_version: 6,
            wave_width_bits: 64,
            neutral_profile: profile.target_profile_spec(),
        }
    }

    fn close_semantic_capabilities_v1(
        target: &Self::Target,
        input: ProductionSemanticCapabilityInputV1<'_>,
    ) -> Result<Self::CapabilityClosure, ProductionBackendErrorV1> {
        let ProductionSemanticCapabilityInputV1::V13 { canonical, epoch } = input;
        let launch_evidence = dialect_amdgcn::ProductionTargetLaunchEvidenceV13::from_exact_values(
            canonical,
            epoch,
            [],
        )
        .map_err(ProductionBackendErrorV1::CapabilityClosure)?;
        let closure = dialect_amdgcn::legalize_production_target_capabilities_v13(
            canonical,
            epoch,
            &launch_evidence,
            target.profile,
        )
        .map_err(ProductionBackendErrorV1::CapabilityClosure)?;
        let subject = closure.subject();
        let launch_subject = closure.launch_evidence().subject();
        let launch_subject = ProductionBackendGraphSubjectV1 {
            semantic_operation_version: 1,
            canonical_graph_version: match launch_subject.version() {
                dialect_amdgcn::ProductionCanonicalGraphVersionV1::V12 => 12,
                dialect_amdgcn::ProductionCanonicalGraphVersionV1::V13 => 13,
            },
            digest: launch_subject.digest(),
            canonical_length: launch_subject.canonical_length(),
            epoch: launch_subject.epoch(),
        };
        Ok(AmdProductionCapabilityClosureV1 {
            profile: target.profile,
            record: ProductionBackendCapabilityRecordV1 {
                identity: closure.identity(),
                subject: ProductionBackendGraphSubjectV1 {
                    semantic_operation_version: 1,
                    canonical_graph_version: match subject.version() {
                        dialect_amdgcn::ProductionCanonicalGraphVersionV1::V12 => 12,
                        dialect_amdgcn::ProductionCanonicalGraphVersionV1::V13 => 13,
                    },
                    digest: subject.digest(),
                    canonical_length: subject.canonical_length(),
                    epoch: subject.epoch(),
                },
                launch_subject,
                decision_count: closure.decisions().len(),
            },
            closure,
        })
    }

    fn lower_v13_module_v1(
        target: &Self::Target,
        canonical: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrV13,
        epoch: u64,
        closure: &Self::CapabilityClosure,
    ) -> Result<String, ProductionBackendErrorV1> {
        if closure.profile != target.profile {
            return Err(ProductionBackendErrorV1::CapabilityClosureChanged);
        }
        let lowered = dialect_amdgcn::lower_verified_canonical_kir_v13_to_amd_llvm_ir_v1(
            canonical,
            epoch,
            closure.closure.launch_evidence(),
            target.profile,
        )
        .map_err(ProductionBackendErrorV1::V13Lowering)?;
        if lowered.capability_closure() != &closure.closure {
            return Err(ProductionBackendErrorV1::CapabilityClosureChanged);
        }
        Ok(lowered.llvm_ir().to_owned())
    }

    fn bind_worker_layout_v1(
        _target: &Self::Target,
        llvm_ir: &str,
    ) -> Result<String, ProductionBackendErrorV1> {
        dialect_amdgcn::bind_production_llvm22_worker_layout_v1(llvm_ir)
            .map_err(ProductionBackendErrorV1::WorkerLayout)
    }

    fn prepare_lineage_replay_v1(
        target: &Self::Target,
        neutral_kernel_ir: &[u8],
        target_module: &fe2o3_kernel_ir::Module,
        target_optimization: &fe2o3_kernel_opt::KernelIrPlironOptimizationReportV2,
        pre_descriptor_llvm: &str,
    ) -> Result<Self::LineageReplay, ProductionBackendErrorV1> {
        let evidence =
            dialect_amdgcn::CanonicalProductionKirToLlvmReplayEvidenceV1::from_optimized_live_inputs_v4(
                neutral_kernel_ir,
                target_module,
                target_optimization,
                target.profile,
                pre_descriptor_llvm,
            )
            .map_err(ProductionBackendErrorV1::LineageReplay)?;
        Ok(AmdProductionLineageReplayV1 {
            profile: target.profile,
            evidence,
        })
    }

    fn validate_frozen_v3_lineage_replay_v1(
        replay: Self::LineageReplay,
        kernel_ir: &fe2o3_compiler_lineage::InertKernelIrReceiptV3,
        expected_target_bound_kir: fe2o3_compiler_lineage::TargetLineageIdentityV3,
        target: ProductionBackendTargetContractV1,
    ) -> Result<ProductionBackendLineageReceiptV1, ProductionBackendErrorV1> {
        let expected_target = Self::target_contract(&AmdProductionTargetV1 {
            profile: replay.profile,
        });
        if target != expected_target {
            return Err(ProductionBackendErrorV1::LineageReplayChanged);
        }
        let receipt =
            fe2o3_compiler_lineage::InertAmdgpuLoweringReceiptV3::from_canonical_preimage(
                replay.evidence.canonical_bytes(),
            )
            .map_err(ProductionBackendErrorV1::LineageReceipt)?;
        {
            let validated =
                fe2o3_verifier::validate_compiler_kir_to_llvm_replay_v1(kernel_ir, &receipt)
                    .map_err(ProductionBackendErrorV1::LineageReplayValidation)?;
            let evidence = validated.replay().evidence();
            let observed_target_bound_kir = fe2o3_compiler_lineage::TargetLineageIdentityV3::new(
                evidence.target_bound_kernel_ir_identity().sha256(),
                evidence.target_bound_kernel_ir_identity().byte_len(),
            )
            .map_err(ProductionBackendErrorV1::LineageTarget)?;
            if observed_target_bound_kir != expected_target_bound_kir
                || evidence.profile() != replay.profile
                || evidence.profile().device_target() != target.canonical_target()
            {
                return Err(ProductionBackendErrorV1::LineageReplayChanged);
            }
        }
        Ok(ProductionBackendLineageReceiptV1 { inner: receipt })
    }
}

#[derive(Debug)]
pub(crate) enum ProductionBackendErrorV1 {
    CapabilityClosure(dialect_amdgcn::ProductionTargetCapabilityErrorV1),
    CapabilityCanonical(dialect_amdgcn::ProductionTargetCapabilityCanonicalErrorV1),
    CapabilityModel(fe2o3_target_spec::TargetCapabilityModelIdentityErrorV1),
    CapabilityDecision(fe2o3_target_spec::TargetCapabilityQueryErrorV1),
    FinalGraphTargetContract(fe2o3_pliron::ProductionFinalGraphTargetContractErrorV1),
    V13Lowering(dialect_amdgcn::ProductionV13AmdLoweringErrorV1),
    WorkerLayout(dialect_amdgcn::ProductionLlvmLayoutBindingErrorV1),
    LineageReplay(dialect_amdgcn::ProductionKirToLlvmReplayErrorV1),
    LineageReceipt(fe2o3_compiler_lineage::LineageErrorV3),
    LineageTarget(fe2o3_compiler_lineage::ProductionTargetLineageErrorV3),
    LineageReplayValidation(fe2o3_verifier::CompilerKirToLlvmReplayValidationErrorV1),
    CapabilityClosureChanged,
    LineageReplayChanged,
    #[cfg(test)]
    ObjectLoweringUnavailable {
        backend_family: &'static str,
    },
}

impl fmt::Display for ProductionBackendErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CapabilityClosure(error) => {
                write!(formatter, "semantic capability closure failed: {error}")
            }
            Self::CapabilityCanonical(error) => {
                write!(
                    formatter,
                    "semantic capability closure replay failed: {error}"
                )
            }
            Self::CapabilityModel(error) => {
                write!(formatter, "semantic capability model failed: {error}")
            }
            Self::CapabilityDecision(error) => {
                write!(formatter, "semantic capability decision failed: {error}")
            }
            Self::FinalGraphTargetContract(error) => {
                write!(formatter, "final-graph target contract failed: {error}")
            }
            Self::V13Lowering(error) => write!(formatter, "V13 lowering failed: {error}"),
            Self::WorkerLayout(error) => write!(formatter, "worker layout binding failed: {error}"),
            Self::LineageReplay(error) => {
                write!(
                    formatter,
                    "backend KIR-to-LLVM lineage replay failed: {error}"
                )
            }
            Self::LineageReceipt(error) => {
                write!(formatter, "backend frozen lineage receipt failed: {error}")
            }
            Self::LineageTarget(error) => {
                write!(formatter, "backend target-lineage identity failed: {error}")
            }
            Self::LineageReplayValidation(error) => write!(
                formatter,
                "backend-independent KIR-to-LLVM lineage validation failed: {error}"
            ),
            Self::CapabilityClosureChanged => formatter
                .write_str("backend lowering did not retain the exact semantic capability closure"),
            Self::LineageReplayChanged => formatter.write_str(
                "backend lineage replay changed its target, profile, or target-bound KIR",
            ),
            #[cfg(test)]
            Self::ObjectLoweringUnavailable { backend_family } => write!(
                formatter,
                "backend family {backend_family:?} has no production object lowering adapter",
            ),
        }
    }
}

impl std::error::Error for ProductionBackendErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CapabilityClosure(error) => Some(error),
            Self::CapabilityCanonical(error) => Some(error),
            Self::CapabilityModel(error) => Some(error),
            Self::CapabilityDecision(error) => Some(error),
            Self::FinalGraphTargetContract(error) => Some(error),
            Self::V13Lowering(error) => Some(error),
            Self::WorkerLayout(error) => Some(error),
            Self::LineageReplay(error) => Some(error),
            Self::LineageReceipt(error) => Some(error),
            Self::LineageTarget(error) => Some(error),
            Self::LineageReplayValidation(error) => Some(error),
            Self::CapabilityClosureChanged | Self::LineageReplayChanged => None,
            #[cfg(test)]
            Self::ObjectLoweringUnavailable { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    struct SyntheticBackendV1;

    #[derive(Clone, Copy)]
    struct SyntheticTargetV1;

    #[derive(Debug)]
    struct SyntheticClosureV1 {
        record: ProductionBackendCapabilityRecordV1,
    }

    struct SyntheticLineageReplayV1;

    impl ProductionBackendAdapterV1 for SyntheticBackendV1 {
        type Target = SyntheticTargetV1;
        type CapabilityClosure = SyntheticClosureV1;
        type LineageReplay = SyntheticLineageReplayV1;

        fn from_configured_target(target: &str) -> Option<Self::Target> {
            (target == "portable64:checked-").then_some(SyntheticTargetV1)
        }

        fn from_live_cpu(cpu: &str) -> Option<Self::Target> {
            (cpu == "portable64").then_some(SyntheticTargetV1)
        }

        fn target_contract(_target: &Self::Target) -> ProductionBackendTargetContractV1 {
            ProductionBackendTargetContractV1 {
                backend_family: "synthetic-test-v1",
                canonical_target: "portable64:checked-",
                rustc_target: "portable64-unknown-none",
                rustc_data_layout: "e-p:64:64",
                worker_data_layout: "e-p:64:64",
                pointer_width_bits: 64,
                cpu: "portable64",
                rustc_features: "+bounded-grid",
                code_object_version: 1,
                wave_width_bits: 32,
                neutral_profile: fe2o3_target_spec::TargetProfileSpecV1::from_static_parts(
                    fe2o3_target_spec::TargetVendorV1::Other,
                    fe2o3_target_spec::TargetArchitectureFamilyV1::Other,
                    "portable64",
                    Some("portable64-unknown-none"),
                    Some("portable64-unknown-none"),
                    fe2o3_target_spec::TargetArtifactFormatV1::Unknown,
                    fe2o3_target_spec::TargetExecutionModelV1::GpuGrid,
                    Some("e-p:64:64"),
                    SYNTHETIC_TARGET_FEATURES_V1,
                ),
            }
        }

        fn close_semantic_capabilities_v1(
            _target: &Self::Target,
            input: ProductionSemanticCapabilityInputV1<'_>,
        ) -> Result<Self::CapabilityClosure, ProductionBackendErrorV1> {
            let ProductionSemanticCapabilityInputV1::V13 { canonical, epoch } = input;
            let mut digest = Sha256::new();
            digest.update(b"fe2o3.synthetic-backend-capability-closure.v1\0");
            digest.update(canonical.identity().digest());
            digest.update(canonical.identity().canonical_length().to_le_bytes());
            digest.update(epoch.to_le_bytes());
            Ok(SyntheticClosureV1 {
                record: ProductionBackendCapabilityRecordV1 {
                    identity: digest.finalize().into(),
                    subject: ProductionBackendGraphSubjectV1 {
                        semantic_operation_version: 1,
                        canonical_graph_version: 13,
                        digest: *canonical.identity().digest(),
                        canonical_length: canonical.identity().canonical_length(),
                        epoch,
                    },
                    launch_subject: ProductionBackendGraphSubjectV1 {
                        semantic_operation_version: 1,
                        canonical_graph_version: 13,
                        digest: *canonical.identity().digest(),
                        canonical_length: canonical.identity().canonical_length(),
                        epoch,
                    },
                    decision_count: 1,
                },
            })
        }

        fn lower_v13_module_v1(
            _target: &Self::Target,
            _canonical: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrV13,
            _epoch: u64,
            _closure: &Self::CapabilityClosure,
        ) -> Result<String, ProductionBackendErrorV1> {
            Err(ProductionBackendErrorV1::ObjectLoweringUnavailable {
                backend_family: "synthetic-test-v1",
            })
        }

        fn bind_worker_layout_v1(
            _target: &Self::Target,
            _llvm_ir: &str,
        ) -> Result<String, ProductionBackendErrorV1> {
            Err(ProductionBackendErrorV1::ObjectLoweringUnavailable {
                backend_family: "synthetic-test-v1",
            })
        }

        fn prepare_lineage_replay_v1(
            _target: &Self::Target,
            _neutral_kernel_ir: &[u8],
            _target_module: &fe2o3_kernel_ir::Module,
            _target_optimization: &fe2o3_kernel_opt::KernelIrPlironOptimizationReportV2,
            _pre_descriptor_llvm: &str,
        ) -> Result<Self::LineageReplay, ProductionBackendErrorV1> {
            Err(ProductionBackendErrorV1::ObjectLoweringUnavailable {
                backend_family: "synthetic-test-v1",
            })
        }

        fn validate_frozen_v3_lineage_replay_v1(
            _replay: Self::LineageReplay,
            _kernel_ir: &fe2o3_compiler_lineage::InertKernelIrReceiptV3,
            _expected_target_bound_kir: fe2o3_compiler_lineage::TargetLineageIdentityV3,
            _target: ProductionBackendTargetContractV1,
        ) -> Result<ProductionBackendLineageReceiptV1, ProductionBackendErrorV1> {
            Err(ProductionBackendErrorV1::ObjectLoweringUnavailable {
                backend_family: "synthetic-test-v1",
            })
        }
    }

    fn return_only_v13() -> fe2o3_kernel_ir::VerifiedCanonicalKernelIrV13 {
        let mut block = fe2o3_kernel_ir::BasicBlock::new(fe2o3_kernel_ir::BlockId(0));
        block.terminator = Some(fe2o3_kernel_ir::Terminator::Return { values: vec![] });
        let mut module = fe2o3_kernel_ir::Module::new("synthetic-target-boundary");
        module
            .functions
            .push(fe2o3_kernel_ir::Function::kernel_entry(
                "entry",
                fe2o3_kernel_ir::Signature::new(vec![], vec![]),
                vec![],
                vec![block],
            ));
        let mut kernel = fe2o3_kernel_ir::Kernel::new(
            "kernel",
            "entry",
            fe2o3_kernel_ir::LaunchDomain::D1 {
                x: fe2o3_kernel_ir::LaunchExtent::Static(1),
            },
        );
        kernel.workgroup_size = Some(fe2o3_kernel_ir::WorkgroupSize::new(1, 1, 1));
        module.kernels.push(kernel);
        fe2o3_kernel_ir::VerifiedCanonicalKernelIrV13::from_module(module).unwrap()
    }

    #[test]
    fn synthetic_target_crosses_neutral_selection_and_closure_but_not_object_lowering() {
        let target = SyntheticBackendV1::from_configured_target("portable64:checked-").unwrap();
        assert!(SyntheticBackendV1::from_live_cpu("portable64").is_some());
        let contract = SyntheticBackendV1::target_contract(&target);
        contract.neutral_profile().validate().unwrap();
        let canonical = return_only_v13();
        let closure = SyntheticBackendV1::close_semantic_capabilities_v1(
            &target,
            ProductionSemanticCapabilityInputV1::V13 {
                canonical: &canonical,
                epoch: 9,
            },
        )
        .unwrap();
        assert_eq!(closure.record.subject.semantic_operation_version(), 1);
        assert_eq!(closure.record.subject.canonical_graph_version(), 13);
        assert_eq!(
            closure.record.subject.digest(),
            *canonical.identity().digest()
        );
        assert_eq!(closure.record.subject.epoch(), 9);
        assert_eq!(closure.record.launch_subject, closure.record.subject);
        assert_eq!(closure.record.decision_count, 1);

        let rustc_layout =
            crate::semantic_layout_bridge::SemanticLayoutTargetV1::new_with_codegen_profile(
                contract.rustc_target(),
                contract.rustc_data_layout(),
                contract.pointer_width_bits(),
                contract.cpu(),
                "",
                contract.rustc_features(),
            )
            .unwrap();
        let lineage_target = crate::production_semantic_lineage_v3::PreparedProductionSemanticLineageTargetV1::try_prepare(
            contract,
            rustc_layout,
        )
        .unwrap();
        assert_eq!(lineage_target.contract(), contract);

        let neutral_record = format!("{contract:?}:{:?}", closure.record).to_ascii_lowercase();
        assert!(!neutral_record.contains("amd"));
        assert!(!neutral_record.contains("gfx"));
        assert!(matches!(
            SyntheticBackendV1::lower_v13_module_v1(&target, &canonical, 9, &closure),
            Err(ProductionBackendErrorV1::ObjectLoweringUnavailable {
                backend_family: "synthetic-test-v1"
            })
        ));

        let target_core = include_str!("production_target_v1.rs")
            .split("#[cfg(test)]\nmod tests")
            .next()
            .unwrap();
        let lineage_core = include_str!("production_semantic_lineage_v3.rs")
            .split("#[cfg(test)]\nmod layout_tests")
            .next()
            .unwrap();
        let importer_brand = include_str!("collector/production_importer_v1.rs")
            .split("pub(crate) fn kernel_context_target_brand_identity_from_contract_v1")
            .nth(1)
            .unwrap()
            .split("fn kernel_context_launch_brand_identity_v1")
            .next()
            .unwrap();
        for forbidden in [
            concat!("ProductionAmd", "TargetProfileV1"),
            concat!("ProductionTarget", "CapabilityClosureV13"),
            concat!("CanonicalProduction", "KirToLlvmReplayEvidenceV1"),
            concat!("legacy_lineage_", "profile_v1"),
            "gfx942",
            "gfx950",
        ] {
            for (name, source) in [
                ("target core", target_core),
                ("semantic-lineage core", lineage_core),
                ("importer target brand", importer_brand),
            ] {
                assert!(
                    !source.contains(forbidden),
                    "{name} depends on backend-private term {forbidden}",
                );
            }
        }
    }

    #[test]
    fn production_resource_evidence_is_replayed_and_typed() {
        let target = ProductionBackendTargetV1::from_configured_target("gfx942:xnack-").unwrap();
        let canonical = return_only_v13();
        let closure = target
            .close_semantic_capabilities_v1(&canonical, 17)
            .unwrap();
        let evidence = closure.validated_resource_evidence_v1().unwrap();
        assert_eq!(evidence.closure_identity(), closure.identity());
        assert_eq!(evidence.subject(), closure.subject());
        assert_eq!(evidence.target(), target.contract());
        assert_eq!(
            evidence.canonical_closure(),
            closure.inner.closure.canonical_bytes()
        );
        assert!(!evidence.decisions().is_empty());
        assert_eq!(
            evidence.model().profile_fingerprint(),
            evidence.decisions()[0].model().profile_fingerprint(),
        );
        assert!(evidence.decisions().iter().all(|decision| {
            decision.validate().is_ok() && decision.model() == evidence.model()
        }));
    }

    #[test]
    fn compiler_handoff_does_not_depend_on_an_amd_closure_type() {
        let handoff = include_str!("production_worker_handoff.rs");
        for backend_private in [
            "ProductionTargetCapabilityClosureV13",
            "ProductionTargetLaunchEvidenceValueV1",
            "legacy_worker_handoff_closure_v1",
        ] {
            assert!(
                !handoff.contains(backend_private),
                "production handoff leaked backend-private type {backend_private}"
            );
        }
    }
}

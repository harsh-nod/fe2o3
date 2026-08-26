//! Single production-pipeline transaction shell.
//!
//! This module owns the one integration point for issue #175. It deliberately
//! contains no workload recognition. The sole semantic-MIR importer owns the
//! consuming target-authentication boundary and moves an admitted request into
//! a typed stage before the mandatory generic kernel-verification pipeline.

use std::fmt;
use std::marker::PhantomData;
use std::path::PathBuf;

use rustc_middle::ty::TyCtxt;

use crate::artifact_transaction::{BuildAttempt, ProducerIdentity};
use crate::collector::AuthenticatedCollectedKernelClosureV1;
use crate::protected_rustc_invocation::{
    AdmittedProtectedRustcInvocationV1, ProtectedRustcInvocationErrorV1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProductionDisposition {
    HostOnly,
    DeviceTransaction,
}

pub(crate) const fn disposition(device_candidate_count: usize) -> ProductionDisposition {
    if device_candidate_count == 0 {
        ProductionDisposition::HostOnly
    } else {
        ProductionDisposition::DeviceTransaction
    }
}

#[derive(Debug)]
pub(crate) enum ProductionPipelineError {
    CustomLlvmConfiguration,
    EmptyCollectedDeviceClosure,
    SemanticImport(crate::collector::ProductionSemanticImportErrorV1),
    SemanticMiddleEnd(fe2o3_pliron::ProductionSemanticMirErrorV1),
    RankedProjection(crate::production_ranked_projection_v1::ProductionRankedProjectionErrorV1),
    RankedVerification(crate::production_ranked_projection_v1::ProductionRankedVerificationErrorV1),
    TargetNeutralLowering(fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1),
    FormalMemoryAdmission(fe2o3_lower_mir_kernel::ProductionFormalMemoryErrorV1),
    Geometry(crate::production_geometry_v1::ProductionGeometryErrorV1),
    TargetBinding(fe2o3_kernel_ir::VerificationErrors),
    Gfx942Lowering(dialect_amdgcn::LoweringErrors),
    UpstreamLlvmLayoutBinding(String),
    DescriptorEvidence(crate::compiler_descriptor::CompilerDescriptorError),
    SemanticLineage(crate::production_semantic_lineage_v3::ProductionSemanticLineageErrorV3),
    RustcLineageMismatch,
    ProtectedRustcInvocation(ProtectedRustcInvocationErrorV1),
    ExtractionCannotPublish,
    WorkerHandoff(crate::production_worker_handoff::ProductionWorkerHandoffError),
    StrictV3Publication(fe2o3_artifact_transaction::CompilerModuleHandoffErrorV3),
}

impl fmt::Display for ProductionPipelineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CustomLlvmConfiguration => formatter.write_str(
                "production compilation rejects caller-selected LLVM arguments or passes before transaction construction",
            ),
            Self::EmptyCollectedDeviceClosure => formatter.write_str(
                "production compilation requires a nonempty collector-sealed device closure",
            ),
            Self::SemanticImport(error) => write!(formatter, "production compilation {error}"),
            Self::SemanticMiddleEnd(error) => {
                write!(formatter, "production compilation exact semantic middle end failed: {error}")
            }
            Self::RankedProjection(error) => {
                write!(formatter, "production compilation general kernel verification failed: {error}")
            }
            Self::RankedVerification(error) => {
                write!(formatter, "production compilation ranked verification failed: {error}")
            }
            Self::TargetNeutralLowering(error) => {
                write!(formatter, "production compilation target-neutral lowering failed: {error}")
            }
            Self::FormalMemoryAdmission(error) => {
                write!(formatter, "production compilation formal memory admission failed: {error}")
            }
            Self::Geometry(error) => {
                write!(formatter, "production compilation geometry validation failed: {error}")
            }
            Self::TargetBinding(error) => {
                write!(formatter, "production compilation gfx942 target binding failed: {error}")
            }
            Self::Gfx942Lowering(error) => {
                write!(formatter, "production compilation gfx942 LLVM lowering failed: {error}")
            }
            Self::UpstreamLlvmLayoutBinding(error) => {
                write!(formatter, "production compilation upstream LLVM layout binding failed: {error}")
            }
            Self::DescriptorEvidence(error) => {
                write!(formatter, "production compilation descriptor evidence failed: {error}")
            }
            Self::SemanticLineage(error) => write!(formatter, "production compilation {error}"),
            Self::RustcLineageMismatch => formatter.write_str(
                "production compilation rustc preflight plan is not bound to the retained identity inventory",
            ),
            Self::ProtectedRustcInvocation(error) => write!(
                formatter,
                "production compilation final protected rustc invocation validation failed: {error}"
            ),
            Self::ExtractionCannotPublish => formatter.write_str(
                "production extraction custody cannot publish a compiler-module handoff",
            ),
            Self::WorkerHandoff(error) => {
                write!(formatter, "production compilation compiler-module handoff failed: {error}")
            }
            Self::StrictV3Publication(error) => {
                write!(formatter, "production compilation strict V3 publication failed: {error}")
            }
        }
    }
}

impl std::error::Error for ProductionPipelineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SemanticImport(error) => Some(error),
            Self::SemanticMiddleEnd(error) => Some(error),
            Self::RankedProjection(error) => Some(error),
            Self::RankedVerification(error) => Some(error),
            Self::TargetNeutralLowering(error) => Some(error),
            Self::FormalMemoryAdmission(error) => Some(error),
            Self::Geometry(error) => Some(error),
            Self::TargetBinding(error) => Some(error),
            Self::Gfx942Lowering(error) => Some(error),
            Self::DescriptorEvidence(error) => Some(error),
            Self::SemanticLineage(error) => Some(error),
            Self::ProtectedRustcInvocation(error) => Some(error),
            Self::WorkerHandoff(error) => Some(error),
            Self::StrictV3Publication(error) => Some(error),
            Self::CustomLlvmConfiguration
            | Self::EmptyCollectedDeviceClosure
            | Self::RustcLineageMismatch
            | Self::UpstreamLlvmLayoutBinding(_) => None,
            Self::ExtractionCannotPublish => None,
        }
    }
}

pub(crate) fn reject_custom_llvm_configuration(
    has_custom_llvm_configuration: bool,
) -> Result<(), ProductionPipelineError> {
    if has_custom_llvm_configuration {
        Err(ProductionPipelineError::CustomLlvmConfiguration)
    } else {
        Ok(())
    }
}

pub(super) struct CollectedRustStage<'tcx> {
    tcx: TyCtxt<'tcx>,
    closure: AuthenticatedCollectedKernelClosureV1<'tcx>,
    typed_descriptor_roots: Vec<crate::compiler_descriptor::TypedDescriptorRootV1>,
    transaction: ProductionTransactionBindings,
}

struct ProductionTransactionBindings {
    producer: ProducerIdentity,
    output_dir: PathBuf,
    compiler_ffi_envelope: Option<fe2o3_compiler_ffi::CompilerFfiEnvelopeV1>,
    compiler_custody: ProductionCompilerCustody,
}

enum ProductionCompilerCustody {
    ProtectedV3 {
        invocation: Box<AdmittedProtectedRustcInvocationV1>,
        attempt: BuildAttempt,
    },
    ExtractionOnly,
}

impl ProductionCompilerCustody {
    fn protected(invocation: AdmittedProtectedRustcInvocationV1, attempt: BuildAttempt) -> Self {
        Self::ProtectedV3 {
            invocation: Box::new(invocation),
            attempt,
        }
    }

    const fn extraction_only() -> Self {
        Self::ExtractionOnly
    }

    fn has_publication_attempt(&self) -> bool {
        matches!(self, Self::ProtectedV3 { .. })
    }

    fn into_publication_custody(
        self,
    ) -> Result<(BuildAttempt, Box<AdmittedProtectedRustcInvocationV1>), ProductionPipelineError>
    {
        match self {
            Self::ProtectedV3 {
                invocation,
                attempt,
            } => Ok((attempt, invocation)),
            Self::ExtractionOnly => Err(ProductionPipelineError::ExtractionCannotPublish),
        }
    }
}

struct AuthenticatedProductionBindings {
    rustc_identity_inventory: crate::collector::AuthenticatedRustcIdentityInventoryV3,
    rustc_preflight_plan: crate::collector::AuthenticatedRustcPreflightPlanV3,
    rustc_target: crate::production_target_v1::AuthenticatedProductionTargetV1,
    reference_effect_bindings: crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1,
    typed_descriptor_roots: Vec<crate::compiler_descriptor::TypedDescriptorRootV1>,
    transaction: ProductionTransactionBindings,
}

pub(super) struct AdmittedSemanticMirStage {
    semantic_mir: fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1,
    bindings: AuthenticatedProductionBindings,
}

pub(super) struct EquivalentSemanticMirStage {
    semantic_mir: fe2o3_pliron::ProductionSemanticMirOwnerV1,
    bindings: AuthenticatedProductionBindings,
}

/// Move-only owner of one production compilation stage.
///
/// Its fields and stage types stay private so no caller can synthesize or
/// bypass a transition. The transaction carries no artifact, publication,
/// load, launch, or runtime authority.
pub(crate) struct ProductionCompilation<'tcx, Stage> {
    stage: Stage,
    invariant_session: PhantomData<fn(TyCtxt<'tcx>) -> TyCtxt<'tcx>>,
}

/// Move-only production stage retaining rustc identities, transaction
/// bindings, admitted semantic MIR, and the owner-held verified PLIRON graph.
pub(crate) struct RankedVerifiedProductionCompilation {
    ranked: crate::production_ranked_projection_v1::ProductionRankedSemanticProgramV1,
    bindings: AuthenticatedProductionBindings,
}

/// Move-only production stage retaining exact semantic ownership, verified
/// Kernel IR, correspondence evidence, and the original transaction bindings.
pub(crate) struct TargetNeutralProductionCompilation {
    lowered: fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1,
    ranked_verification: crate::production_ranked_projection_v1::AuthenticatedRankedVerificationV5,
    bindings: AuthenticatedProductionBindings,
}

/// Move-only production stage retaining exact semantic ownership, verified
/// Kernel IR, composed formal/ranked memory evidence, and transaction bindings.
pub(crate) struct FormalMemoryAdmittedProductionCompilation {
    admitted: fe2o3_lower_mir_kernel::ProductionFormalMemoryOwnerV1,
    ranked_verification: crate::production_ranked_projection_v1::AuthenticatedRankedVerificationV5,
    bindings: AuthenticatedProductionBindings,
}

/// Move-only production stage retaining formal admission, exact target-bound
/// Kernel IR, deterministic gfx942 LLVM text, and transaction bindings.
pub(crate) struct Gfx942LoweredProductionCompilation {
    admitted: fe2o3_lower_mir_kernel::ProductionFormalMemoryOwnerV1,
    ranked_verification: crate::production_ranked_projection_v1::AuthenticatedRankedVerificationV5,
    target_module: fe2o3_kernel_ir::Module,
    llvm_ir: String,
    bindings: AuthenticatedProductionBindings,
}

/// Private handoff input that can only be constructed by the exact production
/// target-lowering stage. It grants no publication or artifact authority.
pub(crate) struct AuthenticatedProductionGfx942Module {
    admitted: fe2o3_lower_mir_kernel::ProductionFormalMemoryOwnerV1,
    target_module: fe2o3_kernel_ir::Module,
    llvm_ir: String,
    typed_descriptor_roots: Vec<crate::compiler_descriptor::TypedDescriptorRootV1>,
    compiler_ffi_envelope: Option<fe2o3_compiler_ffi::CompilerFfiEnvelopeV1>,
}

struct PreparedProductionWorkerPublication {
    producer: ProducerIdentity,
    output_dir: PathBuf,
    attempt: BuildAttempt,
    invocation: Box<AdmittedProtectedRustcInvocationV1>,
    semantic_lineage: crate::production_semantic_lineage_v3::PreparedProductionSemanticLineageV3,
    rustc_target: crate::production_target_v1::AuthenticatedProductionTargetV1,
    prepared: crate::production_worker_handoff::PreparedProductionWorkerHandoff,
}

impl AuthenticatedProductionGfx942Module {
    pub(crate) fn into_parts(
        self,
    ) -> (
        fe2o3_lower_mir_kernel::ProductionFormalMemoryOwnerV1,
        fe2o3_kernel_ir::Module,
        String,
        Vec<crate::compiler_descriptor::TypedDescriptorRootV1>,
        Option<fe2o3_compiler_ffi::CompilerFfiEnvelopeV1>,
    ) {
        (
            self.admitted,
            self.target_module,
            self.llvm_ir,
            self.typed_descriptor_roots,
            self.compiler_ffi_envelope,
        )
    }
}

impl TargetNeutralProductionCompilation {
    fn admit_formal_memory(
        self,
    ) -> Result<FormalMemoryAdmittedProductionCompilation, ProductionPipelineError> {
        let Self {
            lowered,
            ranked_verification,
            bindings,
        } = self;
        let admitted = fe2o3_lower_mir_kernel::ProductionFormalMemoryOwnerV1::try_admit(lowered)
            .map_err(ProductionPipelineError::FormalMemoryAdmission)?;
        Ok(FormalMemoryAdmittedProductionCompilation {
            admitted,
            ranked_verification,
            bindings,
        })
    }
}

impl FormalMemoryAdmittedProductionCompilation {
    fn lower_gfx942(self) -> Result<Gfx942LoweredProductionCompilation, ProductionPipelineError> {
        let Self {
            admitted,
            ranked_verification,
            bindings,
        } = self;
        let semantic = admitted.semantic_kir().semantic().semantic();
        let [semantic_root] = semantic.roots() else {
            return Err(ProductionPipelineError::Geometry(
                crate::production_geometry_v1::ProductionGeometryErrorV1::KernelClosure,
            ));
        };
        let semantic_function = semantic
            .functions()
            .get(semantic_root.index() as usize)
            .ok_or(ProductionPipelineError::Geometry(
                crate::production_geometry_v1::ProductionGeometryErrorV1::KernelClosure,
            ))?;
        let [typed_root] = bindings.typed_descriptor_roots.as_slice() else {
            return Err(ProductionPipelineError::Geometry(
                crate::production_geometry_v1::ProductionGeometryErrorV1::KernelClosure,
            ));
        };
        let source_launch = typed_root
            .source_launch()
            .ok_or(ProductionPipelineError::Geometry(
            crate::production_geometry_v1::ProductionGeometryErrorV1::NonExactDescriptorWorkgroup,
        ))?;
        crate::production_geometry_v1::derive_production_geometry_v1(
            admitted.semantic_kir().module(),
            semantic_function,
            source_launch,
        )
        .map_err(ProductionPipelineError::Geometry)?;

        let mut target_module = admitted.semantic_kir().module().clone();
        let target = fe2o3_kernel_ir::gfx942_xnack_minus_target_capability();
        let wave = fe2o3_kernel_ir::TargetCapability::WaveWidth(fe2o3_kernel_ir::WaveWidth::Wave64);
        target_module.required_capabilities.insert(target.clone());
        target_module.required_capabilities.insert(wave.clone());
        let [kernel] = target_module.kernels.as_mut_slice() else {
            return Err(ProductionPipelineError::Geometry(
                crate::production_geometry_v1::ProductionGeometryErrorV1::KernelClosure,
            ));
        };
        kernel.required_capabilities.insert(target.clone());
        kernel.required_capabilities.insert(wave.clone());
        let kernel_id = kernel.id.clone();
        let entry_id = kernel.entry.clone();
        let entry = target_module
            .functions
            .iter_mut()
            .find(|function| function.id == entry_id)
            .ok_or(ProductionPipelineError::Geometry(
                crate::production_geometry_v1::ProductionGeometryErrorV1::KernelClosure,
            ))?;
        entry.required_capabilities.insert(target);
        entry.required_capabilities.insert(wave);
        fe2o3_kernel_ir::verify_module(&target_module)
            .map_err(ProductionPipelineError::TargetBinding)?;
        let dialect_llvm_ir =
            dialect_amdgcn::lower_kernel_to_gfx942_xnack_minus_llvm_ir(&target_module, &kernel_id)
                .map_err(ProductionPipelineError::Gfx942Lowering)?;
        let llvm_ir = bind_production_upstream_llvm_layout_v1(dialect_llvm_ir)
            .map_err(ProductionPipelineError::UpstreamLlvmLayoutBinding)?;
        Ok(Gfx942LoweredProductionCompilation {
            admitted,
            ranked_verification,
            target_module,
            llvm_ir,
            bindings,
        })
    }
}

fn bind_production_upstream_llvm_layout_v1(dialect_llvm_ir: String) -> Result<String, String> {
    const TRIPLE_HEADER: &str = "target triple = \"amdgcn-amd-amdhsa\"\n";
    let dialect_layout = format!(
        "target datalayout = \"{}\"\n",
        dialect_amdgcn::GFX942_XNACK_MINUS_DATA_LAYOUT
    );
    let expected_prefix = format!("{TRIPLE_HEADER}{dialect_layout}\n");
    if !dialect_llvm_ir.starts_with(&expected_prefix)
        || dialect_llvm_ir.matches("target triple =").count() != 1
        || dialect_llvm_ir.matches("target datalayout =").count() != 1
    {
        return Err(
            "verified gfx942 lowering did not retain one canonical target header".to_owned(),
        );
    }

    let upstream_layout = crate::production_target_v1::PRODUCTION_RUSTC_DATA_LAYOUT_V1;
    let mut bound = String::with_capacity(
        dialect_llvm_ir.len() + upstream_layout.len().saturating_sub(dialect_layout.len()),
    );
    bound.push_str(TRIPLE_HEADER);
    bound.push_str("target datalayout = \"");
    bound.push_str(upstream_layout);
    bound.push_str("\"\n\n");
    bound.push_str(&dialect_llvm_ir[expected_prefix.len()..]);
    Ok(bound)
}

impl Gfx942LoweredProductionCompilation {
    pub(crate) fn module(&self) -> &fe2o3_kernel_ir::Module {
        &self.target_module
    }

    pub(crate) fn llvm_ir(&self) -> &str {
        &self.llvm_ir
    }

    pub(crate) fn workgroup_size(&self) -> Option<fe2o3_kernel_ir::WorkgroupSize> {
        self.target_module
            .kernels
            .first()
            .and_then(|kernel| kernel.workgroup_size)
    }

    pub(crate) fn semantic_function_count(&self) -> usize {
        self.admitted
            .semantic_kir()
            .semantic()
            .semantic()
            .functions()
            .len()
    }

    pub(crate) fn correspondence_block_count(&self) -> usize {
        self.admitted.semantic_kir().correspondence().blocks().len()
    }

    pub(crate) fn formal_witness_extent(&self) -> u64 {
        self.admitted.witness_extent()
    }

    pub(crate) fn formal_allocation_count(&self) -> usize {
        self.admitted.obligations().allocations().len()
    }

    pub(crate) fn formal_access_count(&self) -> usize {
        self.admitted.obligations().accesses().len()
    }

    pub(crate) fn ranked_dynamic_index_discharge_count(&self) -> usize {
        self.admitted.ranked_discharged_reasons().len()
    }

    pub(crate) fn runtime_bounds_requirement_count(&self) -> usize {
        self.admitted.obligations().bounds_requirements().len()
    }

    pub(crate) fn runtime_alias_requirement_count(&self) -> usize {
        self.admitted
            .obligations()
            .runtime_alias_requirements()
            .len()
    }

    pub(crate) fn inter_invocation_conflict_count(&self) -> usize {
        self.admitted
            .obligations()
            .inter_invocation_conflicts()
            .len()
    }

    pub(crate) fn retained_identity_and_transaction_binding_count(&self) -> usize {
        let _ = (
            &self.bindings.rustc_identity_inventory,
            &self.bindings.rustc_preflight_plan,
            &self.bindings.typed_descriptor_roots,
            &self.bindings.transaction.producer,
            &self.bindings.transaction.output_dir,
            &self.bindings.transaction.compiler_ffi_envelope,
        );
        6 + usize::from(
            self.bindings
                .transaction
                .compiler_custody
                .has_publication_attempt(),
        )
    }

    pub(crate) fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }

    fn prepare_worker_handoff(
        self,
    ) -> Result<PreparedProductionWorkerPublication, ProductionPipelineError> {
        eprintln!(
            "[rustc-codegen-fe2o3] production compilation lowered {} admitted semantic function(s) into verified target-neutral Kernel IR module `{}` with {} exact block correspondence record(s), then admitted composed formal/ranked memory evidence for a {}-invocation structural witness with {} allocation(s), {} formal access(es), {} ranked dynamic-index discharge(s), {} runtime bounds requirement(s), {} runtime alias requirement(s), and {} inter-invocation conflict(s), and lowered exact target-bound KIR with compiler-selected-or-retained workgroup {:?} to {} byte(s) of deterministic gfx942:xnack- LLVM text while retaining {} identity/transaction binding(s); artifact/launch authority {}; preparing exact compiler-module handoff",
            self.semantic_function_count(),
            self.module().id,
            self.correspondence_block_count(),
            self.formal_witness_extent(),
            self.formal_allocation_count(),
            self.formal_access_count(),
            self.ranked_dynamic_index_discharge_count(),
            self.runtime_bounds_requirement_count(),
            self.runtime_alias_requirement_count(),
            self.inter_invocation_conflict_count(),
            self.workgroup_size(),
            self.llvm_ir().len(),
            self.retained_identity_and_transaction_binding_count(),
            self.grants_artifact_or_launch_authority(),
        );
        let Self {
            admitted,
            ranked_verification,
            target_module,
            llvm_ir,
            bindings,
        } = self;
        let AuthenticatedProductionBindings {
            rustc_identity_inventory,
            rustc_preflight_plan,
            rustc_target,
            reference_effect_bindings,
            typed_descriptor_roots,
            transaction,
        } = bindings;
        let ProductionTransactionBindings {
            producer,
            output_dir,
            compiler_ffi_envelope,
            compiler_custody,
        } = transaction;
        if rustc_preflight_plan.rustc_identity_inventory_sha256()
            != rustc_identity_inventory.sha256()
        {
            return Err(ProductionPipelineError::RustcLineageMismatch);
        }
        drop(reference_effect_bindings);
        let (attempt, invocation) = compiler_custody.into_publication_custody()?;
        let semantic_lineage = crate::production_semantic_lineage_v3::PreparedProductionSemanticLineageV3::try_prepare(
            &rustc_identity_inventory,
            &rustc_preflight_plan,
            &rustc_target,
            &ranked_verification,
            &admitted,
            &target_module,
            &llvm_ir,
        )
        .map_err(ProductionPipelineError::SemanticLineage)?;
        let compiler_module = AuthenticatedProductionGfx942Module {
            admitted,
            target_module,
            llvm_ir,
            typed_descriptor_roots,
            compiler_ffi_envelope,
        };
        let prepared =
            crate::production_worker_handoff::prepare_production_worker_handoff(compiler_module)
                .map_err(ProductionPipelineError::WorkerHandoff)?;
        Ok(PreparedProductionWorkerPublication {
            producer,
            output_dir,
            attempt,
            invocation,
            semantic_lineage,
            rustc_target,
            prepared,
        })
    }

    fn publish_worker_handoff(
        self,
    ) -> Result<fe2o3_artifact_transaction::CompilerModuleHandoffReceiptV3, ProductionPipelineError>
    {
        let publication = self.prepare_worker_handoff()?;
        let invocation = (*publication.invocation)
            .finish_for_publication()
            .map_err(ProductionPipelineError::ProtectedRustcInvocation)?;
        invocation.revalidate().map_err(|detail| {
            ProductionPipelineError::ProtectedRustcInvocation(
                ProtectedRustcInvocationErrorV1::RetainedCapabilityChanged(detail),
            )
        })?;
        let invocation_descriptor = invocation.descriptor().clone();
        let (module_handoff, compiler_descriptor_source) = publication
            .prepared
            .into_validated_parts()
            .map_err(ProductionPipelineError::WorkerHandoff)?;
        let strict_handoff = publication
            .semantic_lineage
            .finish(
                invocation_descriptor,
                publication.rustc_target.device_target(),
                &compiler_descriptor_source,
                module_handoff,
            )
            .map_err(ProductionPipelineError::SemanticLineage)?;
        fe2o3_artifact_transaction::publish_compiler_module_handoff_v3(
            &publication.output_dir,
            &publication.producer,
            publication.attempt,
            &strict_handoff,
        )
        .map_err(ProductionPipelineError::StrictV3Publication)
    }
}

impl RankedVerifiedProductionCompilation {
    pub(crate) fn ranked_ir(&self) -> &str {
        self.ranked.ranked_ir()
    }

    pub(crate) fn function_name(&self) -> &str {
        self.ranked.function_name()
    }

    pub(crate) fn semantic_function_count(&self) -> usize {
        self.ranked.semantic_function_count()
    }

    pub(crate) fn semantic_callable_count(&self) -> usize {
        self.ranked.semantic_callable_count()
    }

    pub(crate) fn bounds_are_clean(&self) -> bool {
        self.ranked.bounds_are_clean()
    }

    pub(crate) fn all_kernel_checks_are_clean(&self) -> bool {
        self.ranked.all_kernel_checks_are_clean()
    }

    pub(crate) fn retained_identity_and_transaction_binding_count(&self) -> usize {
        let _ = (
            &self.bindings.rustc_identity_inventory,
            &self.bindings.rustc_preflight_plan,
            &self.bindings.typed_descriptor_roots,
            &self.bindings.transaction.producer,
            &self.bindings.transaction.output_dir,
            &self.bindings.transaction.compiler_ffi_envelope,
        );
        6 + usize::from(
            self.bindings
                .transaction
                .compiler_custody
                .has_publication_attempt(),
        )
    }

    pub(crate) fn grants_artifact_or_launch_authority(&self) -> bool {
        self.ranked.grants_artifact_or_launch_authority()
    }
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    /// Retains the collector-sealed closure without granting semantic authority.
    /// The next transition must authenticate every imported MIR fact.
    pub(crate) fn from_collected_device_closure(
        tcx: TyCtxt<'tcx>,
        closure: AuthenticatedCollectedKernelClosureV1<'tcx>,
        producer: ProducerIdentity,
        output_dir: PathBuf,
        build_attempt: BuildAttempt,
        invocation: AdmittedProtectedRustcInvocationV1,
    ) -> Result<Self, ProductionPipelineError> {
        Self::from_collected_device_closure_with_custody(
            tcx,
            closure,
            producer,
            output_dir,
            ProductionCompilerCustody::protected(invocation, build_attempt),
        )
    }

    pub(crate) fn from_collected_device_closure_for_extraction(
        tcx: TyCtxt<'tcx>,
        closure: AuthenticatedCollectedKernelClosureV1<'tcx>,
        producer: ProducerIdentity,
        output_dir: PathBuf,
    ) -> Result<Self, ProductionPipelineError> {
        Self::from_collected_device_closure_with_custody(
            tcx,
            closure,
            producer,
            output_dir,
            ProductionCompilerCustody::extraction_only(),
        )
    }

    fn from_collected_device_closure_with_custody(
        tcx: TyCtxt<'tcx>,
        closure: AuthenticatedCollectedKernelClosureV1<'tcx>,
        producer: ProducerIdentity,
        output_dir: PathBuf,
        compiler_custody: ProductionCompilerCustody,
    ) -> Result<Self, ProductionPipelineError> {
        if closure.function_count() == 0 {
            return Err(ProductionPipelineError::EmptyCollectedDeviceClosure);
        }
        let typed_descriptor_roots = closure
            .rederive_typed_descriptor_roots(tcx)
            .map_err(ProductionPipelineError::DescriptorEvidence)?;
        let compiler_ffi_envelope = closure.compiler_ffi_observation().cloned();
        Ok(Self {
            stage: CollectedRustStage {
                tcx,
                closure,
                typed_descriptor_roots,
                transaction: ProductionTransactionBindings {
                    producer,
                    output_dir,
                    compiler_ffi_envelope,
                    compiler_custody,
                },
            },
            invariant_session: PhantomData,
        })
    }

    fn import_semantic_mir(
        self,
    ) -> Result<ProductionCompilation<'tcx, AdmittedSemanticMirStage>, ProductionPipelineError>
    {
        let CollectedRustStage {
            tcx,
            closure,
            typed_descriptor_roots,
            transaction,
        } = self.stage;
        let (
            semantic_mir,
            rustc_identity_inventory,
            rustc_preflight_plan,
            rustc_target,
            reference_effect_bindings,
        ) = crate::collector::construct_production_semantic_mir_v1(tcx, closure)
            .map_err(ProductionPipelineError::SemanticImport)?;
        Ok(ProductionCompilation {
            stage: AdmittedSemanticMirStage {
                semantic_mir,
                bindings: AuthenticatedProductionBindings {
                    rustc_identity_inventory,
                    rustc_preflight_plan,
                    rustc_target,
                    reference_effect_bindings,
                    typed_descriptor_roots,
                    transaction,
                },
            },
            invariant_session: PhantomData,
        })
    }

    /// Consumes the only production transaction through import and verification.
    pub(crate) fn verify_general_kernel_checks(
        self,
    ) -> Result<RankedVerifiedProductionCompilation, ProductionPipelineError> {
        let admitted = self.import_semantic_mir()?;
        admitted
            .construct_semantic_middle_end()?
            .verify_general_kernel_checks()
    }

    /// Consumes the sole production transaction through exact semantic MIR,
    /// formal memory admission, and exact gfx942 LLVM lowering.
    pub(crate) fn lower_gfx942(
        self,
    ) -> Result<Gfx942LoweredProductionCompilation, ProductionPipelineError> {
        let admitted = self.import_semantic_mir()?;
        admitted
            .construct_semantic_middle_end()?
            .verify_general_kernel_checks()?
            .lower_target_neutral()?
            .admit_formal_memory()?
            .lower_gfx942()
    }

    /// Publishes the exact production compiler module into the managed,
    /// preselected attempt-scoped protocol. This grants no link, artifact, load,
    /// or launch authority.
    pub(crate) fn publish_worker_handoff(
        self,
    ) -> Result<fe2o3_artifact_transaction::CompilerModuleHandoffReceiptV3, ProductionPipelineError>
    {
        self.lower_gfx942()?.publish_worker_handoff()
    }

    /// Retains the original extraction milestone while consuming the same
    /// transaction and importer as the production backend.
    pub(crate) fn require_semantic_mir_import(self) -> ProductionPipelineError {
        match self.import_semantic_mir() {
            Ok(transaction) => match transaction.construct_semantic_middle_end() {
                Ok(transaction) => transaction.require_target_neutral_lowering(),
                Err(error) => error,
            },
            Err(error) => error,
        }
    }
}

impl<'tcx> ProductionCompilation<'tcx, AdmittedSemanticMirStage> {
    fn construct_semantic_middle_end(
        self,
    ) -> Result<ProductionCompilation<'tcx, EquivalentSemanticMirStage>, ProductionPipelineError>
    {
        let AdmittedSemanticMirStage {
            semantic_mir,
            bindings,
        } = self.stage;
        let semantic_mir = fe2o3_pliron::ProductionSemanticMirOwnerV1::try_new(
            semantic_mir,
            fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
        )
        .map_err(ProductionPipelineError::SemanticMiddleEnd)?;
        Ok(ProductionCompilation {
            stage: EquivalentSemanticMirStage {
                semantic_mir,
                bindings,
            },
            invariant_session: PhantomData,
        })
    }
}

impl<'tcx> ProductionCompilation<'tcx, EquivalentSemanticMirStage> {
    fn require_target_neutral_lowering(self) -> ProductionPipelineError {
        let EquivalentSemanticMirStage {
            semantic_mir,
            bindings,
        } = self.stage;
        let error =
            crate::collector::ProductionSemanticImportErrorV1::TargetNeutralLoweringPending {
                functions: semantic_mir.semantic().functions().len(),
                callables: semantic_mir.semantic().callables().len(),
                rustc_identity_inventory_sha256: bindings.rustc_identity_inventory.sha256(),
                rustc_preflight_plan_sha256: bindings.rustc_preflight_plan.sha256(),
                semantic_sha256: *semantic_mir.semantic().semantic_sha256().as_bytes(),
            };
        drop((semantic_mir, bindings));
        ProductionPipelineError::SemanticImport(error)
    }

    fn verify_general_kernel_checks(
        self,
    ) -> Result<RankedVerifiedProductionCompilation, ProductionPipelineError> {
        let EquivalentSemanticMirStage {
            semantic_mir,
            bindings,
        } = self.stage;
        crate::compiler_descriptor::validate_production_v1_semantic_ownership_evidence(
            &bindings.typed_descriptor_roots,
            semantic_mir.semantic(),
        )
        .map_err(ProductionPipelineError::DescriptorEvidence)?;
        let [typed_root] = bindings.typed_descriptor_roots.as_slice() else {
            return Err(ProductionPipelineError::Geometry(
                crate::production_geometry_v1::ProductionGeometryErrorV1::KernelClosure,
            ));
        };
        let source_rank = typed_root
            .source_launch()
            .ok_or(ProductionPipelineError::Geometry(
                crate::production_geometry_v1::ProductionGeometryErrorV1::NonExactDescriptorWorkgroup,
            ))?
            .rank();
        let ranked =
            crate::production_ranked_projection_v1::project_and_verify_ranked_semantic_mir_v1(
                semantic_mir,
                source_rank,
                &bindings.reference_effect_bindings,
            )
            .map_err(ProductionPipelineError::RankedProjection)?;
        Ok(RankedVerifiedProductionCompilation { ranked, bindings })
    }
}

impl RankedVerifiedProductionCompilation {
    fn lower_target_neutral(
        self,
    ) -> Result<TargetNeutralProductionCompilation, ProductionPipelineError> {
        let Self { ranked, bindings } = self;
        let (receipt, ranked_verification) = ranked
            .into_verified_receipt()
            .map_err(ProductionPipelineError::RankedVerification)?;
        debug_assert_eq!(
            ranked_verification.has_authenticated_functional_verification(),
            receipt
                .lowering()
                .has_retained_policy_checked_refinement_staging()
        );
        debug_assert!(ranked_verification.retained_functional_verification_is_coherent());
        let [typed_root] = bindings.typed_descriptor_roots.as_slice() else {
            return Err(ProductionPipelineError::Geometry(
                crate::production_geometry_v1::ProductionGeometryErrorV1::KernelClosure,
            ));
        };
        let source_rank = typed_root
            .source_launch()
            .ok_or(ProductionPipelineError::Geometry(
                crate::production_geometry_v1::ProductionGeometryErrorV1::NonExactDescriptorWorkgroup,
            ))?
            .rank();
        let lowered =
            fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1::try_lower_after_ranked_checks(
                receipt,
                fe2o3_lower_mir_kernel::ProductionSemanticKirLimitsV1::default(),
                source_rank,
            )
            .map_err(ProductionPipelineError::TargetNeutralLowering)?;
        Ok(TargetNeutralProductionCompilation {
            lowered,
            ranked_verification,
            bindings,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_only_and_device_dispositions_are_exact() {
        assert_eq!(disposition(0), ProductionDisposition::HostOnly);
        assert_eq!(disposition(1), ProductionDisposition::DeviceTransaction);
        assert_eq!(
            disposition(usize::MAX),
            ProductionDisposition::DeviceTransaction
        );
    }

    #[test]
    fn private_production_implementation_is_unversioned() {
        let backend = include_str!("lib.rs");
        let pipeline = include_str!("production_pipeline.rs");
        assert!(backend.contains("mod production_pipeline;"));
        for retired in [
            concat!("production_pipeline", "_v1"),
            concat!("ProductionPipelineError", "V1"),
            concat!("ProductionCompilation", "V1"),
            concat!("ProductionDisposition", "V1"),
            concat!("ProductionCompilerCustody", "V1"),
            concat!("RetainedProductionDeviceAdmission", "V1"),
        ] {
            assert!(!backend.contains(retired), "backend retains {retired}");
            assert!(!pipeline.contains(retired), "pipeline retains {retired}");
        }
    }

    #[test]
    fn custom_llvm_configuration_is_terminal_before_construction() {
        assert!(reject_custom_llvm_configuration(false).is_ok());
        assert!(matches!(
            reject_custom_llvm_configuration(true),
            Err(ProductionPipelineError::CustomLlvmConfiguration)
        ));
    }

    #[test]
    fn production_layout_binding_uses_the_authenticated_upstream_spelling() {
        let legacy = format!(
            "target triple = \"amdgcn-amd-amdhsa\"\ntarget datalayout = \"{}\"\n\ndefine void @body() {{ ret void }}\n",
            dialect_amdgcn::GFX942_XNACK_MINUS_DATA_LAYOUT
        );
        let bound = bind_production_upstream_llvm_layout_v1(legacy).unwrap();
        assert!(bound.starts_with(&format!(
            "target triple = \"amdgcn-amd-amdhsa\"\ntarget datalayout = \"{}\"\n\n",
            crate::production_target_v1::PRODUCTION_RUSTC_DATA_LAYOUT_V1
        )));
        assert!(bound.ends_with("define void @body() { ret void }\n"));
        assert_eq!(bound.matches("target triple =").count(), 1);
        assert_eq!(bound.matches("target datalayout =").count(), 1);
    }

    #[test]
    fn production_layout_binding_rejects_noncanonical_headers() {
        let canonical = format!(
            "target triple = \"amdgcn-amd-amdhsa\"\ntarget datalayout = \"{}\"\n\ndefine void @body() {{ ret void }}\n",
            dialect_amdgcn::GFX942_XNACK_MINUS_DATA_LAYOUT
        );
        for hostile in [
            canonical.replacen("target triple", "source_filename", 1),
            canonical.replacen("\n\n", "\n", 1),
            format!("{canonical}target datalayout = \"e-p:64:64\"\n"),
        ] {
            assert!(bind_production_upstream_llvm_layout_v1(hostile).is_err());
        }
    }

    #[test]
    fn worker_publication_cannot_bypass_general_pliron_checks() {
        let source = include_str!("production_pipeline.rs");
        let transaction = source
            .split("pub(crate) fn lower_gfx942(")
            .nth(1)
            .expect("gfx942 production transaction")
            .split("pub(crate) fn publish_worker_handoff(")
            .next()
            .expect("bounded transaction body");
        let verify = transaction
            .find(".verify_general_kernel_checks()?")
            .expect("mandatory general PLIRON checks");
        let lower = transaction
            .find(".lower_target_neutral()?")
            .expect("target-neutral lowering");
        assert!(verify < lower, "lowering ran before general PLIRON checks");
        assert!(
            include_str!("production_ranked_projection_v1.rs")
                .contains("prepare_reference_effect_request_v2")
        );
    }

    #[test]
    fn referenced_kernels_complete_all_functional_gates_before_kir_lowering() {
        let projection = include_str!("production_ranked_projection_v1.rs");
        let semantic = projection
            .find("derive_and_reconcile_mir_pliron_semantic_contract_v1")
            .expect("compiler-owned semantic-contract derivation");
        let parallel = projection
            .find("derive_and_require_parallel_reference_contract_v1")
            .expect("compiler-owned parallel-contract derivation");
        let aggregate = projection
            .find("authenticate_mir_pliron_contract_per_compilation_v1")
            .expect("aggregate per-compilation Verus gate");
        assert!(semantic < parallel && parallel < aggregate);

        let pipeline = include_str!("production_pipeline.rs");
        let verification = pipeline
            .find(".into_verified_receipt()")
            .expect("ranked verification transition");
        let lowering = pipeline
            .find("ProductionSemanticKirOwnerV1::try_lower_after_ranked_checks")
            .expect("KIR lowering transition");
        assert!(
            verification < lowering,
            "KIR lowering ran before functional verification"
        );
    }

    #[test]
    fn production_publication_has_one_protected_custody_path() {
        let pipeline = include_str!("production_pipeline.rs");
        let worker = include_str!("production_worker_handoff.rs");
        for removed in [
            concat!("ProductionCompilerModule", "PublicationV1"),
            concat!("PreparedProductionCompiler", "PublicationV1"),
            concat!("ProtectedHandoff", "RequiresV2"),
            concat!("UnprotectedHandoff", "RequiresV1"),
            concat!("publish_worker_handoff", "_v3"),
            concat!("publish_prepared_production_v1", "_worker_handoff("),
            concat!("PreparedProductionV1", "WorkerHandoffV1"),
            concat!("PreparedProductionLineage", "WorkerHandoffV3"),
            concat!("prepare_production_v1", "_worker_handoff"),
        ] {
            assert!(
                !pipeline.contains(removed) && !worker.contains(removed),
                "obsolete production publication variant remains: {removed}",
            );
        }
        assert!(pipeline.contains("ProductionCompilerCustody::protected(invocation)"));
        assert!(pipeline.contains(concat!("publish_compiler_module_handoff", "_v3")));
    }

    #[test]
    fn production_module_contains_no_profile_selection_vocabulary() {
        let sources = [
            include_str!("production_pipeline.rs"),
            include_str!("collector/production_importer_v1.rs"),
            include_str!("rustc_semantic_adapter_v1.rs"),
            include_str!("rustc_semantic_plan_v1.rs"),
            include_str!("production_semantic_fn_abi_v1.rs"),
            include_str!("production_semantic_types_v1.rs"),
            include_str!("production_semantic_terminal_v1.rs"),
            include_str!("reference_effect_v1.rs"),
        ];
        for forbidden in [
            concat!("General", "Gemm"),
            concat!("Flash", "Attention"),
            concat!("Row", "Softmax"),
            concat!("Moe", "Top2"),
            concat!("export", "_name"),
            concat!("source", " substring"),
            concat!("MIR", " transcript"),
            concat!("legacy", "-v1"),
            concat!("kernel-ir", "-v1"),
            concat!("Collection", "Result"),
            concat!("target: AmdGpu", "Target"),
        ] {
            assert!(
                !sources[0].contains(forbidden),
                "production transaction contains forbidden selector term {forbidden:?}"
            );
        }

        for forbidden_importer_term in [
            concat!("General", "Gemm"),
            concat!("Flash", "Attention"),
            concat!("Row", "Softmax"),
            concat!("Moe", "Top2"),
            concat!("source", " substring"),
            concat!("MIR", " transcript"),
            concat!("legacy", "-v1"),
            concat!("kernel-ir", "-v1"),
        ] {
            assert!(
                sources
                    .iter()
                    .skip(1)
                    .all(|source| !source.contains(forbidden_importer_term)),
                "production importer contains forbidden selector term {forbidden_importer_term:?}"
            );
        }

        for forbidden_dependency in [
            concat!("mir_import", "_v2"),
            concat!("same_session", "_rustc_v1"),
            concat!("frontend_record", "_bridge"),
            concat!("semantic_type", "_adapter_v2"),
            concat!("source_", "debug"),
            concat!("semantic_", "features"),
            concat!("crate::", "collected_"),
            concat!("collected_", "general_gemm_v1"),
        ] {
            assert!(
                sources
                    .iter()
                    .skip(1)
                    .all(|source| !source.contains(forbidden_dependency)),
                "production importer depends on qualification module {forbidden_dependency:?}"
            );
        }
    }

    #[test]
    fn production_backend_authenticates_target_before_monomorphization() {
        let backend = include_str!("lib.rs");
        let codegen = backend
            .split_once("fn codegen_crate")
            .expect("codegen entry")
            .1;
        let authentication = codegen
            .find("authenticate_before_collection")
            .expect("pre-collection target authentication");
        let monomorphization = codegen
            .find("collect_and_partition_mono_items")
            .expect("rustc monomorphization");
        assert!(authentication < monomorphization);
    }

    #[test]
    fn process_isolated_extraction_uses_the_production_transaction() {
        let driver = include_str!("production_rustc_driver_v1.rs");
        for required in [
            "reject_custom_llvm_configuration",
            "ProductionCompilation::from_collected_device_closure_for_extraction",
            "require_semantic_mir_import",
        ] {
            assert!(
                driver.contains(required),
                "production extraction driver bypassed required transaction step {required:?}",
            );
        }
        for forbidden in [
            "construct_production_semantic_mir_v1",
            "require_production_semantic_import_v1",
        ] {
            assert!(
                !driver.contains(forbidden),
                "production extraction driver directly called importer entry {forbidden:?}",
            );
        }
    }
}

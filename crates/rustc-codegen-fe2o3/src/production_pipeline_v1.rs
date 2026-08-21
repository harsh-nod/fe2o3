//! Single production-pipeline transaction shell.
//!
//! This module owns the one integration point for issue #175. It deliberately
//! contains no workload recognition. The sole semantic-MIR importer owns the
//! consuming target-authentication boundary and moves an admitted request into
//! a typed stage before generic ranked-memory verification.

use std::fmt;
use std::marker::PhantomData;
use std::path::PathBuf;

use rustc_middle::ty::TyCtxt;

use crate::artifact_transaction::{BuildAttempt, ProducerIdentity};
use crate::collector::AuthenticatedCollectedKernelClosureV1;

pub(crate) const PRODUCTION_PIPELINE_V1: &str = "production-v1";
const PRODUCTION_GFX942_DEFAULT_WORKGROUP_X_V1: u32 = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProductionDispositionV1 {
    HostOnly,
    DeviceTransaction,
}

pub(crate) const fn disposition(device_candidate_count: usize) -> ProductionDispositionV1 {
    if device_candidate_count == 0 {
        ProductionDispositionV1::HostOnly
    } else {
        ProductionDispositionV1::DeviceTransaction
    }
}

#[derive(Debug)]
pub(crate) enum ProductionPipelineErrorV1 {
    CustomLlvmConfiguration,
    EmptyCollectedDeviceClosure,
    SemanticImport(crate::collector::ProductionSemanticImportErrorV1),
    SemanticMiddleEnd(fe2o3_pliron::ProductionSemanticMirErrorV1),
    RankedProjection(crate::production_ranked_projection_v1::ProductionRankedProjectionErrorV1),
    TargetNeutralLowering(fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1),
    FormalMemoryAdmission(fe2o3_lower_mir_kernel::ProductionFormalMemoryErrorV1),
    TargetBinding(fe2o3_kernel_ir::VerificationErrors),
    Gfx942Lowering(dialect_amdgcn::LoweringErrors),
    UpstreamLlvmLayoutBinding(String),
    WorkerHandoff(crate::worker_v2_producer::WorkerV2ProducerError),
}

impl fmt::Display for ProductionPipelineErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CustomLlvmConfiguration => formatter.write_str(
                "production-v1 rejects caller-selected LLVM arguments or passes before transaction construction",
            ),
            Self::EmptyCollectedDeviceClosure => formatter.write_str(
                "production-v1 requires a nonempty collector-sealed device closure",
            ),
            Self::SemanticImport(error) => write!(formatter, "production-v1 {error}"),
            Self::SemanticMiddleEnd(error) => {
                write!(formatter, "production-v1 exact semantic middle end failed: {error}")
            }
            Self::RankedProjection(error) => {
                write!(formatter, "production-v1 ranked-memory verification failed: {error}")
            }
            Self::TargetNeutralLowering(error) => {
                write!(formatter, "production-v1 target-neutral lowering failed: {error}")
            }
            Self::FormalMemoryAdmission(error) => {
                write!(formatter, "production-v1 formal memory admission failed: {error}")
            }
            Self::TargetBinding(error) => {
                write!(formatter, "production-v1 gfx942 target binding failed: {error}")
            }
            Self::Gfx942Lowering(error) => {
                write!(formatter, "production-v1 gfx942 LLVM lowering failed: {error}")
            }
            Self::UpstreamLlvmLayoutBinding(error) => {
                write!(formatter, "production-v1 upstream LLVM layout binding failed: {error}")
            }
            Self::WorkerHandoff(error) => {
                write!(formatter, "production-v1 Worker V2 handoff failed: {error}")
            }
        }
    }
}

impl std::error::Error for ProductionPipelineErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SemanticImport(error) => Some(error),
            Self::SemanticMiddleEnd(error) => Some(error),
            Self::RankedProjection(error) => Some(error),
            Self::TargetNeutralLowering(error) => Some(error),
            Self::FormalMemoryAdmission(error) => Some(error),
            Self::TargetBinding(error) => Some(error),
            Self::Gfx942Lowering(error) => Some(error),
            Self::WorkerHandoff(error) => Some(error),
            Self::CustomLlvmConfiguration
            | Self::EmptyCollectedDeviceClosure
            | Self::UpstreamLlvmLayoutBinding(_) => None,
        }
    }
}

pub(crate) fn reject_custom_llvm_configuration(
    has_custom_llvm_configuration: bool,
) -> Result<(), ProductionPipelineErrorV1> {
    if has_custom_llvm_configuration {
        Err(ProductionPipelineErrorV1::CustomLlvmConfiguration)
    } else {
        Ok(())
    }
}

pub(super) struct CollectedRustStageV1<'tcx> {
    tcx: TyCtxt<'tcx>,
    closure: AuthenticatedCollectedKernelClosureV1<'tcx>,
    transaction: ProductionTransactionBindingsV1,
}

struct ProductionTransactionBindingsV1 {
    producer: ProducerIdentity,
    output_dir: PathBuf,
    build_attempt: Option<BuildAttempt>,
    compiler_ffi_envelope: Option<fe2o3_compiler_ffi::CompilerFfiEnvelopeV1>,
}

struct AuthenticatedProductionBindingsV1 {
    rustc_identity_inventory_sha256: [u8; 32],
    rustc_preflight_plan_sha256: [u8; 32],
    transaction: ProductionTransactionBindingsV1,
}

pub(super) struct AdmittedSemanticMirStageV1 {
    semantic_mir: fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1,
    bindings: AuthenticatedProductionBindingsV1,
}

pub(super) struct EquivalentSemanticMirStageV1 {
    semantic_mir: fe2o3_pliron::ProductionSemanticMirOwnerV1,
    bindings: AuthenticatedProductionBindingsV1,
}

/// Move-only owner of one production compilation stage.
///
/// Its fields and stage types stay private so no caller can synthesize or
/// bypass a transition. The transaction carries no artifact, publication,
/// load, launch, or runtime authority.
pub(crate) struct ProductionCompilationV1<'tcx, Stage> {
    stage: Stage,
    invariant_session: PhantomData<fn(TyCtxt<'tcx>) -> TyCtxt<'tcx>>,
}

/// Move-only production stage retaining rustc identities, transaction
/// bindings, admitted semantic MIR, and the owner-held verified PLIRON graph.
pub(crate) struct RankedVerifiedProductionCompilationV1 {
    ranked: crate::production_ranked_projection_v1::ProductionRankedSemanticProgramV1,
    bindings: AuthenticatedProductionBindingsV1,
}

/// Move-only production stage retaining exact semantic ownership, verified
/// Kernel IR, correspondence evidence, and the original transaction bindings.
pub(crate) struct TargetNeutralProductionCompilationV1 {
    lowered: fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1,
    bindings: AuthenticatedProductionBindingsV1,
}

/// Move-only production stage retaining exact semantic ownership, verified
/// Kernel IR, complete formal memory obligations, and transaction bindings.
pub(crate) struct FormalMemoryAdmittedProductionCompilationV1 {
    admitted: fe2o3_lower_mir_kernel::ProductionFormalMemoryOwnerV1,
    bindings: AuthenticatedProductionBindingsV1,
}

/// Move-only production stage retaining formal admission, exact target-bound
/// Kernel IR, deterministic gfx942 LLVM text, and transaction bindings.
pub(crate) struct Gfx942LoweredProductionCompilationV1 {
    admitted: fe2o3_lower_mir_kernel::ProductionFormalMemoryOwnerV1,
    target_module: fe2o3_kernel_ir::Module,
    llvm_ir: String,
    bindings: AuthenticatedProductionBindingsV1,
}

/// Private handoff input that can only be constructed by the exact production
/// target-lowering stage. It grants no publication or artifact authority.
pub(crate) struct AuthenticatedProductionGfx942ModuleV1 {
    target_module: fe2o3_kernel_ir::Module,
    llvm_ir: String,
    compiler_ffi_envelope: Option<fe2o3_compiler_ffi::CompilerFfiEnvelopeV1>,
}

impl AuthenticatedProductionGfx942ModuleV1 {
    pub(crate) fn into_parts(
        self,
    ) -> (
        fe2o3_kernel_ir::Module,
        String,
        Option<fe2o3_compiler_ffi::CompilerFfiEnvelopeV1>,
    ) {
        (self.target_module, self.llvm_ir, self.compiler_ffi_envelope)
    }
}

impl TargetNeutralProductionCompilationV1 {
    fn admit_formal_memory(
        self,
    ) -> Result<FormalMemoryAdmittedProductionCompilationV1, ProductionPipelineErrorV1> {
        let Self { lowered, bindings } = self;
        let admitted = fe2o3_lower_mir_kernel::ProductionFormalMemoryOwnerV1::try_admit(lowered)
            .map_err(ProductionPipelineErrorV1::FormalMemoryAdmission)?;
        Ok(FormalMemoryAdmittedProductionCompilationV1 { admitted, bindings })
    }
}

impl FormalMemoryAdmittedProductionCompilationV1 {
    fn lower_gfx942(
        self,
    ) -> Result<Gfx942LoweredProductionCompilationV1, ProductionPipelineErrorV1> {
        let Self { admitted, bindings } = self;
        let mut target_module = admitted.semantic_kir().module().clone();
        let target = fe2o3_kernel_ir::gfx942_xnack_minus_target_capability();
        let wave = fe2o3_kernel_ir::TargetCapability::WaveWidth(fe2o3_kernel_ir::WaveWidth::Wave64);
        target_module.required_capabilities.insert(target.clone());
        target_module.required_capabilities.insert(wave.clone());
        let kernel = target_module
            .kernels
            .first_mut()
            .expect("formal admission requires exactly one kernel");
        kernel.workgroup_size.get_or_insert_with(|| {
            fe2o3_kernel_ir::WorkgroupSize::new(PRODUCTION_GFX942_DEFAULT_WORKGROUP_X_V1, 1, 1)
        });
        kernel.required_capabilities.insert(target.clone());
        kernel.required_capabilities.insert(wave.clone());
        let kernel_id = kernel.id.clone();
        let entry_id = kernel.entry.clone();
        let entry = target_module
            .functions
            .iter_mut()
            .find(|function| function.id == entry_id)
            .expect("verified formal admission retains the kernel entry");
        entry.required_capabilities.insert(target);
        entry.required_capabilities.insert(wave);
        fe2o3_kernel_ir::verify_module(&target_module)
            .map_err(ProductionPipelineErrorV1::TargetBinding)?;
        let legacy_llvm_ir =
            dialect_amdgcn::lower_kernel_to_gfx942_xnack_minus_llvm_ir(&target_module, &kernel_id)
                .map_err(ProductionPipelineErrorV1::Gfx942Lowering)?;
        let llvm_ir = bind_production_upstream_llvm_layout_v1(legacy_llvm_ir)
            .map_err(ProductionPipelineErrorV1::UpstreamLlvmLayoutBinding)?;
        Ok(Gfx942LoweredProductionCompilationV1 {
            admitted,
            target_module,
            llvm_ir,
            bindings,
        })
    }
}

fn bind_production_upstream_llvm_layout_v1(legacy_llvm_ir: String) -> Result<String, String> {
    const TRIPLE_HEADER: &str = "target triple = \"amdgcn-amd-amdhsa\"\n";
    let legacy_layout = format!(
        "target datalayout = \"{}\"\n",
        dialect_amdgcn::GFX942_XNACK_MINUS_DATA_LAYOUT
    );
    let expected_prefix = format!("{TRIPLE_HEADER}{legacy_layout}\n");
    if !legacy_llvm_ir.starts_with(&expected_prefix)
        || legacy_llvm_ir.matches("target triple =").count() != 1
        || legacy_llvm_ir.matches("target datalayout =").count() != 1
    {
        return Err(
            "verified gfx942 lowering did not retain one canonical target header".to_owned(),
        );
    }

    let upstream_layout = crate::production_target_v1::PRODUCTION_RUSTC_DATA_LAYOUT_V1;
    let mut bound = String::with_capacity(
        legacy_llvm_ir.len() + upstream_layout.len().saturating_sub(legacy_layout.len()),
    );
    bound.push_str(TRIPLE_HEADER);
    bound.push_str("target datalayout = \"");
    bound.push_str(upstream_layout);
    bound.push_str("\"\n\n");
    bound.push_str(&legacy_llvm_ir[expected_prefix.len()..]);
    Ok(bound)
}

impl Gfx942LoweredProductionCompilationV1 {
    pub(crate) fn module(&self) -> &fe2o3_kernel_ir::Module {
        &self.target_module
    }

    pub(crate) fn llvm_ir(&self) -> &str {
        &self.llvm_ir
    }

    pub(crate) fn workgroup_size(&self) -> fe2o3_kernel_ir::WorkgroupSize {
        self.target_module.kernels[0]
            .workgroup_size
            .expect("gfx942 lowering requires an exact workgroup size")
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
            &self.bindings.rustc_identity_inventory_sha256,
            &self.bindings.rustc_preflight_plan_sha256,
            &self.bindings.transaction.producer,
            &self.bindings.transaction.output_dir,
            &self.bindings.transaction.compiler_ffi_envelope,
        );
        5 + usize::from(self.bindings.transaction.build_attempt.is_some())
    }

    pub(crate) fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }

    fn publish_worker_handoff(
        self,
    ) -> Result<fe2o3_artifact_transaction::CompilerModuleHandoffReceiptV1, ProductionPipelineErrorV1>
    {
        eprintln!(
            "[rustc-codegen-fe2o3] production-v1 lowered {} admitted semantic function(s) into verified target-neutral Kernel IR module `{}` with {} exact block correspondence record(s), then admitted complete formal memory obligations for a {}-invocation structural witness with {} allocation(s), {} access(es), {} runtime bounds requirement(s), {} runtime alias requirement(s), and {} inter-invocation conflict(s), and lowered exact target-bound KIR with compiler-selected-or-retained workgroup {:?} to {} byte(s) of deterministic gfx942:xnack- LLVM text while retaining {} identity/transaction binding(s); artifact/launch authority {}; preparing exact Worker V2 handoff",
            self.semantic_function_count(),
            self.module().id,
            self.correspondence_block_count(),
            self.formal_witness_extent(),
            self.formal_allocation_count(),
            self.formal_access_count(),
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
            target_module,
            llvm_ir,
            bindings,
        } = self;
        let AuthenticatedProductionBindingsV1 {
            rustc_identity_inventory_sha256,
            rustc_preflight_plan_sha256,
            transaction,
        } = bindings;
        let ProductionTransactionBindingsV1 {
            producer,
            output_dir,
            build_attempt,
            compiler_ffi_envelope,
        } = transaction;
        let compiler_module = AuthenticatedProductionGfx942ModuleV1 {
            target_module,
            llvm_ir,
            compiler_ffi_envelope,
        };
        let prepared =
            crate::worker_v2_producer::prepare_production_v1_worker_handoff(compiler_module)
                .map_err(ProductionPipelineErrorV1::WorkerHandoff)?;
        let attempt = build_attempt.ok_or({
            ProductionPipelineErrorV1::WorkerHandoff(
                crate::worker_v2_producer::WorkerV2ProducerError::MissingBuildAttempt,
            )
        })?;
        let receipt = crate::worker_v2_producer::publish_prepared_production_v1_worker_handoff(
            &output_dir,
            &producer,
            attempt,
            prepared,
        )
        .map_err(ProductionPipelineErrorV1::WorkerHandoff)?;
        drop((
            admitted,
            rustc_identity_inventory_sha256,
            rustc_preflight_plan_sha256,
        ));
        Ok(receipt)
    }
}

impl RankedVerifiedProductionCompilationV1 {
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

    pub(crate) fn retained_identity_and_transaction_binding_count(&self) -> usize {
        let _ = (
            &self.bindings.rustc_identity_inventory_sha256,
            &self.bindings.rustc_preflight_plan_sha256,
            &self.bindings.transaction.producer,
            &self.bindings.transaction.output_dir,
            &self.bindings.transaction.compiler_ffi_envelope,
        );
        5 + usize::from(self.bindings.transaction.build_attempt.is_some())
    }

    pub(crate) fn grants_artifact_or_launch_authority(&self) -> bool {
        self.ranked.grants_artifact_or_launch_authority()
    }
}

impl<'tcx> ProductionCompilationV1<'tcx, CollectedRustStageV1<'tcx>> {
    /// Retains the collector-sealed closure without granting semantic authority.
    /// The next transition must authenticate every imported MIR fact.
    pub(crate) fn from_collected_device_closure(
        tcx: TyCtxt<'tcx>,
        closure: AuthenticatedCollectedKernelClosureV1<'tcx>,
        producer: ProducerIdentity,
        output_dir: PathBuf,
        build_attempt: Option<BuildAttempt>,
    ) -> Result<Self, ProductionPipelineErrorV1> {
        if closure.function_count() == 0 {
            return Err(ProductionPipelineErrorV1::EmptyCollectedDeviceClosure);
        }
        let compiler_ffi_envelope = closure.compiler_ffi_observation().cloned();
        Ok(Self {
            stage: CollectedRustStageV1 {
                tcx,
                closure,
                transaction: ProductionTransactionBindingsV1 {
                    producer,
                    output_dir,
                    build_attempt,
                    compiler_ffi_envelope,
                },
            },
            invariant_session: PhantomData,
        })
    }

    fn import_semantic_mir(
        self,
    ) -> Result<ProductionCompilationV1<'tcx, AdmittedSemanticMirStageV1>, ProductionPipelineErrorV1>
    {
        let CollectedRustStageV1 {
            tcx,
            closure,
            transaction,
        } = self.stage;
        let (semantic_mir, rustc_identity_inventory_sha256, rustc_preflight_plan_sha256) =
            crate::collector::construct_production_semantic_mir_v1(tcx, closure)
                .map_err(ProductionPipelineErrorV1::SemanticImport)?;
        Ok(ProductionCompilationV1 {
            stage: AdmittedSemanticMirStageV1 {
                semantic_mir,
                bindings: AuthenticatedProductionBindingsV1 {
                    rustc_identity_inventory_sha256,
                    rustc_preflight_plan_sha256,
                    transaction,
                },
            },
            invariant_session: PhantomData,
        })
    }

    /// Consumes the only production transaction through import and verification.
    pub(crate) fn verify_ranked_memory(
        self,
    ) -> Result<RankedVerifiedProductionCompilationV1, ProductionPipelineErrorV1> {
        self.import_semantic_mir()?
            .construct_semantic_middle_end()?
            .verify_ranked_memory()
    }

    /// Consumes the sole production transaction through exact semantic MIR,
    /// formal memory admission, and exact gfx942 LLVM lowering.
    pub(crate) fn lower_gfx942(
        self,
    ) -> Result<Gfx942LoweredProductionCompilationV1, ProductionPipelineErrorV1> {
        self.import_semantic_mir()?
            .construct_semantic_middle_end()?
            .lower_target_neutral()?
            .admit_formal_memory()?
            .lower_gfx942()
    }

    /// Publishes the exact production compiler module into the managed,
    /// attempt-scoped Worker V2 protocol. This grants no link, artifact, load,
    /// or launch authority.
    pub(crate) fn publish_worker_handoff(
        self,
    ) -> Result<fe2o3_artifact_transaction::CompilerModuleHandoffReceiptV1, ProductionPipelineErrorV1>
    {
        self.lower_gfx942()?.publish_worker_handoff()
    }

    /// Retains the original extraction milestone while consuming the same
    /// transaction and importer as the production backend.
    pub(crate) fn require_semantic_mir_import(self) -> ProductionPipelineErrorV1 {
        match self.import_semantic_mir() {
            Ok(transaction) => match transaction.construct_semantic_middle_end() {
                Ok(transaction) => transaction.require_target_neutral_lowering(),
                Err(error) => error,
            },
            Err(error) => error,
        }
    }
}

impl<'tcx> ProductionCompilationV1<'tcx, AdmittedSemanticMirStageV1> {
    fn construct_semantic_middle_end(
        self,
    ) -> Result<
        ProductionCompilationV1<'tcx, EquivalentSemanticMirStageV1>,
        ProductionPipelineErrorV1,
    > {
        let AdmittedSemanticMirStageV1 {
            semantic_mir,
            bindings,
        } = self.stage;
        let semantic_mir = fe2o3_pliron::ProductionSemanticMirOwnerV1::try_new(
            semantic_mir,
            fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
        )
        .map_err(ProductionPipelineErrorV1::SemanticMiddleEnd)?;
        Ok(ProductionCompilationV1 {
            stage: EquivalentSemanticMirStageV1 {
                semantic_mir,
                bindings,
            },
            invariant_session: PhantomData,
        })
    }
}

impl<'tcx> ProductionCompilationV1<'tcx, EquivalentSemanticMirStageV1> {
    fn lower_target_neutral(
        self,
    ) -> Result<TargetNeutralProductionCompilationV1, ProductionPipelineErrorV1> {
        let EquivalentSemanticMirStageV1 {
            semantic_mir,
            bindings,
        } = self.stage;
        let lowered = fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1::try_lower(
            semantic_mir,
            fe2o3_lower_mir_kernel::ProductionSemanticKirLimitsV1::default(),
        )
        .map_err(ProductionPipelineErrorV1::TargetNeutralLowering)?;
        Ok(TargetNeutralProductionCompilationV1 { lowered, bindings })
    }

    fn require_target_neutral_lowering(self) -> ProductionPipelineErrorV1 {
        let EquivalentSemanticMirStageV1 {
            semantic_mir,
            bindings,
        } = self.stage;
        let error =
            crate::collector::ProductionSemanticImportErrorV1::TargetNeutralLoweringPending {
                functions: semantic_mir.semantic().functions().len(),
                callables: semantic_mir.semantic().callables().len(),
                rustc_identity_inventory_sha256: bindings.rustc_identity_inventory_sha256,
                rustc_preflight_plan_sha256: bindings.rustc_preflight_plan_sha256,
                semantic_sha256: *semantic_mir.semantic().semantic_sha256().as_bytes(),
            };
        drop((semantic_mir, bindings));
        ProductionPipelineErrorV1::SemanticImport(error)
    }

    fn verify_ranked_memory(
        self,
    ) -> Result<RankedVerifiedProductionCompilationV1, ProductionPipelineErrorV1> {
        let EquivalentSemanticMirStageV1 {
            semantic_mir,
            bindings,
        } = self.stage;
        let ranked =
            crate::production_ranked_projection_v1::project_and_verify_ranked_semantic_mir_v1(
                semantic_mir,
            )
            .map_err(ProductionPipelineErrorV1::RankedProjection)?;
        Ok(RankedVerifiedProductionCompilationV1 { ranked, bindings })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_only_and_device_dispositions_are_exact() {
        assert_eq!(disposition(0), ProductionDispositionV1::HostOnly);
        assert_eq!(disposition(1), ProductionDispositionV1::DeviceTransaction);
        assert_eq!(
            disposition(usize::MAX),
            ProductionDispositionV1::DeviceTransaction
        );
    }

    #[test]
    fn custom_llvm_configuration_is_terminal_before_construction() {
        assert!(reject_custom_llvm_configuration(false).is_ok());
        assert!(matches!(
            reject_custom_llvm_configuration(true),
            Err(ProductionPipelineErrorV1::CustomLlvmConfiguration)
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
    fn production_module_contains_no_profile_selection_vocabulary() {
        let sources = [
            include_str!("production_pipeline_v1.rs"),
            include_str!("collector/production_importer_v1.rs"),
            include_str!("rustc_semantic_adapter_v1.rs"),
            include_str!("rustc_semantic_plan_v1.rs"),
            include_str!("production_semantic_fn_abi_v1.rs"),
            include_str!("production_semantic_types_v1.rs"),
            include_str!("production_semantic_terminal_v1.rs"),
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
            "ProductionCompilationV1::from_collected_device_closure",
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

#![feature(rustc_private)]

extern crate rustc_abi;
extern crate rustc_ast;
extern crate rustc_codegen_llvm;
extern crate rustc_codegen_ssa;
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_metadata;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;
extern crate rustc_target;

mod closure_profile_v1;
mod collector;
mod compiler_descriptor;
mod compiler_ffi_adapter;
mod compiler_module_contract;
mod device_ffi;
mod kernel_ir_codegen;
mod monomorphization_dead;
#[cfg(test)]
mod process_execution;
mod production_backend_v1;
mod production_geometry_v1;
mod production_mir_pliron_verus_join_v1;
pub mod production_pipeline;
mod production_policy;
mod production_ranked_projection_v1;
mod production_reference_bounds_v2;
mod production_reference_effect_join_v2;
mod production_rustc_driver_v1;
mod production_rustc_drop_v1;
mod production_rustc_intrinsic_v1;
mod production_semantic_body_v1;
mod production_semantic_debug_v1;
mod production_semantic_fn_abi_v1;
mod production_semantic_lineage_v3;
mod production_semantic_terminal_v1;
mod production_semantic_types_v1;
mod production_target_v1;
mod production_worker_handoff;
mod protected_compiler_execution;
mod protected_rustc_invocation;
mod reference_effect_bijection_v1;
mod reference_effect_v1;
mod rust_type_layout_general;
mod rust_type_layout_v3;
mod rustc_semantic_adapter_v1;
mod rustc_semantic_plan_v1;
pub mod semantic_layout_bridge;
mod single_codegen_ownership_v1;
mod static_registration;
#[cfg(test)]
mod test_temp_dir;
mod trusted_device_items;
mod tutorial_hardware_qualification_v1;

pub use tutorial_hardware_qualification_v1::{
    TutorialHardwareQualificationErrorV1, TutorialHardwareQualificationReceiptInputV1,
    TutorialHardwareReceiptReplayLedgerV1, VerifiedTutorialHardwareQualificationReceiptV1,
    verify_tutorial_hardware_qualification_receipt_v1,
};

/// Opaque move-only custody for an exact compiler-ranked root roster.
///
/// The compiler alone constructs and consumes this stage. It is named here so
/// compile-fail coverage can enforce that safe callers cannot clone it or
/// dismantle its private semantic and per-root custody.
///
/// ```compile_fail
/// use rustc_codegen_fe2o3::ProductionRankedSemanticProjectionRosterReceiptV1;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<ProductionRankedSemanticProjectionRosterReceiptV1>();
/// ```
///
/// ```compile_fail
/// use rustc_codegen_fe2o3::ProductionRankedSemanticProjectionRosterReceiptV1;
/// fn dismantle(receipt: ProductionRankedSemanticProjectionRosterReceiptV1) {
///     let ProductionRankedSemanticProjectionRosterReceiptV1 {
///         semantic_owner: _,
///         source_order_roots: _,
///         canonical_kernel_order: _,
///         canonical_roster_identity: _,
///     } = receipt;
/// }
/// ```
#[doc(hidden)]
pub use production_ranked_projection_v1::ProductionRankedSemanticProjectionRosterReceiptV1;

#[doc(hidden)]
pub use production_rustc_driver_v1::{
    run_production_amdgpu_llvm_extraction_driver_v1, run_production_extraction_driver_v1,
    run_production_gfx942_compiler_handoff_extraction_driver_v1,
    run_production_gfx942_llvm_extraction_driver_v1, run_production_ranked_extraction_driver_v1,
    run_production_simulation_bundle_extraction_driver_v1,
    run_production_simulation_bundle_extraction_driver_v2,
    run_production_simulation_bundle_extraction_driver_v3,
    run_production_simulation_bundle_extraction_driver_v4,
    run_production_simulation_bundle_extraction_driver_v5,
    run_production_simulation_bundle_extraction_driver_v6,
    run_production_simulation_bundle_extraction_driver_v8,
};

use fe2o3_artifact_transaction as artifact_transaction;
use rustc_codegen_ssa::traits::CodegenBackend;
use rustc_codegen_ssa::{CompiledModules, CrateInfo};
use rustc_data_structures::fx::FxIndexMap;
use rustc_metadata::EncodedMetadata;
use rustc_middle::dep_graph::{WorkProduct, WorkProductId};
use rustc_middle::ty::TyCtxt;
use rustc_middle::ty::print::with_no_trimmed_paths;
use rustc_session::Session;
use rustc_session::config::OutputFilenames;
use std::any::Any;
use std::env;
use std::fmt;
use std::path::{Path, PathBuf};

pub const TARGET_ENV: &str = "FE2O3_TARGET";
pub const BACKEND_ENV: &str = "FE2O3_BACKEND";
pub const VERBOSE_ENV: &str = "FE2O3_VERBOSE";
pub const DUMP_MIR_ENV: &str = "FE2O3_DUMP_MIR";
pub const DUMP_LLVM_ENV: &str = "FE2O3_DUMP_LLVM";
pub const OBSOLETE_CODEGEN_PIPELINE_ENV: &str = "FE2O3_CODEGEN_PIPELINE";
pub const HSACO_DIR_ENV: &str = "FE2O3_HSACO_DIR";
pub const BUILD_ATTEMPT_ENV: &str = artifact_transaction::BUILD_ATTEMPT_ENV_V1;

fn encode_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";

    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(DIGITS[usize::from(byte >> 4)]));
        encoded.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    encoded
}

pub struct Fe2o3CodegenBackend {
    config: BackendConfig,
    llvm_backend: Box<dyn CodegenBackend>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
enum BuildAttemptSelection {
    #[default]
    Direct,
    Managed(artifact_transaction::BuildAttempt),
    Invalid(String),
}

impl BuildAttemptSelection {
    fn from_env() -> Self {
        match env::var(BUILD_ATTEMPT_ENV) {
            Err(env::VarError::NotPresent) => Self::Direct,
            Err(env::VarError::NotUnicode(_)) => {
                Self::Invalid(format!("{BUILD_ATTEMPT_ENV} is not valid UTF-8"))
            }
            Ok(value) => match artifact_transaction::BuildAttempt::from_env_value(&value) {
                Ok(attempt) => Self::Managed(attempt),
                Err(error) => Self::Invalid(format!(
                    "{BUILD_ATTEMPT_ENV} is not a canonical build attempt: {error}"
                )),
            },
        }
    }

    fn resolve(&self) -> Result<Option<artifact_transaction::BuildAttempt>, &str> {
        match self {
            Self::Direct => Ok(None),
            Self::Managed(attempt) => Ok(Some(*attempt)),
            Self::Invalid(reason) => Err(reason),
        }
    }
}

struct RetainedProductionDeviceAdmission {
    target: production_target_v1::RetainedProductionTargetV1,
    compiler_execution: protected_compiler_execution::AdmittedProtectedCompilerExecutionV1,
    build_attempt: artifact_transaction::BuildAttempt,
}

#[derive(Clone, Debug, Default)]
pub struct BackendConfig {
    pub verbose: bool,
    pub dump_mir: bool,
    pub dump_llvm: bool,
    production_environment_rejection: Option<String>,
    build_attempt: BuildAttemptSelection,
    pub hsaco_output_dir: Option<PathBuf>,
    pub target: ProductionDeviceTarget,
}

impl BackendConfig {
    pub fn from_env() -> Self {
        Self {
            verbose: env_flag(VERBOSE_ENV),
            dump_mir: env_flag(DUMP_MIR_ENV),
            dump_llvm: env_flag(DUMP_LLVM_ENV),
            production_environment_rejection: production_policy::environment_rejection(),
            build_attempt: BuildAttemptSelection::from_env(),
            hsaco_output_dir: env::var(HSACO_DIR_ENV).ok().map(PathBuf::from),
            target: ProductionDeviceTarget::from_env_or_default(),
        }
    }
}

fn has_custom_llvm_configuration(session: &Session) -> bool {
    !session.opts.cg.llvm_args.is_empty() || !session.opts.cg.passes.is_empty()
}

impl CodegenBackend for Fe2o3CodegenBackend {
    fn name(&self) -> &'static str {
        "fe2o3"
    }

    fn init(&self, sess: &Session) {
        self.llvm_backend.init(sess);
    }

    fn print_version(&self) {
        println!(
            "rustc-codegen-fe2o3 {} (wrapping rustc_codegen_llvm)",
            env!("CARGO_PKG_VERSION")
        );
        self.llvm_backend.print_version();
    }

    fn target_cpu(&self, sess: &Session) -> String {
        self.llvm_backend.target_cpu(sess)
    }

    fn target_config(&self, sess: &Session) -> rustc_codegen_ssa::TargetConfig {
        self.llvm_backend.target_config(sess)
    }

    fn provide(&self, providers: &mut rustc_middle::util::Providers) {
        self.llvm_backend.provide(providers);
        single_codegen_ownership_v1::install_codegen_unit_provider_v1(providers);
    }

    fn codegen_crate(&self, tcx: TyCtxt<'_>, crate_info: &CrateInfo) -> Box<dyn Any> {
        with_no_trimmed_paths!({
            if let Some(reason) = &self.config.production_environment_rejection {
                tcx.dcx().fatal(format!(
                    "[rustc-codegen-fe2o3] production preflight failed: {reason}"
                ));
            }
            let mut protected_rustc_invocation =
                protected_rustc_invocation::admit_for_production_codegen().unwrap_or_else(|error| {
                    tcx.dcx().fatal(format!(
                        "[rustc-codegen-fe2o3] protected rustc invocation admission failed without fallback: {error}"
                    ))
                });
            let build_attempt = match self.config.build_attempt.resolve() {
                Ok(attempt) => attempt,
                Err(reason) => tcx.dcx().fatal(format!(
                    "[rustc-codegen-fe2o3] invalid managed build attempt: {reason}"
                )),
            };
            let production_root_count =
                collector::count_production_roots_before_monomorphization_v1(tcx);
            let mut production_device_admission = if production_root_count > 0 {
                self.config
                    .target
                    .require_explicit_for_production()
                    .unwrap_or_else(|error| {
                        tcx.dcx().fatal(format!(
                            "[rustc-codegen-fe2o3] production target selection failed: {error}"
                        ))
                    });
                let build_attempt = build_attempt.unwrap_or_else(|| {
                    tcx.dcx().fatal(format!(
                        "[rustc-codegen-fe2o3] production compilation requires a managed {BUILD_ATTEMPT_ENV} before monomorphization"
                    ))
                });
                Some(RetainedProductionDeviceAdmission {
                    target: production_target_v1::RetainedProductionTargetV1::authenticate_before_collection(
                        tcx,
                        &self.config.target,
                    )
                    .unwrap_or_else(|error| {
                        tcx.dcx().fatal(format!(
                            "[rustc-codegen-fe2o3] production target authentication failed before monomorphization without fallback: {error}"
                            ))
                        }),
                    compiler_execution: protected_compiler_execution::admit_for_production_codegen()
                        .unwrap_or_else(|error| {
                            tcx.dcx().fatal(format!(
                                "[rustc-codegen-fe2o3] protected compiler-execution admission failed without fallback: {error}"
                            ))
                        }),
                    build_attempt,
                })
            } else {
                None
            };
            let mono_partitions = tcx.collect_and_partition_mono_items(());
            let kernel_count = collector::count_kernels_in_cgus(tcx, mono_partitions.codegen_units);
            if production_device_admission.is_some() != (kernel_count > 0) {
                let reason = if production_device_admission.is_some() {
                    "authenticated device roots disappeared during monomorphization"
                } else {
                    "a device root appeared only after pre-monomorphization admission"
                };
                tcx.dcx().fatal(format!(
                    "[rustc-codegen-fe2o3] production root custody changed across monomorphization: {reason}; compilation failed closed"
                ));
            }
            if kernel_count == 0 {
                single_codegen_ownership_v1::reject_unclaimed_reserved_roots_v1(
                    tcx,
                    mono_partitions.codegen_units,
                )
                .unwrap_or_else(|error| {
                    tcx.dcx().fatal(format!(
                        "[rustc-codegen-fe2o3] single-code-generation ownership failed: {error}"
                    ))
                });
            }
            let crate_name = tcx.crate_name(rustc_hir::def_id::LOCAL_CRATE);
            let output_dir = match managed_artifact_output(&self.config, kernel_count) {
                Ok(output_dir) => output_dir,
                Err(()) => tcx.dcx().fatal(format!(
                    "[rustc-codegen-fe2o3] {HSACO_DIR_ENV} must name a managed artifact directory when compiling kernels"
                )),
            };
            if self.config.verbose || kernel_count > 0 {
                let reported_target = production_device_admission
                    .as_ref()
                    .map(|admission| admission.target.canonical_name())
                    .unwrap_or_else(|| self.config.target.non_authoritative_name());
                eprintln!(
                    "[rustc-codegen-fe2o3] crate `{crate_name}`: {} CGU(s), {kernel_count} kernel candidate(s), target {}",
                    mono_partitions.codegen_units.len(),
                    reported_target,
                );
            }

            let mut production_device_transaction_complete = false;
            let mut ownership_occurrence = None;
            match production_pipeline::disposition(kernel_count) {
                production_pipeline::ProductionDisposition::HostOnly => {}
                production_pipeline::ProductionDisposition::DeviceTransaction => {
                    let RetainedProductionDeviceAdmission {
                        target,
                        compiler_execution,
                        build_attempt,
                    } = production_device_admission
                        .take()
                        .expect("device admission presence was validated after monomorphization");
                    let authenticated_target_name = target.canonical_name();
                    let has_custom_llvm_configuration = has_custom_llvm_configuration(tcx.sess);
                    if let Err(error) = production_pipeline::reject_custom_llvm_configuration(
                        has_custom_llvm_configuration,
                    ) {
                        tcx.dcx().fatal(format!("[rustc-codegen-fe2o3] {error}"));
                    }
                    let closure = match collector::collect_authenticated_kernel_closure_v1(
                            tcx,
                            mono_partitions.codegen_units,
                            self.config.verbose,
                            target,
                        ) {
                            Ok(closure) => closure,
                            Err(error) => tcx.dcx().fatal(format!(
                                "[rustc-codegen-fe2o3] production collection failed without fallback: {error}"
                            )),
                        };
                    let typed_roots =
                        closure
                            .rederive_typed_descriptor_roots(tcx)
                            .unwrap_or_else(|error| {
                                tcx.dcx().fatal(format!(
                                    "[rustc-codegen-fe2o3] ownership root custody failed: {error}"
                                ))
                            });
                    let typed_claims = typed_roots
                        .iter()
                        .map(|root| {
                            single_codegen_ownership_v1::AuthenticatedTypedRootClaimV1::new(
                                root.logical_name(),
                                root.entry_symbol(),
                                root.kernel_binding_bytes(),
                            )
                        })
                        .collect::<Vec<_>>();
                    let authenticated_roots =
                        single_codegen_ownership_v1::rederive_roots_from_collector_custody_v1(
                            tcx,
                            mono_partitions.codegen_units,
                            &closure,
                            kernel_count,
                            &typed_claims,
                        )
                        .unwrap_or_else(|error| {
                            tcx.dcx().fatal(format!(
                                "[rustc-codegen-fe2o3] exact ownership root custody failed: {error}"
                            ))
                        });
                    let ownership = single_codegen_ownership_v1::build_receipt_v1(
                        tcx,
                        mono_partitions.codegen_units,
                        &authenticated_roots,
                        &closure,
                    )
                    .unwrap_or_else(|error| {
                        tcx.dcx().fatal(format!(
                            "[rustc-codegen-fe2o3] single-code-generation ownership failed: {error}"
                        ))
                    });
                    let output_dir = output_dir
                        .expect("device output was required above")
                        .to_path_buf();
                    let invocation = protected_rustc_invocation.take().unwrap_or_else(|| {
                            tcx.dcx().fatal(
                                "[rustc-codegen-fe2o3] production compilation requires protected rustc invocation custody",
                            )
                        });
                    let producer = match artifact_transaction::ProducerIdentity::from_rustc_invocation_descriptor_v3(
                        invocation.descriptor(),
                    ) {
                        Ok(producer) => producer,
                        Err(error) => tcx.dcx().fatal(format!(
                            "[rustc-codegen-fe2o3] protected rustc producer identity failed: {error}"
                        )),
                    };
                    let (ownership, final_v13_symbols) = ownership.into_parts();
                    let publication =
                        production_pipeline::ProductionCompilation::from_collected_device_closure(
                            tcx,
                            closure,
                            producer.clone(),
                            output_dir,
                            build_attempt,
                            invocation,
                            compiler_execution,
                        )
                        .and_then(|transaction| {
                            transaction.publish_worker_handoff(final_v13_symbols)
                        })
                        .map(|subject| subject.outer_handoff().byte_len());
                    match publication {
                        Ok(publication_length) => {
                            let occurrence =
                                single_codegen_ownership_v1::install_receipt_v1(tcx, ownership)
                                    .unwrap_or_else(|error| {
                                        tcx.dcx().fatal(format!(
                                            "[rustc-codegen-fe2o3] single-code-generation ownership installation failed: {error}"
                                        ))
                                    });
                            occurrence
                                .preflight_delegated_codegen_units(tcx)
                                .unwrap_or_else(|error| {
                                    tcx.dcx().fatal(format!(
                                        "[rustc-codegen-fe2o3] single-code-generation preflight failed: {error}"
                                    ))
                                });
                            ownership_occurrence = Some(occurrence);
                            production_device_transaction_complete = true;
                            eprintln!(
                                "[rustc-codegen-fe2o3] production compilation published {} canonical byte(s) of inert exact {} LLVM handoff into the preselected managed compiler-module transaction; link, artifact, load, and launch authority remain false",
                                publication_length, authenticated_target_name,
                            );
                        }
                        Err(error) => tcx.dcx().fatal(format!("[rustc-codegen-fe2o3] {error}")),
                    }
                }
            }
            if kernel_count > 0 && !production_device_transaction_complete {
                tcx.dcx().fatal(
                    "[rustc-codegen-fe2o3] production compilation did not complete its device transaction; qualification fallback is forbidden",
                );
            }
            // The delegated backend sees only the host partition through the
            // codegen-unit provider installed above. Device roots, helpers,
            // and compiler-only registrations remain owned by final V13.
            let delegated = self.llvm_backend.codegen_crate(tcx, crate_info);
            Box::new(single_codegen_ownership_v1::DelegatedCodegenV1::new(
                delegated,
                ownership_occurrence,
            ))
        })
    }

    fn join_codegen(
        &self,
        ongoing_codegen: Box<dyn Any>,
        sess: &Session,
        outputs: &OutputFilenames,
    ) -> (CompiledModules, FxIndexMap<WorkProductId, WorkProduct>) {
        let mut delegated = ongoing_codegen
            .downcast::<single_codegen_ownership_v1::DelegatedCodegenV1>()
            .unwrap_or_else(|_| {
                panic!("join_codegen received state not returned by fe2o3 codegen_crate")
            });
        let result = self
            .llvm_backend
            .join_codegen(delegated.take_inner(), sess, outputs);
        delegated
            .verify_delegated_objects(&result.0)
            .unwrap_or_else(|error| {
                panic!("single-code-generation object verification failed: {error}")
            });
        delegated.retire().unwrap_or_else(|error| {
            panic!("single-code-generation ownership retirement failed: {error}")
        });
        result
    }

    fn link(
        &self,
        sess: &Session,
        compiled_modules: CompiledModules,
        crate_info: CrateInfo,
        metadata: EncodedMetadata,
        outputs: &OutputFilenames,
    ) {
        self.llvm_backend
            .link(sess, compiled_modules, crate_info, metadata, outputs);
    }
}

#[unsafe(no_mangle)]
pub fn __rustc_codegen_backend() -> Box<dyn CodegenBackend> {
    let config = BackendConfig::from_env();
    let llvm_backend = rustc_codegen_llvm::LlvmCodegenBackend::new();

    Box::new(Fe2o3CodegenBackend {
        config,
        llvm_backend,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionDeviceTarget {
    name: String,
    explicit: bool,
}

impl ProductionDeviceTarget {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            explicit: true,
        }
    }

    pub fn from_env_or_default() -> Self {
        Self::from_optional_value(env::var(TARGET_ENV).ok())
    }

    fn from_optional_value(value: Option<String>) -> Self {
        value
            .filter(|value| !value.trim().is_empty())
            .map(Self::new)
            .unwrap_or_else(Self::host_only_unselected)
    }

    fn host_only_unselected() -> Self {
        Self {
            name: "<host-only:no-production-target>".to_owned(),
            explicit: false,
        }
    }

    fn require_explicit_for_production(&self) -> Result<&str, &'static str> {
        (self.explicit && !self.name.trim().is_empty())
            .then_some(self.name.as_str())
            .ok_or(
            "FE2O3_TARGET must explicitly select a supported production target; the host-only placeholder has no device authority",
        )
    }

    fn non_authoritative_name(&self) -> &str {
        "<host-only:production-target-not-authenticated>"
    }

    pub fn as_str(&self) -> &str {
        &self.name
    }
}

impl Default for ProductionDeviceTarget {
    fn default() -> Self {
        Self::from_env_or_default()
    }
}

impl fmt::Display for ProductionDeviceTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}

/// Source-compatible name for callers written before production target
/// selection moved behind the target-neutral backend boundary.
pub type AmdGpuTarget = ProductionDeviceTarget;

fn env_flag(name: &str) -> bool {
    matches!(
        env::var(name).as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes") | Ok("YES")
    )
}

fn managed_artifact_output(
    config: &BackendConfig,
    kernel_count: usize,
) -> Result<Option<&Path>, ()> {
    match config.hsaco_output_dir.as_deref() {
        Some(path) if !path.as_os_str().is_empty() => Ok(Some(path)),
        Some(_) => Err(()),
        None if kernel_count == 0 => Ok(None),
        None => Err(()),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BackendConfig, BuildAttemptSelection, ProductionDeviceTarget, managed_artifact_output,
    };
    use std::path::{Path, PathBuf};
    #[test]
    fn admitted_protected_modules_publish_only_through_capability_v5() {
        let backend = include_str!("lib.rs")
            .split_once("\n#[cfg(test)]\nmod tests {")
            .expect("backend test module boundary")
            .0;
        let production_pipeline = include_str!("production_pipeline.rs")
            .split_once("\n#[cfg(test)]\nmod tests {")
            .expect("production pipeline test module boundary")
            .0;
        let production = backend
            .split("let mut production_device_transaction_complete")
            .nth(1)
            .expect("production transaction tracking exists")
            .split("self.llvm_backend.codegen_crate(tcx, crate_info)")
            .next()
            .expect("bounded production transaction");
        assert!(production.contains("protected_rustc_invocation.take()"));
        assert!(backend.contains("protected_compiler_execution::admit_for_production_codegen()"));
        assert!(production.contains("from_rustc_invocation_descriptor_v3"));
        assert!(production.contains("invocation.descriptor()"));
        assert!(!production.contains("local_crate_source_file"));
        assert!(!production.contains("ProducerIdentity::from_codegen"));
        assert!(production.contains("production_device_admission"));
        assert!(production.contains(".take()"));
        assert!(!production.contains("build_attempt.unwrap_or_else"));
        assert!(production.contains("from_collected_device_closure("));
        assert!(production.contains("publish_worker_handoff(final_v13_symbols)"));
        assert!(!production.contains("from_collected_device_closure_with_protected_invocation_v3"));
        assert!(!production.contains("publish_worker_handoff_v3"));
        assert!(!production.contains("None =>"));
        assert!(!production.contains("QualificationOracle"));
        assert!(!production.contains("qualification_selection"));
        assert!(!production_pipeline.contains("Option<BuildAttempt>"));
        assert!(production_pipeline.contains("publish_compiler_capability_handoff_v5("));
        assert!(
            production_pipeline
                .contains("publish_compiler_execution_receipt_transport_for_capability_v5(")
        );
        assert!(!production_pipeline.contains("publish_compiler_module_handoff_v3("));
        assert!(!production_pipeline.contains("publish_compiler_execution_receipt_transport_v1("));
        let admission = backend
            .find("let mut production_device_admission")
            .expect("production device admission");
        let monomorphization = backend
            .find("let mono_partitions = tcx.collect_and_partition_mono_items")
            .expect("monomorphization boundary");
        assert!(admission < monomorphization);
    }

    #[test]
    fn backend_configuration_is_feature_invariant_and_selector_free() {
        let backend = include_str!("lib.rs");
        let configuration = backend
            .split("pub struct BackendConfig")
            .nth(1)
            .expect("backend configuration exists")
            .split("impl CodegenBackend")
            .next()
            .expect("bounded backend configuration");
        assert!(configuration.contains("production_environment_rejection"));
        assert!(configuration.contains("production_policy::environment_rejection()"));
        assert!(!configuration.contains("QualificationSelection"));
        assert!(!configuration.contains("qualification_selection"));
    }

    #[test]
    fn production_target_is_explicit_and_substitution_is_not_authoritative() {
        let absent = ProductionDeviceTarget::from_optional_value(None);
        assert_eq!(absent.as_str(), "<host-only:no-production-target>");
        assert!(absent.require_explicit_for_production().is_err());

        let empty = ProductionDeviceTarget::from_optional_value(Some("  ".to_owned()));
        assert!(empty.require_explicit_for_production().is_err());

        let configured = ProductionDeviceTarget::from_optional_value(Some("gfx950".to_owned()));
        assert_eq!(configured.require_explicit_for_production(), Ok("gfx950"));
        assert!(
            ProductionDeviceTarget::new(" ")
                .require_explicit_for_production()
                .is_err()
        );
        assert_eq!(
            configured.non_authoritative_name(),
            "<host-only:production-target-not-authenticated>"
        );

        let backend = include_str!("lib.rs")
            .split_once("\n#[cfg(test)]\nmod tests {")
            .expect("backend test module boundary")
            .0;
        assert!(!backend.contains("Self::new(\"gfx1100\")"));
        assert!(!backend.contains("inert exact gfx942:xnack- LLVM handoff"));
        assert!(backend.contains("let authenticated_target_name = target.canonical_name()"));
        assert!(backend.contains("publication_length, authenticated_target_name"));
    }

    #[test]
    fn kernels_require_an_explicit_managed_artifact_directory() {
        let mut config = BackendConfig::default();
        assert_eq!(managed_artifact_output(&config, 0), Ok(None));
        assert_eq!(managed_artifact_output(&config, 1), Err(()));

        config.hsaco_output_dir = Some(PathBuf::new());
        assert_eq!(managed_artifact_output(&config, 1), Err(()));

        config.hsaco_output_dir = Some(PathBuf::from("target/fe2o3"));
        assert_eq!(
            managed_artifact_output(&config, 1),
            Ok(Some(Path::new("target/fe2o3")))
        );
    }

    #[test]
    fn build_attempt_selection_is_direct_or_exactly_canonical() {
        assert_eq!(BuildAttemptSelection::default().resolve(), Ok(None));
        let value = concat!(
            "7:11111111111111111111111111111111:",
            "2222222222222222222222222222222222222222222222222222222222222222"
        );
        let selection = BuildAttemptSelection::Managed(
            fe2o3_artifact_transaction::BuildAttempt::from_env_value(value).unwrap(),
        );
        assert_eq!(selection.resolve().unwrap().unwrap().to_env_value(), value);
        assert!(
            BuildAttemptSelection::Invalid("bad".to_owned())
                .resolve()
                .is_err()
        );
    }
}

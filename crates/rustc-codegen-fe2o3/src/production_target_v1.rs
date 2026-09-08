//! Retained source and device target inputs for the production transaction.

use std::fmt;

use rustc_middle::ty::TyCtxt;

use crate::ProductionDeviceTarget;
use crate::production_backend_v1::{ProductionBackendTargetContractV1, ProductionBackendTargetV1};
use crate::semantic_layout_bridge::{
    SemanticLayoutBridgeError, SemanticLayoutTargetV1, rustc_semantic_layout_target_v1,
};

#[cfg(test)]
pub(crate) const PRODUCTION_RUSTC_DATA_LAYOUT_V1: &str =
    crate::production_backend_v1::PRODUCTION_RUSTC_DATA_LAYOUT_V1;
/// Exact worker data layout returned by the selected production backend.
#[cfg(test)]
pub(crate) const PRODUCTION_WORKER_DATA_LAYOUT_V1: &str =
    crate::production_backend_v1::PRODUCTION_WORKER_DATA_LAYOUT_V1;

/// Move-only proof that the live rustc session was the exact production target
/// before monomorphization or production MIR collection began.
#[derive(Debug)]
pub(crate) struct RetainedProductionTargetV1 {
    backend: ProductionBackendTargetV1,
    rustc_layout: SemanticLayoutTargetV1,
}

/// Exact target facts authenticated from the live device rustc session.
///
/// This is move-only and crate-private so configured device labels cannot be
/// substituted for the target that actually answered layout and FnAbi queries.
#[derive(Debug)]
pub(crate) struct AuthenticatedProductionTargetV1 {
    backend: ProductionBackendTargetV1,
    rustc_layout: SemanticLayoutTargetV1,
}

impl RetainedProductionTargetV1 {
    pub(crate) fn authenticate_before_collection(
        tcx: TyCtxt<'_>,
        configured_target: &ProductionDeviceTarget,
    ) -> Result<Self, ProductionTargetErrorV1> {
        let retained = Self::authenticate_live_before_collection(tcx)?;
        let configured_backend = production_backend_for_configured_target_v1(configured_target)
            .ok_or_else(|| ProductionTargetErrorV1::ConfiguredTarget {
                observed: configured_target.as_str().to_owned(),
            })?;
        if !configured_backend.same_target(&retained.backend) {
            return Err(ProductionTargetErrorV1::ConfiguredTargetMismatch {
                configured: configured_target.as_str().to_owned(),
                observed: retained.backend.contract().canonical_target().to_owned(),
            });
        }
        Ok(retained)
    }

    pub(crate) fn authenticate_live_before_collection(
        tcx: TyCtxt<'_>,
    ) -> Result<Self, ProductionTargetErrorV1> {
        let rustc_layout = rustc_semantic_layout_target_v1(tcx)
            .map_err(ProductionTargetErrorV1::RustcObservation)?;
        let observed_cpu = rustc_layout.active_cpu().unwrap_or("unavailable");
        let backend = ProductionBackendTargetV1::from_live_cpu(observed_cpu).ok_or_else(|| {
            ProductionTargetErrorV1::LiveCpu {
                observed: observed_cpu.to_owned(),
            }
        })?;
        validate_authoritative_rustc_target_v1(&backend, &rustc_layout)?;
        Ok(Self {
            backend,
            rustc_layout,
        })
    }

    pub(crate) fn canonical_name(&self) -> &'static str {
        self.backend.contract().canonical_target()
    }

    pub(crate) fn authenticate_import_session(
        self,
        tcx: TyCtxt<'_>,
    ) -> Result<AuthenticatedProductionTargetV1, ProductionTargetErrorV1> {
        let observed = rustc_semantic_layout_target_v1(tcx)
            .map_err(ProductionTargetErrorV1::RustcObservation)?;
        if observed != self.rustc_layout {
            return Err(ProductionTargetErrorV1::RustcSessionChanged);
        }
        validate_authoritative_rustc_target_v1(&self.backend, &observed)?;
        Ok(AuthenticatedProductionTargetV1 {
            backend: self.backend,
            rustc_layout: observed,
        })
    }
}

impl AuthenticatedProductionTargetV1 {
    pub(crate) const fn backend(&self) -> &ProductionBackendTargetV1 {
        &self.backend
    }

    pub(crate) fn contract(&self) -> ProductionBackendTargetContractV1 {
        self.backend.contract()
    }

    pub(crate) fn rustc_layout(&self) -> &SemanticLayoutTargetV1 {
        &self.rustc_layout
    }

    pub(crate) fn device_target(&self) -> fe2o3_compiler_ffi::DeviceTargetV1 {
        self.backend.device_target()
    }
}

fn validate_authoritative_rustc_target_v1(
    backend: &ProductionBackendTargetV1,
    target: &SemanticLayoutTargetV1,
) -> Result<(), ProductionTargetErrorV1> {
    let contract = backend.contract();
    contract.neutral_profile().validate().map_err(|error| {
        ProductionTargetErrorV1::BackendProfileInvalid {
            detail: error.to_string(),
        }
    })?;
    for (field, value) in [
        ("backend family", contract.backend_family()),
        ("worker data layout", contract.worker_data_layout()),
    ] {
        if value.is_empty() {
            return Err(ProductionTargetErrorV1::BackendProfileInvalid {
                detail: format!("{field} is empty"),
            });
        }
    }
    require_exact_target_text("LLVM target", contract.rustc_target(), target.llvm_target())?;
    require_exact_target_text(
        "data layout",
        contract.rustc_data_layout(),
        target.data_layout(),
    )?;
    if target.default_pointer_width_bits() != contract.pointer_width_bits() {
        return Err(ProductionTargetErrorV1::RustcTargetMismatch {
            field: "default pointer width",
            expected: contract.pointer_width_bits().to_string(),
            observed: target.default_pointer_width_bits().to_string(),
        });
    }
    require_exact_target_text(
        "active CPU",
        contract.cpu(),
        target.active_cpu().unwrap_or("unavailable"),
    )?;
    require_exact_target_text(
        "active target features",
        contract.rustc_features(),
        target.active_features().unwrap_or("unavailable"),
    )
}

fn require_exact_target_text(
    field: &'static str,
    expected: &'static str,
    observed: &str,
) -> Result<(), ProductionTargetErrorV1> {
    if observed == expected {
        Ok(())
    } else {
        Err(ProductionTargetErrorV1::RustcTargetMismatch {
            field,
            expected: expected.to_owned(),
            observed: observed.to_owned(),
        })
    }
}

fn production_backend_for_configured_target_v1(
    target: &ProductionDeviceTarget,
) -> Option<ProductionBackendTargetV1> {
    ProductionBackendTargetV1::from_configured_target(target.as_str())
}

#[derive(Debug)]
pub(crate) enum ProductionTargetErrorV1 {
    ConfiguredTarget {
        observed: String,
    },
    ConfiguredTargetMismatch {
        configured: String,
        observed: String,
    },
    LiveCpu {
        observed: String,
    },
    BackendProfileInvalid {
        detail: String,
    },
    RustcObservation(SemanticLayoutBridgeError),
    RustcSessionChanged,
    RustcTargetMismatch {
        field: &'static str,
        expected: String,
        observed: String,
    },
}

impl fmt::Display for ProductionTargetErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConfiguredTarget { observed } => write!(
                formatter,
                "production compilation found no backend for configured device target {observed:?}"
            ),
            Self::ConfiguredTargetMismatch {
                configured,
                observed,
            } => write!(
                formatter,
                "configured production device target {configured:?} does not match the live rustc device target {observed:?}"
            ),
            Self::LiveCpu { observed } => write!(
                formatter,
                "production compilation found no backend for live rustc target CPU {observed:?}"
            ),
            Self::BackendProfileInvalid { detail } => write!(
                formatter,
                "production compilation selected an invalid target profile: {detail}"
            ),
            Self::RustcObservation(error) => {
                write!(
                    formatter,
                    "production compilation could not capture the rustc target: {error}"
                )
            }
            Self::RustcSessionChanged => formatter.write_str(
                "production compilation rustc target facts changed between collection and semantic import",
            ),
            Self::RustcTargetMismatch {
                field,
                expected,
                observed,
            } => write!(
                formatter,
                "production compilation requires authoritative rustc {field} {expected:?}; found {observed:?}",
            ),
        }
    }
}

impl std::error::Error for ProductionTargetErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RustcObservation(error) => Some(error),
            Self::ConfiguredTarget { .. }
            | Self::ConfiguredTargetMismatch { .. }
            | Self::LiveCpu { .. }
            | Self::BackendProfileInvalid { .. }
            | Self::RustcSessionChanged
            | Self::RustcTargetMismatch { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProductionDeviceTarget;
    use fe2o3_amd_target::{
        PRODUCTION_GFX942_DEVICE_CPU_V1, PRODUCTION_GFX942_RUSTC_FEATURES_V1,
        PRODUCTION_GFX942_RUSTC_TARGET_V1,
    };
    use rustc_driver::{Callbacks, Compilation};
    use rustc_interface::interface::Compiler;
    use std::fs;
    use std::process::Command;

    #[test]
    fn production_device_target_accepts_only_exact_admitted_target_ids() {
        assert_eq!(
            production_backend_for_configured_target_v1(&ProductionDeviceTarget::new(
                "gfx942:xnack-"
            ))
            .unwrap()
            .contract()
            .canonical_target(),
            "gfx942:xnack-"
        );
        assert_eq!(
            production_backend_for_configured_target_v1(&ProductionDeviceTarget::new(
                "gfx950:xnack-"
            ))
            .unwrap()
            .contract()
            .canonical_target(),
            "gfx950:xnack-"
        );
        for rejected in ["gfx942", "gfx942:xnack+", "gfx950", "GFX942:xnack-"] {
            assert!(
                production_backend_for_configured_target_v1(&ProductionDeviceTarget::new(rejected))
                    .is_none()
            );
        }
    }

    #[test]
    fn rustc_and_worker_layouts_retain_distinct_measured_contracts() {
        assert_ne!(
            PRODUCTION_RUSTC_DATA_LAYOUT_V1,
            PRODUCTION_WORKER_DATA_LAYOUT_V1
        );
        assert_eq!(
            PRODUCTION_RUSTC_DATA_LAYOUT_V1,
            format!("e-m:e-{}", &PRODUCTION_WORKER_DATA_LAYOUT_V1[2..])
        );
        assert_eq!(
            PRODUCTION_RUSTC_DATA_LAYOUT_V1,
            fe2o3_compiler_ffi::EXTERNAL_DEVICE_LIBRARY_GFX942_DATA_LAYOUT_V1
        );
        assert_ne!(
            PRODUCTION_WORKER_DATA_LAYOUT_V1,
            fe2o3_compiler_ffi::EXTERNAL_DEVICE_LIBRARY_GFX942_DATA_LAYOUT_V1
        );
    }

    #[test]
    fn production_rustc_target_requires_every_authoritative_axis() {
        let backend = production_backend_for_configured_target_v1(&ProductionDeviceTarget::new(
            "gfx942:xnack-",
        ))
        .unwrap();
        let pointer_width = backend.contract().pointer_width_bits();
        let exact = SemanticLayoutTargetV1::new_with_codegen_profile(
            PRODUCTION_GFX942_RUSTC_TARGET_V1,
            PRODUCTION_RUSTC_DATA_LAYOUT_V1,
            pointer_width,
            PRODUCTION_GFX942_DEVICE_CPU_V1,
            "",
            PRODUCTION_GFX942_RUSTC_FEATURES_V1,
        )
        .unwrap();
        validate_authoritative_rustc_target_v1(&backend, &exact).unwrap();

        let substitutions = [
            SemanticLayoutTargetV1::new_with_codegen_profile(
                "x86_64-unknown-linux-gnu",
                PRODUCTION_RUSTC_DATA_LAYOUT_V1,
                pointer_width,
                PRODUCTION_GFX942_DEVICE_CPU_V1,
                "",
                PRODUCTION_GFX942_RUSTC_FEATURES_V1,
            )
            .unwrap(),
            SemanticLayoutTargetV1::new_with_codegen_profile(
                PRODUCTION_GFX942_RUSTC_TARGET_V1,
                "e-p:64:64",
                pointer_width,
                PRODUCTION_GFX942_DEVICE_CPU_V1,
                "",
                PRODUCTION_GFX942_RUSTC_FEATURES_V1,
            )
            .unwrap(),
            SemanticLayoutTargetV1::new_with_codegen_profile(
                PRODUCTION_GFX942_RUSTC_TARGET_V1,
                PRODUCTION_RUSTC_DATA_LAYOUT_V1,
                32,
                PRODUCTION_GFX942_DEVICE_CPU_V1,
                "",
                PRODUCTION_GFX942_RUSTC_FEATURES_V1,
            )
            .unwrap(),
            SemanticLayoutTargetV1::new_with_codegen_profile(
                PRODUCTION_GFX942_RUSTC_TARGET_V1,
                PRODUCTION_RUSTC_DATA_LAYOUT_V1,
                pointer_width,
                "gfx950",
                "",
                PRODUCTION_GFX942_RUSTC_FEATURES_V1,
            )
            .unwrap(),
            SemanticLayoutTargetV1::new_with_codegen_profile(
                PRODUCTION_GFX942_RUSTC_TARGET_V1,
                PRODUCTION_RUSTC_DATA_LAYOUT_V1,
                pointer_width,
                PRODUCTION_GFX942_DEVICE_CPU_V1,
                "",
                "-wavefrontsize32,+wavefrontsize64,+xnack",
            )
            .unwrap(),
            SemanticLayoutTargetV1::new_with_codegen_profile(
                PRODUCTION_GFX942_RUSTC_TARGET_V1,
                PRODUCTION_RUSTC_DATA_LAYOUT_V1,
                pointer_width,
                PRODUCTION_GFX942_DEVICE_CPU_V1,
                "",
                "-wavefrontsize32,+wavefrontsize64",
            )
            .unwrap(),
            SemanticLayoutTargetV1::new_with_codegen_profile(
                PRODUCTION_GFX942_RUSTC_TARGET_V1,
                PRODUCTION_RUSTC_DATA_LAYOUT_V1,
                pointer_width,
                PRODUCTION_GFX942_DEVICE_CPU_V1,
                "",
                "+wavefrontsize32,-wavefrontsize64,-xnack",
            )
            .unwrap(),
        ];
        for substitution in substitutions {
            assert!(matches!(
                validate_authoritative_rustc_target_v1(&backend, &substitution),
                Err(ProductionTargetErrorV1::RustcTargetMismatch { .. })
            ));
        }
    }

    #[derive(Default)]
    struct TargetCallbacks {
        result: Option<Result<(), String>>,
    }

    impl Callbacks for TargetCallbacks {
        fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
            self.result = Some(
                rustc_semantic_layout_target_v1(tcx)
                    .map_err(|error| error.to_string())
                    .and_then(|target| {
                        let backend =
                            ProductionBackendTargetV1::from_configured_target("gfx942:xnack-")
                                .expect("test backend target");
                        validate_authoritative_rustc_target_v1(&backend, &target)
                            .map_err(|error| error.to_string())
                    }),
            );
            Compilation::Stop
        }
    }

    #[test]
    fn built_in_amdgcn_session_exposes_exact_gfx942_xnack_minus_target_facts() {
        // This no-core probe qualifies only rustc's target-session facts. A real
        // AMD core/dependency graph remains required before MIR import.
        let directory = crate::test_temp_dir::TestTempDir::create("fe2o3-production-target");
        let source = directory.path().join("target.rs");
        fs::write(
            &source,
            r#"
                #![feature(no_core, lang_items)]
                #![no_core]
                #![allow(dead_code, internal_features)]

                #[lang = "pointee_sized"]
                trait PointeeSized {}
                #[lang = "meta_sized"]
                trait MetaSized: PointeeSized {}
                #[lang = "sized"]
                trait Sized: MetaSized {}

                fn kernel() {}
            "#,
        )
        .unwrap();
        let mut command = Command::new("rustc");
        command.args(["--print", "sysroot"]);
        let sysroot = crate::process_execution::capture_output(&mut command).unwrap();
        assert!(sysroot.status.success());
        let args = vec![
            "rustc".to_owned(),
            "--crate-name".to_owned(),
            "fe2o3_production_target".to_owned(),
            "--crate-type".to_owned(),
            "lib".to_owned(),
            "--edition".to_owned(),
            "2024".to_owned(),
            "--target".to_owned(),
            PRODUCTION_GFX942_RUSTC_TARGET_V1.to_owned(),
            "-Ctarget-cpu=gfx942".to_owned(),
            "-Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32".to_owned(),
            "-Zno-codegen".to_owned(),
            "--sysroot".to_owned(),
            String::from_utf8(sysroot.stdout).unwrap().trim().to_owned(),
            source.display().to_string(),
        ];
        let mut callbacks = TargetCallbacks::default();
        rustc_driver::run_compiler(&args, &mut callbacks);
        callbacks
            .result
            .expect("target callback did not run")
            .unwrap();
    }
}

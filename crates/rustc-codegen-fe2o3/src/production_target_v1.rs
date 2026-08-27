//! Retained source and device target inputs for the production transaction.

use std::fmt;

use fe2o3_amd_target::ProductionAmdTargetProfileV1;
use rustc_middle::ty::TyCtxt;

use crate::AmdGpuTarget;
use crate::semantic_layout_bridge::{
    SemanticLayoutBridgeError, SemanticLayoutTargetV1, rustc_semantic_layout_target_v1,
};

pub(crate) const PRODUCTION_RUSTC_DATA_LAYOUT_V1: &str = "e-m:e-p:64:64-p1:64:64-p2:32:32-p3:32:32-p4:64:64-p5:32:32-p6:32:32-p7:160:256:256:32-p8:128:128:128:48-p9:192:256:256:32-i64:64-v16:16-v24:32-v32:32-v48:64-v96:128-v192:256-v256:256-v512:512-v1024:1024-v2048:2048-n32:64-S32-A5-G1-ni:7:8:9";
const PRODUCTION_RUSTC_POINTER_WIDTH_V1: u16 = 64;

/// Move-only proof that the live rustc session was the exact production target
/// before monomorphization or production MIR collection began.
#[derive(Debug)]
pub(crate) struct RetainedProductionTargetV1 {
    profile: ProductionAmdTargetProfileV1,
    rustc_layout: SemanticLayoutTargetV1,
}

/// Exact target facts authenticated from the live AMDGPU rustc session.
///
/// This is move-only and crate-private so configured device labels cannot be
/// substituted for the target that actually answered layout and FnAbi queries.
#[derive(Debug)]
pub(crate) struct AuthenticatedProductionTargetV1 {
    profile: ProductionAmdTargetProfileV1,
    rustc_layout: SemanticLayoutTargetV1,
}

impl RetainedProductionTargetV1 {
    pub(crate) fn authenticate_before_collection(
        tcx: TyCtxt<'_>,
        configured_cpu: &AmdGpuTarget,
    ) -> Result<Self, ProductionTargetErrorV1> {
        let retained = Self::authenticate_live_before_collection(tcx)?;
        let configured_profile = production_profile_for_configured_cpu_v1(configured_cpu)
            .ok_or_else(|| ProductionTargetErrorV1::ConfiguredCpu {
                observed: configured_cpu.as_str().to_owned(),
            })?;
        if configured_profile != retained.profile {
            return Err(ProductionTargetErrorV1::ConfiguredCpuMismatch {
                configured: configured_cpu.as_str().to_owned(),
                observed: retained.profile.cpu().to_owned(),
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
        let profile = ProductionAmdTargetProfileV1::from_cpu(observed_cpu).ok_or_else(|| {
            ProductionTargetErrorV1::LiveCpu {
                observed: observed_cpu.to_owned(),
            }
        })?;
        validate_authoritative_rustc_target_v1(profile, &rustc_layout)?;
        Ok(Self {
            profile,
            rustc_layout,
        })
    }

    pub(crate) fn canonical_name(&self) -> &'static str {
        self.profile.device_target()
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
        validate_authoritative_rustc_target_v1(self.profile, &observed)?;
        Ok(AuthenticatedProductionTargetV1 {
            profile: self.profile,
            rustc_layout: observed,
        })
    }
}

impl AuthenticatedProductionTargetV1 {
    pub(crate) const fn profile(&self) -> ProductionAmdTargetProfileV1 {
        self.profile
    }

    pub(crate) fn rustc_layout(&self) -> &SemanticLayoutTargetV1 {
        &self.rustc_layout
    }

    pub(crate) fn device_target(&self) -> fe2o3_compiler_ffi::DeviceTargetV1 {
        fe2o3_compiler_ffi::DeviceTargetV1::parse(self.profile.device_target())
            .expect("the authenticated production target is valid")
    }
}

fn validate_authoritative_rustc_target_v1(
    profile: ProductionAmdTargetProfileV1,
    target: &SemanticLayoutTargetV1,
) -> Result<(), ProductionTargetErrorV1> {
    require_exact_target_text("LLVM target", profile.rustc_target(), target.llvm_target())?;
    require_exact_target_text(
        "data layout",
        PRODUCTION_RUSTC_DATA_LAYOUT_V1,
        target.data_layout(),
    )?;
    if target.default_pointer_width_bits() != PRODUCTION_RUSTC_POINTER_WIDTH_V1 {
        return Err(ProductionTargetErrorV1::RustcTargetMismatch {
            field: "default pointer width",
            expected: PRODUCTION_RUSTC_POINTER_WIDTH_V1.to_string(),
            observed: target.default_pointer_width_bits().to_string(),
        });
    }
    require_exact_target_text(
        "active CPU",
        profile.cpu(),
        target.active_cpu().unwrap_or("unavailable"),
    )?;
    require_exact_target_text(
        "active target features",
        profile.rustc_features(),
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

fn production_profile_for_configured_cpu_v1(
    target: &AmdGpuTarget,
) -> Option<ProductionAmdTargetProfileV1> {
    ProductionAmdTargetProfileV1::from_cpu(target.as_str())
}

#[derive(Debug)]
pub(crate) enum ProductionTargetErrorV1 {
    ConfiguredCpu {
        observed: String,
    },
    ConfiguredCpuMismatch {
        configured: String,
        observed: String,
    },
    LiveCpu {
        observed: String,
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
            Self::ConfiguredCpu { observed } => write!(
                formatter,
                "production compilation requires configured target CPU \"gfx942\" or \"gfx950\"; found {observed:?}"
            ),
            Self::ConfiguredCpuMismatch {
                configured,
                observed,
            } => write!(
                formatter,
                "configured production target CPU {configured:?} does not match the live rustc target CPU {observed:?}"
            ),
            Self::LiveCpu { observed } => write!(
                formatter,
                "production compilation requires live rustc target CPU \"gfx942\" or \"gfx950\"; found {observed:?}"
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
            Self::ConfiguredCpu { .. }
            | Self::ConfiguredCpuMismatch { .. }
            | Self::LiveCpu { .. }
            | Self::RustcSessionChanged
            | Self::RustcTargetMismatch { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AmdGpuTarget;
    use fe2o3_amd_target::{
        PRODUCTION_GFX942_DEVICE_CPU_V1, PRODUCTION_GFX942_RUSTC_FEATURES_V1,
        PRODUCTION_GFX942_RUSTC_TARGET_V1,
    };
    use rustc_driver::{Callbacks, Compilation};
    use rustc_interface::interface::Compiler;
    use std::fs;
    use std::process::Command;

    #[test]
    fn production_device_target_accepts_only_exact_admitted_cpus() {
        assert_eq!(
            production_profile_for_configured_cpu_v1(&AmdGpuTarget::new("gfx942")),
            Some(ProductionAmdTargetProfileV1::Gfx942)
        );
        assert_eq!(
            production_profile_for_configured_cpu_v1(&AmdGpuTarget::new("gfx950")),
            Some(ProductionAmdTargetProfileV1::Gfx950)
        );
        for rejected in ["gfx942:xnack-", "gfx942:xnack+", "gfx950:xnack-", "GFX942"] {
            assert_eq!(
                production_profile_for_configured_cpu_v1(&AmdGpuTarget::new(rejected)),
                None
            );
        }
    }

    #[test]
    fn production_rustc_target_requires_every_authoritative_axis() {
        let exact = SemanticLayoutTargetV1::new_with_codegen_profile(
            PRODUCTION_GFX942_RUSTC_TARGET_V1,
            PRODUCTION_RUSTC_DATA_LAYOUT_V1,
            PRODUCTION_RUSTC_POINTER_WIDTH_V1,
            PRODUCTION_GFX942_DEVICE_CPU_V1,
            "",
            PRODUCTION_GFX942_RUSTC_FEATURES_V1,
        )
        .unwrap();
        validate_authoritative_rustc_target_v1(ProductionAmdTargetProfileV1::Gfx942, &exact)
            .unwrap();

        let substitutions = [
            SemanticLayoutTargetV1::new_with_codegen_profile(
                "x86_64-unknown-linux-gnu",
                PRODUCTION_RUSTC_DATA_LAYOUT_V1,
                PRODUCTION_RUSTC_POINTER_WIDTH_V1,
                PRODUCTION_GFX942_DEVICE_CPU_V1,
                "",
                PRODUCTION_GFX942_RUSTC_FEATURES_V1,
            )
            .unwrap(),
            SemanticLayoutTargetV1::new_with_codegen_profile(
                PRODUCTION_GFX942_RUSTC_TARGET_V1,
                "e-p:64:64",
                PRODUCTION_RUSTC_POINTER_WIDTH_V1,
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
                PRODUCTION_RUSTC_POINTER_WIDTH_V1,
                "gfx950",
                "",
                PRODUCTION_GFX942_RUSTC_FEATURES_V1,
            )
            .unwrap(),
            SemanticLayoutTargetV1::new_with_codegen_profile(
                PRODUCTION_GFX942_RUSTC_TARGET_V1,
                PRODUCTION_RUSTC_DATA_LAYOUT_V1,
                PRODUCTION_RUSTC_POINTER_WIDTH_V1,
                PRODUCTION_GFX942_DEVICE_CPU_V1,
                "",
                "-wavefrontsize32,+wavefrontsize64,+xnack",
            )
            .unwrap(),
            SemanticLayoutTargetV1::new_with_codegen_profile(
                PRODUCTION_GFX942_RUSTC_TARGET_V1,
                PRODUCTION_RUSTC_DATA_LAYOUT_V1,
                PRODUCTION_RUSTC_POINTER_WIDTH_V1,
                PRODUCTION_GFX942_DEVICE_CPU_V1,
                "",
                "-wavefrontsize32,+wavefrontsize64",
            )
            .unwrap(),
            SemanticLayoutTargetV1::new_with_codegen_profile(
                PRODUCTION_GFX942_RUSTC_TARGET_V1,
                PRODUCTION_RUSTC_DATA_LAYOUT_V1,
                PRODUCTION_RUSTC_POINTER_WIDTH_V1,
                PRODUCTION_GFX942_DEVICE_CPU_V1,
                "",
                "+wavefrontsize32,-wavefrontsize64,-xnack",
            )
            .unwrap(),
        ];
        for substitution in substitutions {
            assert!(matches!(
                validate_authoritative_rustc_target_v1(
                    ProductionAmdTargetProfileV1::Gfx942,
                    &substitution,
                ),
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
                        validate_authoritative_rustc_target_v1(
                            ProductionAmdTargetProfileV1::Gfx942,
                            &target,
                        )
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
        let sysroot = Command::new("rustc")
            .args(["--print", "sysroot"])
            .output()
            .unwrap();
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

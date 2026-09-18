use super::*;
use fe2o3_kernel_ir::{F32MathFunction, F32MathImplementation, FloatOperation, FunctionRole};

// Test-only observation request, never consulted by a shipping compiler path.
const CHILD_EXP_CONTRACT: &str = "FE2O3_TEST_CHECKED_OUTPUT_EXP_CONTRACT";

pub(super) fn configure_child(command: &mut Command, enabled: bool) {
    command.env_remove(CHILD_EXP_CONTRACT);
    if enabled {
        command.env(CHILD_EXP_CONTRACT, "1");
    }
}

fn requested() -> Result<bool, SourceFailure> {
    match env::var_os(CHILD_EXP_CONTRACT) {
        None => Ok(false),
        Some(value) if value == "1" => Ok(true),
        Some(_) => Err(SourceFailure::new(
            SourceStage::Observation,
            "invalid test Exp contract request",
        )),
    }
}

pub(super) fn check_actual_if_requested(
    stage: &dispatch::Stage,
) -> Result<Option<[u8; 32]>, SourceFailure> {
    if !requested()? {
        return Ok(None);
    }
    let output = stage.output();
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget =
        fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
    let mut declarations = 0;
    let mut calls = 0;
    for function in &output.module().functions {
        let descriptor =
            FloatOperation::f32_math_descriptor_with_budget_v1(&function.id, &mut budget)
                .map_err(|error| SourceFailure::new(SourceStage::Observation, error))?;
        if function.role == FunctionRole::ExternalImport {
            assert_eq!(
                descriptor,
                Some((F32MathFunction::Exp, F32MathImplementation::OcmlAbiV1))
            );
            declarations += 1;
        }
        for operation in function
            .body
            .iter()
            .flat_map(|body| &body.blocks)
            .flat_map(|block| &block.operations)
        {
            if let OperationKind::Call { callee, arguments } = &operation.kind
                && let Some(contract) =
                    FloatOperation::f32_math_descriptor_with_budget_v1(callee, &mut budget)
                        .map_err(|error| SourceFailure::new(SourceStage::Observation, error))?
            {
                assert_eq!(
                    contract,
                    (F32MathFunction::Exp, F32MathImplementation::OcmlAbiV1)
                );
                assert_eq!(arguments.len(), 1);
                assert!(
                    matches!(operation.results.as_slice(), [result] if result.ty == fe2o3_kernel_ir::Type::F32)
                );
                calls += 1;
            }
        }
    }
    assert_eq!((declarations, calls), (1, 1));
    assert_eq!(budget.storage(), 0);
    let source = match stage {
        dispatch::Stage::Direct(stage) => *stage
            .output()
            .source_semantic_kir()
            .canonical_kernel_ir_identity()
            .digest(),
        dispatch::Stage::Erased(stage) => *stage
            .output()
            .original_source()
            .executable()
            .canonical()
            .identity()
            .digest(),
    };
    Ok(Some(source))
}

pub(super) fn check_handoff_if_requested(
    handoff: &fe2o3_compiler_ffi::CompilerModuleHandoffV2,
    original_source_digest: Option<[u8; 32]>,
    actual_o_digest: &[u8; 32],
) -> Result<(), SourceFailure> {
    if !requested()? {
        return Ok(());
    }
    use fe2o3_compiler_ffi::*;
    let envelope = handoff.envelope();
    let identity = match (
        inspect_production_gfx942_compiler_ffi_envelope_v1(envelope),
        inspect_production_gfx950_compiler_ffi_envelope_v1(envelope),
    ) {
        (
            Some(ProductionGfx942CompilerFfiEnvelopeKindV1::OcmlExpF32 {
                canonical_kernel_ir_identity,
            }),
            None,
        )
        | (
            None,
            Some(ProductionGfx950CompilerFfiEnvelopeKindV1::OcmlExpF32 {
                canonical_kernel_ir_identity,
            }),
        ) => canonical_kernel_ir_identity,
        other => {
            return Err(SourceFailure::new(
                SourceStage::Observation,
                format!("exact target OCML Exp envelope absent: {other:?}"),
            ));
        }
    };
    // The import contract retains source N; worker construction independently
    // replays actual O against the final native text and descriptors.
    assert_eq!(Some(identity), original_source_digest);
    assert_ne!(&identity, actual_o_digest);
    assert_eq!(
        envelope.directional_symbols().imports().collect::<Vec<_>>(),
        ["__ocml_exp_f32"]
    );
    let llvm = std::str::from_utf8(handoff.module_bytes()).unwrap();
    assert!(llvm.contains("call float @__ocml_exp_f32(float "), "{llvm}");
    for attribute in [
        "\"denormal-fp-math-f32\"=\"ieee,ieee\"",
        "\"unsafe-fp-math\"=\"false\"",
        "\"no-infs-fp-math\"=\"false\"",
        "\"no-nans-fp-math\"=\"false\"",
        "\"no-signed-zeros-fp-math\"=\"false\"",
        "\"fp-contract\"=\"off\"",
    ] {
        assert!(
            llvm.contains(attribute),
            "missing strict attribute: {attribute}"
        );
    }
    assert!(!llvm.contains("call fast float @__ocml_exp_f32"));
    Ok(())
}

#[test]
#[ignore = "requires pinned nightly rust-src, AMD dependencies and ordinary-source compilation; no OCML numerical simulator"]
fn ordinary_rust_exp_reaches_actual_o_strict_native_and_exact_ffi_envelope_both_profiles() {
    for profile in [
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950,
    ] {
        ordinary_rust_checked_output_cases_for_profile(
            &[
                OrdinarySourceCase::F32Exp,
                OrdinarySourceCase::RetainedF32Exp,
            ],
            profile,
        );
    }
}

#[test]
fn exp_observation_request_does_not_reuse_a_stale_inherited_setting() {
    let mut command = Command::new("unused-test-command");
    command.env(CHILD_EXP_CONTRACT, "malformed");
    configure_child(&mut command, false);
    assert!(
        command
            .get_envs()
            .any(|(key, value)| key == CHILD_EXP_CONTRACT && value.is_none())
    );
    configure_child(&mut command, true);
    assert!(
        command
            .get_envs()
            .any(|(key, value)| key == CHILD_EXP_CONTRACT
                && value == Some(std::ffi::OsStr::new("1")))
    );
}

#[test]
fn a_canonical_but_foreign_import_owner_cannot_impersonate_the_compiler_exp_envelope() {
    use fe2o3_compiler_ffi::*;
    use reserved_fe2o3_symbols::{
        DEVICE_FFI_DIRECTION_IMPORT_V1, DeviceFfiContractFieldsV1, DeviceFfiDirectionV1,
        derive_device_ffi_contract_id_v1,
    };
    // Inert protocol negatives, not fabricated authenticated compiler success
    // or a claim that an external OCML library has been qualified/executed.
    for target_text in ["gfx942:xnack-", "gfx950:xnack-"] {
        let target = DeviceTargetV1::parse(target_text).unwrap();
        let identity = [0x37; 32];
        let semantic_text = "37".repeat(32);
        let symbol = "__ocml_exp_f32";
        let physical_abi = "C(f32[size=4,align=4])->f32[size=4,align=4]";
        let fields = DeviceFfiContractFieldsV1 {
            direction: DEVICE_FFI_DIRECTION_IMPORT_V1,
            symbol,
            calling_convention: "C",
            code_object_version: 6,
            target: target_text,
            physical_abi,
            effects: "none",
            semantic_identity: &semantic_text,
        };
        let contract = CompilerFfiContractV1::new(
            derive_device_ffi_contract_id_v1(fields),
            DeviceFfiDirectionV1::Import,
            CompilerFfiLinkRoleV1::RequiresExternalDefinition,
            target,
            CodeObjectVersion::V6,
            CompilerFfiSourceOwnerV1::new("foreign", "foreign::exp", [7; 16], "foreign_exp")
                .unwrap(),
            symbol,
            physical_abi,
            "none",
            identity,
        )
        .unwrap();
        let mut builder =
            CompilerFfiEnvelopeBuilderV1::new(target, CodeObjectVersion::V6, 1).unwrap();
        builder.push(contract).unwrap();
        let envelope = builder.finish().unwrap();
        assert_eq!(
            envelope.directional_symbols().imports().collect::<Vec<_>>(),
            [symbol]
        );
        assert_eq!(
            inspect_production_gfx942_compiler_ffi_envelope_v1(&envelope),
            None
        );
        assert_eq!(
            inspect_production_gfx950_compiler_ffi_envelope_v1(&envelope),
            None
        );
        assert!(!envelope.authenticates_compiler_origin());
        assert!(!envelope.grants_link_authority());
    }
}

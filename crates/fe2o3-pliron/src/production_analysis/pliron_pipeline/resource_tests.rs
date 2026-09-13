#[cfg(test)]
mod tests {
    use dialect_gpu::{ExecutionDomainAttr, ExecutionLayoutOp};
    use dialect_kernel::{
        DIALECT_NAME, IndexConstantOp, ReturnOp, TensorConvergenceAttr, TensorLayoutOp,
        register_dialect,
    };
    use fe2o3_kernel_ir::TensorLayoutContractV1;
    use pliron::{
        builtin::{
            attributes::UnitAttr,
            ops::FuncOp,
            types::{FunctionType, IntegerType, Signedness},
        },
        context::Context,
        dialect::DialectName,
        op::Op,
        operation::Operation,
        r#type::Typed,
    };

    use super::*;

    include!("replacement_limits_v1_tests.rs");
    include!("replacement_epoch_v1_tests.rs");

    include!("cache_release_v1_tests.rs");

    #[test]
    fn resource_admission_phase_survives_pipeline_error_conversion() {
        use crate::ProductionAnalysisResourcePhaseV1 as Phase;

        for phase in [Phase::RaceFreedom, Phase::ReportValidation] {
            let bound =
                ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, 3, 2, 1).unwrap();
            let error = ProductionAnalysisResourceLimitsV1::new(2, 3)
                .require(phase, bound)
                .unwrap_err();
            let error = resource_upper_bound_error_v1(error);
            assert_eq!(
                error,
                ProductionPlironPreloweringErrorV2::ResourceLimit {
                    phase,
                    producing_pass: None,
                    resource: "work upper bound",
                },
            );
            assert!(error.to_string().starts_with(&format!(
                "production PLIRON resource limit was exceeded [{}]: work upper bound",
                phase.code(),
            )));
        }
    }

    #[test]
    fn effect_resource_composition_skips_unreachable_nested_ownership_only_when_empty() {
        use crate::ProductionAnalysisResourcePhaseV1 as Phase;

        let empty_effect = ProductionAnalysisResourceUpperBoundV1::checked_phase(
            Phase::EffectRefinement,
            63,
            0,
            0,
        )
        .unwrap();
        let ownership = ProductionAnalysisResourceUpperBoundV1::checked_phase(
            Phase::HierarchicalOwnership,
            100,
            7,
            11,
        )
        .unwrap();
        let empty = compose_effect_refinement_resource_upper_bound_v1(
            ProductionAnalysisInputCensusV1 {
                effect_refinement_contracts: 0,
                ..ProductionAnalysisInputCensusV1::default()
            },
            empty_effect,
            ownership,
        )
        .unwrap();
        assert_eq!(empty, empty_effect);

        let nonempty_effect = ProductionAnalysisResourceUpperBoundV1::checked_phase(
            Phase::EffectRefinement,
            63,
            2,
            3,
        )
        .unwrap();
        let nonempty = compose_effect_refinement_resource_upper_bound_v1(
            ProductionAnalysisInputCensusV1 {
                effect_refinement_contracts: 1,
                ..ProductionAnalysisInputCensusV1::default()
            },
            nonempty_effect,
            ownership,
        )
        .unwrap();
        assert_eq!(nonempty.work_upper_bound(), 163);
        assert_eq!(nonempty.retained_storage_upper_bound(), 2);
        assert_eq!(nonempty.peak_storage_upper_bound(), 23);
    }

    #[test]
    fn stage_resource_failure_retains_the_producing_pass() {
        use crate::ProductionAnalysisResourcePhaseV1 as Phase;

        let pass = KernelCheckPassKindV1::WorkgroupMemory;
        for phase in [
            Phase::WorkgroupMemory,
            Phase::PassPreservation,
            Phase::ReportValidation,
        ] {
            let error = stage_resource_upper_bound_error_v1(
                crate::production_analysis::ProductionAnalysisResourceLimitV1 {
                    phase,
                    resource: "peak storage upper bound",
                },
                pass,
            );
            assert_eq!(
                error,
                ProductionPlironPreloweringErrorV2::ResourceLimit {
                    phase,
                    producing_pass: Some(pass),
                    resource: "peak storage upper bound",
                },
            );
            assert!(
                error
                    .to_string()
                    .contains(&format!("(producing pass: {})", pass.name()))
            );
        }
    }

    fn setup() -> Context {
        let mut context = Context::new();
        register_dialect(
            &mut context,
            &DialectName::try_new(DIALECT_NAME).expect("valid kernel dialect name"),
        )
        .expect("register kernel dialect");
        dialect_gpu::register_dialect(&mut context).expect("register gpu dialect");
        context
    }

    fn valid_tensor_function(context: &mut Context, name: &str) -> (FuncOp, ReturnOp) {
        let function = FuncOp::new(
            context,
            name.try_into().expect("valid function name"),
            FunctionType::get(context, vec![], vec![]),
        );
        let entry = function.get_entry_block(context);
        let layout = ExecutionLayoutOp::new_with_domain(
            context,
            7,
            [64, 1, 1],
            [64, 1, 1],
            64,
            ExecutionDomainAttr::FullPhysicalWorkgroups,
        );
        let tensor = TensorLayoutOp::new(
            context,
            &TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64(),
            TensorConvergenceAttr::UniformSubgroup,
            64,
        );
        let ret = ReturnOp::new(context);
        for operation in [
            layout.get_operation(),
            tensor.get_operation(),
            ret.get_operation(),
        ] {
            operation.insert_at_back(entry, context);
        }
        (function, ret)
    }

    #[test]
    fn production_v2_brackets_every_pass_with_live_exact_identity() {
        let context = &mut setup();
        let (function, _) = valid_tensor_function(context, "preserved_pipeline");

        let report = require_production_pliron_checks_before_lowering_v2(context, &function)
            .expect("valid tensor function passes the preserved production pipeline");

        assert!(report.is_clean());
        assert!(report.preservation().is_exact_identity());
        assert_eq!(report.preservation().certificates().len(), 9);
        assert_eq!(
            report
                .preservation()
                .certificates()
                .iter()
                .map(|certificate| certificate.pass())
                .collect::<Vec<_>>(),
            PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2,
        );
    }

    #[test]
    fn production_has_one_fixed_nine_stage_manifest_and_no_v1_route() {
        assert_eq!(
            PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2,
            crate::PRODUCTION_PLIRON_PASS_CONTRACTS_V1.map(|contract| contract.pass()),
        );
        assert_eq!(PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2.len(), 9);
        assert_eq!(
            PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2[4],
            KernelCheckPassKindV1::HierarchicalOwnership,
        );

        let source = concat!(
            include_str!("../pliron_pipeline.rs"),
            include_str!("repairs_v1.rs"),
            include_str!("resource_tests.rs"),
        );
        for removed_declaration in [
            concat!("pub struct ProductionPlironPreloweringReport", "V1"),
            concat!("pub enum ProductionPlironPreloweringError", "V1"),
            concat!("pub const PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_", "V1"),
            concat!(
                "pub fn require_production_pliron_checks_before_lowering_",
                "v1"
            ),
            concat!(
                "pub fn require_production_pliron_checks_with_atomic_target_before_lowering_",
                "v1"
            ),
        ] {
            assert!(
                !source.contains(removed_declaration),
                "removed production route declaration remains: {removed_declaration}",
            );
        }
        assert_eq!(
            source
                .lines()
                .filter(|line| {
                    line.trim_start()
                        .starts_with("pub const PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_")
                })
                .count(),
            1,
            "an alternate public production pass sequence remains",
        );
    }

    #[test]
    fn mutation_is_blamed_on_the_active_pass_and_has_a_compiler_repair() {
        let context = &mut setup();
        let (function, ret) = valid_tensor_function(context, "mutating_analysis");
        let inserted = IndexConstantOp::new(context, 1);
        let provider = LivePlironStructuralIdentityProviderV1::new(context, &function);
        let mut preservation = begin_production_pliron_pass_contract_session_v1(provider).unwrap();

        let error = preservation
            .run_contiguous_pass(KernelCheckPassKindV1::TensorLayout, || {
                inserted
                    .get_operation()
                    .insert_before(context, ret.get_operation());
                Ok::<_, ()>(())
            })
            .unwrap_err();

        assert!(matches!(
            error,
            PlironPassPreservationErrorV1::StructuralIdentityChanged {
                pass: KernelCheckPassKindV1::TensorLayout,
                source_code: "FE2O3-PRESERVE-010",
                ..
            }
        ));
        let wrapped = ProductionPlironPreloweringErrorV2::Preservation(error);
        let rendered = wrapped.to_string();
        assert!(rendered.contains("analysis-only pass TensorLayout"));
        assert!(rendered.contains("FE2O3-PRESERVE-010"));
        assert!(rendered.contains("FE2O3-FIX-PASS-PRESERVATION"));
    }

    fn assert_transient_mutation(
        error: &PlironPassPreservationErrorV1,
        pass: KernelCheckPassKindV1,
    ) {
        assert!(matches!(
            error,
            PlironPassPreservationErrorV1::MutationAttempted {
                pass: Some(observed),
                ..
            } if *observed == pass
        ));
        assert_eq!(error.code(), "FE2O3-PRESERVE-020");
        let repair = pass_preservation_repair_for_error_v1(error);
        assert_eq!(repair.pass(), pass);
        assert_eq!(
            repair.action(),
            KernelCheckRepairActionV1::PreservePassSemantics
        );
    }

    #[test]
    fn restored_attributes_and_operation_structure_are_rejected() {
        let context = &mut setup();
        let (function, ret) = valid_tensor_function(context, "restored_attribute");
        let key = "transient_test_attribute".try_into().unwrap();
        let operation = ret.get_operation();
        let provider = LivePlironStructuralIdentityProviderV1::new(context, &function);
        let mut preservation = begin_production_pliron_pass_contract_session_v1(provider).unwrap();
        let error = preservation
            .run_contiguous_pass(KernelCheckPassKindV1::TensorLayout, || {
                let original = operation.deref(context).attributes.clone();
                operation
                    .deref_mut(context)
                    .attributes
                    .set(key, UnitAttr::new());
                operation.deref_mut(context).attributes = original;
                Ok::<_, ()>(())
            })
            .unwrap_err();
        assert_transient_mutation(&error, KernelCheckPassKindV1::TensorLayout);

        let context = &mut setup();
        let (function, ret) = valid_tensor_function(context, "restored_operation_kind");
        let different_kind = IndexConstantOp::new(context, 9);
        let provider = LivePlironStructuralIdentityProviderV1::new(context, &function);
        let mut preservation = begin_production_pliron_pass_contract_session_v1(provider).unwrap();
        let error = preservation
            .run_contiguous_pass(KernelCheckPassKindV1::TensorLayout, || {
                different_kind
                    .get_operation()
                    .insert_before(context, ret.get_operation());
                different_kind.get_operation().unlink(context);
                Ok::<_, ()>(())
            })
            .unwrap_err();
        assert_transient_mutation(&error, KernelCheckPassKindV1::TensorLayout);
    }

    #[test]
    fn restored_value_type_and_cfg_successor_are_rejected() {
        let context = &mut setup();
        let (function, ret) = valid_tensor_function(context, "restored_value_type");
        let constant = IndexConstantOp::new(context, 1);
        constant
            .get_operation()
            .insert_before(context, ret.get_operation());
        let value = constant.get_operation().deref(context).get_result(0);
        let original_type = value.get_type(context);
        let alternate_type: pliron::r#type::TypeHandle =
            IntegerType::get(context, 32, Signedness::Signless).into();
        let provider = LivePlironStructuralIdentityProviderV1::new(context, &function);
        let mut preservation = begin_production_pliron_pass_contract_session_v1(provider).unwrap();
        let error = preservation
            .run_contiguous_pass(KernelCheckPassKindV1::TensorLayout, || {
                value.set_type(context, alternate_type);
                value.set_type(context, original_type);
                Ok::<_, ()>(())
            })
            .unwrap_err();
        assert_transient_mutation(&error, KernelCheckPassKindV1::TensorLayout);

        let context = &mut setup();
        let (function, ret) = valid_tensor_function(context, "restored_cfg");
        let entry = function.get_entry_block(context);
        let terminator = ret.get_operation();
        let provider = LivePlironStructuralIdentityProviderV1::new(context, &function);
        let mut preservation = begin_production_pliron_pass_contract_session_v1(provider).unwrap();
        let error = preservation
            .run_contiguous_pass(KernelCheckPassKindV1::TensorLayout, || {
                Operation::push_successor(terminator, context, entry);
                assert_eq!(Operation::pop_successor(terminator, context), entry);
                Ok::<_, ()>(())
            })
            .unwrap_err();
        assert_transient_mutation(&error, KernelCheckPassKindV1::TensorLayout);
    }

    #[test]
    fn failed_mutable_borrow_and_mutation_before_error_are_rejected() {
        let context = &mut setup();
        let (function, ret) = valid_tensor_function(context, "failed_mutable_borrow");
        let operation = ret.get_operation();
        let provider = LivePlironStructuralIdentityProviderV1::new(context, &function);
        let mut preservation = begin_production_pliron_pass_contract_session_v1(provider).unwrap();
        let error = preservation
            .run_contiguous_pass(KernelCheckPassKindV1::TensorLayout, || {
                let read = operation.deref(context);
                assert!(operation.try_deref_mut(context).is_err());
                drop(read);
                Ok::<_, ()>(())
            })
            .unwrap_err();
        assert_transient_mutation(&error, KernelCheckPassKindV1::TensorLayout);

        let context = &mut setup();
        let (function, ret) = valid_tensor_function(context, "mutation_before_error");
        let operation = ret.get_operation();
        let key = "restored_before_error".try_into().unwrap();
        let provider = LivePlironStructuralIdentityProviderV1::new(context, &function);
        let mut preservation = begin_production_pliron_pass_contract_session_v1(provider).unwrap();
        let error = preservation
            .run_contiguous_pass(KernelCheckPassKindV1::TensorLayout, || {
                let original = operation.deref(context).attributes.clone();
                operation
                    .deref_mut(context)
                    .attributes
                    .set(key, UnitAttr::new());
                operation.deref_mut(context).attributes = original;
                Err::<(), _>("analysis rejected after restoring its mutation")
            })
            .unwrap_err();
        assert_transient_mutation(&error, KernelCheckPassKindV1::TensorLayout);
    }

    #[test]
    fn read_only_stage_is_clean_and_later_mutation_has_exact_stage_attribution() {
        let context = &mut setup();
        let (function, ret) = valid_tensor_function(context, "stage_attribution");
        let operation = ret.get_operation();
        let provider = LivePlironStructuralIdentityProviderV1::new(context, &function);
        let mut preservation = begin_production_pliron_pass_contract_session_v1(provider).unwrap();

        preservation
            .run_contiguous_pass(KernelCheckPassKindV1::TensorLayout, || {
                assert_eq!(operation.deref(context).get_num_results(), 0);
                Ok::<_, ()>(())
            })
            .expect("read-only stage preserves the epoch")
            .unwrap();

        let error = preservation
            .run_contiguous_pass(KernelCheckPassKindV1::MemoryBounds, || {
                let original = operation.deref(context).attributes.clone();
                operation.deref_mut(context).attributes = original.clone();
                operation.deref_mut(context).attributes = original;
                Ok::<_, ()>(())
            })
            .unwrap_err();
        assert_transient_mutation(&error, KernelCheckPassKindV1::MemoryBounds);

        let context = &mut setup();
        let (function, ret) = valid_tensor_function(context, "stale_capability");
        let operation = ret.get_operation();
        let provider = LivePlironStructuralIdentityProviderV1::new(context, &function);
        let mut preservation = begin_production_pliron_pass_contract_session_v1(provider).unwrap();
        preservation
            .run_contiguous_pass(KernelCheckPassKindV1::TensorLayout, || Ok::<_, ()>(()))
            .unwrap()
            .unwrap();

        let original = operation.deref(context).attributes.clone();
        operation.deref_mut(context).attributes = original.clone();
        operation.deref_mut(context).attributes = original;
        let error = preservation
            .run_contiguous_pass(KernelCheckPassKindV1::MemoryBounds, || Ok::<_, ()>(()))
            .unwrap_err();
        assert!(matches!(
            error,
            PlironPassPreservationErrorV1::StaleMutationEpoch {
                pass: KernelCheckPassKindV1::MemoryBounds,
                ..
            }
        ));
        assert_eq!(error.code(), "FE2O3-PRESERVE-021");
        let repair = pass_preservation_repair_for_error_v1(&error);
        assert_eq!(repair.pass(), KernelCheckPassKindV1::MemoryBounds);
    }

    #[test]
    fn rejected_analysis_is_compared_before_its_error_is_returned() {
        let context = &mut setup();
        let (function, _) = valid_tensor_function(context, "rejected_analysis");
        let provider = LivePlironStructuralIdentityProviderV1::new(context, &function);
        let mut preservation = begin_production_pliron_pass_contract_session_v1(provider).unwrap();

        let rejected = preservation
            .run_contiguous_pass(KernelCheckPassKindV1::TensorLayout, || {
                Err::<(), _>("analysis rejected")
            })
            .expect("unchanged IR is certified even when the analysis rejects");
        assert_eq!(rejected, Err("analysis rejected"));

        for pass in PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2
            .iter()
            .copied()
            .skip(1)
        {
            preservation
                .run_contiguous_pass(pass, || Ok::<_, ()>(()))
                .unwrap()
                .unwrap();
        }
        assert!(preservation.finish().unwrap().is_exact_identity());
    }

    #[test]
    fn identity_prerequisite_failure_proposes_a_source_structure_repair() {
        let error = ProductionPlironPreloweringErrorV2::Preservation(
            PlironPassPreservationErrorV1::IdentityUnavailable {
                source_code: "FE2O3-PRESERVE-001",
                detail: "unsupported production type".to_owned(),
            },
        );
        let repair = error.repair_hints().remove(0);
        assert_eq!(repair.pass(), KernelCheckPassKindV1::Structural);
        assert_eq!(repair.action(), KernelCheckRepairActionV1::RepairStructure);
        assert!(
            repair
                .message()
                .contains("closed production ranked PLIRON subset")
        );
    }
}

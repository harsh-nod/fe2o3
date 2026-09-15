use super::*;

struct MaxProbe {
    completed: bool,
}

impl Callbacks for MaxProbe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("ordered_max_import_source.rs".into()),
            input: include_str!("ordered_max_source.rs").into(),
        };
    }

    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        assert_eq!(
            crate::collector::session_crate_binding(tcx),
            Some(registration_binding_v1())
        );
        let target = crate::production_target_v1::RetainedProductionTargetV1::authenticate_live_before_collection(tcx).unwrap();
        let partitions = tcx.collect_and_partition_mono_items(());
        let closure = crate::collector::collect_authenticated_kernel_closure_v1(
            tcx,
            partitions.codegen_units,
            false,
            target,
        )
        .unwrap();
        let typed = closure.rederive_typed_descriptor_roots(tcx).unwrap();
        let imported = construct_production_semantic_mir_v1(
            tcx,
            closure,
            DebugSourceCaptureRequestV2::Disabled,
        )
        .expect("actual Wave16 maximum must retain its exact source contract");
        let mir = &imported.semantic_mir;
        assert_eq!(mir.wire_version(), SemanticMirWireVersionV1::V22);
        let decoded = AdmittedInertSemanticMirV1::decode_exact_v22_canonical(
            mir.canonical_encoding(),
            SemanticMirLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(decoded.canonical_encoding(), mir.canonical_encoding());
        let mut maximums = 0;
        for callable in mir.callables() {
            let SemanticCallableDeclV1::CompilerIntrinsic {
                binding,
                operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
                ..
            } = callable
            else {
                continue;
            };
            match contract.operation() {
                SemanticExecutionCapabilityOperationV1::SubgroupPartition(
                    SemanticSubgroupPartitionOperationV1::ReduceMaxF32 {
                        partition_reference,
                        partition,
                        element,
                        width,
                        partition_width,
                    },
                ) => {
                    maximums += 1;
                    assert_eq!((width, partition_width), (64, 16));
                    shared_edge(mir, partition_reference, partition);
                    assert_eq!(
                        binding.abi().source_argument_ownership(),
                        [
                            SemanticSourceArgumentOwnershipV1::SharedBorrow,
                            SemanticSourceArgumentOwnershipV1::ByValue
                        ]
                    );
                    assert_eq!(
                        contract.signature().arguments().collect::<Vec<_>>(),
                        [partition_reference, element]
                    );
                    assert_eq!(contract.source_identity(), binding.identity());
                    assert_eq!(
                        mir.types()[element.index() as usize].shape(),
                        &SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 32 })
                    );
                }
                SemanticExecutionCapabilityOperationV1::SubgroupPartition(
                    SemanticSubgroupPartitionOperationV1::ReduceSumF32 { .. },
                ) => panic!("maximum was relabeled as sum"),
                _ => {}
            }
        }
        assert_eq!(maximums, 1, "one real terminal instance used twice");
        // Includes ranked admission, actual loan/owner checks, AMD and simulation.
        lowering_tests::check(imported, typed);
        self.completed = true;
        Compilation::Stop
    }
}

#[test]
#[ignore = "requires complete cached FE2O3_CORE_TRY_DEVICE_RMETA, HOST_DEPS, AMDGPU_CORE and AMDGPU_BUILTINS; never runs Cargo"]
fn ordered_max_full_import_gfx950_v22() {
    run(
        "collector::production_importer_v1::subgroup_partition_v1::workgroup_source_v1::canonical_transport_v1::import_tests::ordered_max_import_tests::ordered_max_full_import_gfx950_v22",
        |args| {
            let mut probe = MaxProbe { completed: false };
            rustc_driver::run_compiler(args, &mut probe);
            assert!(
                probe.completed,
                "actual source must reach canonical lowering and both consumers"
            );
        },
    );
}

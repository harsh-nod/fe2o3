fn assert_slice_metadata_argument_calls_v1(bundle: &fe2o3_kernel_ir::VerifiedSimulationBundleV5) {
    use fe2o3_kernel_ir::{CastKind, FunctionRole, OperationKind, ScalarType, Type};
    use fe2o3_mir_model::semantic_mir_v1::*;

    let semantic = AdmittedInertSemanticMirV1::decode_current_production_canonical(
        bundle.semantic_mir(),
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    let unsigned = |ty: SemanticTypeIdV1| {
        matches!(
            semantic.types()[ty.index() as usize].shape(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 64
            })
        )
    };
    assert!(
        semantic.functions().iter().any(|function| {
            if function.abi().extern_abi() != SemanticExternAbiV1::RustCall {
                return false;
            }
            let [_, tuple] = function.abi().source_input_types() else {
                return false;
            };
            let SemanticTypeShapeV1::Tuple(fields) =
                semantic.types()[tuple.index() as usize].shape()
            else {
                return false;
            };
            let [first, unit, last] = fields.fields() else {
                return false;
            };
            unsigned(*first)
                && unsigned(*last)
                && matches!(
                    semantic.types()[unit.index() as usize].shape(),
                    SemanticTypeShapeV1::Unit
                )
                && function.abi().source_argument_ownership()[0]
                    == SemanticSourceArgumentOwnershipV1::ByValue
        }),
        "the owned RustCall source tuple must retain its ignored middle field"
    );
    assert!(
        semantic
            .functions()
            .iter()
            .flat_map(|function| function.blocks())
            .flat_map(|block| block.statements())
            .any(|statement| {
                matches!(statement.kind(), SemanticStatementKindV1::Assign(assignment)
                if matches!(assignment.value().kind(), SemanticRvalueKindV1::Unary {
                    operation: SemanticUnaryOpV1::PointerMetadata, ..
                }))
            }),
        "the retained source must compute slice metadata"
    );

    let (_, module) =
        fe2o3_kernel_ir::VerifiedCanonicalKernelIrV10::from_canonical_bytes_with_module(
            bundle.canonical_kir_v10().to_vec(),
        )
        .unwrap();
    let [kernel] = module.kernels.as_slice() else {
        panic!("one selected source kernel")
    };
    let entry = module.function(&kernel.entry).unwrap();
    let body = entry.body.as_ref().unwrap();
    let operations = body
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .collect::<Vec<_>>();
    let types = body
        .parameters
        .iter()
        .copied()
        .zip(entry.signature.parameters.iter())
        .chain(
            body.blocks
                .iter()
                .flat_map(|block| &block.parameters)
                .map(|value| (value.id, &value.ty)),
        )
        .chain(
            operations
                .iter()
                .flat_map(|operation| &operation.results)
                .map(|value| (value.id, &value.ty)),
        )
        .collect::<std::collections::BTreeMap<_, _>>();
    let casts = operations
        .iter()
        .filter(|operation| {
            matches!(&operation.kind, OperationKind::Cast { kind: CastKind::Bitcast, value, to }
            if types.get(value).copied() == Some(&Type::INDEX)
                && *to == Type::Scalar(ScalarType::U64)
                && operation.results.len() == 1 && operation.results[0].ty == *to)
        })
        .map(|operation| operation.results[0].id)
        .collect::<Vec<_>>();
    assert!(
        !casts.is_empty(),
        "source metadata must have an actual INDEX to U64 transport cast"
    );
    let mut calls_by_arity = [0; 3];
    let mut consumes_cast = false;
    for operation in &operations {
        let OperationKind::Call { callee, arguments } = &operation.kind else {
            continue;
        };
        let helper = module.function(callee).unwrap();
        if helper.role != FunctionRole::InternalHelper
            || helper.signature.parameters != vec![Type::Scalar(ScalarType::U64); arguments.len()]
            || helper.signature.results != [Type::Scalar(ScalarType::U64)]
        {
            continue;
        }
        if (1..=3).contains(&arguments.len()) {
            calls_by_arity[arguments.len() - 1] += 1;
        }
        consumes_cast |= arguments.iter().any(|argument| casts.contains(argument));
    }
    assert!(
        consumes_cast,
        "a retained helper call must consume a real metadata conversion"
    );
    assert!(
        calls_by_arity[0] >= 1 && calls_by_arity[1] >= 2 && calls_by_arity[2] >= 1,
        "scalar, repeated, mixed and owned RustCall calls must survive: {calls_by_arity:?}"
    );
}

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn ordinary_source_slice_metadata_arguments_match_rust_in_simulation() {
    const CANARY: u64 = 0xa5c3_7e19_6d82_f04b;
    let target = ScratchTarget::new();
    for architecture in ["gfx942", "gfx950"] {
        let bundle_path = target
            .path()
            .join(format!("slice-metadata-arguments-{architecture}.fe2sim"));
        let exported = output(
            simulation_export_command_for_feature(
                architecture,
                &bundle_path,
                &target.path().join(architecture),
                Some(5),
                "slice_metadata_arguments",
            ),
            "export ordinary Rust slice metadata helper arguments",
        );
        assert!(exported.status.success(), "{}", exported.stderr);
        let bundle = fe2o3_kernel_ir::VerifiedSimulationBundleV5::from_canonical_bytes(
            std::fs::read(&bundle_path).unwrap(),
        )
        .unwrap();
        assert_slice_metadata_argument_calls_v1(&bundle);
        for (length, ordinary) in [
            (0_usize, 0_u64),
            (1, u64::MAX),
            (7, 0x1_0000_0001),
            (65, u64::MAX - 2),
            (129, 0x8000_0000_0000_0000),
        ] {
            let n = length as u64;
            let expected = [
                n.wrapping_add(3),
                n.wrapping_mul(6),
                n.wrapping_mul(5).wrapping_add(ordinary),
                n.wrapping_mul(5).wrapping_add(ordinary.wrapping_mul(2)),
            ];
            let input = (0..length)
                .flat_map(|index| (index as u32).wrapping_mul(0x9e37_79b9).to_le_bytes())
                .collect::<Vec<_>>();
            for grid in [64_usize, 128] {
                let initial = CANARY.to_le_bytes().repeat(grid + 3);
                let mut arguments = vec![
                    json!({ "kind": "buffer", "element": "u32", "access": "read_only",
                        "alignment": 4, "bytes": format!("0x{}", hex(&input)) }),
                    json!({ "kind": "scalar", "type": "u64", "bits": format!("0x{ordinary:016x}") }),
                ];
                for _ in 0..4 {
                    arguments.push(
                        json!({ "kind": "buffer", "element": "u64", "access": "read_write",
                        "alignment": 8, "bytes": format!("0x{}", hex(&initial)) }),
                    );
                }
                let request_path = target.path().join("slice-metadata-arguments-request.json");
                std::fs::write(&request_path, serde_json::to_vec(&json!({
                    "schema": "fe2o3-simulation-request-v1", "kernel": "slice_metadata_arguments",
                    "grid": [grid, 1, 1], "workgroup": [64, 1, 1], "arguments": arguments,
                })).unwrap()).unwrap();
                let admitted =
                    fe2o3_kir_sim_cli::load_debug_simulation_bundle_v5(&bundle_path, &request_path)
                        .unwrap();
                let execution = admitted
                    .input()
                    .module
                    .simulate(
                        &admitted.input().request,
                        admitted.input().simulation_target(),
                        admitted.input().simulation_limits,
                    )
                    .unwrap_or_else(|error| {
                        panic!("{architecture} length={length} ordinary={ordinary:#x}: {error:?}")
                    });
                assert_eq!(execution.invocations_executed(), grid as u64);
                assert_eq!(execution.buffer(0).unwrap().bytes(), input);
                for (index, value) in expected.into_iter().enumerate() {
                    let mut bytes = value.to_le_bytes().repeat(grid);
                    bytes.extend_from_slice(&CANARY.to_le_bytes().repeat(3));
                    assert_eq!(
                        execution.buffer(index + 2).unwrap().bytes(),
                        bytes,
                        "{architecture} length={length} ordinary={ordinary:#x} output={index}"
                    );
                }
            }
        }
    }
}

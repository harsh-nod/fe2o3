fn assert_private_array_cfg_source(bundle_path: &Path, kernel: &str, needs_loop: bool) {
    use fe2o3_mir_model::semantic_mir_v1::{
        AdmittedInertSemanticMirV1, SemanticMirLimitsV1, SemanticOperandV1 as O,
        SemanticProjectionKindV1 as P, SemanticRvalueKindV1 as R, SemanticStatementKindV1 as S,
        SemanticTypeShapeV1 as T,
    };
    let bundle = fe2o3_kernel_ir::VerifiedSimulationBundleV3::from_canonical_bytes(
        std::fs::read(bundle_path).unwrap(),
    )
    .unwrap();
    let semantic = AdmittedInertSemanticMirV1::decode_current_production_canonical(
        bundle.semantic_mir(),
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    let [root] = semantic.roots() else {
        panic!("fixture must export exactly one original source root");
    };
    let function = &semantic.functions()[root.index() as usize];
    assert_eq!(
        function.kernel_entry().unwrap().export_symbol().as_bytes(),
        kernel.as_bytes()
    );
    let mut edges = vec![Vec::new(); function.blocks().len()];
    for (block, body) in function.blocks().iter().enumerate() {
        body.terminator()
            .kind()
            .try_for_each_edge::<std::convert::Infallible>(|edge| {
                edges[block].push(edge.target().index() as usize);
                Ok(())
            })
            .unwrap();
    }
    let mut reachable = vec![false; edges.len()];
    let mut pending = vec![function.entry().index() as usize];
    while let Some(block) = pending.pop() {
        if !std::mem::replace(&mut reachable[block], true) {
            pending.extend_from_slice(&edges[block]);
        }
    }
    let on_cycle = |start: usize| {
        let mut pending = edges[start].clone();
        let mut visited = vec![false; edges.len()];
        while let Some(block) = pending.pop() {
            if block == start {
                return true;
            }
            if !std::mem::replace(&mut visited[block], true) {
                pending.extend_from_slice(&edges[block]);
            }
        }
        false
    };
    let array_local = |local: fe2o3_mir_model::semantic_mir_v1::SemanticLocalIdV1| {
        matches!(
            semantic.types()[function.locals()[local.index() as usize].ty().index() as usize]
                .shape(),
            T::Array { length: 3, .. }
        )
    };
    let mut initializers = Vec::new();
    let mut fixed_reads = Vec::new();
    let mut dynamic_reads = Vec::new();
    for (block, body) in function.blocks().iter().enumerate() {
        for (statement_index, statement) in body.statements().iter().enumerate() {
            let S::Assign(assignment) = statement.kind() else {
                continue;
            };
            let destination = assignment.destination();
            if array_local(destination.local()) && destination.projections().is_empty() {
                initializers.push((destination.local(), block, statement_index));
            }
            let R::Use(O::Copy(place) | O::Move(place)) = assignment.value().kind() else {
                continue;
            };
            if !array_local(place.local()) {
                continue;
            }
            match place.projections() {
                [projection] => match projection.kind() {
                    P::ConstantIndex {
                        offset: 0,
                        from_end: false,
                        ..
                    } => {
                        fixed_reads.push((place.local(), block, statement_index));
                    }
                    P::Index(_) if reachable[block] => dynamic_reads.push(place.local()),
                    _ => {}
                },
                _ => {}
            }
        }
    }
    assert!(
        fixed_reads.iter().any(|&(local, block, statement)| {
            reachable[block]
                && dynamic_reads.contains(&local)
                && initializers.iter().any(|&(candidate, initializer, _)| {
                    candidate == local && initializer != block && reachable[initializer]
                })
                && !initializers
                    .iter()
                    .any(|&(candidate, initializer, before)| {
                        candidate == local && initializer == block && before < statement
                    })
                && (!needs_loop || on_cycle(block))
        }),
        "{kernel}: original MIR must retain a constant element read without a same-block initializer, plus a dynamic read of the same array; loop={needs_loop}"
    );
    let module = fe2o3_kernel_ir::decode_module_v7(bundle.inner_v2().canonical_kir_v7()).unwrap();
    let [entry] = module.kernels.as_slice() else {
        panic!("fixture must export exactly one physical root");
    };
    assert_eq!(entry.id.as_str(), kernel);
    let physical = module
        .functions
        .iter()
        .find(|row| row.id == entry.entry)
        .unwrap();
    let operations = physical
        .body
        .as_ref()
        .unwrap()
        .blocks
        .iter()
        .flat_map(|block| &block.operations);
    assert!(
        operations.clone().any(|operation| matches!(
            operation.kind,
            fe2o3_kernel_ir::OperationKind::Alloca {
                count: Some(_),
                address_space: fe2o3_kernel_ir::AddressSpace::Private,
                ..
            }
        )),
        "{kernel}: retained physical array must not disappear before translation validation"
    );
    assert!(operations.clone().any(|operation| matches!(&operation.kind,
        fe2o3_kernel_ir::OperationKind::Load { access, .. }
            if access.address_space == fe2o3_kernel_ir::AddressSpace::Private)));
    assert!(operations.clone().any(|operation| matches!(&operation.kind,
        fe2o3_kernel_ir::OperationKind::Store { access, .. }
            if access.address_space == fe2o3_kernel_ir::AddressSpace::Private)));
}

fn run_private_array_cfg_source(feature: &str, needs_loop: bool) {
    const CANARY: u32 = 0xa5c3_7e19;
    let target = ScratchTarget::new();
    for architecture in ["gfx942", "gfx950"] {
        let bundle_path = target
            .path()
            .join(format!("{feature}-{architecture}.fe2sim"));
        eprintln!("PRIVATE_ARRAY_CFG_EXPORT_BEGIN feature={feature} target={architecture}");
        let exported = output(
            simulation_export_command_for_feature(
                architecture,
                &bundle_path,
                &target.path().join(architecture),
                Some(3),
                feature,
            ),
            "export ordinary Rust private array CFG",
        );
        assert!(exported.status.success(), "{}", exported.stderr);
        assert_private_array_cfg_source(&bundle_path, feature, needs_loop);
        eprintln!(
            "PRIVATE_ARRAY_CFG_SOURCE_PASS feature={feature} target={architecture} cross_block=true loop={needs_loop} retained_physical_array=true"
        );
        let mut cases = 0;
        for original in [[7_u32, 0x1357_9bdf, u32::MAX], [0, 91, 0x8000_0000]] {
            for rounds in [0_u32, 1, 3] {
                let mut values = original;
                if needs_loop {
                    for _ in 0..rounds {
                        values = [
                            values[0].wrapping_add(original[1]),
                            original[1],
                            original[2],
                        ];
                    }
                } else if rounds != 0 {
                    values = [original[2], original[1], original[0]];
                }
                for selector in 0..3_u64 {
                    for grid in [64_usize, 128] {
                        let mut arguments = original.into_iter().map(|value|
                            json!({"kind":"scalar", "type":"u32", "bits":format!("0x{value:08x}")})
                        ).collect::<Vec<_>>();
                        arguments.push(json!({"kind":"scalar", "type":"u64", "bits":format!("0x{selector:016x}")}));
                        arguments.push(json!({"kind":"scalar", "type":"u32", "bits":format!("0x{rounds:08x}")}));
                        arguments.push(json!({"kind":"buffer", "element":"u32", "access":"read_write",
                            "alignment":4, "bytes":format!("0x{}",hex(&CANARY.to_le_bytes().repeat(grid+3)))}));
                        let request_path = target.path().join("private-array-cfg-request.json");
                        std::fs::write(
                            &request_path,
                            serde_json::to_vec(&json!({
                                "schema":"fe2o3-simulation-request-v1", "kernel":feature,
                                "grid":[grid,1,1], "workgroup":[64,1,1], "arguments":arguments,
                            }))
                            .unwrap(),
                        )
                        .unwrap();
                        let admitted = fe2o3_kir_sim_cli::load_debug_simulation_bundle_v3(
                            &bundle_path,
                            &request_path,
                        )
                        .unwrap();
                        let input = admitted.input();
                        let mut observed = ArrayIndexObservation::default();
                        let execution = input.module.simulate_observed_with_sink(
                            &input.request, input.simulation_target(),
                            fe2o3_kir_sim::SimulationLimitsV1 { max_events: 262_144, ..input.simulation_limits },
                            &mut observed,
                        ).unwrap_or_else(|error| panic!("{feature}/{architecture}: rounds={rounds}, selector={selector}, grid={grid}: {error:?}"));
                        let selected = values[0].wrapping_add(values[selector as usize]);
                        let mut expected = selected.to_le_bytes().repeat(grid);
                        expected.extend_from_slice(&CANARY.to_le_bytes().repeat(3));
                        assert_eq!(execution.buffer(5).unwrap().bytes(), expected);
                        assert_eq!(execution.invocations_executed(), grid as u64);
                        assert_eq!(observed.writes, grid);
                        assert!(!observed.failed);
                        cases += 1;
                    }
                }
            }
        }
        assert_eq!(cases, 36);
        eprintln!(
            "PRIVATE_ARRAY_CFG_EXPORT_PASS feature={feature} target={architecture} cases={cases}"
        );
    }
}

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn ordinary_source_private_array_cross_block_reaches_production_translation_and_simulates() {
    run_private_array_cfg_source("private_array_cross_block", false);
}

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn ordinary_source_private_array_loop_reaches_production_translation_and_simulates() {
    run_private_array_cfg_source("private_array_loop", true);
}

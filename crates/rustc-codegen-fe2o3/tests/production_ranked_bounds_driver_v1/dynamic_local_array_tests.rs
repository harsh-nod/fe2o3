#[derive(Default)]
struct ArrayIndexObservation {
    output: Option<u64>,
    writes: usize,
    failed: bool,
}

impl fe2o3_kir_sim::SimulationEventSinkV1 for ArrayIndexObservation {
    fn record(
        &mut self,
        event: &fe2o3_kir_sim::SimulationEventV1,
    ) -> Result<(), fe2o3_kir_sim::SimulationEventSinkErrorV1> {
        use fe2o3_kir_sim::{SimulationEventKindV1 as E, SimulationExecutionOutcomeV1 as O};
        match &event.kind {
            E::AllocationPreexisting {
                allocation,
                address_space: fe2o3_kernel_ir::AddressSpace::Global,
                ..
            } => assert!(self.output.replace(*allocation).is_none()),
            E::MemoryWrite { allocation, .. }
            | E::MemoryAtomic {
                allocation,
                committed: Some(_),
                ..
            } if self.output == Some(*allocation) => self.writes += 1,
            E::InvocationEnd { outcome: O::Failed } => self.failed = true,
            _ => {}
        }
        Ok(())
    }
}

fn assert_dynamic_array_source_mir(bundle_path: &Path) {
    use fe2o3_mir_model::semantic_mir_v1::{
        AdmittedInertSemanticMirV1, SemanticAssertMessageV1 as Assert, SemanticMirLimitsV1,
        SemanticOperandV1 as O, SemanticProjectionKindV1 as P, SemanticRvalueKindV1 as R,
        SemanticScalarTypeV1 as Scalar, SemanticStatementKindV1 as S,
        SemanticTerminatorKindV1 as Term, SemanticTypeShapeV1 as T,
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
    assert!(
        semantic.functions().iter().any(|function| {
            let dynamic = function
                .blocks()
                .iter()
                .flat_map(|block| block.statements())
                .any(|statement| {
                    let S::Assign(assignment) = statement.kind() else {
                        return false;
                    };
                    let R::Use(O::Copy(place) | O::Move(place)) = assignment.value().kind() else {
                        return false;
                    };
                    let local = &function.locals()[place.local().index() as usize];
                    let T::Array { element, length: 3 } =
                        semantic.types()[local.ty().index() as usize].shape()
                    else {
                        return false;
                    };
                    if !matches!(
                        semantic.types()[element.index() as usize].shape(),
                        T::Scalar(Scalar::Integer {
                            signed: false,
                            bits: 32
                        })
                    ) {
                        return false;
                    }
                    place.projections().iter().any(|projection| {
                        let P::Index(index) = projection.kind() else {
                            return false;
                        };
                        let ty = function.locals()[index.index() as usize].ty();
                        matches!(
                            semantic.types()[ty.index() as usize].shape(),
                            T::Scalar(Scalar::Integer {
                                signed: false,
                                bits: 64
                            })
                        )
                    })
                });
            dynamic
                && function.blocks().iter().any(|block| {
                    matches!(
                        block.terminator().kind(),
                        Term::Assert {
                            expected: true,
                            message: Assert::BoundsCheck { .. },
                            ..
                        }
                    )
                })
        }),
        "source must retain dynamic array indexing and its Rust bounds assertion"
    );
}

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn ordinary_source_dynamic_local_array_matches_rust_in_simulation() {
    use fe2o3_kir_sim::{SimulationErrorV1, SimulationExecutionErrorKindV1};
    const CANARY: u32 = 0xa5c3_7e19;
    let target = ScratchTarget::new();
    for architecture in ["gfx942", "gfx950"] {
        let bundle_path = target
            .path()
            .join(format!("dynamic-array-{architecture}.fe2sim"));
        let exported = output(
            simulation_export_command_for_feature(
                architecture,
                &bundle_path,
                &target.path().join(architecture),
                Some(3),
                "dynamic_local_array",
            ),
            "export ordinary Rust dynamic local array",
        );
        assert!(exported.status.success(), "{}", exported.stderr);
        assert_dynamic_array_source_mir(&bundle_path);
        for values in [[7_u32, 0x1357_9bdf, u32::MAX], [0, 91, 0x8000_0000]] {
            for grid in [64_usize, 128] {
                for selector in [0_u64, 1, 2, 3, 1_u64 << 32, u64::MAX] {
                    let mut arguments = values
                        .into_iter()
                        .map(|value| {
                            json!({
                                "kind":"scalar", "type":"u32", "bits":format!("0x{value:08x}")
                            })
                        })
                        .collect::<Vec<_>>();
                    arguments.push(json!({"kind":"scalar", "type":"u64",
                        "bits":format!("0x{selector:016x}")}));
                    arguments.push(json!({"kind":"buffer", "element":"u32", "access":"read_write",
                        "alignment":4, "bytes":format!("0x{}", hex(&CANARY.to_le_bytes().repeat(grid+3)))}));
                    let request_path = target.path().join("dynamic-array-request.json");
                    std::fs::write(
                        &request_path,
                        serde_json::to_vec(&json!({
                            "schema":"fe2o3-simulation-request-v1", "kernel":"dynamic_local_array",
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
                    let result = input.module.simulate_observed_with_sink(
                        &input.request,
                        input.simulation_target(),
                        input.simulation_limits,
                        &mut observed,
                    );
                    assert!(observed.output.is_some());
                    if selector < 3 {
                        let execution = result.unwrap_or_else(|error| {
                            panic!("{architecture} grid={grid} selector={selector}: {error:?}")
                        });
                        let mut expected = values[selector as usize].to_le_bytes().repeat(grid);
                        expected.extend_from_slice(&CANARY.to_le_bytes().repeat(3));
                        assert_eq!(execution.buffer(4).unwrap().bytes(), expected);
                        assert_eq!(execution.invocations_executed(), grid as u64);
                        assert_eq!(observed.writes, grid);
                        assert!(!observed.failed);
                    } else {
                        let SimulationErrorV1::Execution(error) = result.unwrap_err() else {
                            panic!("expected dynamic bounds trap");
                        };
                        assert_eq!(
                            error.kind,
                            SimulationExecutionErrorKindV1::ReachedUnreachable
                        );
                        assert!(error.invocation.is_some() && error.site.is_some());
                        assert!(error.observation_failure.is_none());
                        assert_eq!(observed.writes, 0);
                        assert!(observed.failed);
                    }
                }
            }
        }
    }
}

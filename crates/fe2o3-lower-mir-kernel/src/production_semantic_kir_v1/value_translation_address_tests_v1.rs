// These are formal-memory component checks. The synthetic receipt in the
// ranked fixture does not authenticate the executable KIR address or a launch.
fn value_translation_formal_memory(
    fixture: &ValueTranslationFixtureV1,
) -> fe2o3_kernel_ir::FormalMemoryObligationAnalysis {
    fe2o3_kernel_ir::derive_kernel_memory_obligations_for_launch(
        &fixture.module,
        &fixture.module.kernels[0].id,
        fe2o3_kernel_ir::ExplicitLaunchExtent::Exact {
            rank: 1,
            extents: [64, 1, 1],
        },
        fe2o3_kernel_ir::FormalIndexWidth::Bits64,
    )
    .expect("the valid executable fixture must admit formal-memory analysis")
}

#[test]
fn value_translation_formal_memory_retains_exact_linear_address() {
    let fixture = value_translation_fixture(
        ProductionSemanticBinaryOpV2::Add,
        u64::from(1.0_f32.to_bits()),
    );
    let analysis = value_translation_formal_memory(&fixture);
    assert!(
        matches!(
            analysis,
            fe2o3_kernel_ir::FormalMemoryObligationAnalysis::Complete(_)
        ),
        "the actual linear address must remain fully modeled: {analysis:?}"
    );
}

#[test]
fn value_translation_formal_memory_rejects_nonlinear_executable_address() {
    let mut fixture = value_translation_fixture(
        ProductionSemanticBinaryOpV2::Add,
        u64::from(1.0_f32.to_bits()),
    );
    let original_ranked = fixture.lowering.kernel().clone();
    let original_sources = fixture.sources;
    let original_module = fixture.module.clone();
    let operations = &mut fixture.module.functions[0].body.as_mut().unwrap().blocks[0].operations;
    assert!(matches!(
        operations[1].kind,
        OperationKind::Binary {
            op: BinaryOp::Multiply,
            lhs: ValueId(1),
            rhs: ValueId(1),
        }
    ));
    let OperationKind::GetElementPointer { base, offset } = &mut operations[3].kind else {
        panic!("fixture must retain the exact indexed pointer")
    };
    assert_eq!((*base, *offset), (ValueId(3), ValueId(1)));
    *offset = ValueId(2);
    verify_module(&fixture.module).expect("the nonlinear mutation remains structurally valid KIR");

    let analysis = value_translation_formal_memory(&fixture);
    let fe2o3_kernel_ir::FormalMemoryObligationAnalysis::Incomplete { reasons, .. } = analysis
    else {
        panic!("i*i must not acquire a linear-address proof from the source fixture")
    };
    assert!(
        matches!(reasons.as_slice(),
            [FormalMemoryIncompleteReason::UnsupportedIndexExpression {
                location: FunctionOperationLocation { block: BlockId(0), operation_index: 3 },
                index: ValueId(2), allocation,
            }] if allocation.parameter_index() == 0
        ),
        "exact nonlinear pointer failure required: {reasons:?}"
    );
    assert_eq!(fixture.lowering.kernel(), &original_ranked);
    assert_eq!(fixture.sources, original_sources);

    // Restore only the mutated address and require whole-module identity.
    let OperationKind::GetElementPointer { offset, .. } =
        &mut fixture.module.functions[0].body.as_mut().unwrap().blocks[0].operations[3].kind
    else {
        unreachable!()
    };
    *offset = ValueId(1);
    assert_eq!(fixture.module, original_module);
}

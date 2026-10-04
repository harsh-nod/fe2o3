#[test]
fn exact_v18_lifecycle_preflight_keeps_every_old_wire_profile_closed() {
    use fe2o3_kernel_ir::{ExecutionOperationV15 as Op, ExecutionRoleV15 as Role, ValueDef};
    let mut module = Module::new("lifecycle-profile-controls");
    let mut function = call_depth_test_function("entry", &[], true);
    function.body.as_mut().unwrap().blocks[0].operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(0), Type::Execution(Role::Context)),
            OperationKind::Execution(Op::ContextIssue),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(1), Type::Execution(Role::Workgroup)),
            OperationKind::Execution(Op::WorkgroupDerive {
                context: ValueId(0),
            }),
        ),
        Operation::new(
            vec![],
            OperationKind::Execution(Op::ScopeEnd {
                workgroup: ValueId(1),
                discarded: vec![],
            }),
        ),
    ];
    module.functions.push(function);
    module.kernels.push(fe2o3_kernel_ir::Kernel::new(
        "entry",
        "entry",
        fe2o3_kernel_ir::LaunchDomain::D1 {
            x: LaunchExtent::Static(1),
        },
    ));
    let request = SimulationRequestV1::new("entry", [1, 1, 1], [1, 1, 1], vec![]);
    for wire in [7, 9, 10, 11, 12, 16, 17, 19, 20, 21, 22] {
        let error = preflight(
            &module,
            0,
            &request,
            None,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
            wire,
        )
        .unwrap_err();
        let SimulationPreflightErrorV1::Unsupported(report) = error else {
            panic!("expected unchanged lifecycle profile refusal for {wire}: {error:?}");
        };
        assert_eq!(report.total_findings(), 5);
        assert!(
            report
                .findings()
                .iter()
                .all(|finding| finding.feature == UnsupportedFeatureV1::InertExecutionV15)
        );
        for operation in 0..3 {
            assert!(
                report
                    .findings()
                    .iter()
                    .any(|finding| finding.operation == Some(operation))
            );
        }
    }
    assert!(
        preflight(
            &module,
            0,
            &request,
            None,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
            fe2o3_kernel_ir::KERNEL_IR_VERSION_V18
        )
        .is_ok()
    );
}

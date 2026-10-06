fn execution_header_oracle_v178() -> usize {
    use fe2o3_kernel_ir::ExecutionRoleV15 as Role;
    assert_eq!(
        size_of::<ExecutionByteOperationV178>(),
        size_of::<(Operation, u8, Option<usize>, Option<usize>)>()
    );
    2 * size_of::<ExecutionByteOperationV178>()
        + 2 * size_of::<Result<Option<ExecutionByteOperationV178>>>()
        + size_of::<(u8, [Option<Role>; 2], Option<ValueId>)>()
        + size_of::<([usize; 6], [Option<usize>; 4], [Result<()>; 3])>()
}

#[test]
fn byte_execution_roles_are_not_scalar_pointer_or_transport_values() {
    use fe2o3_kernel_ir::ExecutionRoleV15 as Role;
    for role in [
        Role::Context,
        Role::Workgroup,
        Role::MaskedTileU32 {
            lanes: 4,
            elements: 2,
        },
        Role::LaneFragmentU32 {
            lanes: 4,
            elements: 2,
        },
    ] {
        assert!(value_type(&Type::Execution(role)).is_err());
        let emitted = run(0, LIMIT, LIMIT, |out| {
            emit_value_type(
                &Type::Execution(role),
                FormalIndexWidth::Bits64,
                "value",
                out,
            )
        });
        assert!(emitted.0.is_err());
    }
    assert_eq!(execution::role_code(Role::Context).unwrap(), 0);
    assert_eq!(execution::role_code(Role::Workgroup).unwrap(), 1);
    assert!(
        execution::role_code(Role::MaskedTileU32 {
            lanes: 4,
            elements: 2
        })
        .is_err()
    );
    assert!(
        execution::role_code(Role::LaneFragmentU32 {
            lanes: 4,
            elements: 2
        })
        .is_err()
    );
}

#[test]
fn byte_execution_registry_checks_slot_epoch_frame_and_reciprocal_parent() {
    let source = include_str!("mixed_optimizer_execution_byte_v178.vrs");
    for required in [
        "capability.identity.definition == definition && capability.active",
        "state.frames.active.contains(capability.identity.frame)",
        "previous.identity.definition == definition && previous.site == site",
        "previous.child.is_none() && (role == 0 || !previous.active)",
        "Some(previous.identity.epoch + 1)",
        "!byte_execution_mentions_v178(resident, definition,",
        "capability.identity.frame == frame",
        "parent.frame == frame",
        "child.frame == frame",
        "code == 1 && destination != receiver",
        "parent.child.is_none()",
        "parent.identity == reference",
        "parent.child == Some(child.identity)",
        "reference.definition != receiver",
        "MemoryExecutionCapabilityV178 { active: false, ..child }",
        "MemoryExecutionCapabilityV178 { child: None, ..parent }",
    ] {
        assert!(
            source.contains(required),
            "missing capability condition: {required}"
        );
    }
    assert!(!source.contains("assume("));
    assert!(!source.contains("external_body"));
}

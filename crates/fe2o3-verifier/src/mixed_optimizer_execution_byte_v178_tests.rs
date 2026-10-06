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

#[test]
fn byte_execution_cannot_enter_native_arguments_or_unrelated_forwarding() {
    use std::collections::BTreeSet;

    let vocabulary = include_str!("mixed_optimizer_byte_memory_v30.rs");
    let variants = vocabulary
        .split_once("enum MemoryValueV30 {")
        .unwrap()
        .1
        .split_once("\n}\n")
        .unwrap()
        .0;
    let declaration: syn::ItemEnum =
        syn::parse_str(&format!("enum MemoryValueV30 {{{variants}\n}}")).unwrap();
    let expected: BTreeSet<_> = declaration
        .variants
        .iter()
        .map(|variant| variant.ident.to_string())
        .collect();

    fn collect(pattern: &syn::Pat, names: &mut BTreeSet<String>) {
        let path = match pattern {
            syn::Pat::Or(pattern) => {
                for case in &pattern.cases {
                    collect(case, names);
                }
                return;
            }
            syn::Pat::Path(pattern) => &pattern.path,
            syn::Pat::TupleStruct(pattern) => &pattern.path,
            _ => panic!("expected a closed memory value variant, not a wildcard"),
        };
        assert_eq!(path.segments.len(), 2);
        assert_eq!(path.segments[0].ident, "MemoryValueV30");
        assert!(names.insert(path.segments[1].ident.to_string()));
    }

    for (source, name) in [
        (
            include_str!("mixed_optimizer_store_consensus_typed_v46.vrs"),
            "forwarding_value_separate_v46",
        ),
        (
            include_str!("original_semantic_mir_native_provenance_v39.vrs"),
            "invocation_native_value_provenance_v39",
        ),
    ] {
        let body = source
            .split_once(&format!("spec fn {name}"))
            .unwrap()
            .1
            .split("\nproof fn ")
            .next()
            .unwrap();
        let function: syn::ItemFn = syn::parse_str(&format!("fn {name}{body}")).unwrap();
        let [syn::Stmt::Expr(syn::Expr::Match(expression), None)] = function.block.stmts.as_slice()
        else {
            panic!("expected one closed value classifier");
        };
        let mut actual = BTreeSet::new();
        for arm in &expression.arms {
            assert!(arm.guard.is_none());
            collect(&arm.pat, &mut actual);
            if let syn::Pat::TupleStruct(pattern) = &arm.pat {
                if pattern.path.segments.last().unwrap().ident == "Execution" {
                    let syn::Expr::Lit(value) = arm.body.as_ref() else {
                        panic!("execution capability must be explicitly refused");
                    };
                    assert!(matches!(&value.lit, syn::Lit::Bool(value) if !value.value));
                }
            }
        }
        assert_eq!(actual, expected, "{name}");
    }
    let source = include_str!("original_semantic_mir_invocation_source_function_v36.rs");
    let argument = source
        .split_once("spec fn invocation_source_external_argument_v36")
        .unwrap()
        .1
        .split_once("struct InvocationSourceStatementObservationV36")
        .unwrap()
        .0;
    assert!(argument.contains("MemoryValueV30::Execution(_) => false"));
    let frames = include_str!("original_semantic_mir_invocation_source_frames_v36.rs");
    let escape = frames
        .split_once("spec fn invocation_source_value_escapes_frame_v36(")
        .unwrap()
        .1
        .split_once("proof fn invocation_source_pointer_value_escapes_frame_v77")
        .unwrap()
        .0;
    assert!(escape.contains("MemoryValueV30::Execution(_) => true"));
}

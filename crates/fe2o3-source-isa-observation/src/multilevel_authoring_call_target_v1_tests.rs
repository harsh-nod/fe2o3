//! Synthetic verified-codec fixtures, not authenticated source or execution evidence.
use super::*;
use fe2o3_kernel_ir::{Constant, FunctionRole};

fn call_module() -> Module {
    let mut module = fixture_module(false, ScalarType::U32, "v_or_b32", None, false);
    module.functions[1].id = "opaque-left".into();
    let mut right = module.functions[1].clone();
    right.id = "opaque-right".into();
    module.functions.push(right);
    module.functions[0].body.as_mut().unwrap().blocks[0].operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(700), Type::Scalar(ScalarType::U32)),
            OperationKind::Constant(Constant::U32(256)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(19), Type::Scalar(ScalarType::U32)),
            OperationKind::Constant(Constant::U32(512)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(900), Type::Scalar(ScalarType::U32)),
            OperationKind::Call {
                callee: "opaque-right".into(),
                arguments: vec![ValueId(19), ValueId(700)],
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(901), Type::Scalar(ScalarType::U32)),
            OperationKind::Call {
                callee: "opaque-left".into(),
                arguments: vec![ValueId(700), ValueId(19)],
            },
        ),
    ];
    module
}

fn call_snapshot(module: Module) -> AuthoringSnapshotV1 {
    AuthoringSnapshotV1::from_bundle_v6(fixture_bundle(module, "gfx942:xnack-", false)).unwrap()
}

fn call_selector(snapshot: &AuthoringSnapshotV1, operation: u32) -> AuthoringRegionSelectorV1 {
    let mut selected = selector(snapshot, 1);
    selected.operations[0] = AuthoringOperationCoordinateV1 {
        function: 0,
        block: 0,
        operation,
    };
    selected
}

#[test]
fn call_target_v1_uses_retained_id_and_positional_typed_arguments_only() {
    let snapshot = call_snapshot(call_module());
    let selected = call_selector(&snapshot, 2);
    let before = snapshot.bundle.canonical_kir_v11().to_vec();
    let summary = snapshot.summary();
    let page = snapshot
        .operation_page(&summary.bundle_identity, 0, 64)
        .unwrap();
    let report = snapshot.inspect_call_target_v1(&selected).unwrap();
    assert_eq!(report.schema, "fe2o3-authoring-call-target-v1");
    assert_eq!(report.authority, AUTHORITY);
    assert_eq!(report.selector, selected);
    assert_eq!(report.callee.function, 2);
    assert_eq!(report.callee.function_id, "opaque-right");
    assert_eq!(report.callee.role, "internal_helper");
    assert_eq!(report.caller.function, 0);
    assert_eq!(report.caller.function_id, "entry");
    assert_eq!(report.caller.role, "kernel_entry");
    assert_eq!(report.caller.kernel_registrations.len(), 1);
    assert_eq!(report.caller.kernel_registrations[0].kernel, 0);
    assert_eq!(report.caller.kernel_registrations[0].kernel_id, "entry");
    assert_eq!(
        report.caller.kernel_registrations[0].entry_function_id,
        "entry"
    );
    assert_eq!(report.call, page.operations[2]);
    assert_eq!(
        report
            .arguments
            .iter()
            .map(|row| (row.position, row.operand.value, row.formal.value))
            .collect::<Vec<_>>(),
        vec![(0, 19, 40), (1, 700, 7)]
    );
    for row in &report.arguments {
        assert_eq!(row.operand.ty, "Scalar(U32)");
        assert_eq!(row.formal.ty, "Scalar(U32)");
    }
    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].position, 0);
    assert_eq!(report.results[0].result.value, 900);
    assert_eq!(report.results[0].result.ty, "Scalar(U32)");
    assert_eq!(report.results[0].signature_type, "Scalar(U32)");
    assert_eq!(
        report.correspondence,
        "exact_retained_call_operand_to_formal_position"
    );
    assert_eq!(report.transitive_helper_closure, "not_traversed");
    assert_eq!(
        report.dynamic_invocation,
        "unavailable_static_call_site_only"
    );
    assert_eq!(report.physical_abi, "unavailable_logical_canonical_call");
    assert_eq!(report.call.source_binding, "unavailable_no_source_span");
    assert!(report.call.source_spans.is_empty());
    assert!(serde_json::to_vec(&report).unwrap().len() <= MAX_AUTHORING_REPORT_BYTES_V1);
    assert_eq!(report, snapshot.inspect_call_target_v1(&selected).unwrap());
    assert_eq!(before, snapshot.bundle.canonical_kir_v11());
    assert_eq!(summary, snapshot.summary());
    assert_eq!(
        page,
        snapshot
            .operation_page(&summary.bundle_identity, 0, 64)
            .unwrap()
    );
    let old_json = serde_json::to_value(&page.operations[2]).unwrap();
    assert_eq!(old_json["kind"], "call");
    assert!(old_json["semantic_detail"].is_null());
    assert!(old_json.get("callee").is_none());
    assert!(old_json.get("call_target").is_none());
}

#[test]
fn call_target_v1_distinguishes_targets_and_repeated_static_occurrences() {
    let snapshot = call_snapshot(call_module());
    let first = snapshot
        .inspect_call_target_v1(&call_selector(&snapshot, 2))
        .unwrap();
    let second = snapshot
        .inspect_call_target_v1(&call_selector(&snapshot, 3))
        .unwrap();
    assert_eq!(first.callee.function, 2);
    assert_eq!(second.callee.function, 1);
    assert_ne!(first.callee.function_id, second.callee.function_id);
    assert_eq!(second.arguments[0].operand.value, 700);
    assert_eq!(second.arguments[0].formal.value, 40);

    let mut module = call_module();
    module.functions[0].body.as_mut().unwrap().blocks[0].operations[3].kind = OperationKind::Call {
        callee: "opaque-right".into(),
        arguments: vec![ValueId(19), ValueId(19)],
    };
    let snapshot = call_snapshot(module);
    let first = snapshot
        .inspect_call_target_v1(&call_selector(&snapshot, 2))
        .unwrap();
    let second = snapshot
        .inspect_call_target_v1(&call_selector(&snapshot, 3))
        .unwrap();
    assert_eq!(first.callee, second.callee);
    assert_ne!(first.call.coordinate, second.call.coordinate);
    assert_ne!(
        first.results[0].result.value,
        second.results[0].result.value
    );
    assert_eq!(
        second.arguments[0].operand.value,
        second.arguments[1].operand.value
    );
    assert_ne!(
        second.arguments[0].formal.value,
        second.arguments[1].formal.value
    );
    assert_eq!(
        second.dynamic_invocation,
        "unavailable_static_call_site_only"
    );
}

#[test]
fn call_target_v1_preserves_exact_available_call_source_spans() {
    let mut module = call_module();
    module.functions[1].body.as_mut().unwrap().blocks[0].operations[0] = Operation::effect_free(
        ValueDef::new(ValueId(90), Type::Scalar(ScalarType::U32)),
        OperationKind::Call {
            callee: "opaque-right".into(),
            arguments: vec![ValueId(40), ValueId(7)],
        },
    );
    let snapshot = call_snapshot(module);
    let selected = selector(&snapshot, 1);
    let old = snapshot.select_region(&selected).unwrap();
    let report = snapshot.inspect_call_target_v1(&selected).unwrap();
    assert_eq!(report.call, old.operations[0]);
    assert_eq!(report.call.source_spans.len(), 2);
    assert_eq!(
        report.call.source_binding,
        "bundle_content_bound_not_authenticated"
    );
    assert!(!report.authority.source_authenticated);
    assert_eq!(report.callee.function, 2);
}

#[test]
fn call_target_v1_rejects_exact_identity_coordinate_and_selection_mismatches() {
    let snapshot = call_snapshot(call_module());
    let selected = call_selector(&snapshot, 2);
    let cases = [
        (0, AuthoringErrorV1::StaleBundleIdentity),
        (1, AuthoringErrorV1::StaleCanonicalIdentity),
        (2, AuthoringErrorV1::IncompatibleTarget),
        (3, AuthoringErrorV1::InvalidCoordinate),
        (4, AuthoringErrorV1::EmptySelection),
    ];
    for (change, expected) in cases {
        let mut bad = selected.clone();
        match change {
            0 => bad.bundle_identity = "00".repeat(32),
            1 => bad.canonical_kir_digest = "00".repeat(32),
            2 => bad.target = "gfx950:xnack-".into(),
            3 => bad.operations[0].function = u32::MAX,
            4 => bad.operations.clear(),
            _ => unreachable!(),
        }
        assert_eq!(
            snapshot.inspect_call_target_v1(&bad),
            Err(AuthoringCallTargetErrorV1::Snapshot(expected))
        );
    }
    let mut multiple = selected.clone();
    multiple
        .operations
        .push(call_selector(&snapshot, 3).operations[0]);
    assert_eq!(
        snapshot.inspect_call_target_v1(&multiple),
        Err(AuthoringCallTargetErrorV1::ExpectedSingleOperation)
    );
    assert_eq!(
        snapshot.inspect_call_target_v1(&call_selector(&snapshot, 0)),
        Err(AuthoringCallTargetErrorV1::NotCall)
    );
    let mut too_many = selected;
    too_many.operations = vec![too_many.operations[0]; MAX_AUTHORING_REGION_OPERATIONS_V1 + 1];
    assert_eq!(
        snapshot.inspect_call_target_v1(&too_many),
        Err(AuthoringCallTargetErrorV1::Snapshot(
            AuthoringErrorV1::ResourceLimit
        ))
    );
}

#[test]
fn call_target_v1_rejects_unknown_selector_fields_and_bad_coordinate_shapes() {
    let snapshot = call_snapshot(call_module());
    let selected = call_selector(&snapshot, 2);
    let mut value = serde_json::to_value(&selected).unwrap();
    value["callee"] = "opaque-left".into();
    assert!(serde_json::from_value::<AuthoringRegionSelectorV1>(value).is_err());
    let mut value = serde_json::to_value(&selected).unwrap();
    value["operations"][0]["formal"] = 40.into();
    assert!(serde_json::from_value::<AuthoringRegionSelectorV1>(value).is_err());
    let mut value = serde_json::to_value(selected).unwrap();
    value["operations"][0]["operation"] = (-1).into();
    assert!(serde_json::from_value::<AuthoringRegionSelectorV1>(value).is_err());
}

// The following corruption controls exercise defensive query guards directly on
// a test-private decoded owner. They do not make malformed bytes pass Bundle V6
// verification and are not public mutation or source-admission demonstrations.
#[test]
fn call_target_v1_defensively_refuses_missing_ambiguous_and_unsupported_targets() {
    let mut missing = call_snapshot(call_module());
    let selected = call_selector(&missing, 2);
    missing.module.functions.pop();
    assert_eq!(
        missing.inspect_call_target_v1(&selected),
        Err(AuthoringCallTargetErrorV1::MissingCallee)
    );

    let mut ambiguous = call_snapshot(call_module());
    let duplicate = ambiguous.module.functions[2].clone();
    ambiguous.module.functions.push(duplicate);
    let selected = call_selector(&ambiguous, 2);
    assert_eq!(
        ambiguous.inspect_call_target_v1(&selected),
        Err(AuthoringCallTargetErrorV1::AmbiguousCallee)
    );

    for role in [
        FunctionRole::ExternalImport,
        FunctionRole::DeviceFfiExport,
        FunctionRole::KernelEntry,
    ] {
        let mut snapshot = call_snapshot(call_module());
        let selected = call_selector(&snapshot, 2);
        snapshot.module.functions[2].role = role;
        assert_eq!(
            snapshot.inspect_call_target_v1(&selected),
            Err(AuthoringCallTargetErrorV1::UnsupportedCallee)
        );
    }
    let mut snapshot = call_snapshot(call_module());
    let selected = call_selector(&snapshot, 2);
    snapshot.module.functions[2].body = None;
    assert_eq!(
        snapshot.inspect_call_target_v1(&selected),
        Err(AuthoringCallTargetErrorV1::UnsupportedCallee)
    );
}

fn assert_binding_refused(mutate: impl FnOnce(&mut AuthoringSnapshotV1)) {
    let mut snapshot = call_snapshot(call_module());
    let selected = call_selector(&snapshot, 2);
    mutate(&mut snapshot);
    assert_eq!(
        snapshot.inspect_call_target_v1(&selected),
        Err(AuthoringCallTargetErrorV1::InvalidCallBinding)
    );
}

#[test]
fn call_target_v1_defensively_checks_actual_typed_arity_and_formal_definitions() {
    assert_binding_refused(|snapshot| {
        let OperationKind::Call { arguments, .. } =
            &mut snapshot.module.functions[0].body.as_mut().unwrap().blocks[0].operations[2].kind
        else {
            unreachable!()
        };
        arguments.pop();
    });
    assert_binding_refused(|snapshot| {
        snapshot.module.functions[2]
            .body
            .as_mut()
            .unwrap()
            .parameters
            .pop();
    });
    assert_binding_refused(|snapshot| {
        snapshot.module.functions[0].body.as_mut().unwrap().blocks[0].operations[2]
            .results
            .clear();
    });
    assert_binding_refused(|snapshot| {
        snapshot.module.functions[0].body.as_mut().unwrap().blocks[0].operations[1].results[0].ty =
            Type::Scalar(ScalarType::I32);
    });
    assert_binding_refused(|snapshot| {
        snapshot.module.functions[0].body.as_mut().unwrap().blocks[0].operations[2].results[0].ty =
            Type::Scalar(ScalarType::I32);
    });
    assert_binding_refused(|snapshot| {
        snapshot.module.functions[2].signature.results[0] = Type::Scalar(ScalarType::U64);
    });
    assert_binding_refused(|snapshot| {
        snapshot.module.functions[2]
            .body
            .as_mut()
            .unwrap()
            .parameters[0] = ValueId(999_999);
    });
    assert_binding_refused(|snapshot| {
        // An ordinary same-typed result is not a function formal parameter.
        snapshot.module.functions[2]
            .body
            .as_mut()
            .unwrap()
            .parameters[0] = ValueId(90);
    });
    assert_binding_refused(|snapshot| {
        // Repeating one valid formal cannot bind both parameter positions.
        snapshot.module.functions[2]
            .body
            .as_mut()
            .unwrap()
            .parameters[1] = ValueId(40);
    });
}

fn arity_module(count: usize) -> Module {
    let mut module = call_module();
    for function in &mut module.functions[1..] {
        function.signature.parameters = vec![Type::Scalar(ScalarType::U32); count];
        function.signature.results = vec![Type::Scalar(ScalarType::U32); count];
        let body = function.body.as_mut().unwrap();
        body.parameters = (0..count)
            .map(|index| match index {
                0 => ValueId(40),
                1 => ValueId(7),
                _ => ValueId(1000 + index as u32),
            })
            .collect();
        if count == 0 {
            for (index, operation) in body.blocks[0].operations.iter_mut().enumerate() {
                operation.kind = OperationKind::Constant(Constant::U32(index as u32));
            }
        }
        body.blocks[0].terminator = Some(Terminator::Return {
            values: vec![ValueId(3); count],
        });
    }
    for (index, operation) in module.functions[0].body.as_mut().unwrap().blocks[0].operations[2..]
        .iter_mut()
        .enumerate()
    {
        let OperationKind::Call { arguments, .. } = &mut operation.kind else {
            unreachable!()
        };
        *arguments = (0..count)
            .map(|position| ValueId(if position % 2 == 0 { 19 } else { 700 }))
            .collect();
        operation.results = (0..count)
            .map(|position| {
                ValueDef::new(
                    ValueId(2000 + index as u32 * 100 + position as u32),
                    Type::Scalar(ScalarType::U32),
                )
            })
            .collect();
    }
    module
}

#[test]
fn call_target_v1_accepts_zero_and_exact_value_limit_and_refuses_one_more() {
    for count in [0, MAX_AUTHORING_CALL_VALUES_V1] {
        let snapshot = call_snapshot(arity_module(count));
        let report = snapshot
            .inspect_call_target_v1(&call_selector(&snapshot, 2))
            .unwrap();
        assert_eq!(report.arguments.len(), count);
        assert_eq!(report.results.len(), count);
        assert_eq!(report.call.inputs.len(), count);
        assert_eq!(report.call.results.len(), count);
    }
    let snapshot = call_snapshot(arity_module(MAX_AUTHORING_CALL_VALUES_V1 + 1));
    assert_eq!(
        snapshot.inspect_call_target_v1(&call_selector(&snapshot, 2)),
        Err(AuthoringCallTargetErrorV1::Snapshot(
            AuthoringErrorV1::ResourceLimit
        ))
    );
}

#[test]
fn call_target_v1_defensive_identity_and_comparison_work_bounds_are_finite() {
    for length in [MAX_TEXT_BYTES, MAX_TEXT_BYTES + 1] {
        let mut snapshot = call_snapshot(call_module());
        let selected = call_selector(&snapshot, 2);
        let identity = "z".repeat(length);
        snapshot.module.functions[2].id = identity.clone().into();
        let OperationKind::Call { callee, .. } =
            &mut snapshot.module.functions[0].body.as_mut().unwrap().blocks[0].operations[2].kind
        else {
            unreachable!()
        };
        *callee = identity.into();
        if length == MAX_TEXT_BYTES + 1 {
            assert_eq!(
                snapshot.inspect_call_target_v1(&selected),
                Err(AuthoringCallTargetErrorV1::Snapshot(
                    AuthoringErrorV1::ResourceLimit
                ))
            );
        } else {
            assert!(snapshot.inspect_call_target_v1(&selected).is_ok());
            for index in 0..256 {
                let mut unrelated = snapshot.module.functions[1].clone();
                unrelated.id = format!("{index:04}{}", "a".repeat(MAX_TEXT_BYTES - 4)).into();
                snapshot.module.functions.push(unrelated);
            }
            assert_eq!(
                snapshot.inspect_call_target_v1(&selected),
                Err(AuthoringCallTargetErrorV1::Snapshot(
                    AuthoringErrorV1::ResourceLimit
                ))
            );
        }
    }
}

#[test]
fn call_target_v1_refusals_preserve_the_original_observations() {
    let snapshot = call_snapshot(call_module());
    let before = snapshot.summary();
    let bytes = snapshot.bundle.canonical_kir_v11().to_vec();
    let page = snapshot
        .operation_page(&before.bundle_identity, 0, 64)
        .unwrap();
    let mut stale = call_selector(&snapshot, 2);
    stale.canonical_kir_digest = "ff".repeat(32);
    assert!(snapshot.inspect_call_target_v1(&stale).is_err());
    assert!(
        snapshot
            .inspect_call_target_v1(&call_selector(&snapshot, 0))
            .is_err()
    );
    assert_eq!(before, snapshot.summary());
    assert_eq!(bytes, snapshot.bundle.canonical_kir_v11());
    assert_eq!(
        page,
        snapshot
            .operation_page(&before.bundle_identity, 0, 64)
            .unwrap()
    );
}

#[test]
fn call_target_v1_caller_registration_is_retained_not_a_name_convention() {
    let mut module = call_module();
    module.kernels[0].id = "independent-launch-id".into();
    let snapshot = call_snapshot(module);
    let report = snapshot
        .inspect_call_target_v1(&call_selector(&snapshot, 2))
        .unwrap();
    assert_eq!(report.caller.function_id, "entry");
    assert_eq!(report.caller.role, "kernel_entry");
    assert_eq!(
        report.caller.kernel_registrations,
        vec![AuthoringCallKernelRegistrationV1 {
            kernel: 0,
            kernel_id: "independent-launch-id".into(),
            entry_function_id: "entry".into(),
        }]
    );
    assert!(!report.authority.source_authenticated);
    assert!(!report.authority.grants_load_or_launch);
}

#[test]
fn call_target_v1_preserves_multiple_registrations_and_excludes_other_entries() {
    let mut module = call_module();
    let mut alias = module.kernels[0].clone();
    alias.id = "second-launch-id".into();
    module.kernels.push(alias);
    let mut other = module.functions[0].clone();
    other.id = "other-entry-function".into();
    module.functions.push(other);
    let mut other_kernel = module.kernels[0].clone();
    other_kernel.id = "other-launch-id".into();
    other_kernel.entry = "other-entry-function".into();
    module.kernels.push(other_kernel);
    let snapshot = call_snapshot(module);
    let selected = call_selector(&snapshot, 2);
    let report = snapshot.inspect_call_target_v1(&selected).unwrap();
    assert_eq!(
        report
            .caller
            .kernel_registrations
            .iter()
            .map(|row| (
                row.kernel,
                row.kernel_id.as_str(),
                row.entry_function_id.as_str()
            ))
            .collect::<Vec<_>>(),
        vec![(0, "entry", "entry"), (1, "second-launch-id", "entry")]
    );
    let mut other_selected = selected;
    other_selected.operations[0].function = 3;
    let other = snapshot.inspect_call_target_v1(&other_selected).unwrap();
    assert_eq!(other.caller.function, 3);
    assert_eq!(other.caller.function_id, "other-entry-function");
    assert_eq!(other.caller.kernel_registrations.len(), 1);
    assert_eq!(other.caller.kernel_registrations[0].kernel, 2);
    assert_eq!(
        other.caller.kernel_registrations[0].kernel_id,
        "other-launch-id"
    );
    assert_eq!(other.callee, report.callee);
}

#[test]
fn call_target_v1_helper_callers_are_not_registered_kernel_entries() {
    let mut module = call_module();
    module.functions[1].role = FunctionRole::InternalHelper;
    module.functions[1].body.as_mut().unwrap().blocks[0].operations[0] = Operation::effect_free(
        ValueDef::new(ValueId(90), Type::Scalar(ScalarType::U32)),
        OperationKind::Call {
            callee: "opaque-right".into(),
            arguments: vec![ValueId(40), ValueId(7)],
        },
    );
    let snapshot = call_snapshot(module);
    let report = snapshot
        .inspect_call_target_v1(&selector(&snapshot, 1))
        .unwrap();
    assert_eq!(report.caller.function, 1);
    assert_eq!(report.caller.function_id, "opaque-left");
    assert_eq!(report.caller.role, "internal_helper");
    assert!(report.caller.kernel_registrations.is_empty());
    assert_eq!(report.callee.function, 2);
}

#[test]
fn call_target_v1_defensively_refuses_missing_duplicate_or_wrong_role_registration() {
    let mut missing = call_snapshot(call_module());
    let selected = call_selector(&missing, 2);
    missing.module.kernels.clear();
    assert_eq!(
        missing.inspect_call_target_v1(&selected),
        Err(AuthoringCallTargetErrorV1::MissingKernelRegistration)
    );

    let mut duplicate = call_snapshot(call_module());
    let selected = call_selector(&duplicate, 2);
    let duplicate_kernel = duplicate.module.kernels[0].clone();
    duplicate.module.kernels.push(duplicate_kernel);
    assert_eq!(
        duplicate.inspect_call_target_v1(&selected),
        Err(AuthoringCallTargetErrorV1::InvalidCallerRegistration)
    );

    let mut wrong = call_snapshot(call_module());
    let selected = call_selector(&wrong, 2);
    wrong.module.functions[0].role = FunctionRole::InternalHelper;
    assert_eq!(
        wrong.inspect_call_target_v1(&selected),
        Err(AuthoringCallTargetErrorV1::InvalidCallerRegistration)
    );
    let mut unsupported = call_snapshot(call_module());
    let selected = call_selector(&unsupported, 2);
    // Test-private corruption only: Bundle V6's canonical V11 encoder does not
    // admit device-FFI export roles, and an external caller cannot have a body.
    for role in [FunctionRole::DeviceFfiExport, FunctionRole::ExternalImport] {
        unsupported.module.functions[0].role = role;
        assert_eq!(
            unsupported.inspect_call_target_v1(&selected),
            Err(AuthoringCallTargetErrorV1::UnsupportedCaller)
        );
    }

    let mut empty = call_snapshot(call_module());
    let selected = call_selector(&empty, 2);
    empty.module.kernels[0].id = "".into();
    assert_eq!(
        empty.inspect_call_target_v1(&selected),
        Err(AuthoringCallTargetErrorV1::InvalidCallerRegistration)
    );
}

#[test]
fn call_target_v1_exact_kernel_registration_count_limit_is_enforced() {
    for count in [
        MAX_AUTHORING_CALL_KERNELS_V1,
        MAX_AUTHORING_CALL_KERNELS_V1 + 1,
    ] {
        let mut module = call_module();
        let prototype = module.kernels[0].clone();
        module.kernels = (0..count)
            .map(|index| {
                let mut kernel = prototype.clone();
                kernel.id = format!("launch-{index}").into();
                kernel
            })
            .collect();
        let snapshot = call_snapshot(module);
        let report = snapshot.inspect_call_target_v1(&call_selector(&snapshot, 2));
        if count == MAX_AUTHORING_CALL_KERNELS_V1 {
            let report = report.unwrap();
            assert_eq!(report.caller.kernel_registrations.len(), count);
            for (index, row) in report.caller.kernel_registrations.iter().enumerate() {
                assert_eq!(row.kernel, index as u32);
                assert_eq!(row.kernel_id, format!("launch-{index}"));
                assert_eq!(row.entry_function_id, report.caller.function_id);
            }
        } else {
            assert_eq!(
                report,
                Err(AuthoringCallTargetErrorV1::Snapshot(
                    AuthoringErrorV1::ResourceLimit
                ))
            );
        }
    }
}

#[test]
fn call_target_v1_defensive_caller_registration_text_and_shared_work_are_bounded() {
    for (caller, length) in [
        (false, MAX_TEXT_BYTES),
        (false, MAX_TEXT_BYTES + 1),
        (true, MAX_TEXT_BYTES),
        (true, MAX_TEXT_BYTES + 1),
    ] {
        let mut snapshot = call_snapshot(call_module());
        let selected = call_selector(&snapshot, 2);
        let identity = "c".repeat(length);
        if caller {
            snapshot.module.functions[0].id = identity.clone().into();
            snapshot.module.kernels[0].entry = identity.into();
        } else {
            snapshot.module.kernels[0].id = identity.into();
        }
        let report = snapshot.inspect_call_target_v1(&selected);
        if length == MAX_TEXT_BYTES {
            assert!(report.is_ok());
        } else {
            assert_eq!(
                report,
                Err(AuthoringCallTargetErrorV1::Snapshot(
                    AuthoringErrorV1::ResourceLimit
                ))
            );
        }
    }

    let mut snapshot = call_snapshot(call_module());
    let selected = call_selector(&snapshot, 2);
    let caller_id = "c".repeat(MAX_TEXT_BYTES);
    snapshot.module.functions[0].id = caller_id.clone().into();
    snapshot.module.kernels[0].entry = caller_id.into();
    // Same-length unrelated entries charge the comparison budget even though
    // they are not returned. This is test-private corruption, not valid intake.
    for index in 0..256 {
        let mut unrelated = snapshot.module.kernels[0].clone();
        unrelated.id = format!("unrelated-{index}").into();
        unrelated.entry = "z".repeat(MAX_TEXT_BYTES).into();
        snapshot.module.kernels.push(unrelated);
    }
    assert_eq!(
        snapshot.inspect_call_target_v1(&selected),
        Err(AuthoringCallTargetErrorV1::Snapshot(
            AuthoringErrorV1::ResourceLimit
        ))
    );
}

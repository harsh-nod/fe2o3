use dialect_dispatch::{
    DependencyIntentOp, DependencyKindAttr, DispatchIdAttr, DispatchIntentOpInterface,
    DispatchModeAttr, GraphCapacityAttr, GraphIntentOp, GraphRefType, RegistrationError,
    RegistrationOutcome, SelectionIntentOp, SelectionPolicyAttr, WorkspaceClassAttr,
    WorkspaceIntentOp, WorkspaceLifetimeAttr, register_dialect, selection_intent_op_attr_names,
};
use pliron::{
    attribute::{AttrObj, verify_attr},
    builtin::{
        attributes::{BytesAttr, UnitAttr},
        types::UnitType,
    },
    combine::{Parser, eof},
    context::Context,
    identifier::Identifier,
    op::{Op, op_cast, verify_op},
    operation::{Operation, OperationParserConfig},
    parsable::{Parsable, parse_from_str},
    printable::Printable,
    r#type::TypeHandle,
};

fn id(seed: u64) -> DispatchIdAttr {
    DispatchIdAttr::new([seed, seed + 1, seed + 2, seed + 3])
}

#[test]
fn registration_is_real_duplicate_safe_and_round_trips_entities() {
    let mut context = Context::new();
    assert_eq!(
        register_dialect(&mut context),
        Ok(RegistrationOutcome::Registered)
    );
    assert_eq!(
        register_dialect(&mut context),
        Ok(RegistrationOutcome::AlreadyRegistered)
    );

    let attribute: AttrObj = Box::new(id(10));
    let attribute_text = attribute.disp(&context).to_string();
    let parsed_attribute = parse_from_str(
        AttrObj::parser(()).skip(eof()),
        &mut context,
        &attribute_text,
    )
    .expect("registered dispatch attribute must parse");
    assert_eq!(
        parsed_attribute.downcast_ref::<DispatchIdAttr>(),
        Some(&id(10))
    );

    let ty: TypeHandle = GraphRefType::get(&context).into();
    let type_text = ty.disp(&context).to_string();
    let parsed_type = parse_from_str(TypeHandle::parser(()).skip(eof()), &mut context, &type_text)
        .expect("registered dispatch type must parse");
    assert!(parsed_type.deref(&context).is::<GraphRefType>());

    let graph = GraphIntentOp::new(
        &mut context,
        id(20),
        GraphCapacityAttr::Nodes64,
        DispatchModeAttr::UnfusedFinite,
    );
    let operation_text = graph.disp(&context).to_string();
    let parsed_operation = parse_from_str(
        Operation::parser(OperationParserConfig {
            look_for_outlined_attrs: false,
        })
        .skip(eof()),
        &mut context,
        &operation_text,
    )
    .expect("registered dispatch operation must parse");
    let parsed = Operation::get_op_dyn(parsed_operation, &context);
    assert!(parsed.is::<GraphIntentOp>());
    verify_op(&*parsed, &context).expect("parsed graph intent must verify");
}

#[test]
fn hostile_registration_marker_is_rejected() {
    let mut context = Context::new();
    let key = Identifier::try_from("fe2o3_dialect_dispatch_explicit_registration")
        .expect("valid marker key");
    let hostile = context.aux_data.insert(Box::new("foreign"));
    context.aux_data_map.insert(key.clone(), hostile);
    assert_eq!(
        register_dialect(&mut context),
        Err(RegistrationError::MarkerCollision)
    );
    context.aux_data.remove(hostile);
    assert_eq!(
        register_dialect(&mut context),
        Err(RegistrationError::CorruptMarker)
    );
}

#[test]
fn fixed_width_ids_have_bounded_parsing_and_reject_zero() {
    let mut context = Context::new();
    register_dialect(&mut context).expect("dispatch registration");

    let valid = "fedcba9876543210".repeat(4);
    let parsed = parse_from_str(DispatchIdAttr::parser(()).skip(eof()), &mut context, &valid)
        .expect("exactly 256 bits of hexadecimal identity");
    assert!(!parsed.is_zero());

    for hostile in ["b".repeat(63), "b".repeat(65), "b".repeat(4_096)] {
        assert!(
            parse_from_str(
                DispatchIdAttr::parser(()).skip(eof()),
                &mut context,
                &hostile
            )
            .is_err(),
            "hostile dispatch identity length {} parsed",
            hostile.len()
        );
    }
    assert!(verify_attr(&DispatchIdAttr::new([0; 4]), &context).is_err());
}

#[test]
fn valid_intents_are_inert_and_non_authoritative() {
    let mut context = Context::new();
    register_dialect(&mut context).expect("dispatch registration");

    let graph_id = id(100);
    let graph = GraphIntentOp::new(
        &mut context,
        graph_id.clone(),
        GraphCapacityAttr::Nodes256,
        DispatchModeAttr::FiniteFusion,
    );
    let dependency = DependencyIntentOp::new(
        &mut context,
        graph_id.clone(),
        id(110),
        id(120),
        DependencyKindAttr::Completion,
    );
    let workspace = WorkspaceIntentOp::new(
        &mut context,
        graph_id.clone(),
        id(130),
        graph_id.clone(),
        WorkspaceClassAttr::Bytes1048576,
        WorkspaceLifetimeAttr::Graph,
    );
    let selection = SelectionIntentOp::new(
        &mut context,
        graph_id,
        id(140),
        id(150),
        SelectionPolicyAttr::SafeFallback,
    );

    for op in [&graph as &dyn Op, &dependency, &workspace, &selection] {
        verify_op(op, &context).expect("valid bounded dispatch intent");
        let interface = op_cast::<dyn DispatchIntentOpInterface>(op)
            .expect("dispatch intent interface must be registered");
        assert!(!interface.is_executable());
        assert!(!interface.grants_runtime_authority());
    }
}

#[test]
fn verifier_rejects_reflexive_dependencies_and_bad_workspace_owners() {
    let mut context = Context::new();
    register_dialect(&mut context).expect("dispatch registration");

    let node = id(200);
    let reflexive = DependencyIntentOp::new(
        &mut context,
        id(210),
        node.clone(),
        node,
        DependencyKindAttr::Data,
    );
    assert!(verify_op(&reflexive, &context).is_err());

    let graph_id = id(220);
    let wrong_graph_owner = WorkspaceIntentOp::new(
        &mut context,
        graph_id,
        id(230),
        id(240),
        WorkspaceClassAttr::Bytes65536,
        WorkspaceLifetimeAttr::Graph,
    );
    assert!(verify_op(&wrong_graph_owner, &context).is_err());

    let graph_id = id(250);
    let wrong_node_owner = WorkspaceIntentOp::new(
        &mut context,
        graph_id.clone(),
        id(260),
        graph_id,
        WorkspaceClassAttr::Bytes4096,
        WorkspaceLifetimeAttr::Node,
    );
    assert!(verify_op(&wrong_node_owner, &context).is_err());
}

#[test]
fn verifier_rejects_ambiguous_selection_intents() {
    let mut context = Context::new();
    register_dialect(&mut context).expect("dispatch registration");

    let exact_distinct = SelectionIntentOp::new(
        &mut context,
        id(300),
        id(310),
        id(320),
        SelectionPolicyAttr::Exact,
    );
    assert!(verify_op(&exact_distinct, &context).is_err());

    let same = id(330);
    let fallback_same = SelectionIntentOp::new(
        &mut context,
        id(340),
        same.clone(),
        same,
        SelectionPolicyAttr::SafeFallback,
    );
    assert!(verify_op(&fallback_same, &context).is_err());

    let zero_graph = GraphIntentOp::new(
        &mut context,
        DispatchIdAttr::new([0; 4]),
        GraphCapacityAttr::Nodes16,
        DispatchModeAttr::PersistentService,
    );
    assert!(verify_op(&zero_graph, &context).is_err());
}

#[test]
fn verifier_rejects_wrong_missing_extra_and_structural_payloads() {
    let mut context = Context::new();
    register_dialect(&mut context).expect("dispatch registration");

    let wrong_policy = SelectionIntentOp::new(
        &mut context,
        id(400),
        id(410),
        id(420),
        SelectionPolicyAttr::SafeFallback,
    );
    wrong_policy
        .get_operation()
        .deref_mut(&context)
        .attributes
        .set(
            selection_intent_op_attr_names::ATTR_KEY_DISPATCH_SELECTION_INTENT_POLICY.clone(),
            UnitAttr,
        );
    assert!(verify_op(&wrong_policy, &context).is_err());

    let missing = GraphIntentOp::new(
        &mut context,
        id(430),
        GraphCapacityAttr::Nodes16,
        DispatchModeAttr::UnfusedFinite,
    );
    missing
        .get_operation()
        .deref_mut(&context)
        .attributes
        .0
        .remove(
            &*dialect_dispatch::graph_intent_op_attr_names::ATTR_KEY_DISPATCH_GRAPH_INTENT_MODE,
        );
    assert!(verify_op(&missing, &context).is_err());

    let extra_graph = GraphIntentOp::new(
        &mut context,
        id(440),
        GraphCapacityAttr::Nodes64,
        DispatchModeAttr::PersistentService,
    );
    let extra_dependency = DependencyIntentOp::new(
        &mut context,
        id(441),
        id(450),
        id(460),
        DependencyKindAttr::Visibility,
    );
    let workspace_graph_id = id(442);
    let extra_workspace = WorkspaceIntentOp::new(
        &mut context,
        workspace_graph_id.clone(),
        id(451),
        workspace_graph_id,
        WorkspaceClassAttr::Bytes4096,
        WorkspaceLifetimeAttr::Graph,
    );
    let extra_selection = SelectionIntentOp::new(
        &mut context,
        id(443),
        id(452),
        id(462),
        SelectionPolicyAttr::DeterministicRanked,
    );
    for op in [
        &extra_graph as &dyn Op,
        &extra_dependency,
        &extra_workspace,
        &extra_selection,
    ] {
        op.get_operation().deref_mut(&context).attributes.set(
            Identifier::try_from("dispatch_hostile_extra").expect("valid key"),
            BytesAttr::new(vec![0xde, 0xad, 0xbe, 0xef]),
        );
        assert!(
            verify_op(op, &context).is_err(),
            "{} accepted an undeclared byte payload",
            op.get_opid()
        );
    }

    let result_type = UnitType::get(&context).into();
    let operation = Operation::new(
        &mut context,
        GraphIntentOp::get_concrete_op_info(),
        vec![result_type],
        vec![],
        vec![],
        0,
    );
    let malformed = GraphIntentOp::from_operation(operation);
    malformed.set_attr_dispatch_graph_intent_graph_id(&context, id(470));
    malformed.set_attr_dispatch_graph_intent_capacity(&context, GraphCapacityAttr::Nodes16);
    malformed.set_attr_dispatch_graph_intent_mode(&context, DispatchModeAttr::UnfusedFinite);
    assert!(verify_op(&malformed, &context).is_err());
}

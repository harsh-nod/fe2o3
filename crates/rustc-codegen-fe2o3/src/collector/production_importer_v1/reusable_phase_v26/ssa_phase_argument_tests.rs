//! Real admitted/expanded SSA transport tests using ordinary u32, not a
//! manufactured phase or live provider receipt. Actual phase callbacks are
//! separate and must authenticate the capability, loans and entire lifecycle.
use super::*;
use crate::collector::ProductionSemanticImportErrorV1;
use fe2o3_pliron::{
    ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1, ProductionSemanticSsaLimitsV1,
    ProductionSemanticSsaOwnerV1,
};

const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const WORD: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const TUPLE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const ROOT: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(0);
fn source() -> SemanticSourceProvenanceV1 {
    SemanticSourceProvenanceV1::unavailable()
}
fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}
fn direct(ty: SemanticTypeIdV1) -> SemanticAbiValueV1 {
    SemanticAbiValueV1::new(
        ty,
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                SemanticAbiExtensionV1::None,
                0,
                None,
            )
            .unwrap(),
        ),
    )
}
fn assign(local: u32, ty: SemanticTypeIdV1, kind: SemanticRvalueKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        source(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(local, ty),
            SemanticRvalueV1::new(ty, kind),
        )),
    )
}
fn block(
    tag: u8,
    statements: Vec<SemanticStatementV1>,
    term: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([tag; 32]),
        source(),
        statements,
        SemanticTerminatorV1::new(source(), term),
    )
    .unwrap()
}
fn function(
    tag: u8,
    role: SemanticFunctionRoleV1,
    abi: SemanticFunctionAbiV1,
    locals: &[(SemanticTypeIdV1, SemanticLocalRoleV1)],
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([tag; 32]),
        role,
        SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
        source(),
        abi,
        locals
            .iter()
            .enumerate()
            .map(|(i, (ty, role))| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([i as u8 + 1; 32]),
                    *ty,
                    *role,
                    source(),
                )
            })
            .collect(),
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
}
fn owner(copy_pack: bool) -> ProductionSemanticSsaOwnerV1 {
    owner_with_capture(copy_pack, false)
}
fn owner_with_capture(copy_pack: bool, captured: bool) -> ProductionSemanticSsaOwnerV1 {
    owner_with_capture_cfg(copy_pack, captured, false)
}
fn owner_with_capture_cfg(
    copy_pack: bool,
    captured: bool,
    bypass: bool,
) -> ProductionSemanticSsaOwnerV1 {
    use SemanticLocalRoleV1::{Argument, Return, Temporary};
    let scalar = SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 32, 4),
        SemanticScalarValidityRangeV1::new(0, u32::MAX as u128),
    ));
    let types = vec![
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([1; 32]),
            SemanticLayoutIdentityV1::from_sha256([1; 32]),
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                0,
                1,
                SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
                SemanticRustcVariantsV1::Single { index: 0 },
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                1,
                0,
                SemanticTypeLayoutDetailsV1::None,
            )
            .unwrap(),
            SemanticTypeShapeV1::Unit,
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([2; 32]),
            SemanticLayoutIdentityV1::from_sha256([2; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(Some(4), 4, scalar.clone(), false).unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            }),
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([3; 32]),
            SemanticLayoutIdentityV1::from_sha256([3; 32]),
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                Some(4),
                4,
                scalar,
                false,
                SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![WORD]).unwrap()),
        ),
    ];
    let unit = SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore);
    let root_abi = SemanticFunctionAbiV1::from_rustc_with_source_signature(
        SemanticAbiIdentityV1::from_sha256([10; 32]),
        SemanticLayoutIdentityV1::from_sha256([10; 32]),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        1,
        vec![WORD],
        UNIT,
        vec![SemanticAbiArgumentV1::source(direct(WORD))],
        unit.clone(),
    )
    .unwrap();
    let closure_abi = SemanticFunctionAbiV1::from_rustc_with_source_signature(
        SemanticAbiIdentityV1::from_sha256([20; 32]),
        SemanticLayoutIdentityV1::from_sha256([20; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::RustCall,
        false,
        false,
        1,
        vec![if captured { TUPLE } else { UNIT }, TUPLE],
        WORD,
        vec![
            SemanticAbiArgumentV1::source(if captured { direct(TUPLE) } else { unit }),
            SemanticAbiArgumentV1::rust_call_tuple_field(0, direct(WORD)),
        ],
        direct(WORD),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue; 2])
    .unwrap();
    let operand = if copy_pack {
        SemanticOperandV1::Copy(place(1, WORD))
    } else {
        SemanticOperandV1::Move(place(1, WORD))
    };
    let call = SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1::from_index(1),
        vec![
            if captured {
                SemanticOperandV1::Move(place(4, TUPLE))
            } else {
                SemanticOperandV1::Constant(SemanticConstantV1::new(
                    UNIT,
                    SemanticConstantValueV1::ZeroSized,
                ))
            },
            SemanticOperandV1::Move(place(2, TUPLE)),
        ],
        Some(SemanticCallDestinationV1::new(
            place(3, WORD),
            SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::CallReturn,
                SemanticBlockIdV1::from_index(if bypass { 2 } else { 1 }),
            ),
        )),
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap();
    let root = function(
        10,
        SemanticFunctionRoleV1::KernelRoot,
        root_abi,
        &[
            (UNIT, Return),
            (WORD, Argument(0)),
            (TUPLE, Temporary),
            (WORD, Temporary),
            (TUPLE, Temporary),
        ],
        {
            let mut blocks = vec![
                block(
                    if bypass { 2 } else { 1 },
                    {
                        let mut statements = Vec::new();
                        if captured {
                            statements.push(assign(
                                4,
                                TUPLE,
                                SemanticRvalueKindV1::Aggregate(
                                    SemanticAggregateRvalueV1::new(
                                        SemanticAggregateKindV1::Tuple,
                                        vec![SemanticOperandV1::Copy(place(1, WORD))],
                                    )
                                    .unwrap(),
                                ),
                            ));
                        }
                        statements.push(assign(
                            2,
                            TUPLE,
                            SemanticRvalueKindV1::Aggregate(
                                SemanticAggregateRvalueV1::new(
                                    SemanticAggregateKindV1::Tuple,
                                    vec![operand],
                                )
                                .unwrap(),
                            ),
                        ));
                        statements
                    },
                    SemanticTerminatorKindV1::Call(call),
                ),
                block(
                    if bypass { 3 } else { 2 },
                    if bypass {
                        vec![assign(
                            3,
                            WORD,
                            SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(
                                SemanticConstantV1::new(
                                    WORD,
                                    SemanticConstantValueV1::Scalar(
                                        SemanticScalarValueV1::new(17, 4).unwrap(),
                                    ),
                                ),
                            )),
                        )]
                    } else {
                        vec![]
                    },
                    SemanticTerminatorKindV1::Return,
                ),
            ];
            if bypass {
                blocks.insert(
                    0,
                    block(
                        1,
                        vec![],
                        SemanticTerminatorKindV1::SwitchInt {
                            discriminant: SemanticOperandV1::Copy(place(1, WORD)),
                            targets: SemanticSwitchTargetsV1::new(
                                vec![SemanticSwitchTargetV1::new(
                                    0,
                                    SemanticControlFlowEdgeV1::new(
                                        SemanticEdgeRoleV1::SwitchValue,
                                        SemanticBlockIdV1::from_index(1),
                                    ),
                                )],
                                SemanticControlFlowEdgeV1::new(
                                    SemanticEdgeRoleV1::SwitchOtherwise,
                                    SemanticBlockIdV1::from_index(2),
                                ),
                            )
                            .unwrap(),
                        },
                    ),
                );
            }
            blocks
        },
    )
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"phase_transport_only".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([10; 32]),
        SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
    ));
    let field = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(2),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), WORD).unwrap()],
        WORD,
    )
    .unwrap();
    let closure = function(
        20,
        SemanticFunctionRoleV1::InternalHelper,
        closure_abi,
        &[
            (WORD, Return),
            (if captured { TUPLE } else { UNIT }, Argument(0)),
            (TUPLE, Argument(1)),
            (WORD, Temporary),
            (WORD, Temporary),
        ],
        vec![block(
            1,
            {
                let mut statements = vec![assign(
                    3,
                    WORD,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(field)),
                )];
                if captured {
                    let field = SemanticPlaceV1::new(
                        SemanticLocalIdV1::from_index(1),
                        vec![
                            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), WORD)
                                .unwrap(),
                        ],
                        WORD,
                    )
                    .unwrap();
                    statements.push(assign(
                        4,
                        WORD,
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field)),
                    ));
                }
                statements.push(assign(
                    0,
                    WORD,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(
                        if captured { 4 } else { 3 },
                        WORD,
                    ))),
                ));
                statements
            },
            SemanticTerminatorKindV1::Return,
        )],
    );
    let source = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([40; 32])),
        types,
        vec![],
        vec![],
        vec![],
        vec![root, closure],
        vec![ROOT],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(source, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}
fn request<'a>(
    owner: &'a ProductionSemanticSsaOwnerV1,
    query: &ProductionSemanticSsaSourceQueryV1<'_>,
    view: &SemanticExpandedRootV1,
) -> Request<'a> {
    let closure = view
        .instances()
        .iter()
        .find(|frame| frame.function().index() == 1)
        .unwrap();
    let call_block = SemanticBlockIdV1::from_index(0);
    let pack = assignment(query, Site::new(call_block, Some(0))).unwrap();
    let SemanticRvalueKindV1::Aggregate(a) = pack.value().kind() else {
        panic!("tuple fixture")
    };
    let mut budget = 100_000;
    let issued = query
        .operand_use(
            Site::new(call_block, Some(0)),
            &a.operands()[0],
            &mut || spend(&mut budget, 1).is_ok(),
        )
        .unwrap()
        .value();
    Request {
        closure_source: &owner.source_semantic().functions()[1],
        wrapper: closure.parent().unwrap(),
        closure: view.local_origins()[closure.local_start() as usize].instance(),
        invoke_block: call_block,
        tuple_type: TUPLE,
        phase_type: WORD,
        issued_local: SemanticLocalIdV1::from_index(1),
        issued_value: issued,
    }
}

fn capture_request<'a>(
    owner: &'a ProductionSemanticSsaOwnerV1,
    query: &ProductionSemanticSsaSourceQueryV1<'a>,
    view: &'a SemanticExpandedRootV1,
) -> super::super::closure_capture::Request<'a> {
    let closure = view
        .instances()
        .iter()
        .find(|frame| frame.function().index() == 1)
        .unwrap();
    let mut work = 100_000;
    let instance = view.local_origins()[closure.local_start() as usize].instance();
    let entry = mapped_block(view, instance, SemanticBlockIdV1::from_index(0), &mut work).unwrap();
    let read = assignment(query, source_site(view, entry, 1, &mut work).unwrap()).unwrap();
    super::super::closure_capture::Request {
        closure_source: &owner.source_semantic().functions()[1],
        wrapper: closure.parent().unwrap(),
        closure: instance,
        invoke_block: SemanticBlockIdV1::from_index(0),
        environment: SemanticLocalIdV1::from_index(4),
        environment_value: value_at(
            query,
            Site::new(SemanticBlockIdV1::from_index(0), Some(0)),
            SsaVariableIdV1::new(4),
            Event::Define,
            &mut work,
        )
        .unwrap(),
        field: 0,
        destination: read.destination(),
    }
}

#[test]
fn retained_capture_joins_exact_environment_transfer_field_and_result() {
    let owner = owner_with_capture(false, true);
    owner.verify_replay().unwrap();
    let view = owner.execution_view_for_root(ROOT).unwrap();
    let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
    let request = capture_request(&owner, &query, view);
    let local = request.destination.local();
    let mut work = 100_000;
    let result =
        super::super::closure_capture::field_value(&query, view, request, &mut work).unwrap();
    let closure = view
        .instances()
        .iter()
        .find(|frame| frame.function().index() == 1)
        .unwrap();
    let instance = view.local_origins()[closure.local_start() as usize].instance();
    let entry = mapped_block(view, instance, SemanticBlockIdV1::from_index(0), &mut work).unwrap();
    let returned = source_site(view, entry, 2, &mut work).unwrap();
    assert_eq!(
        result,
        value_at(
            &query,
            returned,
            SsaVariableIdV1::new(local.index()),
            Event::Use,
            &mut work
        )
        .unwrap()
    );
}

#[test]
fn retained_capture_rejects_wrong_frame_source_and_foreign_view() {
    let owner = owner_with_capture(false, true);
    let other = owner_with_capture(false, true);
    let view = owner.execution_view_for_root(ROOT).unwrap();
    let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
    for mutation in 0..3 {
        let mut request = capture_request(&owner, &query, view);
        let actual_view = match mutation {
            0 => {
                request.wrapper = request.closure;
                view
            }
            1 => {
                request.closure_source = &owner.source_semantic().functions()[0];
                view
            }
            _ => other.execution_view_for_root(ROOT).unwrap(),
        };
        assert_eq!(
            binding_result(super::super::closure_capture::field_value(
                &query,
                actual_view,
                request,
                &mut 100_000,
            )),
            Err("phase capture substituted its closure, caller, ABI or local owner")
        );
    }
}

#[test]
fn retained_capture_rejects_substituted_environment_ssa() {
    let owner = owner_with_capture(false, true);
    let view = owner.execution_view_for_root(ROOT).unwrap();
    let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
    let mut request = capture_request(&owner, &query, view);
    request.environment_value = value_at(
        &query,
        Site::new(SemanticBlockIdV1::from_index(0), Some(0)),
        SsaVariableIdV1::new(1),
        Event::Use,
        &mut 100_000,
    )
    .unwrap();
    assert_eq!(
        binding_result(super::super::closure_capture::field_value(
            &query,
            view,
            request,
            &mut 100_000,
        )),
        Err("phase capture parameter changed its environment SSA identity")
    );
}

#[test]
fn retained_capture_rejects_wrong_field_and_other_argument_projection() {
    let owner = owner_with_capture(false, true);
    let view = owner.execution_view_for_root(ROOT).unwrap();
    let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
    for wrong_field in [true, false] {
        let mut request = capture_request(&owner, &query, view);
        if wrong_field {
            request.field = 1;
        } else {
            let entry = mapped_block(
                view,
                request.closure,
                SemanticBlockIdV1::from_index(0),
                &mut 100_000,
            )
            .unwrap();
            request.destination =
                assignment(&query, source_site(view, entry, 0, &mut 100_000).unwrap())
                    .unwrap()
                    .destination();
        }
        assert_eq!(
            binding_result(super::super::closure_capture::field_value(
                &query,
                view,
                request,
                &mut 100_000,
            )),
            Err("phase capture read substituted the selected environment field")
        );
    }
}

#[test]
fn retained_capture_charges_exact_existing_work_without_temporary_storage() {
    let owner = owner_with_capture(false, true);
    let view = owner.execution_view_for_root(ROOT).unwrap();
    let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
    let mut remaining = 100_000;
    super::super::closure_capture::field_value(
        &query,
        view,
        capture_request(&owner, &query, view),
        &mut remaining,
    )
    .unwrap();
    let required = 100_000 - remaining;
    assert!(required > 12);
    let mut exact = required;
    super::super::closure_capture::field_value(
        &query,
        view,
        capture_request(&owner, &query, view),
        &mut exact,
    )
    .unwrap();
    assert_eq!(exact, 0);
    assert!(
        super::super::closure_capture::field_value(
            &query,
            view,
            capture_request(&owner, &query, view),
            &mut (required - 1)
        )
        .is_err()
    );
}

fn defined_point(
    query: &ProductionSemanticSsaSourceQueryV1<'_>,
    site: Site,
) -> linear_events::Point {
    let destination = assignment(query, site).unwrap().destination().local();
    let variable = SsaVariableIdV1::new(destination.index());
    let value = value_at(query, site, variable, Event::Define, &mut 100_000).unwrap();
    super::super::boundaries::event_point(query, site, variable, value, Event::Define, &mut 100_000)
        .unwrap()
}

#[test]
fn phase_order_uses_exact_events_and_existing_normal_predecessors() {
    let owner = owner_with_capture(false, true);
    let view = owner.execution_view_for_root(ROOT).unwrap();
    let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
    let request = capture_request(&owner, &query, view);
    let entry = mapped_block(
        view,
        request.closure,
        SemanticBlockIdV1::from_index(0),
        &mut 100_000,
    )
    .unwrap();
    let before = defined_point(&query, Site::new(SemanticBlockIdV1::from_index(0), Some(0)));
    let read = defined_point(&query, source_site(view, entry, 1, &mut 100_000).unwrap());
    let returned = defined_point(&query, source_site(view, entry, 2, &mut 100_000).unwrap());
    assert_eq!(
        binding_result(super::super::phase_order::precedes(
            &query,
            before,
            read,
            &mut 100_000
        )),
        Ok(true)
    );
    assert_eq!(
        binding_result(super::super::phase_order::precedes(
            &query,
            read,
            before,
            &mut 100_000
        )),
        Ok(false)
    );
    assert_eq!(
        binding_result(super::super::phase_order::precedes(
            &query,
            read,
            returned,
            &mut 100_000
        )),
        Ok(true)
    );
    assert_eq!(
        binding_result(super::super::phase_order::precedes(
            &query,
            returned,
            read,
            &mut 100_000
        )),
        Ok(false)
    );
    assert_eq!(
        binding_result(super::super::phase_order::precedes(
            &query,
            read,
            read,
            &mut 100_000
        )),
        Ok(false)
    );
}

#[test]
fn phase_order_rejects_actual_normal_edge_bypassing_the_prior_event() {
    let owner = owner_with_capture_cfg(false, true, true);
    owner.verify_replay().unwrap();
    let view = owner.execution_view_for_root(ROOT).unwrap();
    let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
    let frame = view
        .instances()
        .iter()
        .find(|frame| frame.function().index() == 1)
        .unwrap();
    let instance = view.local_origins()[frame.local_start() as usize].instance();
    let entry = mapped_block(
        view,
        instance,
        SemanticBlockIdV1::from_index(0),
        &mut 100_000,
    )
    .unwrap();
    let exit = mapped_block(
        view,
        frame.parent().unwrap(),
        SemanticBlockIdV1::from_index(2),
        &mut 100_000,
    )
    .unwrap();
    let before = defined_point(&query, source_site(view, entry, 1, &mut 100_000).unwrap());
    let after = defined_point(&query, source_site(view, exit, 0, &mut 100_000).unwrap());
    assert_eq!(
        binding_result(super::super::phase_order::precedes(
            &query,
            before,
            after,
            &mut 100_000
        )),
        Ok(false)
    );
}

#[test]
fn phase_order_rejects_missing_events_and_charges_scratch_capacity() {
    let owner = owner_with_capture(false, true);
    let view = owner.execution_view_for_root(ROOT).unwrap();
    let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
    let request = capture_request(&owner, &query, view);
    let entry = mapped_block(
        view,
        request.closure,
        SemanticBlockIdV1::from_index(0),
        &mut 100_000,
    )
    .unwrap();
    let before = defined_point(&query, Site::new(SemanticBlockIdV1::from_index(0), Some(0)));
    let after = defined_point(&query, source_site(view, entry, 1, &mut 100_000).unwrap());
    let missing = linear_events::Point {
        block: after.block,
        event: u32::MAX,
    };
    assert_eq!(
        binding_result(super::super::phase_order::precedes(
            &query,
            before,
            missing,
            &mut 100_000
        )),
        Err("phase order has no exact reachable event")
    );
    let mut remaining = 100_000;
    assert_eq!(
        binding_result(super::super::phase_order::precedes(
            &query,
            before,
            after,
            &mut remaining
        )),
        Ok(true)
    );
    let required = 100_000 - remaining;
    assert!(required > 4 + view.body().blocks().len());
    let mut exact = required;
    assert_eq!(
        binding_result(super::super::phase_order::precedes(
            &query, before, after, &mut exact
        )),
        Ok(true)
    );
    assert_eq!(exact, 0);
    assert!(
        super::super::phase_order::precedes(&query, before, after, &mut (required - 1)).is_err()
    );
}
fn binding_result<T>(result: PhaseResult<T>) -> std::result::Result<T, &'static str> {
    result.map_err(|error| match error {
        ProductionSemanticImportErrorV1::KernelContextBinding(detail) => detail,
        other => panic!("unexpected boundary error: {other:?}"),
    })
}

fn error_contains<T>(result: PhaseResult<T>, detail: &str) {
    match result {
        Ok(_) => panic!("expected {detail}"),
        Err(error) => assert!(error.to_string().contains(detail), "{error:?}"),
    }
}

#[test]
fn phase_argument_uses_real_normal_call_and_tuple_field_ssa() {
    let owner = owner(false);
    owner.verify_replay().unwrap();
    let view = owner.execution_view_for_root(ROOT).unwrap();
    let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
    let result =
        closure_argument(&query, view, request(&owner, &query, view), &mut 100_000).unwrap();
    let local = &view.local_origins()[result.local.index() as usize];
    assert_eq!(local.function().index(), 1);
    assert_eq!(local.local().index(), 3);
    assert_ne!(result.value, request(&owner, &query, view).issued_value);
}

#[test]
fn phase_argument_rejects_another_equal_body_owner() {
    let owner = owner(false);
    let other = self::owner(false);
    let view = owner.execution_view_for_root(ROOT).unwrap();
    let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
    error_contains(
        closure_argument(
            &query,
            other.execution_view_for_root(ROOT).unwrap(),
            request(&owner, &query, view),
            &mut 100_000,
        ),
        "substituted its source ABI or caller",
    );
}

#[test]
fn phase_argument_rejects_substituted_occurrence_and_tuple_type() {
    let owner = owner(false);
    let view = owner.execution_view_for_root(ROOT).unwrap();
    let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
    for change in 0..2 {
        let mut row = request(&owner, &query, view);
        if change == 0 {
            row.wrapper = row.closure;
        } else {
            row.tuple_type = WORD;
        }
        error_contains(
            closure_argument(&query, view, row, &mut 100_000),
            "substituted its source ABI or caller",
        );
    }
}

#[test]
fn phase_argument_rejects_a_different_same_typed_ssa_value() {
    let owner = owner(false);
    let view = owner.execution_view_for_root(ROOT).unwrap();
    let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
    let result =
        closure_argument(&query, view, request(&owner, &query, view), &mut 100_000).unwrap();
    let mut row = request(&owner, &query, view);
    row.issued_value = result.value;
    error_contains(
        closure_argument(&query, view, row, &mut 100_000),
        "changed or reused its exact SSA value",
    );
}

#[test]
fn phase_argument_rejects_copy_instead_of_the_original_pack_move() {
    let owner = owner(true);
    let view = owner.execution_view_for_root(ROOT).unwrap();
    let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
    error_contains(
        closure_argument(&query, view, request(&owner, &query, view), &mut 100_000),
        "must move its one issued Workgroup",
    );
}

#[test]
fn phase_argument_uses_the_exact_remaining_work_without_reset() {
    let owner = owner(false);
    let view = owner.execution_view_for_root(ROOT).unwrap();
    let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
    let mut remaining = 100_000;
    closure_argument(&query, view, request(&owner, &query, view), &mut remaining).unwrap();
    let needed = 100_000 - remaining;
    assert!(needed > 0 && needed < 100_000);
    let mut exact = needed;
    closure_argument(&query, view, request(&owner, &query, view), &mut exact).unwrap();
    assert_eq!(exact, 0);
    error_contains(
        closure_argument(
            &query,
            view,
            request(&owner, &query, view),
            &mut (needed - 1),
        ),
        "work",
    );
}

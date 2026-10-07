//! Re-admitted hostile source, not a rustc Copy implementation or proof receipt.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::{
    ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1, ProductionSemanticSsaLimitsV1,
    ProductionSemanticSsaOwnerV1,
};

const LIMIT: usize = 100_000_000;
const FLOOR: usize = super::super::super::invocations::tests::FLOOR;

fn replace(
    function: &SemanticFunctionDeclV1,
    abi: SemanticFunctionAbiV1,
    locals: Vec<SemanticLocalDeclV1>,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    let mut rebuilt = SemanticFunctionDeclV1::new(
        function.identity(),
        function.role(),
        function.item_definition_identity(),
        function.monomorphization_identity(),
        function.generic_type_arguments_identity(),
        function.const_generic_arguments_identity(),
        function.source(),
        abi,
        locals,
        function.entry(),
        blocks,
    )
    .unwrap();
    if let Some(entry) = function.kernel_entry() {
        rebuilt = rebuilt.with_kernel_entry(entry.clone());
    }
    rebuilt
}

fn owner(assignment: bool) -> ProductionSemanticSsaOwnerV1 {
    let semantic = AdmittedInertSemanticMirV1::decode_exact_v29_canonical(
        include_bytes!("fixtures/original-tile-descriptor-v163.bin"),
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    let helper_id = SemanticFunctionIdV1::from_index(3);
    let ordinal = 1;
    let helper = &semantic.functions()[helper_id.index() as usize];
    let reference = helper.abi().source_input_types()[ordinal];
    let Shape::Pointer(pointer) = semantic.types()[reference.index() as usize].shape() else {
        panic!("original shared Workgroup helper parameter");
    };
    let workgroup = pointer.pointee();
    assert_eq!(pointer.kind(), SemanticPointerKindV1::Reference);
    assert_eq!(pointer.mutability(), SemanticMutabilityV1::Immutable);
    let declaration = &semantic.types()[workgroup.index() as usize];
    assert_eq!(
        declaration.rust_type_kind(),
        SemanticRustTypeKindV1::Execution(SemanticExecutionRoleV29::Workgroup)
    );
    let SemanticBackendReprV1::ScalarPair { first, second } = declaration.layout().backend_repr()
    else {
        panic!("retained Workgroup scalar-pair ABI");
    };
    for scalar in [first, second] {
        assert!(matches!(
            scalar,
            SemanticBackendScalarV1::Initialized {
                primitive: SemanticBackendPrimitiveV1::Integer {
                    signed: false,
                    bits: 64,
                    ..
                },
                ..
            }
        ));
    }
    let mut functions = semantic.functions().to_vec();
    let mut calls = 0;
    for (index, function) in semantic.functions().iter().enumerate() {
        if index == helper_id.index() as usize {
            continue;
        }
        let mut locals = function.locals().to_vec();
        let mut blocks = function.blocks().to_vec();
        let mut changed = false;
        for (block_index, block) in function.blocks().iter().enumerate() {
            let Terminator::Call(call) = block.terminator().kind() else {
                continue;
            };
            if !matches!(semantic.callables()[call.callee().index() as usize],
                SemanticCallableDeclV1::Defined { function } if function == helper_id)
            {
                continue;
            }
            let (Operand::Copy(reference_place) | Operand::Move(reference_place)) =
                &call.arguments()[ordinal]
            else {
                panic!("original helper reference operand");
            };
            let mut borrowed = None;
            for statement in block.statements() {
                let Statement::Assign(value) = statement.kind() else {
                    continue;
                };
                if value.destination().local() == reference_place.local() {
                    let Rvalue::Borrow {
                        kind: SemanticBorrowKindV1::Shared,
                        place,
                    } = value.value().kind()
                    else {
                        panic!("exact original shared Workgroup borrow");
                    };
                    assert_eq!(place.ty(), workgroup);
                    assert!(place.projections().is_empty());
                    assert!(borrowed.replace(place.clone()).is_none());
                }
            }
            let borrowed = borrowed.unwrap();
            let mut statements = block.statements().to_vec();
            if assignment {
                let local = SemanticLocalIdV1::from_index(locals.len().try_into().unwrap());
                let mut identity = [246; 32];
                identity[28..].copy_from_slice(&local.index().to_be_bytes());
                locals.push(SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256(identity),
                    workgroup,
                    SemanticLocalRoleV1::Temporary,
                    function.source(),
                ));
                statements.push(SemanticStatementV1::new(
                    function.source(),
                    Statement::Assign(SemanticAssignmentV1::new(
                        SemanticPlaceV1::new(local, vec![], workgroup).unwrap(),
                        SemanticRvalueV1::new(
                            workgroup,
                            Rvalue::Use(Operand::Copy(borrowed.clone())),
                        ),
                    )),
                ));
            }
            let mut arguments = call.arguments().to_vec();
            arguments[ordinal] = Operand::Copy(borrowed);
            blocks[block_index] = SemanticBasicBlockV1::new(
                block.identity(),
                block.source(),
                statements,
                SemanticTerminatorV1::new(
                    block.terminator().source(),
                    Terminator::Call(
                        SemanticDirectCallV1::new_callable_with_variadic_argument_abis(
                            call.callee(),
                            arguments,
                            call.variadic_argument_abis().to_vec(),
                            call.destination().cloned(),
                            call.unwind(),
                        )
                        .unwrap(),
                    ),
                ),
            )
            .unwrap();
            changed = true;
            calls += 1;
        }
        if changed {
            functions[index] = replace(function, function.abi().clone(), locals, blocks);
        }
    }
    assert_eq!(calls, 2);
    let mut locals = helper.locals().to_vec();
    let old = locals
        .iter()
        .position(|local| local.role() == SemanticLocalRoleV1::Argument(ordinal as u32))
        .unwrap();
    let declaration = &locals[old];
    locals[old] = SemanticLocalDeclV1::new(
        declaration.identity(),
        reference,
        SemanticLocalRoleV1::Temporary,
        declaration.source(),
    );
    let parameter = SemanticLocalIdV1::from_index(locals.len().try_into().unwrap());
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([245; 32]),
        workgroup,
        SemanticLocalRoleV1::Argument(ordinal as u32),
        helper.source(),
    ));
    let mut blocks = helper.blocks().to_vec();
    let entry = &blocks[helper.entry().index() as usize];
    let mut statements = vec![SemanticStatementV1::new(
        helper.source(),
        Statement::Assign(SemanticAssignmentV1::new(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(old as u32), vec![], reference)
                .unwrap(),
            SemanticRvalueV1::new(
                reference,
                Rvalue::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: SemanticPlaceV1::new(parameter, vec![], workgroup).unwrap(),
                },
            ),
        )),
    )];
    statements.extend_from_slice(entry.statements());
    blocks[helper.entry().index() as usize] = SemanticBasicBlockV1::new(
        entry.identity(),
        entry.source(),
        statements,
        entry.terminator().clone(),
    )
    .unwrap();
    let abi = helper.abi();
    let mut inputs = abi.source_input_types().to_vec();
    inputs[ordinal] = workgroup;
    let mut adjusted = abi.adjusted_arguments().to_vec();
    // Both retained scalar-pair components above are initialized u64 values.
    let attributes = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    adjusted[ordinal] = SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
        workgroup,
        SemanticAbiPassModeV1::Pair {
            first: attributes,
            second: attributes,
        },
    ));
    let mut ownership = abi.source_argument_ownership().to_vec();
    ownership[ordinal] = SemanticSourceArgumentOwnershipV1::ByValue;
    let abi = SemanticFunctionAbiV1::from_rustc_with_source_signature(
        abi.identity(),
        abi.layout_identity(),
        abi.canon_abi(),
        abi.extern_abi(),
        abi.can_unwind(),
        abi.c_variadic(),
        abi.fixed_count(),
        inputs,
        abi.source_output_type(),
        adjusted,
        abi.return_value().clone(),
    )
    .unwrap()
    .with_source_argument_ownership(ownership)
    .unwrap();
    functions[helper_id.index() as usize] = replace(helper, abi, locals, blocks);
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
        semantic.allocations().to_vec(),
        semantic.statics().to_vec(),
        semantic.vtables().to_vec(),
        functions,
        semantic.callables().to_vec(),
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn run(work: usize, storage: usize, assignment: bool) -> (Result<()>, usize, usize, usize) {
    super::super::source_function::tile_fixture_tests::run_fixture_with_plan(
        fe2o3_kernel_ir::ExecutionTileLayoutV1::Blocked,
        work,
        storage,
        |plan, slots, _, out| {
            out.budget.reserve_storage(headers())?;
            let source = plan.source(out)?.source_semantic(out.budget)?;
            let mut observed = 0;
            for instance in 0..plan.root(0, out)?.instances.len() {
                let row = plan.instance(0, instance, out)?;
                if !row.active {
                    continue;
                }
                let function = &source.functions()[row.function.index() as usize];
                let context = Context {
                    slots,
                    types: source.types(),
                    function,
                    root: 0,
                    instance,
                    locals: row.locals.clone(),
                };
                for declaration in function.blocks() {
                    let Terminator::Call(call) = declaration.terminator().kind() else {
                        continue;
                    };
                    if !matches!(source.callables()[call.callee().index() as usize],
                        SemanticCallableDeclV1::Defined { function } if function.index() == 3)
                    {
                        continue;
                    }
                    let (Operand::Copy(reference) | Operand::Move(reference)) =
                        &call.arguments()[1]
                    else {
                        panic!("retained shared Workgroup call argument");
                    };
                    let mut borrowed = None;
                    for statement in declaration.statements() {
                        let Statement::Assign(value) = statement.kind() else {
                            continue;
                        };
                        if value.destination().local() != reference.local() {
                            continue;
                        }
                        let Rvalue::Borrow {
                            kind: SemanticBorrowKindV1::Shared,
                            place,
                        } = value.value().kind()
                        else {
                            panic!("retained shared Workgroup borrow");
                        };
                        assert!(borrowed.replace(place).is_none());
                    }
                    let place = borrowed.unwrap();
                    assert_eq!(
                        source.types()[place.ty().index() as usize].rust_type_kind(),
                        SemanticRustTypeKindV1::Execution(SemanticExecutionRoleV29::Workgroup)
                    );
                    assert!(!slots.is_product_v282(place.ty(), out)?);
                    assert!(!slots.product_type_copyable_v282(place.ty(), out)?);
                    // Deliberate probes use authentic endpoints, not an admitted
                    // original Copy or an executed call/assignment trace.
                    if assignment {
                        let value = SemanticAssignmentV1::new(
                            place.clone(),
                            SemanticRvalueV1::new(
                                place.ty(),
                                Rvalue::Use(Operand::Copy(place.clone())),
                            ),
                        );
                        match aggregates::Transfer::derive(&context, &value, out) {
                            Err(Error::Statement(reason)) => {
                                assert_eq!(
                                    reason,
                                    "original MIR typed byte statement is not modeled"
                                )
                            }
                            Err(error) => return Err(error),
                            Ok(_) => panic!("nominal owner Copy assignment accepted"),
                        }
                        let moved = SemanticAssignmentV1::new(
                            value.destination().clone(),
                            SemanticRvalueV1::new(
                                place.ty(),
                                Rvalue::Use(Operand::Move(place.clone())),
                            ),
                        );
                        let transfer =
                            aggregates::Transfer::derive(&context, &moved, out)?.unwrap();
                        let before = out.text.len();
                        transfer.emit(out)?;
                        assert!(out.text[before..].contains("moved: true"));
                    } else {
                        match context.typed_operand(&Operand::Copy(place.clone()), out) {
                            Err(Error::Statement(reason)) => {
                                assert_eq!(
                                    reason,
                                    "original MIR typed byte statement is not modeled"
                                )
                            }
                            Err(error) => return Err(error),
                            Ok(_) => panic!("nominal owner Copy operand accepted"),
                        }
                        // The Move counterpart uses the same authenticated type/local.
                        // It establishes dispatch support, not a reachable call trace.
                        assert!(matches!(
                            context
                                .typed_operand(&Operand::Move(place.clone()), out)?
                                .kind,
                            OperandKind::Aggregate { moved: true, .. }
                        ));
                    }
                    observed += 1;
                }
            }
            assert_eq!(observed, 2);
            Ok(())
        },
    )
}

#[test]
fn original_nominal_aggregate_copy_calls_and_assignments_refuse_without_changing_moves() {
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as CaptureBudget,
        CanonicalKernelIrWorkBudgetV1 as CaptureWork,
    };
    use fe2o3_lower_mir_kernel::{
        ProductionPendingScopedSourceErrorV29 as Pending, ProductionSemanticKirErrorV1 as Semantic,
        ProductionSourceOwnedViewErrorV18 as Source,
    };
    use fe2o3_pliron::{
        ProductionSemanticSsaEventRoleV1 as CapturedRole,
        ProductionSemanticSsaOccurrenceSiteV1 as CapturedSite,
        ProductionSemanticSsaOperandRoleV1 as CapturedOperand,
    };
    for assignment in [false, true] {
        let mut hostile = owner(assignment);
        let mut capture_work = CaptureWork::new(LIMIT);
        let mut capture_budget = CaptureBudget::new(&mut capture_work, LIMIT);
        let capture = hostile
            .try_capture_occurrences_with_budget_v1(&mut capture_budget)
            .unwrap();
        capture_budget
            .reserve_storage(capture.retained_storage())
            .unwrap();
        let semantic = hostile.source_semantic();
        let occurrences = hostile.occurrences_v1().unwrap();
        let mut sites = Vec::new();
        for (function, declaration) in semantic.functions().iter().enumerate() {
            for (block, body) in declaration.blocks().iter().enumerate() {
                let Terminator::Call(call) = body.terminator().kind() else {
                    continue;
                };
                if !matches!(semantic.callables()[call.callee().index() as usize],
                    SemanticCallableDeclV1::Defined { function } if function.index() == 3)
                {
                    continue;
                }
                let Operand::Copy(place) = &call.arguments()[1] else {
                    panic!("hostile original Copy call");
                };
                assert_eq!(
                    semantic.types()[place.ty().index() as usize].rust_type_kind(),
                    SemanticRustTypeKindV1::Execution(SemanticExecutionRoleV29::Workgroup)
                );
                let statement = if assignment {
                    let index = body.statements().len() - 1;
                    let Statement::Assign(value) = body.statements()[index].kind() else {
                        panic!("hostile assignment");
                    };
                    assert!(
                        matches!(value.value().kind(), Rvalue::Use(Operand::Copy(original)) if original == place)
                    );
                    Some(index as u32)
                } else {
                    None
                };
                let rows = occurrences
                    .function(SemanticFunctionIdV1::from_index(function as u32))
                    .unwrap();
                assert!(std::ptr::eq(rows.owner(), &hostile));
                let source_block = fe2o3_mir_model::SsaBlockIdV1::new(block as u32);
                let call_site = CapturedSite::Terminator {
                    block: source_block,
                };
                let mut expected = vec![(call_site, CapturedOperand::CallArgument(1))];
                if let Some(statement) = statement {
                    expected.push((
                        CapturedSite::Statement {
                            block: source_block,
                            statement,
                        },
                        CapturedOperand::RvalueOperand(0),
                    ));
                }
                // Full source preparation refuses these storage-retained uses
                // during nominal identity planning, before the live-Copy guard.
                for (site, operand) in expected {
                    let mut matching = rows.events().iter().filter(|event| {
                        event.site() == site
                            && event.operand() == operand
                            && event.role() == CapturedRole::BaseUse
                    });
                    let event = matching.next().unwrap();
                    assert!(matching.next().is_none());
                    assert_eq!(event.event().variable().get(), place.local().index());
                    assert!(event.is_reachable());
                    assert!(!event.is_promoted());
                    assert!(event.resolved().is_none());
                }
                sites.push((function as u32, Some(block as u32), statement));
            }
        }
        assert_eq!(sites.len(), 2);
        drop(occurrences);
        drop(hostile);
        capture_budget
            .release_storage(capture.retained_storage())
            .unwrap();
        assert_eq!(capture_budget.storage(), 0);
        let reached = std::cell::Cell::new(false);
        let refused =
            super::super::source_function::tile_fixture_tests::run_fixture_with_owner_v290(
                LIMIT,
                LIMIT,
                || owner(assignment),
                |_, _, _, _| {
                    reached.set(true);
                    Ok(())
                },
            );
        assert!(!reached.get());
        match refused.0 {
            Err(Error::Source(Source::Source(Pending::Source(Semantic::Unsupported {
                function,
                block,
                statement,
                detail,
            })))) => {
                assert_eq!(
                    detail,
                    "nominal identity equations differ from their original source"
                );
                assert_eq!((function, block, statement), (0, None, None));
            }
            result => panic!("expected exact earlier nominal-identity refusal: {result:?}"),
        }
        assert_eq!(refused.2, FLOOR);
        let result = run(LIMIT, LIMIT, assignment);
        result.0.unwrap();
        assert_eq!(result.2, FLOOR);
        assert!(result.3 > FLOOR);
    }
}

fn run_plain(work: usize, storage: usize) -> (Result<()>, usize, usize, usize) {
    super::super::super::invocations::tests::run_source_transform(
        work,
        storage,
        |types, functions| {
            let word = SemanticTypeIdV1::from_index(0);
            let pair = SemanticTypeIdV1::from_index(types.len().try_into().unwrap());
            types.push(SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([220; 32]),
                SemanticLayoutIdentityV1::from_sha256([221; 32]),
                SemanticTypeLayoutV1::aggregate(
                    Some(8),
                    4,
                    SemanticAggregateLayoutV1::new(vec![0, 4], vec![]).unwrap(),
                )
                .unwrap(),
                Shape::Tuple(SemanticAggregateTypeV1::new(vec![word, word]).unwrap()),
            ));
            let function = functions.last_mut().unwrap();
            let mut locals = function.locals().to_vec();
            let first = SemanticLocalIdV1::from_index(locals.len().try_into().unwrap());
            let second = SemanticLocalIdV1::from_index(first.index() + 1);
            for identity in [240, 241] {
                locals.push(SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([identity; 32]),
                    pair,
                    SemanticLocalRoleV1::Temporary,
                    function.source(),
                ));
            }
            let mut blocks = function.blocks().to_vec();
            let block = &blocks[0];
            let mut statements = block.statements().to_vec();
            statements.push(SemanticStatementV1::new(
                function.source(),
                Statement::Assign(SemanticAssignmentV1::new(
                    SemanticPlaceV1::new(first, vec![], pair).unwrap(),
                    SemanticRvalueV1::new(
                        pair,
                        Rvalue::Aggregate(
                            SemanticAggregateRvalueV1::new(
                                SemanticAggregateKindV1::Tuple,
                                [1, 2]
                                    .into_iter()
                                    .map(|local| {
                                        Operand::Copy(
                                            SemanticPlaceV1::new(
                                                SemanticLocalIdV1::from_index(local),
                                                vec![],
                                                word,
                                            )
                                            .unwrap(),
                                        )
                                    })
                                    .collect(),
                            )
                            .unwrap(),
                        ),
                    ),
                )),
            ));
            statements.push(SemanticStatementV1::new(
                function.source(),
                Statement::Assign(SemanticAssignmentV1::new(
                    SemanticPlaceV1::new(second, vec![], pair).unwrap(),
                    SemanticRvalueV1::new(
                        pair,
                        Rvalue::Use(Operand::Copy(
                            SemanticPlaceV1::new(first, vec![], pair).unwrap(),
                        )),
                    ),
                )),
            ));
            blocks[0] = SemanticBasicBlockV1::new(
                block.identity(),
                block.source(),
                statements,
                block.terminator().clone(),
            )
            .unwrap();
            *function = replace(function, function.abi().clone(), locals, blocks);
        },
        |plan, out| {
            super::super::source_function::tests::with_slots(plan, out, |slots, out| {
                out.budget.reserve_storage(headers())?;
                let source = plan.source(out)?.source_semantic(out.budget)?;
                let helper = source.functions().len() - 1;
                let mut observed = 0;
                for instance in 0..plan.root(0, out)?.instances.len() {
                    let row = plan.instance(0, instance, out)?;
                    if !row.active || row.function.index() as usize != helper {
                        continue;
                    }
                    let function = &source.functions()[helper];
                    let Statement::Assign(assignment) =
                        function.blocks()[0].statements().last().unwrap().kind()
                    else {
                        panic!("retained ordinary tuple Copy assignment");
                    };
                    let Rvalue::Use(operand @ Operand::Copy(place)) = assignment.value().kind()
                    else {
                        panic!("retained ordinary tuple Copy operand");
                    };
                    assert_eq!(
                        source.types()[place.ty().index() as usize].rust_type_kind(),
                        SemanticRustTypeKindV1::Ordinary
                    );
                    assert!(!slots.is_product_v282(place.ty(), out)?);
                    let context = Context {
                        slots,
                        types: source.types(),
                        function,
                        root: 0,
                        instance,
                        locals: row.locals.clone(),
                    };
                    assert!(matches!(
                        context.typed_operand(operand, out)?.kind,
                        OperandKind::Aggregate { moved: false, .. }
                    ));
                    let transfer =
                        aggregates::Transfer::derive(&context, assignment, out)?.unwrap();
                    let before = out.text.len();
                    transfer.emit(out)?;
                    assert!(out.text[before..].contains("moved: false"));
                    observed += 1;
                }
                assert_eq!(observed, 2);
                Ok(())
            })
        },
    )
}

#[test]
fn original_ordinary_aggregate_copy_operand_and_assignment_stay_supported() {
    let result = run_plain(LIMIT, LIMIT);
    result.0.unwrap();
    assert_eq!(result.2, FLOOR);
    assert!(result.3 > FLOOR);
}

#[test]
fn original_nominal_aggregate_copy_query_has_exact_accounting() {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    assert_eq!(
        aggregate_copy_headers(),
        h::<SemanticRustTypeKindV1>()
            + h::<&Type>()
            + h::<Option<&Type>>()
            + h::<&Operand>()
            + h::<bool>()
    );
    for mode in 0..3 {
        let run = |work, storage| {
            if mode == 2 {
                run_plain(work, storage)
            } else {
                run(work, storage, mode == 1)
            }
        };
        let measured = run(LIMIT, LIMIT);
        measured.0.unwrap();
        let exact = run(measured.1, measured.3);
        exact.0.unwrap();
        assert_eq!((exact.1, exact.2, exact.3), (measured.1, FLOOR, measured.3));
        let short = run(measured.1 - 1, measured.3);
        assert!(
            matches!(&short.0, Err(Error::Resource(Resource::Work(error))) | Err(Error::Source(fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18::Resource(Resource::Work(error))))
            if error.actual() == measured.1 && error.limit() == measured.1 - 1),
            "{short:?}"
        );
        let short = run(measured.1, measured.3 - 1);
        assert!(
            matches!(&short.0, Err(Error::Resource(Resource::Storage(error))) | Err(Error::Source(fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18::Resource(Resource::Storage(error))))
            if error.actual() == measured.3 && error.limit() == measured.3 - 1),
            "{short:?}"
        );
    }
}

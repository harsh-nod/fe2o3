mod natural_rust_call_assertions {
    use std::collections::BTreeSet;

    use fe2o3_kernel_ir as kir;
    use fe2o3_mir_model::semantic_mir_v1::*;

    fn is_u32(semantic: &AdmittedInertSemanticMirV1, ty: SemanticTypeIdV1) -> bool {
        matches!(
            semantic.types()[ty.index() as usize].shape(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32
            })
        )
    }

    fn is_shared_slice(semantic: &AdmittedInertSemanticMirV1, ty: SemanticTypeIdV1) -> bool {
        let SemanticTypeShapeV1::Pointer(pointer) = semantic.types()[ty.index() as usize].shape()
        else {
            return false;
        };
        pointer.kind() == SemanticPointerKindV1::Reference
            && pointer.mutability() == SemanticMutabilityV1::Immutable
            && pointer.metadata() == SemanticPointerMetadataV1::SliceLength
            && matches!(semantic.types()[pointer.pointee().index() as usize].shape(),
                SemanticTypeShapeV1::Slice { element } if is_u32(semantic, *element))
    }

    fn assert_pair(
        semantic: &AdmittedInertSemanticMirV1,
        ty: SemanticTypeIdV1,
    ) -> [SemanticTypeIdV1; 3] {
        let SemanticTypeShapeV1::Tuple(fields) = semantic.types()[ty.index() as usize].shape()
        else {
            panic!("mixed-ZST value must retain its source tuple");
        };
        let [first, unit, last] = fields.fields() else {
            panic!("source tuple must have three ordered fields");
        };
        assert!(is_u32(semantic, *first) && is_u32(semantic, *last));
        let unit_declaration = &semantic.types()[unit.index() as usize];
        assert!(matches!(
            unit_declaration.shape(),
            SemanticTypeShapeV1::Unit
        ));
        assert_eq!(unit_declaration.layout().size_bytes(), Some(0));
        [*first, *unit, *last]
    }

    fn local_with_role(
        function: &SemanticFunctionDeclV1,
        role: SemanticLocalRoleV1,
    ) -> SemanticLocalIdV1 {
        let mut locals = function
            .locals()
            .iter()
            .enumerate()
            .filter(|(_, local)| local.role() == role);
        let (index, _) = locals.next().expect("unique source-role local");
        assert!(locals.next().is_none());
        SemanticLocalIdV1::from_index(index as u32)
    }

    fn whole_place(operand: &SemanticOperandV1) -> &SemanticPlaceV1 {
        let (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = operand else {
            panic!("retained call operand must have a local origin");
        };
        assert!(place.projections().is_empty());
        place
    }

    type SourceUse = (usize, usize);

    fn terminator_use(function: &SemanticFunctionDeclV1, block: usize) -> SourceUse {
        (block, function.blocks()[block].statements().len())
    }

    fn definition(
        function: &SemanticFunctionDeclV1,
        local: SemanticLocalIdV1,
        (mut block, mut before): SourceUse,
    ) -> (usize, usize, &SemanticAssignmentV1) {
        let live = live_blocks(function);
        let mut seen = BTreeSet::new();
        // Fixture temporaries need only a backward walk, not a general SSA solver.
        loop {
            assert!(live.contains(&SemanticBlockIdV1::from_index(block as u32)));
            assert!(
                seen.insert(block),
                "definition trace must not cross a cycle"
            );
            for (index, statement) in function.blocks()[block].statements()[..before]
                .iter()
                .enumerate()
                .rev()
            {
                match statement.kind() {
                    SemanticStatementKindV1::Assign(assignment)
                        if assignment.destination().local() == local
                            && assignment.destination().projections().is_empty() =>
                    {
                        return (block, index, assignment);
                    }
                    SemanticStatementKindV1::StorageLive(changed)
                    | SemanticStatementKindV1::StorageDead(changed)
                        if *changed == local =>
                    {
                        panic!("definition trace crossed the local's storage lifetime")
                    }
                    SemanticStatementKindV1::Deinitialize(place)
                        if place.local() == local && place.projections().is_empty() =>
                    {
                        panic!("definition trace crossed whole-local deinitialization")
                    }
                    _ => {}
                }
            }
            assert_ne!(
                block,
                function.entry().index() as usize,
                "no definition before this use"
            );
            let mut predecessors = BTreeSet::new();
            for candidate in &live {
                function.blocks()[candidate.index() as usize]
                    .terminator()
                    .kind()
                    .try_for_each_edge::<std::convert::Infallible>(|edge| {
                        if edge.target().index() as usize == block {
                            predecessors.insert(candidate.index() as usize);
                        }
                        Ok(())
                    })
                    .unwrap();
            }
            assert_eq!(
                predecessors.len(),
                1,
                "fixture definition trace requires an unambiguous predecessor"
            );
            block = *predecessors.first().unwrap();
            if let SemanticTerminatorKindV1::Call(call) =
                function.blocks()[block].terminator().kind()
            {
                assert!(
                    !call.destination().is_some_and(|destination| {
                        destination.place().local() == local
                            && destination.place().projections().is_empty()
                    }),
                    "call-defined local is not a retained assignment origin"
                );
            }
            before = function.blocks()[block].statements().len();
        }
    }

    fn origin_local(
        function: &SemanticFunctionDeclV1,
        operand: &SemanticOperandV1,
        mut use_site: SourceUse,
    ) -> (SemanticLocalIdV1, SourceUse) {
        let mut local = whole_place(operand).local();
        let mut seen = BTreeSet::new();
        loop {
            assert!(seen.insert(local), "whole-local forwarding must be acyclic");
            if matches!(
                function.locals()[local.index() as usize].role(),
                SemanticLocalRoleV1::Argument(_)
            ) {
                return (local, use_site);
            }
            let (block, index, assignment) = definition(function, local, use_site);
            let SemanticRvalueKindV1::Use(operand) = assignment.value().kind() else {
                return (local, use_site);
            };
            let place = whole_place(operand);
            assert_eq!(place.ty(), function.locals()[local.index() as usize].ty());
            local = place.local();
            use_site = (block, index);
        }
    }

    fn live_blocks(function: &SemanticFunctionDeclV1) -> BTreeSet<SemanticBlockIdV1> {
        let mut live = BTreeSet::new();
        let mut pending = vec![function.entry()];
        while let Some(block) = pending.pop() {
            if live.insert(block) {
                function.blocks()[block.index() as usize]
                    .terminator()
                    .kind()
                    .try_for_each_edge::<std::convert::Infallible>(|edge| {
                        pending.push(edge.target());
                        Ok(())
                    })
                    .unwrap();
            }
        }
        live
    }

    fn calls<'a>(
        semantic: &AdmittedInertSemanticMirV1,
        function: &'a SemanticFunctionDeclV1,
    ) -> Vec<(usize, &'a SemanticDirectCallV1, SemanticFunctionIdV1)> {
        live_blocks(function)
            .into_iter()
            .filter_map(|block| {
                let SemanticTerminatorKindV1::Call(call) = function.blocks()
                    [block.index() as usize]
                    .terminator()
                    .kind()
                else {
                    return None;
                };
                let SemanticCallableDeclV1::Defined { function: callee } =
                    &semantic.callables()[call.callee().index() as usize]
                else {
                    return None;
                };
                let abi = semantic.functions()[callee.index() as usize].abi();
                assert_eq!(call.arguments().len(), abi.source_input_types().len());
                for (operand, ty) in call.arguments().iter().zip(abi.source_input_types()) {
                    assert_eq!(operand.ty(), *ty);
                }
                assert_eq!(
                    call.destination().unwrap().place().ty(),
                    abi.source_output_type()
                );
                Some((block.index() as usize, call, *callee))
            })
            .collect()
    }

    fn assert_reference(
        semantic: &AdmittedInertSemanticMirV1,
        ty: SemanticTypeIdV1,
        environment: SemanticTypeIdV1,
        mutability: SemanticMutabilityV1,
    ) {
        let SemanticTypeShapeV1::Pointer(pointer) = semantic.types()[ty.index() as usize].shape()
        else {
            panic!("closure receiver must remain a reference");
        };
        assert_eq!(pointer.kind(), SemanticPointerKindV1::Reference);
        assert_eq!(pointer.pointee(), environment);
        assert_eq!(pointer.mutability(), mutability);
        assert_eq!(pointer.metadata(), SemanticPointerMetadataV1::None);
    }

    fn assert_shim_borrow(
        semantic: &AdmittedInertSemanticMirV1,
        shim: &SemanticFunctionDeclV1,
        call_block: usize,
        call: &SemanticDirectCallV1,
        environment: SemanticTypeIdV1,
        shared: bool,
    ) {
        let mut receiver = whole_place(&call.arguments()[0]);
        let mut before = shim.blocks()[call_block].statements().len();
        if shared {
            assert_reference(
                semantic,
                receiver.ty(),
                environment,
                SemanticMutabilityV1::Immutable,
            );
            assert_eq!(
                shim.locals()[receiver.local().index() as usize].role(),
                SemanticLocalRoleV1::Temporary
            );
            let (block, index, assignment) =
                definition(shim, receiver.local(), (call_block, before));
            assert_eq!(block, call_block);
            assert!(index < before);
            assert_eq!(assignment.value().result_type(), receiver.ty());
            let SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place,
            } = assignment.value().kind()
            else {
                panic!("Fn shim must materialize a shared Borrow, not retag its operand");
            };
            let [projection] = place.projections() else {
                panic!("shared reborrow must dereference the existing mutable receiver");
            };
            assert_eq!(projection.kind(), SemanticProjectionKindV1::Dereference);
            assert_eq!(projection.result_type(), environment);
            assert_eq!(place.ty(), environment);
            let (block, mutable_index, mutable) = definition(shim, place.local(), (block, index));
            assert_eq!(block, call_block);
            assert!(mutable_index < index);
            receiver = mutable.destination();
            before = index;
        }
        assert_reference(
            semantic,
            receiver.ty(),
            environment,
            SemanticMutabilityV1::Mutable,
        );
        assert_eq!(
            shim.locals()[receiver.local().index() as usize].role(),
            SemanticLocalRoleV1::Temporary
        );
        let (block, index, assignment) = definition(shim, receiver.local(), (call_block, before));
        assert_eq!(block, call_block);
        assert!(index < before);
        assert_eq!(assignment.value().result_type(), receiver.ty());
        let SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Mutable,
            place,
        } = assignment.value().kind()
        else {
            panic!("once-shim receiver must borrow its own environment");
        };
        assert!(place.projections().is_empty());
        assert_eq!(place.ty(), environment);
        assert_eq!(
            place.local(),
            local_with_role(shim, SemanticLocalRoleV1::Argument(0))
        );
        assert_eq!(
            origin_local(shim, &call.arguments()[1], terminator_use(shim, call_block)).0,
            local_with_role(shim, SemanticLocalRoleV1::Argument(1))
        );
        assert_eq!(
            call.destination().unwrap().place().local(),
            local_with_role(shim, SemanticLocalRoleV1::Return)
        );
        assert!(call.destination().unwrap().place().projections().is_empty());
    }

    pub(super) fn source(semantic: &AdmittedInertSemanticMirV1) {
        let roots = semantic
            .functions()
            .iter()
            .enumerate()
            .filter(|(_, function)| function.role() == SemanticFunctionRoleV1::KernelRoot)
            .map(|(index, _)| SemanticFunctionIdV1::from_index(index as u32))
            .collect::<Vec<_>>();
        let [root] = roots.as_slice() else {
            panic!("one source kernel root")
        };
        let selected = semantic.select_kernel_body_for_root_v1(*root).unwrap();
        let root = &semantic.functions()[selected.body().index() as usize];
        let inputs = root
            .locals()
            .iter()
            .enumerate()
            .filter_map(|(index, local)| {
                (matches!(local.role(), SemanticLocalRoleV1::Argument(_))
                    && is_shared_slice(semantic, local.ty()))
                .then_some(SemanticLocalIdV1::from_index(index as u32))
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(inputs.len(), 2);
        let branches = calls(semantic, root);
        assert_eq!(
            branches.len(),
            2,
            "both generic bound calls must survive extraction"
        );
        let mut functions = BTreeSet::new();
        let mut kinds = BTreeSet::new();
        let mut scalar_captures = Vec::new();
        for (root_block, root_call, mut function_id) in branches {
            let environment = root_call.arguments()[0].ty();
            let SemanticTypeShapeV1::Aggregate(fields) =
                semantic.types()[environment.index() as usize].shape()
            else {
                panic!("generic helper must receive the actual owned closure environment");
            };
            let (capture, capture_use) = origin_local(
                root,
                &root_call.arguments()[0],
                terminator_use(root, root_block),
            );
            let (capture_block, capture_index, assignment) = definition(root, capture, capture_use);
            assert!(
                live_blocks(root).contains(&SemanticBlockIdV1::from_index(capture_block as u32))
            );
            assert_eq!(assignment.value().result_type(), environment);
            let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind() else {
                panic!("closure capture construction must be retained");
            };
            assert_eq!(aggregate.kind(), &SemanticAggregateKindV1::Aggregate);
            assert_eq!(aggregate.operands().len(), fields.fields().len());
            let mut slices = BTreeSet::new();
            let mut scalars = Vec::new();
            for (operand, ty) in aggregate.operands().iter().zip(fields.fields()) {
                assert_eq!(operand.ty(), *ty);
                let origin = origin_local(root, operand, (capture_block, capture_index)).0;
                assert_eq!(root.locals()[origin.index() as usize].ty(), *ty);
                assert!(matches!(
                    root.locals()[origin.index() as usize].role(),
                    SemanticLocalRoleV1::Argument(_)
                ));
                if is_shared_slice(semantic, *ty) {
                    assert!(
                        slices.insert(origin),
                        "distinct slice captures must stay distinct"
                    );
                } else {
                    assert!(
                        is_u32(semantic, *ty),
                        "only the scalar seed and shared slices are captured"
                    );
                    scalars.push(origin);
                }
            }
            assert_eq!(scalars.len(), 1);
            scalar_captures.push(scalars[0]);
            let shared = !slices.is_empty();
            assert!(
                kinds.insert(shared),
                "one natural Fn and one natural FnMut branch"
            );
            if shared {
                assert_eq!(slices, inputs);
            }
            assert_eq!(fields.fields().len(), if shared { 3 } else { 1 });

            // Follow actual defined edges; canonical function and local ranks are opaque.
            for stage in 0..2 {
                assert!(functions.insert(function_id));
                let function = &semantic.functions()[function_id.index() as usize];
                assert_eq!(function.role(), SemanticFunctionRoleV1::InternalHelper);
                assert_eq!(function.abi().extern_abi(), SemanticExternAbiV1::Rust);
                let arguments = function.abi().source_input_types();
                assert_eq!(arguments.len(), 3);
                assert_eq!(arguments[0], environment);
                assert!(is_u32(semantic, arguments[1]) && is_u32(semantic, arguments[2]));
                assert!(
                    function
                        .abi()
                        .source_argument_ownership()
                        .iter()
                        .all(|ownership| *ownership == SemanticSourceArgumentOwnershipV1::ByValue)
                );
                assert_pair(semantic, function.abi().source_output_type());
                let outgoing = calls(semantic, function);
                let [(call_block, call, callee)] = outgoing.as_slice() else {
                    panic!("generic forwarding edge must survive")
                };
                let destination = call.destination().unwrap().place();
                assert!(destination.projections().is_empty());
                assert_eq!(
                    destination.local(),
                    local_with_role(function, SemanticLocalRoleV1::Return)
                );
                assert_eq!(
                    origin_local(
                        function,
                        &call.arguments()[0],
                        terminator_use(function, *call_block)
                    )
                    .0,
                    local_with_role(function, SemanticLocalRoleV1::Argument(0))
                );
                if stage == 0 {
                    for argument in 1..3 {
                        assert_eq!(
                            origin_local(
                                function,
                                &call.arguments()[argument],
                                terminator_use(function, *call_block)
                            )
                            .0,
                            local_with_role(
                                function,
                                SemanticLocalRoleV1::Argument(argument as u32)
                            )
                        );
                    }
                } else {
                    assert_eq!(call.arguments().len(), 2);
                    let fields = assert_pair(semantic, call.arguments()[1].ty());
                    let (tuple, tuple_use) = origin_local(
                        function,
                        &call.arguments()[1],
                        terminator_use(function, *call_block),
                    );
                    let (tuple_block, tuple_index, assignment) =
                        definition(function, tuple, tuple_use);
                    let SemanticRvalueKindV1::Aggregate(tuple) = assignment.value().kind() else {
                        panic!("mixed-ZST call tuple")
                    };
                    assert_eq!(tuple.kind(), &SemanticAggregateKindV1::Tuple);
                    assert_eq!(
                        tuple
                            .operands()
                            .iter()
                            .map(SemanticOperandV1::ty)
                            .collect::<Vec<_>>(),
                        fields
                    );
                    assert_eq!(
                        origin_local(function, &tuple.operands()[0], (tuple_block, tuple_index)).0,
                        local_with_role(function, SemanticLocalRoleV1::Argument(1))
                    );
                    assert_eq!(
                        origin_local(function, &tuple.operands()[2], (tuple_block, tuple_index)).0,
                        local_with_role(function, SemanticLocalRoleV1::Argument(2))
                    );
                }
                function_id = *callee;
            }

            assert!(functions.insert(function_id));
            let shim = &semantic.functions()[function_id.index() as usize];
            assert_eq!(shim.role(), SemanticFunctionRoleV1::InternalHelper);
            assert_eq!(shim.abi().extern_abi(), SemanticExternAbiV1::RustCall);
            assert_eq!(shim.abi().source_input_types().len(), 2);
            assert_eq!(shim.abi().source_input_types()[0], environment);
            assert_eq!(
                shim.abi().source_argument_ownership(),
                [SemanticSourceArgumentOwnershipV1::ByValue; 2]
            );
            assert_pair(semantic, shim.abi().source_input_types()[1]);
            assert_pair(semantic, shim.abi().source_output_type());
            let outgoing = calls(semantic, shim);
            let [(block, call, body_id)] = outgoing.as_slice() else {
                panic!("once-shim must retain its closure body call")
            };
            assert_shim_borrow(semantic, shim, *block, call, environment, shared);
            assert!(functions.insert(*body_id));
            let body = &semantic.functions()[body_id.index() as usize];
            assert_eq!(body.role(), SemanticFunctionRoleV1::InternalHelper);
            assert_eq!(body.abi().extern_abi(), SemanticExternAbiV1::RustCall);
            assert_eq!(
                body.abi().source_input_types()[1],
                shim.abi().source_input_types()[1]
            );
            assert_eq!(
                body.abi().source_output_type(),
                shim.abi().source_output_type()
            );
            assert_eq!(
                body.abi().source_argument_ownership(),
                [
                    if shared {
                        SemanticSourceArgumentOwnershipV1::SharedBorrow
                    } else {
                        SemanticSourceArgumentOwnershipV1::UniqueBorrow
                    },
                    SemanticSourceArgumentOwnershipV1::ByValue,
                ]
            );
            assert!(
                calls(semantic, body).is_empty(),
                "closure body is the leaf of the retained chain"
            );
            assert!(live_blocks(body).into_iter().any(|block| matches!(
                body.blocks()[block.index() as usize].terminator().kind(),
                SemanticTerminatorKindV1::Return
            )));
            let SemanticTypeShapeV1::Tuple(fields) =
                semantic.types()[body.abi().source_input_types()[1].index() as usize].shape()
            else {
                unreachable!()
            };
            for (field, ty) in fields.fields().iter().enumerate() {
                let local = local_with_role(
                    body,
                    SemanticLocalRoleV1::RustCallTupleField {
                        argument: 1,
                        field: field as u32,
                    },
                );
                assert_eq!(body.locals()[local.index() as usize].ty(), *ty);
            }
            let returned = local_with_role(body, SemanticLocalRoleV1::Return);
            for return_block in live_blocks(body).into_iter().filter(|block| {
                matches!(
                    body.blocks()[block.index() as usize].terminator().kind(),
                    SemanticTerminatorKindV1::Return
                )
            }) {
                let (_, _, returned) = definition(
                    body,
                    returned,
                    terminator_use(body, return_block.index() as usize),
                );
                let SemanticRvalueKindV1::Aggregate(result) = returned.value().kind() else {
                    panic!("closure must construct its ordered tuple result")
                };
                assert_eq!(result.kind(), &SemanticAggregateKindV1::Tuple);
                assert_eq!(
                    returned.value().result_type(),
                    body.abi().source_output_type()
                );
                assert_eq!(
                    result
                        .operands()
                        .iter()
                        .map(SemanticOperandV1::ty)
                        .collect::<Vec<_>>(),
                    assert_pair(semantic, body.abi().source_output_type())
                );
            }
            let receiver = local_with_role(body, SemanticLocalRoleV1::Argument(0));
            let writes = live_blocks(body)
                .into_iter()
                .flat_map(|block| body.blocks()[block.index() as usize].statements())
                .filter(|statement| {
                    let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                        return false;
                    };
                    let destination = assignment.destination();
                    let [deref, field] = destination.projections() else {
                        return false;
                    };
                    destination.local() == receiver
                        && deref.kind() == SemanticProjectionKindV1::Dereference
                        && deref.result_type() == environment
                        && matches!(field.kind(), SemanticProjectionKindV1::Field(_))
                        && is_u32(semantic, destination.ty())
                })
                .count();
            assert_eq!(
                writes != 0,
                !shared,
                "FnMut must mutate its captured state through its receiver"
            );
        }
        assert_eq!(kinds, BTreeSet::from([false, true]));
        assert_eq!(scalar_captures[0], scalar_captures[1]);
        assert_eq!(
            functions.len(),
            8,
            "two complete, distinct four-helper chains"
        );
    }

    fn kir_blocks(function: &kir::Function) -> Vec<&kir::BasicBlock> {
        let body = function.body.as_ref().expect("retained definition");
        let mut pending = vec![body.blocks.first().expect("entry block").id];
        let mut seen = BTreeSet::new();
        let mut blocks = Vec::new();
        while let Some(id) = pending.pop() {
            if seen.insert(id) {
                let block = body.blocks.iter().find(|block| block.id == id).unwrap();
                pending.extend(block.terminator.as_ref().unwrap().successors());
                blocks.push(block);
            }
        }
        blocks
    }

    fn kir_calls(function: &kir::Function) -> Vec<&kir::Operation> {
        kir_blocks(function)
            .into_iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| matches!(operation.kind, kir::OperationKind::Call { .. }))
            .collect()
    }

    pub(super) fn lowered(module: &kir::Module) {
        let [kernel] = module.kernels.as_slice() else {
            panic!("one lowered kernel")
        };
        let entry = module.function(&kernel.entry).unwrap();
        assert_eq!(entry.role, kir::FunctionRole::KernelEntry);
        let branches = kir_calls(entry);
        assert_eq!(branches.len(), 2);
        let pair = vec![kir::Type::Scalar(kir::ScalarType::U32); 2];
        let mut seen = BTreeSet::new();
        let mut metadata_counts = Vec::new();
        let mut load_counts = Vec::new();
        for mut operation in branches {
            for depth in 0..4 {
                let kir::OperationKind::Call { callee, .. } = &operation.kind else {
                    unreachable!()
                };
                assert!(
                    seen.insert(callee.clone()),
                    "both retained chains must be acyclic and distinct"
                );
                assert_eq!(
                    operation
                        .results
                        .iter()
                        .map(|value| &value.ty)
                        .collect::<Vec<_>>(),
                    pair.iter().collect::<Vec<_>>()
                );
                let function = module.function(callee).expect("defined call target");
                assert_eq!(function.role, kir::FunctionRole::InternalHelper);
                assert_eq!(function.signature.results, pair);
                let blocks = kir_blocks(function);
                let returns = blocks
                    .iter()
                    .filter_map(|block| match block.terminator.as_ref().unwrap() {
                        kir::Terminator::Return { values } => Some(values),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                assert!(!returns.is_empty());
                assert!(returns.iter().all(|values| values.len() == 2));
                let outgoing = kir_calls(function);
                if depth == 3 {
                    assert!(outgoing.is_empty(), "closure body must remain the leaf");
                    metadata_counts.push(
                        blocks
                            .iter()
                            .flat_map(|block| &block.operations)
                            .filter(|operation| {
                                matches!(operation.kind, kir::OperationKind::SliceLength { .. })
                            })
                            .count(),
                    );
                    load_counts.push(
                        blocks
                            .iter()
                            .flat_map(|block| &block.operations)
                            .filter(|operation| {
                                matches!(
                                    operation.kind,
                                    kir::OperationKind::Load { .. }
                                        | kir::OperationKind::GuardedLoad { .. }
                                )
                            })
                            .count(),
                    );
                } else {
                    let [next] = outgoing.as_slice() else {
                        panic!("generic or shim call was lost")
                    };
                    operation = *next;
                }
            }
        }
        metadata_counts.sort_unstable();
        assert_eq!(metadata_counts[0], 0);
        assert!(
            metadata_counts[1] >= 2,
            "both captured slice bounds must survive"
        );
        // Private state may also load; both slice descriptors must feed real data reads.
        assert!(load_counts.iter().any(|count| *count >= 2));
        assert_eq!(seen.len(), 8);
    }

    fn source_root(semantic: &AdmittedInertSemanticMirV1) -> &SemanticFunctionDeclV1 {
        let roots = semantic
            .functions()
            .iter()
            .enumerate()
            .filter(|(_, function)| function.role() == SemanticFunctionRoleV1::KernelRoot)
            .map(|(index, _)| SemanticFunctionIdV1::from_index(index as u32))
            .collect::<Vec<_>>();
        let [root] = roots.as_slice() else {
            panic!("exactly one source kernel root")
        };
        let selected = semantic.select_kernel_body_for_root_v1(*root).unwrap();
        &semantic.functions()[selected.body().index() as usize]
    }

    fn borrowed_owner(
        semantic: &AdmittedInertSemanticMirV1,
        function: &SemanticFunctionDeclV1,
        operand: &SemanticOperandV1,
        kind: SemanticBorrowKindV1,
        mut use_site: SourceUse,
    ) -> (SemanticLocalIdV1, SourceUse) {
        let operand = whole_place(operand);
        let mut local = operand.local();
        let mut ty = operand.ty();
        let mut seen = BTreeSet::new();
        loop {
            assert!(
                seen.insert(local),
                "reference forwarding and reborrows must be acyclic"
            );
            assert_eq!(function.locals()[local.index() as usize].ty(), ty);
            let SemanticTypeShapeV1::Pointer(pointer) =
                semantic.types()[ty.index() as usize].shape()
            else {
                panic!("borrowed owner trace requires a typed reference")
            };
            assert_eq!(pointer.kind(), SemanticPointerKindV1::Reference);
            assert_eq!(pointer.metadata(), SemanticPointerMetadataV1::None);
            if seen.len() == 1 {
                assert_eq!(
                    pointer.mutability(),
                    match kind {
                        SemanticBorrowKindV1::Shared => SemanticMutabilityV1::Immutable,
                        SemanticBorrowKindV1::Mutable => SemanticMutabilityV1::Mutable,
                        SemanticBorrowKindV1::Fake => panic!("fake borrow is not an owner origin"),
                    }
                );
            }
            let (block, index, assignment) = definition(function, local, use_site);
            assert_eq!(assignment.destination().ty(), ty);
            assert_eq!(assignment.value().result_type(), ty);
            use_site = (block, index);
            match assignment.value().kind() {
                SemanticRvalueKindV1::Use(operand) => {
                    let place = whole_place(operand);
                    assert_eq!(
                        place.ty(),
                        ty,
                        "whole-reference forwarding preserves its type"
                    );
                    local = place.local();
                }
                SemanticRvalueKindV1::Borrow {
                    kind: actual,
                    place,
                } => {
                    assert_eq!(
                        *actual,
                        match pointer.mutability() {
                            SemanticMutabilityV1::Immutable => SemanticBorrowKindV1::Shared,
                            SemanticMutabilityV1::Mutable => SemanticBorrowKindV1::Mutable,
                        }
                    );
                    assert_eq!(place.ty(), pointer.pointee());
                    if place.projections().is_empty() {
                        assert_eq!(
                            function.locals()[place.local().index() as usize].ty(),
                            pointer.pointee()
                        );
                        return (place.local(), use_site);
                    }
                    let [projection] = place.projections() else {
                        panic!("only exact single-dereference reborrow steps preserve this owner")
                    };
                    assert_eq!(projection.kind(), SemanticProjectionKindV1::Dereference);
                    assert_eq!(projection.result_type(), pointer.pointee());
                    let parent_ty = function.locals()[place.local().index() as usize].ty();
                    let SemanticTypeShapeV1::Pointer(parent) =
                        semantic.types()[parent_ty.index() as usize].shape()
                    else {
                        panic!("reborrow base must be a typed reference")
                    };
                    assert_eq!(parent.kind(), SemanticPointerKindV1::Reference);
                    assert_eq!(parent.pointee(), pointer.pointee());
                    assert_eq!(parent.metadata(), pointer.metadata());
                    assert_eq!(parent.address_space(), pointer.address_space());
                    assert_eq!(parent.pointer_width_bits(), pointer.pointer_width_bits());
                    if pointer.mutability() == SemanticMutabilityV1::Mutable {
                        assert_eq!(
                            parent.mutability(),
                            SemanticMutabilityV1::Mutable,
                            "reborrow cannot widen access"
                        );
                    }
                    local = place.local();
                    ty = parent_ty;
                }
                _ => panic!(
                    "helper receiver must originate in typed forwarding or an actual source borrow"
                ),
            }
        }
    }

    pub(super) fn borrowed_source(semantic: &AdmittedInertSemanticMirV1) {
        let root = source_root(semantic);
        let root_calls = calls(semantic, root);
        assert_eq!(
            root_calls.len(),
            2,
            "both FnMut calls must survive extraction"
        );
        assert_eq!(root_calls[0].2, root_calls[1].2, "same instantiated helper");
        let borrowed = root_calls
            .iter()
            .map(|(block, call, _)| {
                borrowed_owner(
                    semantic,
                    root,
                    &call.arguments()[0],
                    SemanticBorrowKindV1::Mutable,
                    terminator_use(root, *block),
                )
            })
            .collect::<Vec<_>>();
        let owners = borrowed
            .iter()
            .map(|(owner, _)| *owner)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            owners.len(),
            1,
            "both calls borrow the same closure environment"
        );
        let (capture_block, capture_index, capture) =
            definition(root, borrowed[0].0, borrowed[0].1);
        for (owner, use_site) in &borrowed[1..] {
            let (block, index, _) = definition(root, *owner, *use_site);
            assert_eq!(
                (block, index),
                (capture_block, capture_index),
                "both borrows retain the same closure initialization"
            );
        }
        let SemanticRvalueKindV1::Aggregate(capture) = capture.value().kind() else {
            panic!("retained closure capture construction")
        };
        let [outer] = capture.operands() else {
            panic!("one captured outer mutable scalar reference")
        };
        let state = borrowed_owner(
            semantic,
            root,
            outer,
            SemanticBorrowKindV1::Mutable,
            (capture_block, capture_index),
        )
        .0;
        assert!(is_u32(semantic, root.locals()[state.index() as usize].ty()));

        let helper = &semantic.functions()[root_calls[0].2.index() as usize];
        assert_eq!(helper.abi().extern_abi(), SemanticExternAbiV1::Rust);
        assert_eq!(helper.abi().source_input_types().len(), 3);
        assert_eq!(
            helper.abi().source_argument_ownership()[0],
            SemanticSourceArgumentOwnershipV1::UniqueBorrow
        );
        assert_pair(semantic, helper.abi().source_output_type());
        let helper_calls = calls(semantic, helper);
        let [(_, call, body)] = helper_calls.as_slice() else {
            panic!("borrowed generic helper must retain its FnMut body call")
        };
        let body = &semantic.functions()[body.index() as usize];
        assert_eq!(body.abi().extern_abi(), SemanticExternAbiV1::RustCall);
        assert_eq!(
            body.abi().source_argument_ownership(),
            [
                SemanticSourceArgumentOwnershipV1::UniqueBorrow,
                SemanticSourceArgumentOwnershipV1::ByValue,
            ]
        );
        assert_pair(semantic, call.arguments()[1].ty());
        assert_pair(semantic, body.abi().source_output_type());
        assert!(calls(semantic, body).is_empty());
    }

    pub(super) fn env_source(semantic: &AdmittedInertSemanticMirV1) {
        let root = source_root(semantic);
        let root_calls = calls(semantic, root);
        assert_eq!(
            root_calls.len(),
            4,
            "two shared and two mutable structural helper calls"
        );
        let mut owners = BTreeSet::new();
        let mut shared = Vec::new();
        let mut mutable = Vec::new();
        for (call_block, call, callee) in root_calls {
            let helper = &semantic.functions()[callee.index() as usize];
            assert_eq!(helper.abi().extern_abi(), SemanticExternAbiV1::Rust);
            assert_pair(semantic, helper.abi().source_output_type());
            assert!(calls(semantic, helper).is_empty());
            let shared_call = helper.abi().source_argument_ownership()[0]
                == SemanticSourceArgumentOwnershipV1::SharedBorrow;
            let kind = if shared_call {
                SemanticBorrowKindV1::Shared
            } else {
                SemanticBorrowKindV1::Mutable
            };
            owners.insert(
                borrowed_owner(
                    semantic,
                    root,
                    &call.arguments()[0],
                    kind,
                    terminator_use(root, call_block),
                )
                .0,
            );
            let SemanticTypeShapeV1::Pointer(pointer) =
                semantic.types()[call.arguments()[0].ty().index() as usize].shape()
            else {
                panic!("ordinary Env receiver remains a source reference")
            };
            let SemanticTypeShapeV1::Aggregate(fields) =
                semantic.types()[pointer.pointee().index() as usize].shape()
            else {
                panic!("ordinary structural Env pointee")
            };
            assert_eq!(fields.fields().len(), 4);
            assert_eq!(
                fields
                    .fields()
                    .iter()
                    .filter(|ty| is_shared_slice(semantic, **ty))
                    .count(),
                2
            );
            if shared_call {
                assert_eq!(call.arguments().len(), 4);
                owners.insert(
                    borrowed_owner(
                        semantic,
                        root,
                        &call.arguments()[1],
                        SemanticBorrowKindV1::Shared,
                        terminator_use(root, call_block),
                    )
                    .0,
                );
                shared.push(callee);
            } else {
                assert_eq!(
                    helper.abi().source_argument_ownership()[0],
                    SemanticSourceArgumentOwnershipV1::UniqueBorrow
                );
                assert_eq!(call.arguments().len(), 2);
                assert_pair(semantic, call.arguments()[1].ty());
                mutable.push(callee);
            }
        }
        assert_eq!(
            owners.len(),
            1,
            "all shared aliases and mutable calls refer to the same Env"
        );
        assert_eq!(shared.len(), 2);
        assert_eq!(mutable.len(), 2);
        assert_eq!(shared[0], shared[1]);
        assert_eq!(mutable[0], mutable[1]);
        assert_ne!(shared[0], mutable[0]);
    }

    pub(super) fn borrowed_lowered(module: &kir::Module, structural_env: bool) {
        let [kernel] = module.kernels.as_slice() else {
            panic!("one lowered kernel")
        };
        let entry = module.function(&kernel.entry).unwrap();
        let root_calls = kir_calls(entry);
        assert_eq!(root_calls.len(), if structural_env { 4 } else { 2 });
        let mut targets = std::collections::BTreeMap::new();
        let mut state_slots = BTreeSet::new();
        let effects = kir::analyze_interprocedural_effects_v1(module).unwrap();
        let mut mutable_calls = 0;
        let mut shared_calls = 0;
        for call in root_calls {
            let kir::OperationKind::Call { callee, arguments } = &call.kind else {
                unreachable!()
            };
            *targets.entry(callee.clone()).or_insert(0) += 1;
            let function = module.function(callee).unwrap();
            assert_eq!(function.role, kir::FunctionRole::InternalHelper);
            assert_eq!(
                function.signature.results,
                vec![kir::Type::Scalar(kir::ScalarType::U32); 2]
            );
            assert_eq!(
                call.results.len(),
                2,
                "mixed-ZST tuple has exactly two physical results"
            );
            let pointers = function
                .signature
                .parameters
                .iter()
                .enumerate()
                .filter_map(|(index, ty)| {
                    let kir::Type::Pointer(pointer) = ty else {
                        return None;
                    };
                    (pointer.address_space == kir::AddressSpace::Private
                        && *pointer.pointee == kir::Type::Scalar(kir::ScalarType::U32))
                    .then_some((index, pointer))
                })
                .collect::<Vec<_>>();
            assert!(
                !pointers.is_empty(),
                "borrowed state must be an actual private address"
            );
            let writable = pointers
                .iter()
                .any(|(_, pointer)| pointer.access == kir::AccessMode::ReadWrite);
            if writable {
                mutable_calls += 1;
                assert_eq!(pointers.len(), 1, "one persistent scalar state slot");
            } else {
                shared_calls += 1;
                assert!(structural_env);
                assert_eq!(
                    pointers.len(),
                    2,
                    "both shared aliases retain the state address"
                );
                assert!(
                    pointers
                        .iter()
                        .all(|(_, pointer)| pointer.access == kir::AccessMode::ReadOnly)
                );
            }
            for (index, _) in pointers {
                state_slots.insert(private_origin(entry, arguments[index]));
            }
            let decision = effects
                .function(callee)
                .expect("retained callee effect summary");
            assert!(
                decision.is_complete(),
                "helper effects must be complete: {decision:?}"
            );
            assert!(decision.summary().reads(kir::AddressSpace::Private));
            assert_eq!(
                decision.summary().writes(kir::AddressSpace::Private),
                writable
            );
            assert!(!decision.summary().writes(kir::AddressSpace::Global));
            if !writable {
                assert!(decision.summary().reads(kir::AddressSpace::Global));
            }
        }
        assert_eq!(targets.len(), if structural_env { 2 } else { 1 });
        assert!(targets.values().all(|count| *count == 2));
        assert_eq!(mutable_calls, 2);
        assert_eq!(shared_calls, if structural_env { 2 } else { 0 });
        assert_eq!(
            state_slots.len(),
            1,
            "repeated calls and shared aliases use the original state slot"
        );
        let state = *state_slots.first().unwrap();
        assert!(
            kir_blocks(entry)
                .iter()
                .flat_map(|block| &block.operations)
                .any(|operation| {
                    matches!(operation.kind, kir::OperationKind::Load { pointer, ref access }
                if access.address_space == kir::AddressSpace::Private
                    && private_origin(entry, pointer) == state)
                }),
            "caller observation must reload the same private state slot"
        );
    }

    fn private_origin(function: &kir::Function, mut value: kir::ValueId) -> kir::ValueId {
        let mut seen = BTreeSet::new();
        loop {
            assert!(
                seen.insert(value),
                "private pointer derivation must be acyclic"
            );
            let operation = kir_blocks(function)
                .into_iter()
                .flat_map(|block| &block.operations)
                .find(|operation| operation.results.iter().any(|result| result.id == value))
                .expect("private state address has a retained definition");
            match &operation.kind {
                kir::OperationKind::Alloca {
                    element,
                    address_space,
                    ..
                } => {
                    assert_eq!(*element, kir::Type::Scalar(kir::ScalarType::U32));
                    assert_eq!(*address_space, kir::AddressSpace::Private);
                    return value;
                }
                kir::OperationKind::Cast {
                    kind: kir::CastKind::RestrictPointerAccess,
                    value: parent,
                    ..
                } => {
                    value = *parent;
                }
                other => panic!(
                    "state must retain its allocation through access restrictions: {other:?}"
                ),
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum NaturalRustCallCase {
    Owned,
    Borrowed,
    Env,
}

impl NaturalRustCallCase {
    fn feature(self) -> &'static str {
        match self {
            Self::Owned => "rust_call_natural_shims",
            Self::Borrowed => "rust_call_natural_borrowed",
            Self::Env => "rust_call_natural_env",
        }
    }

    fn output_count(self) -> usize {
        match self {
            Self::Owned => 4,
            Self::Borrowed => 5,
            Self::Env => 9,
        }
    }

    fn has_data(self) -> bool {
        !matches!(self, Self::Borrowed)
    }
}

fn natural_rust_call_expected(
    case: NaturalRustCallCase,
    scalars: [u32; 3],
    input: &[u32],
    other: &[u32],
    invocation: usize,
) -> Vec<u32> {
    let [seed, lhs, rhs] = scalars;
    let a = lhs.wrapping_add((invocation % 64) as u32);
    let b = rhs.wrapping_sub((invocation / 64) as u32);
    let left = input.get(a as usize).copied().unwrap_or(0);
    let right = other.get(a as usize).copied().unwrap_or(0);
    // Host-only arithmetic: neither the fixture nor the extracted IR supplies the oracle.
    let first = seed.wrapping_add(a).wrapping_sub(b);
    let second = first
        .wrapping_add(b.wrapping_add(7))
        .wrapping_sub(a ^ 0x1357_9bdf);
    let observed = second.wrapping_add(seed ^ 0x2468_ace0);
    match case {
        NaturalRustCallCase::Owned => vec![
            (seed ^ a).wrapping_add(left),
            b.wrapping_sub(right),
            seed,
            first,
        ],
        NaturalRustCallCase::Borrowed => vec![seed, first, first, second, observed],
        NaturalRustCallCase::Env => vec![
            (seed ^ a).wrapping_add(left),
            b.wrapping_sub(seed).wrapping_sub(right),
            seed,
            first,
            first,
            second,
            observed,
            (second ^ b).wrapping_add(input.get(b as usize).copied().unwrap_or(0)),
            a.wrapping_sub(second)
                .wrapping_sub(other.get(b as usize).copied().unwrap_or(0)),
        ],
    }
}

fn run_natural_rust_call_case(case: NaturalRustCallCase) {
    use fe2o3_mir_model::semantic_mir_v1::{AdmittedInertSemanticMirV1, SemanticMirLimitsV1};

    const CANARY: u32 = 0xa5c3_7e19;
    const PREFIX: usize = 2;
    const SUFFIX: usize = 3;
    let feature = case.feature();
    let target = ScratchTarget::new();
    let mut executions = 0_usize;
    for architecture in ["gfx942", "gfx950"] {
        let bundle_path = target
            .path()
            .join(format!("{feature}-{architecture}.fe2sim"));
        let exported = output(
            simulation_export_command_for_feature(
                architecture,
                &bundle_path,
                &target.path().join(architecture),
                Some(6),
                feature,
            ),
            "export ordinary natural Rust calls as Bundle V6",
        );
        assert!(
            exported.status.success(),
            "{feature} {architecture}: source extraction/export failed before simulation:\n{}",
            exported.stderr
        );
        let bundle = fe2o3_kernel_ir::VerifiedSimulationBundleV6::from_canonical_bytes(
            std::fs::read(&bundle_path).expect("successful export must produce its bundle"),
        )
        .unwrap_or_else(|error| panic!("{feature} {architecture}: V6 decode failed: {error:?}"));
        assert_eq!(bundle.target(), format!("{architecture}:xnack-"));
        assert_eq!(bundle.production_kir_identity().version(), 11);
        assert!(!bundle.authenticates_compiler_execution());
        assert!(!bundle.grants_compiler_authority());
        assert!(!bundle.grants_proof_authority());
        assert!(!bundle.grants_artifact_authority());
        assert!(!bundle.grants_hardware_authority());
        assert!(!bundle.grants_load_authority());
        assert!(!bundle.grants_launch_authority());
        let semantic = AdmittedInertSemanticMirV1::decode_current_production_canonical(
            bundle.semantic_mir(),
            SemanticMirLimitsV1::default(),
        )
        .unwrap_or_else(|error| {
            panic!("{feature} {architecture}: source MIR decode failed: {error:?}")
        });
        let (_, module) =
            fe2o3_kernel_ir::VerifiedCanonicalKernelIrV11::from_canonical_bytes_with_module(
                bundle.canonical_kir_v11().to_vec(),
            )
            .unwrap_or_else(|error| {
                panic!("{feature} {architecture}: canonical KIR11 decode failed: {error:?}")
            });
        assert_eq!(module.kernels.len(), 1);
        assert_eq!(module.kernels[0].id.as_str(), feature);
        match case {
            NaturalRustCallCase::Owned => {
                natural_rust_call_assertions::source(&semantic);
                natural_rust_call_assertions::lowered(&module);
            }
            NaturalRustCallCase::Borrowed => {
                natural_rust_call_assertions::borrowed_source(&semantic);
                natural_rust_call_assertions::borrowed_lowered(&module, false);
            }
            NaturalRustCallCase::Env => {
                natural_rust_call_assertions::env_source(&semantic);
                natural_rust_call_assertions::borrowed_lowered(&module, true);
            }
        }
        for (seed, lhs, rhs, input_length, other_length) in [
            (13_u32, 0_u32, 1_u32, 3_usize, 3_usize),
            (0, 0, 0, 0, 0),
            (u32::MAX, 1, 0, 1, 1),
            (0, u32::MAX, u32::MAX, 65, 65),
            (0x8000_0000, 0, 1, 65, 65),
            (u32::MAX, u32::MAX, 11, 0, 129),
            (7, 2, u32::MAX, 129, 0),
            (0xdead_beef, 0, 2, 3, 5),
            (7, 1, 0, 129, 129),
        ] {
            let input = (0..input_length)
                .map(|index| seed.wrapping_add((index as u32).wrapping_mul(0x9e37_79b9)))
                .collect::<Vec<_>>();
            let other = (0..other_length)
                .map(|index| {
                    (seed ^ 0xf00d_1234).wrapping_add((index as u32).wrapping_mul(0x85eb_ca6b))
                })
                .collect::<Vec<_>>();
            if input_length == other_length && input_length != 0 {
                assert_ne!(
                    input, other,
                    "equal metadata must conceal distinct captured data"
                );
            }
            // Swapping equal-length contents catches descriptor substitution; the third case
            // passes the same read-only allocation to both source slice arguments.
            let data_cases = if case.has_data() { 3 } else { 1 };
            for data_case in 0..data_cases {
                let (input, other) = match data_case {
                    0 => (&input, &other),
                    1 => (&other, &input),
                    2 => (&input, &input),
                    _ => unreachable!(),
                };
                let aliased = data_case == 2;
                let input_bytes = input
                    .iter()
                    .flat_map(|value| value.to_le_bytes())
                    .collect::<Vec<_>>();
                let other_bytes = other
                    .iter()
                    .flat_map(|value| value.to_le_bytes())
                    .collect::<Vec<_>>();
                for grid in [64_usize, 128] {
                    for short_outputs in [false, true] {
                        let context = format!(
                            "{feature} {architecture} grid={grid} seed={seed:#x} lhs={lhs:#x} rhs={rhs:#x} input={} other={} data={data_case} short={short_outputs}",
                            input.len(),
                            other.len(),
                        );
                        let mut arguments = [seed, lhs, rhs].into_iter().map(|value| json!({
                            "kind": "scalar", "type": "u32", "bits": format!("0x{value:08x}"),
                        })).collect::<Vec<_>>();
                        let mut shared_buffers = Vec::new();
                        if case.has_data() {
                            if aliased {
                                shared_buffers.push(json!({
                                    "id": 7, "element": "u32", "access": "read_only",
                                    "alignment": 4, "bytes": format!("0x{}", hex(&input_bytes)),
                                }));
                                for _ in 0..2 {
                                    arguments.push(json!({
                                        "kind": "buffer_view", "backing": 7, "element": "u32",
                                        "access": "read_only", "alignment": 4,
                                        "byte_offset": 0, "elements": input.len(),
                                    }));
                                }
                            } else {
                                for bytes in [&input_bytes, &other_bytes] {
                                    arguments.push(json!({
                                        "kind": "buffer", "element": "u32", "access": "read_only",
                                        "alignment": 4, "bytes": format!("0x{}", hex(bytes)),
                                    }));
                                }
                            }
                        }
                        let output_lengths = (0..case.output_count())
                            .map(|ordinal| {
                                if short_outputs {
                                    [0, 1, 63, grid - 1, grid, grid + 3][ordinal % 6]
                                } else {
                                    grid + 3
                                }
                            })
                            .collect::<Vec<_>>();
                        let mut expected = Vec::new();
                        for (ordinal, length) in output_lengths.iter().copied().enumerate() {
                            let canary = CANARY ^ (ordinal as u32).wrapping_mul(0x9e37_79b9);
                            let initial = canary.to_le_bytes().repeat(PREFIX + length + SUFFIX);
                            shared_buffers.push(json!({
                                "id": 100 + ordinal, "element": "u32", "access": "read_write",
                                "alignment": 4, "bytes": format!("0x{}", hex(&initial)),
                            }));
                            arguments.push(json!({
                                "kind": "buffer_view", "backing": 100 + ordinal, "element": "u32",
                                "access": "read_write", "alignment": 4,
                                "byte_offset": PREFIX * 4, "elements": length,
                            }));
                            expected.push(initial);
                        }
                        for invocation in 0..grid {
                            let values = natural_rust_call_expected(
                                case,
                                [seed, lhs, rhs],
                                input,
                                other,
                                invocation,
                            );
                            assert_eq!(values.len(), case.output_count());
                            for (ordinal, value) in values.into_iter().enumerate() {
                                if invocation < output_lengths[ordinal] {
                                    let offset = (PREFIX + invocation) * 4;
                                    expected[ordinal][offset..offset + 4]
                                        .copy_from_slice(&value.to_le_bytes());
                                }
                            }
                        }
                        let request_path = target.path().join("natural-rust-call-request.json");
                        std::fs::write(
                            &request_path,
                            serde_json::to_vec(&json!({
                                "schema": "fe2o3-simulation-request-v1", "kernel": feature,
                                "grid": [grid, 1, 1], "workgroup": [64, 1, 1],
                                "arguments": arguments, "shared_buffers": shared_buffers,
                            }))
                            .unwrap(),
                        )
                        .unwrap();
                        let admitted = fe2o3_kir_sim_cli::load_debug_simulation_bundle_v6(
                            &bundle_path,
                            &request_path,
                        )
                        .unwrap_or_else(|error| {
                            panic!("{context}: canonical simulator admission failed: {error:?}")
                        });
                        let simulation = admitted.input();
                        assert_eq!(simulation.module.identity().wire_version(), 11);
                        assert_eq!(
                            simulation.module.identity().digest(),
                            bundle.canonical_kir_v11_digest()
                        );
                        assert_eq!(
                            simulation.module.identity().canonical_length(),
                            bundle.canonical_kir_v11_length()
                        );
                        assert_eq!(
                            simulation.simulation_bundle_subject(),
                            Some(*bundle.subject_identity())
                        );
                        assert!(!simulation.module.grants_execution_authority());
                        let evidence = simulation.simulation_bundle_evidence().unwrap();
                        assert_eq!(evidence.envelope_version, 6);
                        assert_eq!(evidence.production_kir_version, 11);
                        let execution = simulation
                            .module
                            .simulate(
                                &simulation.request,
                                simulation.simulation_target(),
                                simulation.simulation_limits,
                            )
                            .unwrap_or_else(|error| {
                                panic!("{context}: canonical simulator execution failed: {error:?}")
                            });
                        assert_eq!(execution.invocations_executed(), grid as u64, "{context}");
                        if case.has_data() {
                            if aliased {
                                assert_eq!(
                                    execution
                                        .shared_buffer(fe2o3_kir_sim::BufferBackingIdV1(7))
                                        .unwrap()
                                        .bytes(),
                                    input_bytes,
                                    "{context}: aliased input mutated"
                                );
                            } else {
                                assert_eq!(
                                    execution.buffer(3).unwrap().bytes(),
                                    input_bytes,
                                    "{context}: input mutated"
                                );
                                assert_eq!(
                                    execution.buffer(4).unwrap().bytes(),
                                    other_bytes,
                                    "{context}: other input mutated"
                                );
                            }
                        }
                        for (ordinal, bytes) in expected.into_iter().enumerate() {
                            let actual = execution
                                .shared_buffer(fe2o3_kir_sim::BufferBackingIdV1(
                                    100 + ordinal as u32,
                                ))
                                .unwrap();
                            assert_eq!(
                                actual.bytes(),
                                bytes,
                                "{context}: ordered output {ordinal}, including prefix, suffix, and untouched tail"
                            );
                        }
                        executions += 1;
                    }
                }
            }
        }
    }
    let expected_executions = 2 * 9 * if case.has_data() { 3 } else { 1 } * 2 * 2;
    assert_eq!(
        executions, expected_executions,
        "every positive simulation case must execute"
    );
}

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn ordinary_source_natural_fn_shims_match_rust_in_simulation() {
    for case in [
        NaturalRustCallCase::Owned,
        NaturalRustCallCase::Borrowed,
        NaturalRustCallCase::Env,
    ] {
        run_natural_rust_call_case(case);
    }
}

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn ordinary_source_natural_owned_shims_match_rust_in_simulation() {
    run_natural_rust_call_case(NaturalRustCallCase::Owned);
}

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn ordinary_source_natural_fn_mut_repeated_matches_rust_in_simulation() {
    run_natural_rust_call_case(NaturalRustCallCase::Borrowed);
}

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn ordinary_source_natural_env_aliases_match_rust_in_simulation() {
    run_natural_rust_call_case(NaturalRustCallCase::Env);
}

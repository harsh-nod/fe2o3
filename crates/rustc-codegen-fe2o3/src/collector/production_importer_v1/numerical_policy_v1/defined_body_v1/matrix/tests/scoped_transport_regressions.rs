// Additional cases in the existing authenticated AMD callback harness. The
// original with_workgroup case is retained; this case makes the original
// Context owner available for inert post-issuance source mutations.
pub(super) fn direct_context_source() -> String {
    let source = super::scoped_custody::source();
    let opening = "    let policy = context.numerical_policy::<StrictIeee>();\n    context.with_workgroup(|workgroup| {";
    assert_eq!(source.matches(opening).count(), 1);
    let source = source.replacen(opening, "    let context_borrow = &mut context;\n    let policy = (&*context_borrow).numerical_policy::<StrictIeee>();\n    let workgroup = context_borrow.__compiler_workgroup_capability_current();\n    {", 1);
    let prefix = source
        .strip_suffix("    })\n}\n")
        .expect("exact fixture outer scope");
    format!("{prefix}    }}\n}}\n")
}

fn check_actual_transport_roster(owner: &ProductionSemanticSsaOwnerV1) {
    let mir = owner.source_semantic();
    let root = mir.roots()[0];
    let body = owner.execution_view_for_root(root).unwrap().body();
    let bindings = owner
        .execution_expansion()
        .defined_capability_bindings(mir)
        .unwrap();
    let matrix_reference = bindings
        .iter()
        .find_map(|binding| match binding.contract() {
            SemanticDefinedCapabilityContractV1::PolicyMatrixBind(record)
                if binding.root() == root =>
            {
                Some(record.types().matrix_reference)
            }
            _ => None,
        })
        .expect("actual Matrix Bind receiver reference");
    let mut mutable_context_receivers = 0;
    let mut lane_references = BTreeSet::new();
    for block in body.blocks() {
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            continue;
        };
        match &mir.callables()[call.callee().index() as usize] {
            SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
                ..
            } if matches!(
                contract.operation(),
                SemanticExecutionCapabilityOperationV1::WorkgroupDerive { .. }
            ) =>
            {
                let SemanticExecutionCapabilityOperationV1::WorkgroupDerive { context, .. } =
                    contract.operation()
                else {
                    unreachable!()
                };
                assert_eq!(call.arguments().len(), 1);
                assert_eq!(call.arguments()[0].ty(), context);
                let SemanticTypeShapeV1::Pointer(pointer) =
                    mir.types()[context.index() as usize].shape()
                else {
                    panic!("actual WorkgroupDerive receiver must be a reference");
                };
                assert_eq!(pointer.kind(), SemanticPointerKindV1::Reference);
                assert_eq!(pointer.mutability(), SemanticMutabilityV1::Mutable);
                assert_eq!(pointer.address_space(), 0);
                assert_eq!(pointer.pointer_width_bits(), 64);
                assert_eq!(pointer.metadata(), SemanticPointerMetadataV1::None);
                mutable_context_receivers += 1;
            }
            SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorZero { .. },
                ..
            } => {
                lane_references.insert(call.arguments()[0].ty());
            }
            _ => {}
        }
    }
    assert!(
        mutable_context_receivers > 0,
        "the actual mutable Context delegation must be exercised"
    );
    let mut tuples = BTreeSet::new();
    for statement in body.blocks().iter().flat_map(|block| block.statements()) {
        let SemanticStatementKindV1::Assign(a) = statement.kind() else {
            continue;
        };
        let SemanticRvalueKindV1::Aggregate(aggregate) = a.value().kind() else {
            continue;
        };
        let SemanticTypeShapeV1::Tuple(tuple) =
            mir.types()[a.value().result_type().index() as usize].shape()
        else {
            continue;
        };
        if tuple.fields().len() == 2
            && tuple.fields()[0] == matrix_reference
            && lane_references.contains(&tuple.fields()[1])
        {
            assert_eq!(aggregate.kind(), &SemanticAggregateKindV1::Tuple);
            assert_eq!(
                aggregate
                    .operands()
                    .iter()
                    .map(|operand| operand.ty())
                    .collect::<Vec<_>>(),
                tuple.fields()
            );
            assert_eq!(a.destination().ty(), a.value().result_type());
            tuples.insert(a.value().result_type());
        }
    }
    assert!(
        !tuples.is_empty(),
        "retain the actual Matrix/Lane RustCall tuple constructor"
    );
    let mut fields = BTreeSet::new();
    for statement in body.blocks().iter().flat_map(|block| block.statements()) {
        let SemanticStatementKindV1::Assign(a) = statement.kind() else {
            continue;
        };
        let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(p) | SemanticOperandV1::Move(p)) =
            a.value().kind()
        else {
            continue;
        };
        if !tuples.contains(&body.locals()[p.local().index() as usize].ty()) {
            continue;
        }
        let [projection] = p.projections() else {
            continue;
        };
        if let SemanticProjectionKindV1::Field(field) = projection.kind() {
            let SemanticTypeShapeV1::Tuple(tuple) = mir.types()
                [body.locals()[p.local().index() as usize].ty().index() as usize]
                .shape()
            else {
                unreachable!()
            };
            assert_eq!(tuple.fields().get(field as usize), Some(&p.ty()));
            assert_eq!(projection.result_type(), p.ty());
            assert_eq!(a.value().result_type(), p.ty());
            fields.insert(field);
        }
    }
    assert_eq!(
        fields,
        BTreeSet::from([0, 1]),
        "both actual RustCall fields reach their typed formals"
    );
}

pub(super) fn check_later_context_kills(imported: &Imported) {
    let mir = &imported.semantic_mir;
    let original = owner(
        request(mir, mir.functions().to_vec(), mir.callables().to_vec())
            .admit_current_production(SemanticMirLimitsV1::default())
            .unwrap(),
    );
    check_component(imported, &original).expect("unchanged direct Context issuer must pass first");
    // Select an actual WorkgroupDerive source call in the same ordinary source
    // body as the original owned Context borrow. No function/local IDs are fixed.
    let mut sites = Vec::new();
    for (fi, function) in mir.functions().iter().enumerate() {
        if function.export().is_some() || function.defined_capability_contract().is_some() {
            continue;
        }
        for block in function.blocks() {
            let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                continue;
            };
            let Some(SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
                ..
            }) = mir.callables().get(call.callee().index() as usize)
            else {
                continue;
            };
            let SemanticExecutionCapabilityOperationV1::WorkgroupDerive { context, .. } =
                contract.operation()
            else {
                continue;
            };
            let SemanticTypeShapeV1::Pointer(pointer) =
                mir.types()[context.index() as usize].shape()
            else {
                panic!("exact Context reference");
            };
            assert_eq!(pointer.mutability(), SemanticMutabilityV1::Mutable);
            let referents: std::collections::BTreeMap<_, _> = function
                .blocks()
                .iter()
                .flat_map(|block| block.statements())
                .filter_map(|statement| {
                    let SemanticStatementKindV1::Assign(a) = statement.kind() else {
                        return None;
                    };
                    match a.value().kind() {
                        SemanticRvalueKindV1::Borrow {
                            kind: SemanticBorrowKindV1::Mutable,
                            place,
                        } if place.projections().is_empty() && place.ty() == pointer.pointee() => {
                            Some((place.local(), place.clone()))
                        }
                        _ => None,
                    }
                })
                .collect();
            if referents.is_empty() {
                continue;
            }
            assert_eq!(referents.len(), 1, "exact owned Context referent");
            sites.push((
                fi,
                call.destination().unwrap().edge().target().index() as usize,
                referents.into_values().next().unwrap(),
            ));
        }
    }
    assert_eq!(
        sites.len(),
        1,
        "the direct fixture must expose one post-issuance owner-kill site"
    );
    let (function_index, successor, referent) = sites.pop().unwrap();
    let function = &mir.functions()[function_index];
    let block = &function.blocks()[successor];
    for mutation in 0..3 {
        let kill = match mutation {
            0 => SemanticStatementKindV1::StorageDead(referent.local()),
            1 => SemanticStatementKindV1::Deinitialize(referent.clone()),
            _ => SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                referent.clone(),
                SemanticRvalueV1::new(
                    referent.ty(),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(referent.clone())),
                ),
            )),
        };
        let mut statements = block.statements().to_vec();
        statements.insert(0, SemanticStatementV1::new(block.source(), kill));
        let mut functions = mir.functions().to_vec();
        functions[function_index] = replace_block(
            function,
            successor,
            statements,
            block.terminator().kind().clone(),
        );
        let changed = owner(
            request(mir, functions, mir.callables().to_vec())
                .admit_current_production(SemanticMirLimitsV1::default())
                .unwrap(),
        );
        changed.verify_replay().unwrap();
        let error = check_component(imported, &changed).unwrap_err();
        assert!(
            matches!(error, ProductionSemanticKirErrorV1::Unsupported { detail, .. }
            if detail == "capability loan crosses a move, overwrite, deinitialization or storage death"),
            "post-issuance Context kill {mutation} must hit its retained loan: {error:?}"
        );
    }
}

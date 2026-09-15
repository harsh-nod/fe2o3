// Original-import mutations are inert component inputs, not new frontend
// source authentication. The unchanged real consumer must pass first.
use fe2o3_lower_mir_kernel::ProductionScopedMatrixSourceSessionV1;
use fe2o3_mir_model::SemanticExpandedStatementOriginV1;
use fe2o3_pliron::{ProductionSemanticSsaSourceQueryErrorV1, ProductionSemanticSsaSourceSiteV1};

fn carrier_bf16_consumers(
    imported: &Imported,
    owner: &ProductionSemanticSsaOwnerV1,
) -> Result<usize, ProductionSemanticKirErrorV1> {
    let (input, transfer) = component_input(imported, owner);
    let entry = transfer
        .map(|t| t.checked_ssa_relation(owner, input.selected_root(), 1_048_576))
        .transpose()?;
    let root = input.selected_root();
    let view = owner.execution_view_for_root(root).unwrap();
    let body = view.body();
    let bindings = owner
        .execution_expansion()
        .defined_capability_bindings(owner.source_semantic())
        .unwrap();
    let mut session =
        ProductionScopedMatrixSourceSessionV1::new(owner, &input, entry.as_ref(), 1_048_576)?;
    let mut consumed = Vec::new();
    for (block, data) in body.blocks().iter().enumerate() {
        let SemanticTerminatorKindV1::Call(call) = data.terminator().kind() else {
            continue;
        };
        if !matches!(
            owner
                .source_semantic()
                .callables()
                .get(call.callee().index() as usize),
            Some(SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::GlobalBf16MatrixLoad { .. },
                ..
            })
        ) {
            continue;
        }
        let mut matching = bindings.iter().filter(|binding| {
            binding.root() == root
                && binding.caller_instance() == view.block_origins()[block].instance()
                && matches!(
                    binding.contract(),
                    SemanticDefinedCapabilityContractV1::PolicyMatrixBind(_)
                )
        });
        let binding = matching.next().expect("original BF16 caller Bind");
        assert!(
            matching.next().is_none(),
            "one exact Bind for this original caller"
        );
        session.checked_bf16_lane_at(
            body,
            block as u32,
            call,
            binding.expanded_call_block().index(),
        )?;
        consumed.push(block as u32);
    }
    assert_eq!(
        consumed.len(),
        8,
        "both original closure calls and all lanes remain"
    );
    let (matrix, lanes) = session.finish(&consumed)?;
    matrix.check_consumed(body, &[])?;
    lanes.check_consumed(body, &consumed)?;
    Ok(consumed.len())
}

pub(super) fn check_workgroup_carrier_lifetimes(imported: &Imported) {
    let mir = &imported.semantic_mir;
    assert!(
        mir.functions().len() <= 256,
        "bounded original fixture inventory"
    );
    let unchanged = request(mir, mir.functions().to_vec(), mir.callables().to_vec())
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
    assert_eq!(unchanged.canonical_encoding(), mir.canonical_encoding());
    let baseline = owner(unchanged);
    baseline.verify_replay().unwrap();
    assert_eq!(carrier_bf16_consumers(imported, &baseline).unwrap(), 8);

    let workgroup_references = mir
        .callables()
        .iter()
        .filter_map(|callable| match callable {
            SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
                ..
            } => match contract.operation() {
                SemanticExecutionCapabilityOperationV1::SubgroupDeriveBorrowed {
                    workgroup_reference,
                    width: 64,
                    ..
                } => Some(workgroup_reference),
                _ => None,
            },
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    assert!(!workgroup_references.is_empty());
    let mut selected: Option<(usize, usize, usize, SemanticPlaceV1)> = None;
    let mut visits = 0_usize;
    for (fi, function) in mir.functions().iter().enumerate() {
        if function.export().is_some() || function.defined_capability_contract().is_some() {
            continue;
        }
        for (bi, block) in function.blocks().iter().enumerate() {
            let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                continue;
            };
            for (si, statement) in block.statements().iter().enumerate() {
                visits += 1;
                assert!(visits <= 32_768, "bounded original carrier search");
                let SemanticStatementKindV1::Assign(a) = statement.kind() else {
                    continue;
                };
                let SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place,
                } = a.value().kind()
                else {
                    continue;
                };
                if !place.projections().is_empty() || !a.destination().projections().is_empty() {
                    continue;
                }
                let fields = match mir.types()[place.ty().index() as usize].shape() {
                    SemanticTypeShapeV1::Aggregate(a) => a.fields(),
                    SemanticTypeShapeV1::Tuple(t) => t.fields(),
                    _ => continue,
                };
                if fields
                    .iter()
                    .filter(|ty| workgroup_references.contains(*ty))
                    .count()
                    != 1
                {
                    continue;
                }
                if !call.arguments().iter().any(|operand| {
                    matches!(operand,
                    SemanticOperandV1::Copy(p) | SemanticOperandV1::Move(p)
                        if p == a.destination())
                }) {
                    continue;
                }
                // In this closed repeated-call fixture choose its final direct
                // carrier borrow, so the mutation does not kill a later reborrow.
                if let Some((old_fi, _, _, ref old_place)) = selected {
                    assert_eq!(fi, old_fi, "one original closure carrier owner function");
                    assert_eq!(place, old_place, "one retained carrier local/type");
                }
                selected = Some((fi, bi, si, place.clone()));
            }
        }
    }
    let (fi, bi, si, referent) = selected.expect("original shared Workgroup closure borrow");
    for overwrite in [false, true] {
        let function = &mir.functions()[fi];
        let block = &function.blocks()[bi];
        let mut statements = block.statements().to_vec();
        let mutation = if overwrite {
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                referent.clone(),
                SemanticRvalueV1::new(
                    referent.ty(),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(referent.clone())),
                ),
            ))
        } else {
            SemanticStatementKindV1::StorageDead(referent.local())
        };
        assert!(si < statements.len());
        statements.push(SemanticStatementV1::new(block.source(), mutation));
        let mut functions = mir.functions().to_vec();
        functions[fi] = replace_block(function, bi, statements, block.terminator().kind().clone());
        let changed = request(mir, functions, mir.callables().to_vec())
            .admit_current_production(SemanticMirLimitsV1::default())
            .unwrap();
        assert_ne!(changed.canonical_encoding(), mir.canonical_encoding());
        let changed = owner(changed);
        changed.verify_replay().unwrap();
        let view = changed.execution_view_for_root(mir.roots()[0]).unwrap();
        let query = changed
            .source_query_for_root(view.root(), view.body())
            .unwrap();
        let mut checked = 0;
        let mut visits = 0_usize;
        for (expanded, origin) in view.block_origins().iter().enumerate() {
            if origin.function().index() as usize != fi || origin.block().index() as usize != bi {
                continue;
            }
            for (statement, source) in origin.statements().iter().enumerate() {
                visits += 1;
                assert!(visits <= 32_768, "bounded exact expanded site inventory");
                if *source
                    != (SemanticExpandedStatementOriginV1::Source {
                        statement: si as u32,
                    })
                {
                    continue;
                }
                let SemanticStatementKindV1::Assign(a) =
                    view.body().blocks()[expanded].statements()[statement].kind()
                else {
                    panic!("retained original Borrow assignment")
                };
                let SemanticRvalueKindV1::Borrow { place, .. } = a.value().kind() else {
                    panic!("original Borrow was not rewritten")
                };
                let result = query.borrow_place_use(
                    ProductionSemanticSsaSourceSiteV1::new(
                        SemanticBlockIdV1::from_index(expanded as u32),
                        Some(statement as u32),
                    ),
                    place,
                    &mut || true,
                );
                if overwrite {
                    assert!(
                        matches!(
                            result,
                            Err(ProductionSemanticSsaSourceQueryErrorV1::NoPromotedUse)
                        ),
                        "{result:?}"
                    );
                } else {
                    result.expect("post-Borrow death does not erase the original captured SSA use");
                }
                checked += 1;
            }
        }
        assert!(checked > 0, "the exact source site survives call expansion");
        let error = carrier_bf16_consumers(imported, &changed).unwrap_err();
        let expected = if overwrite {
            "Workgroup carrier borrow has no exact SSA use"
        } else {
            "capability loan crosses a move, overwrite, deinitialization or storage death"
        };
        assert!(
            matches!(error, ProductionSemanticKirErrorV1::Unsupported { detail, .. }
            if detail == expected),
            "overwrite={overwrite}: {error:?}"
        );
    }
    assert_eq!(
        baseline.source_semantic().canonical_encoding(),
        mir.canonical_encoding()
    );
}

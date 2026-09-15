use super::*;

#[test]
fn reusable_lds_replay_keeps_exact_frame_lifetimes_and_no_frame_definitions() {
    let source = source(true);
    let expansion =
        SemanticCallExpansionV1::try_new(&source, SemanticCallExpansionLimitsV1::default())
            .unwrap();
    let view = expansion.root(SemanticFunctionIdV1::from_index(0)).unwrap();
    let relation = DefinedReusableLdsResultsV1::derive(
        &source,
        &expansion,
        view,
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let row = relation.entries()[0];
    assert_eq!(row.parameter_statement(), 2);
    assert_eq!(row.return_statement(), 0);
    let frame = &view.instances()[row.callee().index() as usize];
    for (offset, site) in [row.parameter_block(), row.return_block()]
        .into_iter()
        .enumerate()
    {
        let block = &view.body().blocks()[site.index() as usize];
        assert_eq!(block.statements().len(), 3);
        for index in 0..2 {
            let local = SemanticLocalIdV1::from_index(frame.local_start() + index as u32);
            let statement = if offset == 0 { index } else { index + 1 };
            let expected = if offset == 0 {
                SemanticStatementKindV1::StorageLive(local)
            } else {
                SemanticStatementKindV1::StorageDead(local)
            };
            assert_eq!(block.statements()[statement].kind(), &expected);
            let mut injected = Vec::new();
            relation.append_result_events(site.index(), statement as u32, &mut injected);
            assert!(
                injected.is_empty(),
                "frame storage is not a capability definition"
            );
        }
    }
    for mutation in 0..6 {
        let mut blocks = view.body().blocks().to_vec();
        let returning = mutation >= 3;
        let index = if returning {
            row.return_block()
        } else {
            row.parameter_block()
        }
        .index() as usize;
        let block = &blocks[index];
        let mut statements = block.statements().to_vec();
        let marker = if returning { 1 } else { 0 };
        match mutation % 3 {
            0 => {
                statements[marker] = SemanticStatementV1::new(
                    statements[marker].source(),
                    SemanticStatementKindV1::Nop,
                )
            }
            1 => {
                statements[marker] = SemanticStatementV1::new(
                    statements[marker].source(),
                    if returning {
                        SemanticStatementKindV1::StorageDead(row.allocation())
                    } else {
                        SemanticStatementKindV1::StorageLive(row.allocation())
                    },
                )
            }
            2 => {
                statements.remove(marker);
            }
            _ => unreachable!(),
        }
        blocks[index] = SemanticBasicBlockV1::new(
            block.identity(),
            block.source(),
            statements,
            block.terminator().clone(),
        )
        .unwrap();
        let original = view.body();
        let mut changed = SemanticFunctionDeclV1::new(
            original.identity(),
            original.role(),
            original.item_definition_identity(),
            original.monomorphization_identity(),
            original.generic_type_arguments_identity(),
            original.const_generic_arguments_identity(),
            original.source(),
            original.abi().clone(),
            original.locals().to_vec(),
            original.entry(),
            blocks,
        )
        .unwrap();
        if let Some(entry) = original.kernel_entry() {
            changed = changed.with_kernel_entry(entry.clone());
        }
        assert!(
            matches!(
                relation.verify_markers(&changed),
                Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
            ),
            "mutation {mutation}"
        );
    }
}

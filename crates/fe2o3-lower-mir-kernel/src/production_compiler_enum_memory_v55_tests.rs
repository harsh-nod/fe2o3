#[cfg(test)]
pub(super) fn test_compiler_enum_closed_mutations_v55(
    instance: ProductionCallInstanceIdV1,
    archive: &ExecutionArchiveV29,
    records: &[ScopedCompilerEnumAccessV55],
    rows: &[ScopedMemoryAnchorV29],
    original: &Function,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut baseline = PendingCompilerEnumMemoryV55 {
        allocations: archive
            .enum_spills
            .iter()
            .map(|origin| SourceEnumSpillRowV48 {
                instance: instance.index(),
                origin: origin.clone(),
            })
            .collect(),
        accesses: records
            .iter()
            .map(|record| PendingCompilerEnumAccessV55 {
                instance,
                record: *record,
                block: rows[record.anchor].block,
                operation: rows[record.anchor].position,
                reference: None,
            })
            .collect(),
    };
    baseline
        .allocations
        .sort_unstable_by_key(|row| row.origin.pointer);
    baseline
        .accesses
        .sort_unstable_by_key(|row| (row.block, row.operation));
    for fault in 0..11 {
        let mut pending = baseline.clone();
        let mut function = original.clone();
        let access = pending.accesses[0];
        let block = function
            .body
            .as_mut()
            .unwrap()
            .blocks
            .iter_mut()
            .find(|block| block.id == access.block)
            .unwrap();
        match fault {
            0 => {}
            1 => {
                pending.accesses.remove(0);
            }
            2 => {
                pending.accesses.insert(0, access);
            }
            3 => {
                pending.allocations.remove(0);
            }
            4 => {
                pending
                    .allocations
                    .insert(0, pending.allocations[0].clone());
            }
            5 => {
                pending.accesses[0].operation += 1;
            }
            6 => {
                block
                    .operations
                    .push(block.operations[access.operation].clone());
            }
            7 => {
                // A compiler pointer cannot escape as a stored payload.
                let mut escape = block.operations[access.operation].clone();
                if let OperationKind::Store { value, .. } = &mut escape.kind {
                    *value = access.record.pointer;
                }
                block.operations.push(escape);
            }
            8 => {
                if let OperationKind::Store { access, .. } =
                    &mut block.operations[access.operation].kind
                {
                    access.volatile = true;
                }
            }
            9 => {
                pending.allocations[0].origin.alignment *= 2;
            }
            10 => {
                pending.allocations[0].origin.element = Type::Scalar(ScalarType::I32);
            }
            _ => unreachable!(),
        }
        let floor = budget.storage();
        let result = with_canonical_call_scratch_v1(budget, |budget| {
            check_compiler_enum_closed_memory_v55(&function, &[], &pending, budget).map(|_| ())
        });
        assert_eq!(
            budget.storage(),
            floor,
            "closed compiler census scratch fault {fault}"
        );
        if matches!(
            result,
            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(_))
        ) {
            return result;
        }
        if fault == 0 {
            result?;
        } else {
            assert!(
                result.is_err(),
                "closed compiler census fault {fault} admitted"
            );
        }
    }
    Ok(())
}

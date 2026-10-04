#[derive(Clone, Copy)]
struct ErasedDeletedAssertIndexV1 {
    site: SemanticKirAssertSiteV1,
    control: usize,
}

fn erased_occurrence_vec_v1<T>(
    count: usize,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> ErasedOccurrenceResultV1<Vec<T>> {
    unit_local_vec_v1(count, budget).map_err(ProductionSourceOutputErrorV1::SourceReplay)
}

fn erased_occurrence_push_v1<T>(
    rows: &mut Vec<T>,
    value: T,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> ErasedOccurrenceResultV1<()> {
    unit_local_push_v1(rows, value, budget)
        .map(|_| ())
        .map_err(ProductionSourceOutputErrorV1::SourceReplay)
}

fn erased_deleted_assert_index_v1(
    map: &ErasedSourceCoordinateMapV1<'_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> ErasedOccurrenceResultV1<Vec<ErasedDeletedAssertIndexV1>> {
    let rows = map.deletion.stage.source.rows;
    let mut index = erased_occurrence_vec_v1(rows.control.len(), budget)?;
    for (ordinal, control) in rows.control.iter().enumerate() {
        erased_occurrence_charge_v1(budget, 4)?;
        if !matches!(control.kind, UnitLocalControlKindV1::Assert { .. }) {
            continue;
        }
        let association = rows
            .associations
            .get(control.association)
            .ok_or_else(|| erased_occurrence_invalid_v1("deleted assertion association"))?;
        let block = control
            .source_block
            .ok_or_else(|| erased_occurrence_invalid_v1("deleted assertion source block"))?;
        erased_occurrence_push_v1(
            &mut index,
            ErasedDeletedAssertIndexV1 {
                site: SemanticKirAssertSiteV1::new(
                    association.key.root,
                    association.key.function,
                    block,
                ),
                control: ordinal,
            },
            budget,
        )?;
    }
    assert_origin_sort_v1(&mut index, budget, |a, b, budget| {
        source_output_site_cmp_v1(a.site, b.site, budget)
    })
    .map_err(ProductionSourceOutputErrorV1::SourceOrigin)?;
    for pair in index.windows(2) {
        erased_occurrence_charge_v1(budget, 3)?;
        if pair[0].site == pair[1].site {
            return Err(erased_occurrence_invalid_v1(
                "unique deleted assertion site",
            ));
        }
    }
    Ok(index)
}

fn erased_check_deleted_assertion_v1(
    map: &ErasedSourceCoordinateMapV1<'_>,
    index: &[ErasedDeletedAssertIndexV1],
    site: SemanticKirAssertSiteV1,
    binding: SemanticKirAssertConditionBindingV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> ErasedOccurrenceResultV1<()> {
    map.live(budget)?;
    let ordinal = assert_origin_find_v1(index, budget, |row, budget| {
        source_output_site_cmp_v1(row.site, site, budget)
    })
    .map_err(ProductionSourceOutputErrorV1::SourceOrigin)?
    .ok_or_else(|| erased_occurrence_invalid_v1("fresh deleted assertion coverage"))?;
    erased_occurrence_charge_v1(budget, 32)?;
    let rows = map.deletion.stage.source.rows;
    let control = rows
        .control
        .get(index[ordinal].control)
        .ok_or_else(|| erased_occurrence_invalid_v1("deleted assertion control"))?;
    let association = rows
        .associations
        .get(control.association)
        .ok_or_else(|| erased_occurrence_invalid_v1("deleted assertion association"))?;
    let UnitLocalControlKindV1::Assert {
        target,
        predicate,
        expected,
        selected_successor,
        ..
    } = control.kind
    else {
        return Err(erased_occurrence_invalid_v1("deleted assertion recipe"));
    };
    let SemanticKirAssertConditionOutcomeV1::Emitted {
        condition_use,
        success_edge,
        failure_edge,
        ..
    } = binding.outcome()
    else {
        return Err(erased_occurrence_invalid_v1(
            "deleted assertion emitted origin",
        ));
    };
    let original_block = binding.block();
    let predicate = rows
        .values
        .get(predicate)
        .ok_or_else(|| erased_occurrence_invalid_v1("deleted assertion predicate"))?;
    let native_block = map.original_block(original_block, budget)?;
    let native_condition = map
        .inventory()
        .uses()
        .get(native_block.terminator_uses.start)
        .filter(|row| row.coordinate == condition_use)
        .ok_or_else(|| erased_occurrence_invalid_v1("deleted assertion exact condition use"))?;
    if association.key.root != site.correspondence_owner
        || association.key.function != site.semantic_function
        || association.key.physical != original_block.function.0 as usize
        || control.source_block != Some(site.semantic_block)
        || control.physical_block != native_block.block.id
        || binding.expected() != expected
        || binding.semantic_success() != target
        || selected_successor != u32::from(!expected)
        || success_edge.source != original_block
        || success_edge.successor != selected_successor
        || failure_edge.source != original_block
        || failure_edge.successor != u32::from(expected)
        || condition_use
            != (ErasedOperandV1::TerminatorOperand {
                block: original_block,
                operand: 0,
            })
        || predicate.key != association.key
        || predicate.known_bits != Some(u64::from(expected))
        || !matches!(predicate.native, UnitLocalNativeValueV1::Scalar {
            value, scalar: ScalarType::Bool, ..
        } if value == native_condition.value)
        || map.block(original_block, budget)? != ErasedSourceOccurrenceV1::DeletedLocalHelper
    {
        return Err(erased_occurrence_invalid_v1(
            "deleted assertion source-success polarity",
        ));
    }
    Ok(())
}

fn erased_rebase_assertion_v1(
    map: &ErasedSourceCoordinateMapV1<'_>,
    binding: SemanticKirAssertConditionBindingV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> ErasedOccurrenceResultV1<Option<SemanticKirAssertConditionBindingV1>> {
    let mapped_block = match map.block(binding.block(), budget)? {
        ErasedSourceOccurrenceV1::Retained(block) => block,
        ErasedSourceOccurrenceV1::DeletedLocalHelper => return Ok(None),
        ErasedSourceOccurrenceV1::DeletedUnitCall => {
            return Err(erased_occurrence_invalid_v1(
                "assertion block is not a call",
            ));
        }
    };
    let outcome = match binding.outcome() {
        SemanticKirAssertConditionOutcomeV1::Emitted {
            condition_use,
            definition,
            success_edge,
            failure_edge,
        } => {
            let condition_use = map.operand(condition_use, budget)?.retained()?;
            let definition = map.definition(definition, budget)?.retained()?;
            let success_edge = map.edge(success_edge, budget)?.retained()?;
            let failure_edge = map.edge(failure_edge, budget)?.retained()?;
            erased_occurrence_charge_v1(budget, 5)?;
            if success_edge.source != mapped_block
                || failure_edge.source != mapped_block
                || success_edge.successor != u32::from(!binding.expected())
                || failure_edge.successor != u32::from(binding.expected())
                || condition_use
                    != (ErasedOperandV1::TerminatorOperand {
                        block: mapped_block,
                        operand: 0,
                    })
            {
                return Err(erased_occurrence_invalid_v1(
                    "retained assertion exact polarity",
                ));
            }
            SemanticKirAssertConditionOutcomeV1::Emitted {
                condition_use,
                definition,
                success_edge,
                failure_edge,
            }
        }
        SemanticKirAssertConditionOutcomeV1::ElidedByExistingRule { success_edge } => {
            let success_edge = map.edge(success_edge, budget)?.retained()?;
            erased_occurrence_charge_v1(budget, 1)?;
            if success_edge.source != mapped_block {
                return Err(erased_occurrence_invalid_v1(
                    "retained source elision block",
                ));
            }
            SemanticKirAssertConditionOutcomeV1::ElidedByExistingRule { success_edge }
        }
    };
    Ok(Some(SemanticKirAssertConditionBindingV1 {
        expected: binding.expected(),
        semantic_success: binding.semantic_success(),
        outcome,
    }))
}

// The returned origin view is private and temporary. It contains no row for a
// deleted assertion, so subsequent transport cannot inherit a removed trap.
fn erased_assertion_origins_v1(
    map: &ErasedSourceCoordinateMapV1<'_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> ErasedOccurrenceResultV1<(SealedAssertOriginsV1, Vec<Option<usize>>)> {
    let original = map.deletion.source().assert_origins();
    let deleted = erased_deleted_assert_index_v1(map, budget)?;
    let mut indices = erased_occurrence_vec_v1(original.origins.bindings.len(), budget)?;
    erased_occurrence_charge_v1(budget, original.origins.bindings.len())?;
    indices.resize(original.origins.bindings.len(), None);
    let mut bindings = erased_occurrence_vec_v1(original.origins.bindings.len(), budget)?;
    for (ordinal, binding) in original.origins.bindings.iter().enumerate() {
        erased_occurrence_charge_v1(budget, 2)?;
        if let Some(rebased) = erased_rebase_assertion_v1(map, *binding, budget)? {
            indices[ordinal] = Some(bindings.len());
            erased_occurrence_push_v1(&mut bindings, rebased, budget)?;
        }
    }
    let mut aliases = erased_occurrence_vec_v1(original.origins.aliases.len(), budget)?;
    for alias in &original.origins.aliases {
        erased_occurrence_charge_v1(budget, 3)?;
        let binding = original
            .origins
            .bindings
            .get(alias.binding)
            .copied()
            .ok_or_else(|| erased_occurrence_invalid_v1("original assertion binding"))?;
        let retained = indices
            .get(alias.binding)
            .ok_or_else(|| erased_occurrence_invalid_v1("total assertion disposition"))?;
        if let Some(binding) = retained {
            erased_occurrence_push_v1(
                &mut aliases,
                AssertOriginAliasV1 {
                    site: alias.site,
                    binding: *binding,
                },
                budget,
            )?;
        } else {
            erased_check_deleted_assertion_v1(map, &deleted, alias.site, binding, budget)?;
        }
    }
    let mut functions = erased_occurrence_vec_v1(original.origins.functions.len(), budget)?;
    for function in &original.origins.functions {
        erased_occurrence_charge_v1(budget, 1)?;
        match map.canonical_function(function.canonical, budget)? {
            ErasedSourceOccurrenceV1::Retained(canonical) => {
                erased_occurrence_push_v1(
                    &mut functions,
                    AssertSourceFunctionV1 {
                        canonical,
                        ..*function
                    },
                    budget,
                )?;
            }
            ErasedSourceOccurrenceV1::DeletedLocalHelper => {}
            ErasedSourceOccurrenceV1::DeletedUnitCall => {
                return Err(erased_occurrence_invalid_v1("function is not a call"));
            }
        }
    }
    erased_occurrence_charge_v1(budget, 6)?;
    let payload_storage = aliases
        .capacity()
        .checked_mul(std::mem::size_of::<AssertOriginAliasV1>())
        .and_then(|a| {
            functions
                .capacity()
                .checked_mul(std::mem::size_of::<AssertSourceFunctionV1>())
                .and_then(|b| a.checked_add(b))
        })
        .and_then(|a| {
            bindings
                .capacity()
                .checked_mul(std::mem::size_of::<SemanticKirAssertConditionBindingV1>())
                .and_then(|b| a.checked_add(b))
        })
        .ok_or(ProductionSourceOutputErrorV1::Resource(
            AssertOriginResourceV1::Arithmetic,
        ))?;
    Ok((
        SealedAssertOriginsV1 {
            aliases,
            functions,
            bindings,
            storage: SemanticKirAssertOriginStorageV1 { payload_storage },
        },
        indices,
    ))
}

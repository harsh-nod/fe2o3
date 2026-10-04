// Only a checked current C2 read can mint this occurrence-bound variant claim.
// The access record's ordinary locator fields remain insufficient authority.
mod source_enum_checked_read_v58 {
    use super::*;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(super) struct CheckedRead(std::num::NonZeroUsize);

    #[derive(Default)]
    pub(super) struct Claims(Vec<Option<Receipt>>);

    #[derive(Clone, Copy)]
    pub(super) struct Receipt {
        site: SourceReferenceSiteV29,
        source: usize,
        local: SemanticLocalIdV1,
        generation: u32,
        ty: SemanticTypeIdV1,
        variant: u32,
        snapshot: usize,
    }

    pub(super) fn headers() -> usize {
        // Candidate, intersection pair, and return envelope coexist only in
        // the already-scoped source builder; row backing is charged separately.
        3 * std::mem::size_of::<Option<Receipt>>()
            + std::mem::size_of::<Result<Option<Receipt>, ProductionSemanticKirErrorV1>>()
    }

    impl Claims {
        pub(super) fn append(
            &mut self,
            receipt: Option<Receipt>,
            budget: &mut ArgumentBudgetV1<'_>,
        ) -> Result<Option<CheckedRead>, ProductionSemanticKirErrorV1> {
            let Some(receipt) = receipt else {
                return Ok(None);
            };
            let index = argument_sum_v1(&[self.0.len(), 1])?;
            emission_push_v1(&mut self.0, Some(receipt), budget)?;
            Ok(Some(CheckedRead(
                std::num::NonZeroUsize::new(index).ok_or(ArgumentResourceV1::Accounting)?,
            )))
        }

        pub(super) fn intersect(
            &mut self,
            previous: Option<CheckedRead>,
            next: Option<Receipt>,
            budget: &mut ArgumentBudgetV1<'_>,
        ) -> Result<Option<CheckedRead>, ProductionSemanticKirErrorV1> {
            let Some(previous) = previous else {
                return Ok(None);
            };
            budget.charge_work(8)?;
            let slot = self
                .0
                .get_mut(previous.0.get() - 1)
                .ok_or(ArgumentResourceV1::Accounting)?;
            // All visits must discharge the same predicate. In particular an
            // earlier checked read cannot survive a later untracked visit.
            *slot = match (*slot, next) {
                (Some(old), Some(new))
                    if old.site == new.site
                        && old.source == new.source
                        && old.local == new.local
                        && old.generation == new.generation
                        && old.ty == new.ty
                        && old.variant == new.variant =>
                {
                    Some(new)
                }
                _ => None,
            };
            Ok(slot.is_some().then_some(previous))
        }
    }

    impl CheckedRead {
        pub(super) fn allows(
            self,
            plan: &SourceReferencePlanV29<'_, '_>,
            row: &SourceReferenceAccessRecordV29,
            site: SourceReferenceSiteV29,
            source: &SemanticPlaceV1,
            ty: SemanticTypeIdV1,
            variant: u32,
            budget: &mut dyn SemanticEmissionBudgetV1,
        ) -> Result<bool, ProductionSemanticKirErrorV1> {
            budget.source_reference_owner_v29(plan)?;
            budget.source_reference_charge_v29(
                plan,
                argument_sum_v1(&[19, source.projections().len()])?,
            )?;
            let Some(receipt) = plan
                .checked_enum_reads
                .0
                .get(self.0.get() - 1)
                .ok_or(ArgumentResourceV1::Accounting)?
            else {
                return Ok(false);
            };
            Ok(receipt.site == site
                && receipt.source == source as *const SemanticPlaceV1 as usize
                && receipt.local == source.local()
                && receipt.ty == ty
                && receipt.variant == variant
                && plan.storage_root.is_some()
                && plan.storage_snapshots.get(receipt.snapshot).is_some()
                && row.key.site == receipt.site
                && row.key.source == receipt.source
                && row.key.access == SourceReferenceAccessV29::Read
                && row.source_local == receipt.local
                && row.instance == receipt.site.instance
                && row.local == receipt.local
                && row.generation == receipt.generation
                && row.ty == source.ty()
                && row.loan.is_none()
                && !row.shared_path
                && row.traversed.is_empty()
                && plan.projections.get(row.projections.clone()) == Some(source.projections()))
        }
    }

    pub(super) fn check(
        builder: &SourceReferenceBuilderV29<'_, '_, '_>,
        site: SourceReferenceSiteV29,
        source: &SemanticPlaceV1,
        access: SourceReferenceAccessV29,
        resolved: &SourceReferencePlaceV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<Receipt>, ProductionSemanticKirErrorV1> {
        budget.charge_work(9)?;
        let Some(first) = source.projections().first() else {
            return Ok(None);
        };
        let SemanticProjectionKindV1::Downcast(variant) = first.kind() else {
            return Ok(None);
        };
        if access != SourceReferenceAccessV29::Read
            || resolved.instance != site.instance
            || resolved.local != source.local()
            || resolved.loan.is_some()
            || resolved.shared_path
            || !resolved.traversed.is_empty()
            || resolved.selector_source.is_some()
        {
            return Ok(None);
        }
        budget.charge_work(argument_product_v1(source.projections().len(), 2)?)?;
        if resolved.projections != source.projections()
            || source
                .projections()
                .iter()
                .any(|step| step.kind() == SemanticProjectionKindV1::Dereference)
        {
            return Ok(None);
        }
        let local = builder.local(resolved.instance, resolved.local)?;
        let Some(snapshot) = local.storage else {
            return Ok(None);
        };
        let declaration = builder
            .plan
            .instances
            .instance(site.instance)
            .and_then(|instance| {
                instance
                    .declaration()
                    .locals()
                    .get(source.local().index() as usize)
            })
            .ok_or_else(source_reference_enum_error_v29)?;
        if local.generation != resolved.generation || declaration.ty() != first.result_type() {
            return Err(source_reference_enum_error_v29());
        }
        // Recheck rather than accepting a locator or a caller-supplied success
        // flag. This query checks the full initialized path and active variant
        // in this exact current snapshot, after source CFG edge refinement.
        builder.check_storage_read(resolved, budget)?;
        Ok(Some(Receipt {
            site,
            source: source as *const SemanticPlaceV1 as usize,
            local: source.local(),
            generation: resolved.generation,
            ty: declaration.ty(),
            variant,
            snapshot,
        }))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn receipt(snapshot: usize) -> Receipt {
            Receipt {
                site: SourceReferenceSiteV29 {
                    instance: ProductionCallInstanceIdV1(0),
                    block: SemanticBlockIdV1::from_index(0),
                    statement: Some(0),
                },
                source: 23,
                local: SemanticLocalIdV1::from_index(1),
                generation: 2,
                ty: SemanticTypeIdV1::from_index(3),
                variant: 1,
                snapshot,
            }
        }

        #[test]
        fn checked_enum_read_sparse_claims_intersect_visits_without_reviving_authority() {
            for mutation in 0..8 {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
                let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
                let mut claims = Claims::default();
                let handle = claims.append(Some(receipt(5)), &mut budget).unwrap();
                let retained = budget.storage();
                let mut next = receipt(6);
                match mutation {
                    1 => next.source += 1,
                    2 => next.generation += 1,
                    3 => next.local = SemanticLocalIdV1::from_index(2),
                    4 => next.ty = SemanticTypeIdV1::from_index(4),
                    5 => next.variant = 0,
                    6 => next.site.statement = Some(1),
                    _ => {}
                }
                let updated = claims
                    .intersect(handle, (mutation != 7).then_some(next), &mut budget)
                    .unwrap();
                assert_eq!(updated.is_some(), mutation == 0);
                assert_eq!(claims.0.len(), 1);
                assert_eq!(budget.storage(), retained);
                if mutation == 0 {
                    assert_eq!(claims.0[0].unwrap().snapshot, 6);
                } else {
                    assert!(
                        claims
                            .intersect(updated, Some(receipt(7)), &mut budget)
                            .unwrap()
                            .is_none()
                    );
                    assert!(claims.0[0].is_none());
                }
            }
        }

        #[test]
        fn checked_enum_read_sparse_retention_has_exact_and_one_short_resource_bounds() {
            use std::mem::size_of;
            #[allow(dead_code)]
            struct ReceiptShape {
                site: SourceReferenceSiteV29,
                source: usize,
                local: SemanticLocalIdV1,
                generation: u32,
                ty: SemanticTypeIdV1,
                variant: u32,
                snapshot: usize,
            }
            assert_eq!(size_of::<Option<CheckedRead>>(), size_of::<usize>());
            assert_eq!(size_of::<Claims>(), size_of::<Vec<usize>>());
            assert_eq!(size_of::<Receipt>(), size_of::<ReceiptShape>());
            assert_eq!(
                headers(),
                3 * size_of::<Option<ReceiptShape>>()
                    + size_of::<Result<Option<ReceiptShape>, ProductionSemanticKirErrorV1>>()
            );
            let bytes = 4 * size_of::<Option<ReceiptShape>>();
            for (work_limit, storage_limit, succeeds) in [
                (13, 17 + bytes, true),
                (12, 17 + bytes, false),
                (13, 16 + bytes, false),
            ] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
                let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
                budget.reserve_storage(17).unwrap();
                let mut claims = Claims::default();
                let result = claims
                    .append(Some(receipt(5)), &mut budget)
                    .and_then(|handle| claims.intersect(handle, Some(receipt(6)), &mut budget));
                assert_eq!(result.is_ok(), succeeds);
                if work_limit == 12 {
                    assert!(matches!(
                        result,
                        Err(
                            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Work(_)
                            )
                        )
                    ));
                } else if !succeeds {
                    assert!(matches!(
                        result,
                        Err(
                            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Storage(_)
                            )
                        )
                    ));
                }
                let paid = budget.storage() - 17;
                drop(claims);
                budget.release_storage(paid).unwrap();
                assert_eq!(budget.storage(), 17);
            }
        }
    }
}

fn source_enum_checked_read_at_v58(
    plan: &SourceReferencePlanV29<'_, '_>,
    site: SourceReferenceSiteV29,
    source: &SemanticPlaceV1,
    ty: SemanticTypeIdV1,
    variant: u32,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.source_reference_owner_v29(plan)?;
    budget.source_reference_charge_v29(
        plan,
        argument_sum_v1(&[
            plan.access_sites.len().checked_ilog2().unwrap_or(0) as usize,
            3,
        ])?,
    )?;
    let key = source_reference_access_key_v29(site, source, SourceReferenceAccessV29::Read);
    let Some(index) = plan.access_sites.get(&key) else {
        return Ok(false);
    };
    let row = plan
        .accesses
        .get(*index)
        .ok_or_else(source_reference_enum_error_v29)?;
    match row.checked_enum_read {
        Some(claim) => claim.allows(plan, row, site, source, ty, variant, budget),
        None => Ok(false),
    }
}

fn source_enum_current_binding_same_v58(
    held: &SemanticValueBindingV1,
    archived: &SemanticValueBindingV1,
    nodes: &mut usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    execution_cfg_charge_node_v29(nodes, budget)?;
    Ok(match (held, archived) {
        (SemanticValueBindingV1::Unit, SemanticValueBindingV1::Unit) => true,
        (
            SemanticValueBindingV1::Value {
                id: left,
                ty: left_ty,
            },
            SemanticValueBindingV1::Value {
                id: right,
                ty: right_ty,
            },
        ) => left == right && invocation_equal_types_v1(left_ty, right_ty, budget)?,
        (SemanticValueBindingV1::Aggregate(left), SemanticValueBindingV1::Aggregate(right))
            if left.len() == right.len() =>
        {
            for (left, right) in left.iter().zip(right) {
                if !source_enum_current_binding_same_v58(left, right, nodes, budget)? {
                    return Ok(false);
                }
            }
            true
        }
        (
            SemanticValueBindingV1::Enum {
                discriminant: left,
                discriminant_ty: left_ty,
                semantic_type: left_source,
                variant: left_variant,
                payloads: left_payloads,
            },
            SemanticValueBindingV1::Enum {
                discriminant: right,
                discriminant_ty: right_ty,
                semantic_type: right_source,
                variant: right_variant,
                payloads: right_payloads,
            },
        ) => {
            budget.charge_work(5)?;
            if left != right
                || left_source != right_source
                || left_variant != right_variant
                || left_payloads.len() != right_payloads.len()
                || !invocation_equal_types_v1(left_ty, right_ty, budget)?
            {
                return Ok(false);
            }
            for ((left_key, left), (right_key, right)) in left_payloads.iter().zip(right_payloads) {
                budget.charge_work(2)?;
                if left_key != right_key || left.len() != right.len() {
                    return Ok(false);
                }
                for (left, right) in left.iter().zip(right) {
                    if !source_enum_current_binding_same_v58(left, right, nodes, budget)? {
                        return Ok(false);
                    }
                }
            }
            true
        }
        (
            SemanticValueBindingV1::SourceReference(left),
            SemanticValueBindingV1::SourceReference(right),
        ) => {
            budget.charge_work(argument_product_v1(left.values.len(), 4)?)?;
            left == right
        }
        _ => false,
    })
}

impl SemanticFunctionLoweringV1<'_, '_> {
    fn source_enum_checked_variant_v58(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        source: &SemanticPlaceV1,
        ty: SemanticTypeIdV1,
        variant: u32,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        if self
            .execution
            .as_ref()
            .and_then(|cursor| cursor.references)
            .is_none()
        {
            return Ok(false);
        }
        self.with_emission_budget_v1(|this, budget| {
            let cursor = this
                .execution
                .as_ref()
                .ok_or_else(source_reference_enum_error_v29)?;
            let references = cursor
                .references
                .ok_or_else(source_reference_enum_error_v29)?;
            references.check(budget)?;
            let site = SourceReferenceSiteV29 {
                instance: cursor.instance,
                block,
                statement: statement.map(|value| value as usize),
            };
            if !source_enum_checked_read_at_v58(references.plan, site, source, ty, variant, budget)?
            {
                return Ok(false);
            }
            let execution_site = execution_site_v29(block, statement);
            let Some(frame) = this
                .scoped_memory
                .as_ref()
                .and_then(|recorder| recorder.frame)
            else {
                return Ok(false);
            };
            let Some(ScopedMemoryRoleV29::Operand(role)) = frame.role else {
                return Ok(false);
            };
            if frame.site != execution_site {
                return Err(source_reference_enum_error_v29());
            }
            let Some(ScopedMemoryOccurrenceV29::Promoted { definition, .. }) =
                scoped_payload_occurrence_v29(cursor, execution_site, role, source, budget)?
            else {
                return Ok(false);
            };
            charge_execution_cfg_lookup_v29(this.semantic_ssa_bindings.len(), budget)?;
            let held = this
                .locals
                .get(source.local().index() as usize)
                .and_then(Option::as_ref)
                .ok_or_else(source_reference_enum_error_v29)?;
            let archived = this
                .semantic_ssa_bindings
                .get(&definition)
                .ok_or_else(source_reference_enum_error_v29)?;
            source_enum_current_binding_same_v58(held, archived, &mut 0, budget)
        })
    }
}

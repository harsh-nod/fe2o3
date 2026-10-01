#[cfg_attr(test, derive(Clone))]
struct CompilerSpillV55 {
    instance: ProductionCallInstanceIdV1,
    origin: ExecutionEnumSpillV48,
}

#[cfg(test)]
pub(super) fn check_compiler_spill_test_permits_v55(
    instances: &ExecutionInstancesV29<'_>,
    emitted: &[Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    with_canonical_call_scratch_v1(budget, |budget| {
        let roster = capture_compiler_spills_v55(instances, emitted, slots, budget)?;
        assert_eq!(
            roster.len(),
            2,
            "one scalar spill per repeated helper instance"
        );
        let permit = FramePermitV29 {
            instances,
            slots,
            compiler_spills: &roster,
        };
        for (index, spill) in roster.iter().enumerate() {
            let origin = &spill.origin;
            let body = emitted[spill.instance.index()]
                .as_ref()
                .unwrap()
                .function
                .body
                .as_ref()
                .unwrap();
            let entry = &body.blocks[0];
            let allocation = &entry.operations[origin.emitted_operation];
            permit
                .check(
                    instances,
                    spill.instance,
                    entry.id,
                    origin.emitted_operation,
                    allocation,
                    budget,
                )
                .map_err(map_error)?;
            permit
                .check(
                    instances,
                    instances.root(),
                    entry.id,
                    origin.emitted_operation,
                    allocation,
                    budget,
                )
                .map_err(map_error)?;
            for fault in 0..6 {
                let mut operation = allocation.clone();
                let mut block = entry.id;
                let mut ordinal = origin.emitted_operation;
                let mut child = spill.instance;
                match fault {
                    0 => child = roster[1 - index].instance,
                    1 => ordinal += 1,
                    2 => block.0 = u32::MAX,
                    3 => operation.results[0].id = roster[1 - index].origin.pointer,
                    4 => operation.kind = OperationKind::Constant(Constant::U32(0)),
                    5 => {
                        operation.results[0].ty = Type::pointer(
                            Type::Scalar(ScalarType::U32),
                            AddressSpace::Global,
                            AccessMode::ReadWrite,
                        )
                    }
                    _ => unreachable!(),
                }
                let result = permit.check(instances, child, block, ordinal, &operation, budget);
                if let Err(CallInstanceEmissionErrorV1::Resource(error)) = result {
                    return Err(error.into());
                }
                assert!(
                    matches!(
                        result,
                        Err(CallInstanceEmissionErrorV1::CalleeFrameAllocation)
                    ),
                    "spill permit fault {fault}: {result:?}"
                );
            }
        }
        let mut replay = capture_compiler_spills_v55(instances, emitted, slots, budget)?;
        assert!(compiler_spills_match_v55(&roster, &replay, budget)?);
        let original = replay[0].clone();
        for fault in 0..13 {
            match fault {
                0 => replay[0].instance = replay[1].instance,
                1 => replay[0].origin.local += 1,
                2 => replay[0].origin.source_type = SemanticTypeIdV1::from_index(u32::MAX),
                3 => replay[0].origin.variant += 1,
                4 => replay[0].origin.field += 1,
                5 => replay[0].origin.field_type = SemanticTypeIdV1::from_index(u32::MAX),
                6 => replay[0].origin.component += 1,
                7 => replay[0].origin.emitted_block.0 += 1,
                8 => replay[0].origin.emitted_operation += 1,
                9 => replay[0].origin.pointer = replay[1].origin.pointer,
                10 => replay[0].origin.alignment *= 2,
                11 => replay[0].origin.element = Type::Scalar(ScalarType::I32),
                12 => replay[0] = replay[1].clone(),
                _ => unreachable!(),
            }
            assert!(
                !compiler_spills_match_v55(&roster, &replay, budget)?,
                "spill replay fault {fault}"
            );
            replay[0] = original.clone();
        }
        assert!(!compiler_spills_match_v55(&roster, &replay[..1], budget)?);
        Ok(())
    })
}

type CompilerSpillFrameV55<'a> = (
    &'a ExecutionArchiveV29,
    &'a LoweredFunctionResultV1,
    &'a ScopedSourceSlotInstanceV29,
    &'a ExecutionEnumSpillV48,
    std::slice::Iter<'a, ScopedSourceSlotInstanceV29>,
    std::slice::Iter<'a, ExecutionEnumSpillV48>,
    Vec<CompilerSpillV55>,
    CompilerSpillV55,
    [usize; 8],
);

fn frame_error_v55(error: ProductionSemanticKirErrorV1) -> CallInstanceEmissionErrorV1 {
    match error {
        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error) => error.into(),
        _ => CallInstanceEmissionErrorV1::CalleeFrameAllocation,
    }
}

fn enum_spill_count_v55(
    lowered: &LoweredFunctionResultV1,
) -> Result<u32, ProductionSemanticKirErrorV1> {
    let archive = lowered.execution_observation.as_ref().ok_or_else(invalid)?;
    u32::try_from(archive.enum_spills.len()).map_err(|_| ArgumentResourceV1::Arithmetic.into())
}

fn capture_compiler_spills_v55(
    instances: &ExecutionInstancesV29<'_>,
    emitted: &[Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<CompilerSpillV55>, ProductionSemanticKirErrorV1> {
    let headers = source_reference_emission_headers_v29::<CompilerSpillFrameV55<'_>>()?;
    budget.reserve_storage(headers)?;
    let mut count = 0;
    for instance in &slots.instances {
        budget.charge_work(3)?;
        let lowered = emitted
            .get(instance.instance.index())
            .and_then(Option::as_ref)
            .ok_or_else(invalid)?;
        let entry = lowered
            .function
            .body
            .as_ref()
            .and_then(|body| body.blocks.first())
            .ok_or_else(invalid)?
            .id;
        let (allocations, initializers) =
            prologue_counts(instances, instance, slots, entry, budget)?;
        let first = argument_sum_v1(&[allocations as usize, initializers as usize])?;
        check_scoped_allocation_census_v55(
            instances,
            instance.instance,
            lowered,
            entry,
            first,
            slots
                .slots
                .get(instance.slots.clone())
                .ok_or_else(invalid)?,
            budget,
        )?;
        count = argument_sum_v1(&[count, enum_spill_count_v55(lowered)? as usize])?;
    }
    let mut rows = emission_vec_v1(count, budget)?;
    for instance in &slots.instances {
        budget.charge_work(2)?;
        let archive = emitted[instance.instance.index()]
            .as_ref()
            .ok_or_else(invalid)?
            .execution_observation
            .as_ref()
            .ok_or_else(invalid)?;
        for original in &archive.enum_spills {
            budget.charge_work(size_of::<CompilerSpillV55>())?;
            let origin = ExecutionEnumSpillV48 {
                local: original.local,
                source_type: original.source_type,
                variant: original.variant,
                field: original.field,
                field_type: original.field_type,
                component: original.component,
                emitted_block: original.emitted_block,
                emitted_operation: original.emitted_operation,
                pointer: original.pointer,
                element: emission_binding_clone_type_v1(&original.element, budget)?,
                alignment: original.alignment,
            };
            if rows.len() == rows.capacity() {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            rows.push(CompilerSpillV55 {
                instance: instance.instance,
                origin,
            });
        }
    }
    call_splice_sort_work_v1(rows.len(), budget).map_err(map_error)?;
    rows.sort_unstable_by_key(|row| (row.origin.emitted_block, row.origin.emitted_operation));
    budget.charge_work(rows.len())?;
    if rows.windows(2).any(|pair| {
        (
            pair[0].origin.emitted_block,
            pair[0].origin.emitted_operation,
        ) == (
            pair[1].origin.emitted_block,
            pair[1].origin.emitted_operation,
        )
    }) {
        return Err(invalid());
    }
    budget.release_storage(headers)?;
    Ok(rows)
}

fn compiler_spills_match_v55(
    left: &[CompilerSpillV55],
    right: &[CompilerSpillV55],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.charge_work(1)?;
    if left.len() != right.len() {
        return Ok(false);
    }
    for (left, right) in left.iter().zip(right) {
        budget.charge_work(size_of::<CompilerSpillV55>())?;
        let a = &left.origin;
        let b = &right.origin;
        if left.instance != right.instance
            || (
                a.local,
                a.source_type,
                a.variant,
                a.field,
                a.field_type,
                a.component,
                a.emitted_block,
                a.emitted_operation,
                a.pointer,
                a.alignment,
            ) != (
                b.local,
                b.source_type,
                b.variant,
                b.field,
                b.field_type,
                b.component,
                b.emitted_block,
                b.emitted_operation,
                b.pointer,
                b.alignment,
            )
            || !enum_spill_types_equal_v48(&a.element, &b.element, budget)?
        {
            return Ok(false);
        }
    }
    Ok(true)
}

impl RelocationV29 {
    fn prefix_for_instance_v55(
        &self,
        instance: ProductionCallInstanceIdV1,
    ) -> Result<&PrefixV29, ProductionSemanticKirErrorV1> {
        self.prefixes
            .binary_search_by_key(&instance.index(), |prefix| prefix.instance.index())
            .ok()
            .map(|index| &self.prefixes[index])
            .ok_or_else(invalid)
    }

    fn mapped_enum_spill_v55(
        &self,
        mut row: InstanceMappedSpanV1,
    ) -> Result<InstanceMappedSpanV1, ProductionSemanticKirErrorV1> {
        let original = row.source.coordinates().2;
        if row.removed_call.is_some() || row.segments != [Some(original), None] {
            return Err(invalid());
        }
        let (first, count) = if original.block == self.root {
            let expected = self
                .root_prefix
                .checked_add(self.root_initializers)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            if original.first != expected || original.count != self.root_spills {
                return Err(invalid());
            }
            (
                expected
                    .checked_add(self.moved)
                    .ok_or(ArgumentResourceV1::Arithmetic)?,
                self.root_spills,
            )
        } else {
            let prefix = self.prefix_for_instance_v55(row.instance)?;
            if original.block != prefix.block
                || original.first
                    != prefix
                        .count
                        .checked_add(prefix.initializers)
                        .ok_or(ArgumentResourceV1::Arithmetic)?
                || original.count != prefix.spills
            {
                return Err(invalid());
            }
            (
                prefix
                    .first
                    .checked_add(prefix.count)
                    .ok_or(ArgumentResourceV1::Arithmetic)?,
                prefix.spills,
            )
        };
        row.segments = [
            Some(InstancePhysicalSpanV1 {
                block: self.root,
                first,
                count,
            }),
            None,
        ];
        Ok(row)
    }

    fn check_relocated_compiler_spills_v55(
        &self,
        entry: &BasicBlock,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        for spill in &self.compiler_spills {
            budget.charge_work(8)?;
            charge_execution_cfg_lookup_v29(self.prefixes.len(), budget)?;
            let origin = &spill.origin;
            let operation = if origin.emitted_block == self.root {
                argument_sum_v1(&[origin.emitted_operation, self.moved as usize])?
            } else {
                let prefix = self.prefix_for_instance_v55(spill.instance)?;
                let first =
                    argument_sum_v1(&[prefix.count as usize, prefix.initializers as usize])?;
                let ordinal = origin
                    .emitted_operation
                    .checked_sub(first)
                    .ok_or_else(invalid)?;
                if origin.emitted_block != prefix.block || ordinal >= prefix.spills as usize {
                    return Err(invalid());
                }
                argument_sum_v1(&[prefix.first as usize, prefix.count as usize, ordinal])?
            };
            check_enum_spill_alloca_v55(
                origin,
                entry.operations.get(operation).ok_or_else(invalid)?,
                budget,
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod map_tests_v55 {
    use super::*;

    #[test]
    fn compiler_spill_span_map_keeps_initializer_holes_and_rejects_crossing_spans() {
        let root = BlockId(0);
        let donor = BlockId(1);
        let instance = ProductionCallInstanceIdV1(1);
        let relocation = RelocationV29 {
            root,
            root_prefix: 2,
            root_initializers: 3,
            root_spills: 1,
            moved: 6,
            prefixes: vec![PrefixV29 {
                instance,
                block: donor,
                count: 4,
                initializers: 3,
                spills: 2,
                first: 2,
            }],
            storage: StorageV29 {
                allocations: 3,
                payload_bytes: 12,
                alignment: 4,
            },
            compiler_spills: vec![],
        };
        let span = |block, first, count| InstancePhysicalSpanV1 {
            block,
            first,
            count,
        };
        assert_eq!(
            relocation.ordinary_span(span(donor, 4, 3)).unwrap(),
            span(donor, 0, 3)
        );
        assert_eq!(
            relocation.ordinary_span(span(donor, 9, 5)).unwrap(),
            span(donor, 3, 5)
        );
        assert_eq!(
            relocation.ordinary_span(span(root, 5, 1)).unwrap(),
            span(root, 11, 1)
        );
        for invalid in [
            span(donor, 0, 1),
            span(donor, 3, 2),
            span(donor, 6, 2),
            span(donor, 7, 1),
            span(donor, 8, 2),
        ] {
            assert!(relocation.ordinary_span(invalid).is_err());
        }
        let source = SemanticKirSyntheticOperationSpanV1 {
            correspondence_owner: SemanticFunctionIdV1::from_index(0),
            semantic_function: SemanticFunctionIdV1::from_index(1),
            rule: SemanticKirSyntheticOperationRuleV1::EnumPayloadStorage,
            kernel_ir_block: donor,
            first_operation_ordinal: 7,
            operation_count: 2,
        };
        let row = InstanceMappedSpanV1 {
            instance,
            source: InstanceSpanSourceV1::Synthetic(source),
            segments: [Some(span(donor, 7, 2)), None],
            removed_call: None,
        };
        let mapped = relocation.mapped(row).unwrap();
        assert_eq!(mapped.source, row.source);
        assert_eq!(mapped.instance, row.instance);
        assert_eq!(mapped.segments, [Some(span(root, 6, 2)), None]);
        for fault in 0..4 {
            let mut altered = row;
            match fault {
                0 => altered.instance = ProductionCallInstanceIdV1(2),
                1 => altered.segments[0].as_mut().unwrap().first += 1,
                2 => altered.segments[1] = altered.segments[0],
                3 => {
                    let InstanceSpanSourceV1::Synthetic(ref mut source) = altered.source else {
                        unreachable!()
                    };
                    source.operation_count -= 1;
                    altered.segments[0].as_mut().unwrap().count -= 1;
                }
                _ => unreachable!(),
            }
            assert!(
                relocation.mapped(altered).is_err(),
                "span substitution {fault}"
            );
        }
    }
}

#[derive(Clone, Copy)]
struct StaticPublicationConsumerValuesV1 {
    payload: ValueId,
    flags: ValueId,
    cell: ValueId,
    acquired: ValueId,
    predicate: ValueId,
}

struct StaticPublicationCorrelationV1<'a, 'module> {
    semantic: Option<&'a AdmittedInertSemanticMirV1>,
    semantic_function: SemanticFunctionIdV1,
    kir: &'a KirCorrelationIndexV1<'module>,
    sites: &'a BTreeMap<(FunctionOperationLocation, u32), SemanticAccessSiteV1>,
    ranked: &'a RankedCorrelationIndexV1,
    lowering: &'a ProductionRankedKernelLoweringInputV1,
}

impl StaticPublicationCorrelationV1<'_, '_> {
    fn authenticate_read(
        &self,
        site: SemanticAccessSiteV1,
        logical_site: SemanticAccessSiteV1,
        consumer: KirMemoryConsumerV1,
        count: usize,
        budget: &mut UnsupportedIndexCorrelationBudgetV1,
    ) -> Result<Option<AuthenticatedConditionalReadV1>, ProductionMirPlironTranslationErrorV1> {
        if site.statement.is_some() || site.ordinal != 2 {
            return Ok(None);
        }
        let Some(semantic) = self.semantic else {
            return Ok(None);
        };
        let Some(function) = semantic
            .functions()
            .get(self.semantic_function.index() as usize)
        else {
            return Ok(None);
        };
        let Some(block) = function.blocks().get(site.block as usize) else {
            return Ok(None);
        };
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            return Ok(None);
        };
        if !matches!(
            semantic.callables().get(call.callee().index() as usize),
            Some(SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::StaticPublication128TryReadF32 { .. },
                ..
            })
        ) {
            return Ok(None);
        }
        let valid = site == logical_site
            && count == 3
            && call.arguments().len() == 3
            && call.destination().is_some()
            && consumer.operation_access_ordinal == 0
            && consumer.access == dialect_kernel::AccessKindAttr::Read
            && consumer.memory_space == dialect_kernel::MemorySpaceAttr::Global
            && consumer.atomic.is_none();
        if !valid || self.exact_consumer_roster(site, consumer, budget).is_none() {
            return Err(if budget.remaining == 0 {
                ProductionMirPlironTranslationErrorV1::ResourceLimit
            } else {
                ProductionMirPlironTranslationErrorV1::ValueExpressionMismatch {
                    location: consumer.location,
                }
            });
        }
        // Complete source-owner replay authenticates every generated operand.
        // This extra closed roster check never makes either atomic optional.
        Ok(Some(AuthenticatedConditionalReadV1 {
            location: consumer.location,
            operation_access_ordinal: 0,
            site,
        }))
    }

    fn exact_consumer_roster(
        &self,
        site: SemanticAccessSiteV1,
        read: KirMemoryConsumerV1,
        budget: &mut UnsupportedIndexCorrelationBudgetV1,
    ) -> Option<StaticPublicationConsumerValuesV1> {
        let mut roster = [None; 3];
        for consumer in &self.kir.memory_consumers {
            budget.charge()?;
            let Some(source) = self
                .sites
                .get(&(consumer.location, consumer.operation_access_ordinal))
            else {
                continue;
            };
            if (source.block, source.statement) != (site.block, site.statement) {
                continue;
            }
            let slot = roster.get_mut(source.ordinal as usize)?;
            if consumer.operation_access_ordinal != 0 || slot.replace(*consumer).is_some() {
                return None;
            }
        }
        let [Some(request), Some(acquire), Some(actual_read)] = roster else {
            return None;
        };
        if actual_read.location != read.location
            || request.location.block != read.location.block
            || acquire.location.block != read.location.block
            || request.location.operation_index >= acquire.location.operation_index
            || acquire.location.operation_index >= read.location.operation_index
        {
            return None;
        }
        budget.charge()?;
        let request_op = self.kir.operations.get(&request.location)?;
        budget.charge()?;
        let acquire_op = self.kir.operations.get(&acquire.location)?;
        budget.charge()?;
        let read_op = self.kir.operations.get(&read.location)?;
        let OperationKind::Atomic(request_atomic) = &request_op.kind else {
            return None;
        };
        let OperationKind::Atomic(acquire_atomic) = &acquire_op.kind else {
            return None;
        };
        let request_value = request_atomic.value?;
        if !request_op.results.is_empty()
            || request_op.kind != static_publication_atomic_v1(request.pointer, Some(request_value))
            || acquire_op.kind != static_publication_atomic_v1(request.pointer, None)
            || acquire_atomic.pointer != request_atomic.pointer
            || acquire.pointer != request.pointer
        {
            return None;
        }
        let [observed] = acquire_op.results.as_slice() else {
            return None;
        };
        if observed.ty != Type::Scalar(ScalarType::U32)
            || self.definition(request_value, budget)?.kind
                != OperationKind::Constant(Constant::U32(1))
        {
            return None;
        }
        let [value] = read_op.results.as_slice() else {
            return None;
        };
        let OperationKind::GuardedLoad {
            pointer,
            predicate,
            fallback,
            access,
        } = read_op.kind
        else {
            return None;
        };
        if pointer != read.pointer
            || value.ty != Type::Scalar(ScalarType::F32)
            || access != MemoryAccess::new(AddressSpace::Global, 4)
            || self.definition(fallback, budget)?.kind
                != OperationKind::Constant(Constant::F32Bits(0))
        {
            return None;
        }
        let OperationKind::Binary {
            op: BinaryOp::BitAnd,
            lhs: equality,
            rhs: bound,
        } = self.definition(predicate, budget)?.kind
        else {
            return None;
        };
        let OperationKind::Compare {
            predicate: ComparePredicate::Equal,
            lhs,
            rhs: ready,
        } = self.definition(equality, budget)?.kind
        else {
            return None;
        };
        if lhs != observed.id
            || self.definition(ready, budget)?.kind != OperationKind::Constant(Constant::U32(2))
        {
            return None;
        }
        let OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs: cell,
            rhs: length,
        } = self.definition(bound, budget)?.kind
        else {
            return None;
        };
        let OperationKind::SliceLength { slice: payload } = self.definition(length, budget)?.kind
        else {
            return None;
        };
        let OperationKind::GetElementPointer { base, offset } =
            self.definition(pointer, budget)?.kind
        else {
            return None;
        };
        if self.definition(base, budget)?.kind != (OperationKind::SliceData { slice: payload }) {
            return None;
        }
        let OperationKind::Select {
            condition,
            true_value,
            false_value,
        } = self.definition(offset, budget)?.kind
        else {
            return None;
        };
        if condition != predicate
            || true_value != cell
            || self.definition(false_value, budget)?.kind
                != OperationKind::Constant(Constant::Index(0))
        {
            return None;
        }
        let OperationKind::GetElementPointer {
            base: flag_base,
            offset: flag_cell,
        } = self.definition(request.pointer, budget)?.kind
        else {
            return None;
        };
        let OperationKind::SliceData { slice: flags } = self.definition(flag_base, budget)?.kind
        else {
            return None;
        };
        if flag_cell != cell || flags == payload {
            return None;
        }
        self.exact_ranked_roster(site, budget)?;
        Some(StaticPublicationConsumerValuesV1 {
            payload,
            flags,
            cell,
            acquired: observed.id,
            predicate,
        })
    }

    fn definition(
        &self,
        value: ValueId,
        budget: &mut UnsupportedIndexCorrelationBudgetV1,
    ) -> Option<&Operation> {
        budget.charge()?;
        self.kir.definitions.get(&value).copied()
    }

    fn exact_ranked_roster(
        &self,
        site: SemanticAccessSiteV1,
        budget: &mut UnsupportedIndexCorrelationBudgetV1,
    ) -> Option<()> {
        let mut sources = Vec::with_capacity(3);
        for ordinal in 0..3 {
            budget.charge()?;
            sources.push(
                self.ranked
                    .sources_by_site
                    .get(&SemanticAccessSiteV1 { ordinal, ..site })?,
            );
        }
        let [request, acquire, read] = sources.as_slice() else {
            return None;
        };
        if request.ranked_block != read.ranked_block
            || acquire.ranked_block != read.ranked_block
            || request.ranked_operation >= acquire.ranked_operation
            || acquire.ranked_operation >= read.ranked_operation
        {
            return None;
        }
        let operations = self
            .lowering
            .kernel()
            .blocks()
            .get(read.ranked_block as usize)?
            .operations();
        let ProductionRankedOperationV1::PublicationAtomicStoreU32 {
            view: flags,
            index: cell,
            value: 1,
        } = operations.get(request.ranked_operation as usize)?
        else {
            return None;
        };
        let ProductionRankedOperationV1::PublicationAtomicLoadU32 {
            result: acquired,
            view,
            index,
        } = operations.get(acquire.ranked_operation as usize)?
        else {
            return None;
        };
        if flags != view || cell != index {
            return None;
        }
        let ProductionRankedOperationV1::PredicatedAccess {
            kind: dialect_kernel::AccessKindAttr::Read,
            view: payload,
            index,
            success,
        } = operations.get(read.ranked_operation as usize)?
        else {
            return None;
        };
        let mut guard_extent = None;
        for operation in &operations[..read.ranked_operation as usize] {
            budget.charge()?;
            if let ProductionRankedOperationV1::PublicationReadGuard {
                result,
                success: actual_success,
                index: actual_cell,
                physical_extent,
                acquired: actual_acquired,
            } = operation
                && *index == ProductionRankedValueV1::Local(*result)
                && *success == ProductionRankedValueV1::Local(*actual_success)
                && (actual_cell != cell
                    || *actual_acquired != ProductionRankedValueV1::Local(*acquired)
                    || guard_extent.replace(*physical_extent).is_some())
            {
                return None;
            }
        }
        let extent = guard_extent?;
        let mut exact_extent = false;
        for block in self.lowering.kernel().blocks() {
            budget.charge()?;
            for operation in block.operations() {
                budget.charge()?;
                let (result, shape, dynamic_extents) = match operation {
                    ProductionRankedOperationV1::View {
                        result,
                        element_width: 32,
                        writable: true,
                        shape,
                        dynamic_extents,
                        ..
                    }
                    | ProductionRankedOperationV1::ViewInSpace {
                        result,
                        element_width: 32,
                        writable: true,
                        shape,
                        dynamic_extents,
                        memory_space: dialect_kernel::MemorySpaceAttr::Global,
                        ..
                    } => (result, shape, dynamic_extents),
                    _ => continue,
                };
                if *payload == ProductionRankedValueV1::Local(*result) {
                    if exact_extent
                        || shape.as_slice() != [dialect_kernel::DYNAMIC_EXTENT]
                        || dynamic_extents.as_slice() != [extent]
                    {
                        return None;
                    }
                    exact_extent = true;
                }
            }
        }
        exact_extent.then_some(())
    }

    fn validate_written_constant(
        &self,
        source: &IndexedRankedAccessSourceV1,
        consumer: KirMemoryConsumerV1,
        budget: &mut UnsupportedIndexCorrelationBudgetV1,
    ) -> Result<(), ProductionMirPlironTranslationErrorV1> {
        let invalid = || ProductionMirPlironTranslationErrorV1::ValueExpressionMismatch {
            location: consumer.location,
        };
        let operation = self
            .lowering
            .kernel()
            .blocks()
            .get(source.ranked_block as usize)
            .and_then(|block| block.operations().get(source.ranked_operation as usize))
            .ok_or_else(invalid)?;
        let ProductionRankedOperationV1::PublicationAtomicStoreU32 { value, .. } = operation else {
            return Ok(());
        };
        let actual = self
            .kir
            .operations
            .get(&consumer.location)
            .ok_or_else(invalid)?;
        let OperationKind::Atomic(atomic) = &actual.kind else {
            return Err(invalid());
        };
        let stored = atomic.value.ok_or_else(invalid)?;
        let definition = self.definition(stored, budget).ok_or_else(invalid)?;
        if definition.kind != OperationKind::Constant(Constant::U32(*value)) {
            return Err(invalid());
        }
        Ok(())
    }
}

fn static_publication_ranked_optional_read_v1(
    ranked: &fe2o3_pliron::ProductionRankedKernelV1,
    block: u32,
    operation: u32,
    budget: &mut UnsupportedIndexCorrelationBudgetV1,
) -> Option<bool> {
    let operations = ranked.blocks().get(block as usize)?.operations();
    let Some(ProductionRankedOperationV1::PredicatedAccess {
        kind: dialect_kernel::AccessKindAttr::Read,
        index,
        success,
        ..
    }) = operations.get(operation as usize)
    else {
        return Some(false);
    };
    for candidate in operations.get(..operation as usize)? {
        budget.charge()?;
        if let ProductionRankedOperationV1::PublicationReadGuard {
            result,
            success: actual,
            ..
        } = candidate
            && *index == ProductionRankedValueV1::Local(*result)
            && *success == ProductionRankedValueV1::Local(*actual)
        {
            return Some(true);
        }
    }
    Some(false)
}

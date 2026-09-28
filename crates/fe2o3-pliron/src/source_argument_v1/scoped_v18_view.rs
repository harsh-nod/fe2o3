#[derive(Clone, Copy)]
struct ArgumentViewDataV18<'a> {
    semantic: &'a AdmittedInertSemanticMirV1,
    semantic_function: SemanticFunctionIdV1,
    target: &'a Function,
    logical: &'a fe2o3_mir_model::SemanticLogicalArgumentMapV1<'a>,
    physical: &'a [IndexedArgumentTraceV1<'a>],
    ignored: &'a [Option<&'a SemanticKirIgnoredParameterBindingV1>],
    shapes: &'a [AdjustedArgumentShapeV1],
    cleanup: &'a CanonicalAnalysisCleanupV1<'a>,
    custody: ArgumentQueryCustodyV18,
}

#[derive(Clone, Copy)]
struct ArgumentQueryCustodyV18 {
    slot: usize,
    ledger: ArgumentLedgerV1,
    floor: usize,
}

impl<'s> ArgumentViewDataV18<'s> {
    fn check(&self, budget: &ArgumentBudgetV1<'_>) -> Result<(), ProductionSourceArgumentErrorV1> {
        if self.custody.slot != std::ptr::from_ref(budget) as usize
            || self.custody.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.custody.floor
        {
            self.cleanup.deny_refund();
        }
        if self.cleanup.refund_denied() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        Ok(())
    }

    fn visit_nodes_scoped<'work>(
        &self,
        budget: &mut ArgumentBudgetV1<'work>,
        mut visit: impl for<'n, 'b> FnMut(
            ProductionArgumentNodeV1<'n>,
            &'b mut ArgumentBudgetV1<'work>,
        ) -> Result<(), ProductionSourceArgumentErrorV1>,
    ) -> Result<(), ProductionSourceArgumentErrorV1> {
        self.check(budget)?;
        with_argument_scratch_v18(self.cleanup, budget, move |budget| {
            self.visit_nodes(budget, &mut |node, budget| {
                let floor = budget.storage();
                argument_attempt_v18(self.cleanup, budget, floor, |budget| visit(node, budget))
            })
        })
    }

    fn physical(
        &self,
        slot: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<ProductionPhysicalArgumentV1<'s>>, ProductionSourceArgumentErrorV1> {
        self.check(budget)?;
        budget.charge_work(40)?;
        let Some(&value) = self
            .target
            .body
            .as_ref()
            .and_then(|body| body.parameters.get(slot))
        else {
            return Ok(None);
        };
        let row = self
            .physical
            .binary_search_by_key(&value, |row| row.trace.value())
            .ok()
            .and_then(|index| self.physical.get(index))
            .ok_or(ProductionSourceArgumentErrorV1::CorrespondenceMismatch)?;
        let ty = self
            .target
            .signature
            .parameters
            .get(slot)
            .ok_or(ProductionSourceArgumentErrorV1::CorrespondenceMismatch)?;
        Ok(Some(ProductionPhysicalArgumentV1 {
            slot,
            value,
            ty,
            trace: match row.trace {
                PhysicalArgumentTraceV1::Direct(row) => ProductionArgumentTraceV1::Direct(row),
                PhysicalArgumentTraceV1::Component(row) => {
                    ProductionArgumentTraceV1::Component(row)
                }
            },
        }))
    }

    fn visit_nodes<'work>(
        &self,
        budget: &mut ArgumentBudgetV1<'work>,
        visit: &mut impl for<'n, 'b> FnMut(
            ProductionArgumentNodeV1<'n>,
            &'b mut ArgumentBudgetV1<'work>,
        ) -> Result<(), ProductionSourceArgumentErrorV1>,
    ) -> Result<(), ProductionSourceArgumentErrorV1> {
        let function = &self.semantic.functions()[self.semantic_function.index() as usize];
        let outer = (function.abi().extern_abi()
            == fe2o3_mir_model::semantic_mir_v1::SemanticExternAbiV1::RustCall)
            .then_some(function.abi().fixed_count());
        let mut adjusted = self.logical.adjusted_arguments().peekable();
        let mut slot = 0;
        for source in self.logical.source_arguments() {
            budget.charge_work(1)?;
            let first = slot;
            while let Some(mapped) = adjusted.peek().copied() {
                if mapped.source_argument() != source.ordinal() {
                    break;
                }
                adjusted.next();
                let shape = self
                    .shapes
                    .get(mapped.ordinal() as usize)
                    .ok_or(ProductionSourceArgumentErrorV1::CorrespondenceMismatch)?;
                self.visit_adjusted(source, mapped, *shape, budget, visit)?;
                slot = shape.end;
            }
            if outer == Some(source.ordinal()) {
                let local = match source.binding() {
                    fe2o3_mir_model::SemanticSourceArgumentBindingV1::Whole(local) => {
                        Some((local, &[][..]))
                    }
                    fe2o3_mir_model::SemanticSourceArgumentBindingV1::ExpandedTuple(_) => None,
                };
                visit(
                    ProductionArgumentNodeV1 {
                        source,
                        adjusted: None,
                        ty: source.ty(),
                        source_path: &[],
                        local,
                        ignored: local.and_then(|(id, _)| {
                            self.ignored.get(id.index() as usize).copied().flatten()
                        }),
                        coverage: argument_composite_coverage_v1(first, slot),
                    },
                    budget,
                )?;
            }
        }
        if adjusted.next().is_some() {
            return Err(ProductionSourceArgumentErrorV1::CorrespondenceMismatch);
        }
        Ok(())
    }

    fn visit_adjusted<'work>(
        &self,
        source: fe2o3_mir_model::SemanticSourceArgumentV1<'_>,
        mapped: fe2o3_mir_model::SemanticAdjustedArgumentV1<'_>,
        shape: AdjustedArgumentShapeV1,
        budget: &mut ArgumentBudgetV1<'work>,
        visit: &mut impl for<'n, 'b> FnMut(
            ProductionArgumentNodeV1<'n>,
            &'b mut ArgumentBudgetV1<'work>,
        ) -> Result<(), ProductionSourceArgumentErrorV1>,
    ) -> Result<(), ProductionSourceArgumentErrorV1> {
        with_argument_scratch_v18(self.cleanup, budget, |budget| {
            self.walk_adjusted(source, mapped, shape, budget, visit)
        })
    }

    fn walk_adjusted<'work>(
        &self,
        source: fe2o3_mir_model::SemanticSourceArgumentV1<'_>,
        mapped: fe2o3_mir_model::SemanticAdjustedArgumentV1<'_>,
        shape: AdjustedArgumentShapeV1,
        budget: &mut ArgumentBudgetV1<'work>,
        visit: &mut impl for<'n, 'b> FnMut(
            ProductionArgumentNodeV1<'n>,
            &'b mut ArgumentBudgetV1<'work>,
        ) -> Result<(), ProductionSourceArgumentErrorV1>,
    ) -> Result<(), ProductionSourceArgumentErrorV1> {
        let prefix = mapped
            .tuple_field()
            .map(ProductionArgumentProjectionV1::Field);
        let mut emit = |node: ParameterStructureNodeV1<'_>,
                        budget: &mut ArgumentBudgetV1<'work>| {
            budget.charge_work(1)?;
            let coverage = if node.physical.is_empty() {
                ProductionArgumentCoverageV1::Zero
            } else if shape.atomic || node.represented_leaf {
                let slot = shape.first + node.physical.start;
                let physical = self
                    .physical(slot, budget)?
                    .ok_or(ProductionSourceArgumentErrorV1::CorrespondenceMismatch)?;
                if shape.atomic && node.path.len() != usize::from(prefix.is_some()) {
                    ProductionArgumentCoverageV1::WithinAtomicParameter(physical)
                } else {
                    ProductionArgumentCoverageV1::Parameter(physical)
                }
            } else {
                argument_composite_coverage_v1(
                    shape.first + node.physical.start,
                    shape.first + node.physical.end,
                )
            };
            let local_offset =
                usize::from(mapped.tuple_field().is_some() && mapped.local_field().is_none());
            visit(
                ProductionArgumentNodeV1 {
                    source,
                    adjusted: Some(mapped),
                    ty: node.ty,
                    source_path: node.path,
                    local: Some((mapped.local(), &node.path[local_offset..])),
                    ignored: self
                        .ignored
                        .get(mapped.local().index() as usize)
                        .copied()
                        .flatten(),
                    coverage,
                },
                budget,
            )
        };
        if shape.atomic {
            visit_atomic_argument_structure_v18(
                self.semantic.types(),
                mapped.abi().ty(),
                prefix,
                budget,
                &mut emit,
            )
        } else {
            prepay_argument_shape_v1(self.semantic, mapped.abi().ty(), budget)?;
            let mut path = argument_vec_v1(1)?;
            path.extend(prefix);
            let mut output = Vec::new();
            let mut nodes = 0;
            append_parameter_structure_v1(
                self.semantic.types(),
                mapped.abi().ty(),
                shape.policy,
                &mut path,
                &mut output,
                &mut nodes,
                0,
                &mut |node| emit(node, budget),
            )?;
            if output.len() != shape.end - shape.first {
                return Err(ProductionSourceArgumentErrorV1::CorrespondenceMismatch);
            }
            Ok(())
        }
    }
}

// Atomic carriers are not scalarized. Their metadata tree can be deeper than
// the by-value component cap, so use a charged explicit stack and no pointee walk.
fn visit_atomic_argument_structure_v18<'work>(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    prefix: Option<ProductionArgumentProjectionV1>,
    budget: &mut ArgumentBudgetV1<'work>,
    visit: &mut impl FnMut(
        ParameterStructureNodeV1<'_>,
        &mut ArgumentBudgetV1<'work>,
    ) -> Result<(), ProductionSourceArgumentErrorV1>,
) -> Result<(), ProductionSourceArgumentErrorV1> {
    let (mut frames, mut path) = (Vec::new(), Vec::new());
    let (mut frame_capacity, mut path_capacity) = (0, 0);
    if let Some(prefix) = prefix {
        argument_scratch_push_v1(&mut path, &mut path_capacity, prefix, budget)?;
    }
    argument_scratch_push_v1(
        &mut frames,
        &mut frame_capacity,
        AtomicArgumentFrameV1 {
            ty,
            next: 0,
            path_length: path.len(),
        },
        budget,
    )?;
    while let Some(frame) = frames.last_mut() {
        budget.charge_work(1)?;
        let declaration = &types[frame.ty.index() as usize];
        if let Some((ty, projection)) = atomic_argument_child_v1(declaration.shape(), frame.next)? {
            frame.next = frame
                .next
                .checked_add(1)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            argument_scratch_push_v1(&mut path, &mut path_capacity, projection, budget)?;
            argument_scratch_push_v1(
                &mut frames,
                &mut frame_capacity,
                AtomicArgumentFrameV1 {
                    ty,
                    next: 0,
                    path_length: path.len(),
                },
                budget,
            )?;
        } else {
            let frame = frames
                .pop()
                .ok_or(ProductionSourceArgumentErrorV1::CorrespondenceMismatch)?;
            let end = usize::from(declaration.layout().size_bytes() != Some(0));
            visit(
                ParameterStructureNodeV1 {
                    ty: frame.ty,
                    path: &path[..frame.path_length],
                    physical: 0..end,
                    represented_leaf: false,
                },
                budget,
            )?;
            if let Some(parent) = frames.last() {
                path.truncate(parent.path_length);
            }
        }
    }
    Ok(())
}

/// An actual caller operand paired with its callee's physical parameter slot.
/// The value identities belong to different functions and are not equated.
#[derive(Clone, Copy, Debug)]
pub struct ProductionCallArgumentV1<'a> {
    caller_value: ValueId,
    parameter: ProductionPhysicalArgumentV1<'a>,
}

impl<'a> ProductionCallArgumentV1<'a> {
    /// Actual operand of the caller's Call operation.
    pub const fn caller_value(self) -> ValueId {
        self.caller_value
    }
    /// Checked entry slot in the callee's separate value namespace.
    pub const fn parameter(self) -> ProductionPhysicalArgumentV1<'a> {
        self.parameter
    }
}

/// One source operand joined to a structural node of the callee's entry ABI.
/// Zero components and containment within atomic carriers remain explicit.
pub struct ProductionCallArgumentNodeV1<'a> {
    operand: &'a SemanticOperandV1,
    parameter: ProductionArgumentNodeV1<'a>,
}

impl ProductionCallArgumentNodeV1<'_> {
    /// Original caller operand, evaluated once before RustCall expansion.
    pub const fn operand(&self) -> &SemanticOperandV1 {
        self.operand
    }
    /// Callee source/local paths and zero/component/atomic coverage.
    pub const fn parameter(&self) -> &ProductionArgumentNodeV1<'_> {
        &self.parameter
    }
}

/// Checked destination finishing; a projected address is prepared before operands.
#[derive(Clone, Copy, Debug)]
pub enum ProductionCallDestinationV1<'a> {
    /// SSA or ignored local; there is no result store.
    Local,
    /// Unprojected local backed by retained private storage.
    Retained(&'a Operation),
    /// Projected source place, with the exact prepared-address operation range.
    Projected {
        /// May be empty when the address already exists.
        preparation: &'a [Operation],
        /// Exact unguarded store of the call's scalar result.
        store: &'a Operation,
    },
}

/// One applicable scalar result component on the caller's continuation edge.
#[derive(Clone, Copy, Debug)]
pub struct ProductionCallResultTransportV1<'a> {
    slot: u32,
    value: ValueId,
    conversion: Option<&'a Operation>,
}

impl<'a> ProductionCallResultTransportV1<'a> {
    /// Physical edge slot, not the source SSA argument ordinal.
    pub const fn slot(self) -> u32 {
        self.slot
    }
    /// Actual branch operand in this slot.
    pub const fn value(self) -> ValueId {
        self.value
    }
    /// Exact optional INDEX/U64 transport bitcast.
    pub const fn conversion(self) -> Option<&'a Operation> {
        self.conversion
    }
}

/// A callee return block, including its pre-conversion local value anchor.
#[derive(Clone, Copy, Debug)]
pub struct ProductionCallReturnV1<'a> {
    block: &'a BasicBlock,
    input: Option<ValueId>,
    conversion: Option<&'a Operation>,
}

impl<'a> ProductionCallReturnV1<'a> {
    /// Original callee KIR block and Return terminator.
    pub const fn block(self) -> &'a BasicBlock {
        self.block
    }
    /// None for ignored results; never fabricates a zero-sized KIR value.
    pub const fn input(self) -> Option<ValueId> {
        self.input
    }
    /// Exact optional INDEX -> U64 return conversion.
    pub const fn conversion(self) -> Option<&'a Operation> {
        self.conversion
    }
}

/// Scoped, owner-qualified helper call/result correspondence over immutable MIR/KIR.
///
/// This joins the existing source SSA plan and entry ABI view, not a second graph.
/// Full lowering replay authenticates source operand, address and local provenance;
/// this view is not a semantic-equivalence proof or permission to launch a kernel.
/// Query scratch is charged until the callback returns; immutable owner payload
/// is excluded. Ordinary Result returns restore storage, not work or peak history.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionCallViewV1;
/// fn forge(view: &ProductionCallViewV1<'_, '_>) {
///     let _ = ProductionCallViewV1 { entry: view.entry };
/// }
/// ```
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionCallViewV1;
/// fn escape_node(view: &mut ProductionCallViewV1<'_, '_>) {
///     let mut saved = None;
///     view.visit_arguments(|node| { saved = Some(node); Ok(()) }).unwrap();
///     drop(saved);
/// }
/// ```
pub struct ProductionCallViewV1<'s, 'w> {
    entry: ProductionArgumentViewV1<'s, 'w>,
    caller: &'s SemanticKirFunctionCorrespondenceV1,
    source: &'s SemanticDirectCallV1,
    block: &'s BasicBlock,
    operation: &'s Operation,
    destination: ProductionCallDestinationV1<'s>,
    transport: Option<ProductionCallResultTransportV1<'s>>,
    definitions: &'s [fe2o3_mir_model::SsaArgumentV1],
    arguments: &'s [fe2o3_mir_model::SsaArgumentV1],
    returns: &'s [SemanticKirCallReturnV1],
}

impl<'s, 'w> ProductionCallViewV1<'s, 'w> {
    /// Original root-qualified caller association.
    pub const fn caller(&self) -> &'s SemanticKirFunctionCorrespondenceV1 {
        self.caller
    }
    /// Original source call with unexpanded operands and destination.
    pub const fn source(&self) -> &'s SemanticDirectCallV1 {
        self.source
    }
    /// Original actual Call, with zero or one scalar result.
    pub const fn operation(&self) -> &'s Operation {
        self.operation
    }
    /// Original caller block including its checked continuation.
    pub const fn block(&self) -> &'s BasicBlock {
        self.block
    }
    /// Same complete entry view used by standalone entry consumers.
    pub fn callee(&mut self) -> &mut ProductionArgumentViewV1<'s, 'w> {
        &mut self.entry
    }
    /// Exact destination finishing and applicable preparation.
    pub const fn destination(&self) -> ProductionCallDestinationV1<'s> {
        self.destination
    }
    /// No immediate transport is not evidence that a scalar result was discarded.
    pub const fn result_transport(&self) -> Option<ProductionCallResultTransportV1<'s>> {
        self.transport
    }
    /// Original source-plan definitions on the returning edge, including zero-width locals.
    pub fn edge_definitions(
        &mut self,
    ) -> Result<&'s [fe2o3_mir_model::SsaArgumentV1], ProductionSemanticKirErrorV1> {
        self.entry
            .budget
            .charge_work(argument_sum_v1(&[self.definitions.len(), 1])?)?;
        Ok(self.definitions)
    }
    /// Original source-plan arguments; source ordinals are not physical edge slots.
    pub fn edge_arguments(
        &mut self,
    ) -> Result<&'s [fe2o3_mir_model::SsaArgumentV1], ProductionSemanticKirErrorV1> {
        self.entry
            .budget
            .charge_work(argument_sum_v1(&[self.arguments.len(), 1])?)?;
        Ok(self.arguments)
    }
    /// Joins by checked physical slot, allowing repeated caller ValueIds.
    pub fn physical(
        &mut self,
        slot: usize,
    ) -> Result<Option<ProductionCallArgumentV1<'s>>, ProductionSemanticKirErrorV1> {
        let Some(parameter) = self.entry.physical(slot)? else {
            return Ok(None);
        };
        let OperationKind::Call { arguments, .. } = &self.operation.kind else {
            unreachable!("checked call view");
        };
        Ok(Some(ProductionCallArgumentV1 {
            caller_value: arguments[slot],
            parameter,
        }))
    }
    /// Visits every entry node with its original caller source operand.
    pub fn visit_arguments(
        &mut self,
        mut visit: impl for<'n> FnMut(
            ProductionCallArgumentNodeV1<'n>,
        ) -> Result<(), ProductionSemanticKirErrorV1>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let source = self.source;
        self.entry.visit_nodes(|parameter| {
            let operand = &source.arguments()[parameter.source_argument() as usize];
            visit(ProductionCallArgumentNodeV1 { operand, parameter })
        })
    }
    /// Visits all callee returns, retaining function-local value identities.
    pub fn visit_returns(
        &mut self,
        mut visit: impl FnMut(ProductionCallReturnV1<'s>) -> Result<(), ProductionSemanticKirErrorV1>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let body = self
            .entry
            .data
            .target
            .body
            .as_ref()
            .expect("checked helper body");
        self.entry
            .budget
            .charge_work(argument_product_v1(body.blocks.len(), 42)?)?;
        for block in &body.blocks {
            if !matches!(block.terminator, Some(Terminator::Return { .. })) {
                continue;
            }
            let index = self
                .returns
                .binary_search_by_key(&block.id.0, |row| row.semantic_block.index())
                .expect("checked return coverage");
            let SemanticKirCallReturnKindV1::Return { input, conversion } =
                self.returns[index].kind
            else {
                unreachable!("checked return anchor");
            };
            visit(ProductionCallReturnV1 {
                block,
                input,
                conversion: conversion.map(|ordinal| &block.operations[ordinal as usize]),
            })?;
        }
        Ok(())
    }
}

impl ProductionSemanticKirOwnerV1 {
    /// Borrows a checked call, its complete callee entry ABI and zero/scalar results.
    ///
    /// ```compile_fail
    /// use fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1;
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// use fe2o3_mir_model::semantic_mir_v1::{SemanticFunctionIdV1 as F, SemanticBlockIdV1 as B};
    /// fn escape(owner: &ProductionSemanticKirOwnerV1, budget: &mut Budget<'_>, f: F, b: B) {
    ///     let saved = owner.with_checked_call_v1(f, f, b, budget, |view| Ok(view.operation())).unwrap();
    ///     println!("{saved:?}");
    /// }
    /// ```
    pub fn with_checked_call_v1<'w, R>(
        &self,
        root: SemanticFunctionIdV1,
        caller: SemanticFunctionIdV1,
        block: SemanticBlockIdV1,
        budget: &mut ArgumentBudgetV1<'w>,
        use_view: impl for<'s> FnOnce(
            &mut ProductionCallViewV1<'s, 'w>,
        ) -> Result<R, ProductionSemanticKirErrorV1>,
    ) -> Result<R, ProductionSemanticKirErrorV1> {
        with_owner_call_v1(
            &self.semantic_ssa,
            self.module(),
            &self.correspondence,
            (root, caller, block),
            budget,
            use_view,
        )
    }
}

impl ProductionPreRankedKirOwnerV1 {
    /// Same scoped call view for the production pre-ranked owner; no proof/launch authority.
    ///
    /// ```compile_fail
    /// use fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1;
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// use fe2o3_mir_model::semantic_mir_v1::{SemanticFunctionIdV1 as F, SemanticBlockIdV1 as B};
    /// fn escape(owner: &ProductionPreRankedKirOwnerV1, budget: &mut Budget<'_>, f: F, b: B) {
    ///     let mut saved = None;
    ///     owner.with_checked_call_v1(f, f, b, budget, |view| {
    ///         view.visit_returns(|site| { saved = Some(site); Ok(()) })
    ///     }).unwrap();
    ///     drop(saved);
    /// }
    /// ```
    pub fn with_checked_call_v1<'w, R>(
        &self,
        root: SemanticFunctionIdV1,
        caller: SemanticFunctionIdV1,
        block: SemanticBlockIdV1,
        budget: &mut ArgumentBudgetV1<'w>,
        use_view: impl for<'s> FnOnce(
            &mut ProductionCallViewV1<'s, 'w>,
        ) -> Result<R, ProductionSemanticKirErrorV1>,
    ) -> Result<R, ProductionSemanticKirErrorV1> {
        with_owner_call_v1(
            &self.semantic_ssa,
            self.executable.module(),
            &self.correspondence,
            (root, caller, block),
            budget,
            use_view,
        )
    }
}

fn with_owner_call_v1<'w, R>(
    owner: &ProductionSemanticSsaOwnerV1,
    module: &Module,
    rows: &SemanticKirCorrespondenceV1,
    selected: (
        SemanticFunctionIdV1,
        SemanticFunctionIdV1,
        SemanticBlockIdV1,
    ),
    budget: &mut ArgumentBudgetV1<'w>,
    use_view: impl for<'s> FnOnce(
        &mut ProductionCallViewV1<'s, 'w>,
    ) -> Result<R, ProductionSemanticKirErrorV1>,
) -> Result<R, ProductionSemanticKirErrorV1> {
    let floor = budget.storage();
    let result = with_call_view_v1(owner, module, rows, selected, budget, use_view);
    budget.release_storage(budget.storage() - floor)?;
    result
}

fn with_call_view_v1<'w, R>(
    owner: &ProductionSemanticSsaOwnerV1,
    module: &Module,
    rows: &SemanticKirCorrespondenceV1,
    selected: (
        SemanticFunctionIdV1,
        SemanticFunctionIdV1,
        SemanticBlockIdV1,
    ),
    budget: &mut ArgumentBudgetV1<'w>,
    use_view: impl for<'s> FnOnce(
        &mut ProductionCallViewV1<'s, 'w>,
    ) -> Result<R, ProductionSemanticKirErrorV1>,
) -> Result<R, ProductionSemanticKirErrorV1> {
    use fe2o3_mir_model::{SsaBlockIdV1, SsaEdgeIdV1};
    let mismatch = || ProductionSemanticKirErrorV1::CorrespondenceMismatch;
    let semantic = owner.source_semantic();
    let index_floor = budget.storage();
    let targets = CallTargetIndexV1::new(module, &rows.lowered_functions, budget)?;
    let (caller, target) = targets.source(selected.0, selected.1, budget)?;
    budget.charge_work(argument_product_v1(
        argument_sum_v1(&[
            rows.call_returns.len(),
            rows.terminator_operation_spans.len(),
            2,
        ])?,
        2,
    )?)?;
    let group = |function| {
        argument_group_v1(&rows.call_returns, |row| {
            row.correspondence_owner == selected.0 && row.semantic_function == function
        })
    };
    let spans = |function| {
        argument_group_v1(&rows.terminator_operation_spans, |row| {
            row.correspondence_owner == selected.0 && row.semantic_function == function
        })
    };
    let caller_rows = group(selected.1);
    let caller_spans = spans(selected.1);
    validate_call_correspondence_v1(
        owner,
        caller,
        target,
        &targets,
        caller_rows,
        caller_spans,
        budget,
    )?;
    budget.charge_work(40)?;
    let row = &caller_rows[caller_rows
        .binary_search_by_key(&selected.2, |row| row.semantic_block)
        .map_err(|_| mismatch())?];
    let SemanticKirCallReturnKindV1::Call {
        arguments_first,
        call_operation,
        destination_end,
        destination,
        transport,
    } = row.kind
    else {
        return Err(mismatch());
    };
    let source_function = &semantic.functions()[selected.1.index() as usize];
    let SemanticTerminatorKindV1::Call(source) = source_function.blocks()
        [selected.2.index() as usize]
        .terminator()
        .kind()
    else {
        return Err(mismatch());
    };
    let Some(SemanticCallableDeclV1::Defined { function: callee }) =
        semantic.callables().get(source.callee().index() as usize)
    else {
        return Err(mismatch());
    };
    let (callee_instance, callee_target) = targets.source(selected.0, *callee, budget)?;
    let returns = group(*callee);
    let callee_spans = spans(*callee);
    validate_call_correspondence_v1(
        owner,
        callee_instance,
        callee_target,
        &targets,
        returns,
        callee_spans,
        budget,
    )?;
    let body = target.body.as_ref().ok_or_else(mismatch)?;
    budget.charge_work(argument_sum_v1(&[body.blocks.len(), caller_spans.len()])?)?;
    let block = body
        .blocks
        .iter()
        .find(|block| block.id.0 == selected.2.index())
        .ok_or_else(mismatch)?;
    let span = caller_spans
        .iter()
        .find(|span| span.semantic_block == selected.2)
        .ok_or_else(mismatch)?;
    let operation = &block.operations[call_operation as usize];
    let destination = match destination {
        SemanticKirCallDestinationV1::Local => ProductionCallDestinationV1::Local,
        SemanticKirCallDestinationV1::Retained { .. } => {
            ProductionCallDestinationV1::Retained(&block.operations[destination_end as usize - 1])
        }
        SemanticKirCallDestinationV1::Projected { .. } => ProductionCallDestinationV1::Projected {
            preparation: &block.operations
                [span.first_operation_ordinal as usize..arguments_first as usize],
            store: &block.operations[destination_end as usize - 1],
        },
    };
    let transport = transport.map(|transport| {
        let Some(Terminator::Branch { arguments, .. }) = &block.terminator else {
            unreachable!("checked continuation");
        };
        ProductionCallResultTransportV1 {
            slot: transport.slot,
            value: arguments[transport.slot as usize],
            conversion: transport
                .conversion
                .map(|ordinal| &block.operations[ordinal as usize]),
        }
    });
    let plan = owner
        .plan_for_function(selected.1)
        .ok_or_else(mismatch)?
        .plan();
    let edge = SsaEdgeIdV1::new(SsaBlockIdV1::new(selected.2.index()), 0);
    let definitions = plan.edge_definitions(edge).ok_or_else(mismatch)?;
    let arguments = plan.edge_arguments(edge).ok_or_else(mismatch)?;
    drop(targets);
    budget.release_storage(budget.storage() - index_floor)?;
    budget.charge_work(argument_sum_v1(&[
        rows.parameter_bindings.len(),
        rows.parameter_component_bindings.len(),
        rows.ignored_parameter_bindings.len(),
        3,
    ])?)?;
    let same = |root, function| root == selected.0 && function == *callee;
    with_parameter_correspondence_v1(
        semantic,
        callee_instance,
        callee_target,
        ArgumentTraceV1 {
            direct: argument_group_v1(&rows.parameter_bindings, |row| {
                same(row.correspondence_owner, row.semantic_function)
            }),
            components: argument_group_v1(&rows.parameter_component_bindings, |row| {
                same(row.correspondence_owner, row.semantic_function)
            }),
            ignored: argument_group_v1(&rows.ignored_parameter_bindings, |row| {
                same(row.correspondence_owner, row.semantic_function)
            }),
        },
        budget,
        |entry| {
            let mut view = ProductionCallViewV1 {
                entry: ProductionArgumentViewV1 {
                    data: entry.data,
                    budget: &mut *entry.budget,
                },
                caller,
                source,
                block,
                operation,
                destination,
                transport,
                definitions,
                arguments,
                returns,
            };
            use_view(&mut view)
        },
    )
}

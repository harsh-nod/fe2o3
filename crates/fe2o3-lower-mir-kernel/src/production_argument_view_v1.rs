pub use source_arguments_v1::{
    ProductionArgumentCoverageV1, ProductionArgumentNodeV1, ProductionArgumentProjectionV1,
    ProductionArgumentTraceV1, ProductionPhysicalArgumentV1,
};

/// Scoped checked entry correspondence over one owner-qualified function.
///
/// This borrows existing MIR/KIR and temporary indices; it is not an executable
/// graph, current SSA provenance, borrow authority, or a proof/launch receipt.
/// All indices stay charged while the view lives. Ordinary Result returns
/// restore scratch accounting; unwinding and allocator/RSS bounds are not claimed.
///
/// Copied inert identities may leave a visit; borrowed nodes and paths may not.
/// ```
/// use fe2o3_lower_mir_kernel::{ProductionArgumentViewV1, ProductionSemanticKirErrorV1};
/// fn copy_ids(view: &mut ProductionArgumentViewV1<'_, '_>) -> Result<Vec<u32>, ProductionSemanticKirErrorV1> {
///     let mut ids = Vec::new();
///     view.visit_nodes(|node| { ids.push(node.semantic_type().index()); Ok(()) })?;
///     Ok(ids)
/// }
/// ```
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionArgumentViewV1;
/// fn forge(view: &mut ProductionArgumentViewV1<'_, '_>) {
///     let _ = ProductionArgumentViewV1 { data: view.data, budget: view.budget };
/// }
/// ```
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionArgumentViewV1;
/// fn retain_path(view: &mut ProductionArgumentViewV1<'_, '_>) {
///     let mut paths = Vec::new();
///     view.visit_nodes(|node| { paths.push(node.source_path()); Ok(()) }).unwrap();
///     assert!(!paths.is_empty());
/// }
/// ```
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionArgumentViewV1;
/// fn retain_node(view: &mut ProductionArgumentViewV1<'_, '_>) {
///     let mut saved = None;
///     view.visit_nodes(|node| { saved = Some(node); Ok(()) }).unwrap();
///     assert!(saved.is_some());
/// }
/// ```
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionArgumentViewV1;
/// fn retain_parameter(view: &mut ProductionArgumentViewV1<'_, '_>) {
///     let mut saved = None;
///     view.visit_nodes(|node| { saved = Some(node.coverage()); Ok(()) }).unwrap();
///     assert!(saved.is_some());
/// }
/// ```
pub struct ProductionArgumentViewV1<'s, 'w> {
    data: source_arguments_v1::ProductionArgumentViewV1<'s>,
    budget: &'s mut ArgumentBudgetV1<'w>,
}

impl<'s, 'w> ProductionArgumentViewV1<'s, 'w> {
    /// Exact root, semantic function, physical function, and role association.
    pub fn association(&self) -> &'s SemanticKirFunctionCorrespondenceV1 {
        self.data.association()
    }
    /// Prepays a complete traversal of whole-source mappings, including empty tuples.
    pub fn source_arguments(
        &mut self,
    ) -> Result<
        impl ExactSizeIterator<Item = fe2o3_mir_model::SemanticSourceArgumentV1<'s>> + use<'s>,
        ProductionSemanticKirErrorV1,
    > {
        self.data.source_arguments(self.budget).map_err(Into::into)
    }
    /// Prepays a complete traversal of adjusted FnAbi rows, including ignored arguments.
    pub fn adjusted_arguments(
        &mut self,
    ) -> Result<
        impl ExactSizeIterator<Item = fe2o3_mir_model::SemanticAdjustedArgumentV1<'s>> + use<'s>,
        ProductionSemanticKirErrorV1,
    > {
        self.data
            .adjusted_arguments(self.budget)
            .map_err(Into::into)
    }
    /// Whole-local ignored row, never a fabricated row for a nested zero field.
    pub fn ignored_local(
        &mut self,
        local: SemanticLocalIdV1,
    ) -> Result<Option<&'s SemanticKirIgnoredParameterBindingV1>, ProductionSemanticKirErrorV1>
    {
        self.data
            .ignored_local(self.budget, local)
            .map_err(Into::into)
    }
    /// Looks up an actual signature slot with bounded, charged index work.
    pub fn physical(
        &mut self,
        slot: usize,
    ) -> Result<Option<ProductionPhysicalArgumentV1<'s>>, ProductionSemanticKirErrorV1> {
        self.data.physical(self.budget, slot).map_err(Into::into)
    }
    /// Visits every structural node in postorder, including zero components.
    /// Repeated traversal accumulates work; visitor errors release traversal scratch.
    pub fn visit_nodes(
        &mut self,
        mut visit: impl for<'n> FnMut(
            ProductionArgumentNodeV1<'n>,
        ) -> Result<(), ProductionSemanticKirErrorV1>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let mut visitor_error = None;
        let result = self.data.visit_nodes(self.budget, |node| {
            visit(node).map_err(|error| {
                visitor_error = Some(error);
                source_arguments_v1::ProductionSourceArgumentErrorV1::Visitor
            })
        });
        argument_visitor_result_v1(result, visitor_error)
    }
}

impl ProductionSemanticKirOwnerV1 {
    /// Borrows complete argument correspondence from this immutable owner only.
    ///
    /// Resolves `(root, function)` through the owner's validated function roster;
    /// a root's selected body may differ from the root itself. The callback cannot
    /// retain the view, its scratch-backed paths, or the exclusive budget borrow.
    ///
    /// ```compile_fail
    /// use fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1;
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// use fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1 as Id;
    /// fn escape_parameter(owner: &ProductionSemanticKirOwnerV1, budget: &mut Budget<'_>, id: Id) {
    ///     let saved = owner.with_checked_arguments_v1(id, id, budget, |view| view.physical(0)).unwrap();
    ///     assert!(saved.is_some());
    /// }
    /// ```
    /// ```compile_fail
    /// use fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1;
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// use fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1 as Id;
    /// fn escape(owner: &ProductionSemanticKirOwnerV1, budget: &mut Budget<'_>, id: Id) {
    ///     let mut saved = None;
    ///     owner.with_checked_arguments_v1(id, id, budget, |view| { saved = Some(view); Ok(()) }).unwrap();
    ///     assert!(saved.is_some());
    /// }
    /// ```
    /// ```compile_fail
    /// use fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1;
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// use fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1 as Id;
    /// fn change_budget(owner: &ProductionSemanticKirOwnerV1, budget: &mut Budget<'_>, id: Id) {
    ///     owner.with_checked_arguments_v1(id, id, budget, |_| {
    ///         budget.charge_work(1).unwrap();
    ///         Ok(())
    ///     }).unwrap();
    /// }
    /// ```
    pub fn with_checked_arguments_v1<'w, R>(
        &self,
        root: SemanticFunctionIdV1,
        function: SemanticFunctionIdV1,
        budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'w>,
        use_view: impl for<'s> FnOnce(
            &mut ProductionArgumentViewV1<'s, 'w>,
        ) -> Result<R, ProductionSemanticKirErrorV1>,
    ) -> Result<R, ProductionSemanticKirErrorV1> {
        with_owner_arguments_v1(
            self.semantic_ssa.source_semantic(),
            self.module(),
            &self.correspondence,
            (root, function),
            budget,
            use_view,
        )
    }
}

impl ProductionPreRankedKirOwnerV1 {
    /// Borrows the same complete entry view from the immutable pre-ranked V12 owner.
    /// This grants no ranked verification, functional proof, or launch authority.
    ///
    /// ```compile_fail
    /// use fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1;
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// use fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1 as Id;
    /// fn escape_sources(owner: &ProductionPreRankedKirOwnerV1, budget: &mut Budget<'_>, id: Id) {
    ///     let saved = owner.with_checked_arguments_v1(id, id, budget, |view| view.source_arguments()).unwrap();
    ///     assert!(saved.len() > 0);
    /// }
    /// ```
    pub fn with_checked_arguments_v1<'w, R>(
        &self,
        root: SemanticFunctionIdV1,
        function: SemanticFunctionIdV1,
        budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'w>,
        use_view: impl for<'s> FnOnce(
            &mut ProductionArgumentViewV1<'s, 'w>,
        ) -> Result<R, ProductionSemanticKirErrorV1>,
    ) -> Result<R, ProductionSemanticKirErrorV1> {
        with_owner_arguments_v1(
            self.semantic_ssa.source_semantic(),
            self.executable.module(),
            &self.correspondence,
            (root, function),
            budget,
            use_view,
        )
    }
}

fn with_owner_arguments_v1<'w, R>(
    semantic: &AdmittedInertSemanticMirV1,
    module: &Module,
    rows: &SemanticKirCorrespondenceV1,
    selected: (SemanticFunctionIdV1, SemanticFunctionIdV1),
    budget: &mut ArgumentBudgetV1<'w>,
    use_view: impl for<'s> FnOnce(
        &mut ProductionArgumentViewV1<'s, 'w>,
    ) -> Result<R, ProductionSemanticKirErrorV1>,
) -> Result<R, ProductionSemanticKirErrorV1> {
    budget.charge_work(argument_sum_v1(&[
        rows.lowered_functions.len(),
        rows.parameter_bindings.len(),
        rows.parameter_component_bindings.len(),
        rows.ignored_parameter_bindings.len(),
        4,
    ])?)?;
    let same = |owner, semantic| owner == selected.0 && semantic == selected.1;
    let instance = rows
        .lowered_functions
        .iter()
        .find(|row| same(row.correspondence_owner, row.semantic_function))
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    budget.charge_work(argument_product_v1(
        module.functions.len(),
        argument_sum_v1(&[instance.kernel_ir_function.as_str().len(), 1])?,
    )?)?;
    let target = module
        .function(&instance.kernel_ir_function)
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    with_parameter_correspondence_v1(
        semantic,
        instance,
        target,
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
        use_view,
    )
}

fn argument_group_v1<T>(rows: &[T], same: impl Fn(&T) -> bool) -> &[T] {
    let start = rows.iter().position(&same).unwrap_or(rows.len());
    let count = rows[start..].iter().take_while(|row| same(row)).count();
    &rows[start..start + count]
}

impl ProductionPreRankedKirOwnerV1 {
    /// Authenticate an entry relation borrowing original owners and a live meter.
    /// The complete ABI checker runs here and on every subsequent binding replay.
    /// The meter cannot be reset at the same address while the relation survives.
    ///
    /// ```compile_fail,E0506
    /// use fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1;
    /// use fe2o3_kernel_ir::{CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    ///     CanonicalKernelIrWorkBudgetV1 as Work};
    /// use fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1 as Id;
    /// fn reset(owner: &ProductionPreRankedKirOwnerV1, root: Id) {
    ///     let mut work = Box::new(Work::new(usize::MAX));
    ///     let mut budget = Budget::new(&mut work, usize::MAX);
    ///     let relation = owner.checked_source_argument_relation_v1(root, root, &mut budget).unwrap();
    ///     drop(budget);
    ///     *work = Work::new(usize::MAX);
    ///     let mut replacement = Budget::new(&mut work, usize::MAX);
    ///     relation.replay_v1(relation.canonical_module(), relation.canonical_function(), &mut replacement).unwrap();
    /// }
    /// ```
    ///
    /// Neither the relation nor a checked binding can escape an owned ledger view.
    ///
    /// ```compile_fail
    /// use fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1;
    /// use fe2o3_kernel_ir::CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned;
    /// use fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1 as Id;
    /// fn escape(owner: &ProductionPreRankedKirOwnerV1, ledger: &mut Owned, root: Id) {
    ///     let relation = ledger.with_budget(|budget| {
    ///         owner.checked_source_argument_relation_v1(root, root, budget).unwrap()
    ///     });
    ///     let _ = relation.association();
    /// }
    /// ```
    ///
    /// ```compile_fail
    /// use fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1;
    /// use fe2o3_kernel_ir::{CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned, ValueId};
    /// use fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1 as Id;
    /// fn escape(owner: &ProductionPreRankedKirOwnerV1, ledger: &mut Owned, root: Id, value: ValueId) {
    ///     let binding = ledger.with_budget(|budget| {
    ///         let relation = owner.checked_source_argument_relation_v1(root, root, budget).unwrap();
    ///         relation.bind_whole_parameter_v1(0, value, budget).unwrap()
    ///     });
    ///     let _ = binding.source_argument();
    /// }
    /// ```
    ///
    /// A binding alone also keeps the original meter live after its relation drops.
    ///
    /// ```compile_fail,E0506
    /// use fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1;
    /// use fe2o3_kernel_ir::{CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    ///     CanonicalKernelIrWorkBudgetV1 as Work, ValueId};
    /// use fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1 as Id;
    /// fn reset(owner: &ProductionPreRankedKirOwnerV1, root: Id, value: ValueId) {
    ///     let mut work = Box::new(Work::new(usize::MAX));
    ///     let mut budget = Budget::new(&mut work, usize::MAX);
    ///     let relation = owner.checked_source_argument_relation_v1(root, root, &mut budget).unwrap();
    ///     let binding = relation.bind_whole_parameter_v1(0, value, &mut budget).unwrap();
    ///     drop(relation);
    ///     drop(budget);
    ///     *work = Work::new(usize::MAX);
    ///     let _ = binding.source_argument();
    /// }
    /// ```
    pub fn checked_source_argument_relation_v1<'source, 'work>(
        &'source self,
        root: SemanticFunctionIdV1,
        function: SemanticFunctionIdV1,
        budget: &mut ArgumentBudgetV1<'work>,
    ) -> Result<
        fe2o3_pliron::ProductionSourceArgumentRelationV1<'source, 'work>,
        ProductionSemanticKirErrorV1,
    > {
        let rows = &self.correspondence;
        budget.charge_work(argument_sum_v1(&[
            rows.lowered_functions.len(),
            rows.parameter_bindings.len(),
            rows.parameter_component_bindings.len(),
            rows.ignored_parameter_bindings.len(),
            4,
        ])?)?;
        if budget.storage() < self.retained_analysis_storage_v1() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        let same = |owner, semantic| owner == root && semantic == function;
        let mut associations = rows
            .lowered_functions
            .iter()
            .filter(|row| same(row.correspondence_owner, row.semantic_function));
        let association = associations
            .next()
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        if associations.next().is_some()
            || association.role != SemanticKirFunctionRoleV1::KernelEntry
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        self.semantic_ssa
            .check_source_arguments_v1(
                self.executable.verified_module_ref_v1(),
                association,
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
            )
            .map_err(Into::into)
    }
}

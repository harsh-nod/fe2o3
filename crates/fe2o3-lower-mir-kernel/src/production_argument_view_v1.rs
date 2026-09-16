/// A source/local projection in the borrowed view, not a correspondence wire tag.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionArgumentProjectionV1 {
    /// A tuple or nominal aggregate field.
    Field(u32),
    /// One array element; zero-sized marker arrays retain their full source index.
    ArrayIndex(u64),
}

/// The exact existing emission row for a physical parameter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionArgumentTraceV1<'a> {
    /// One whole local supplies this parameter.
    Direct(&'a SemanticKirParameterBindingV1),
    /// One local projection supplies this parameter.
    Component(&'a SemanticKirParameterComponentBindingV1),
}

/// A borrowed actual KIR parameter, not an ABI byte offset or launch authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionPhysicalArgumentV1<'a> {
    slot: usize,
    value: ValueId,
    ty: &'a Type,
    trace: ProductionArgumentTraceV1<'a>,
}

impl<'a> ProductionPhysicalArgumentV1<'a> {
    /// Actual signature/body parameter ordinal, including no ignored arguments.
    pub const fn slot(self) -> usize {
        self.slot
    }
    /// Actual body parameter value identity.
    pub const fn value(self) -> ValueId {
        self.value
    }
    /// Exact KIR signature type.
    pub const fn ty(&self) -> &'a Type {
        self.ty
    }
    /// Borrowed direct/component emission row.
    pub const fn trace(&self) -> ProductionArgumentTraceV1<'a> {
        self.trace
    }
}

/// Physical coverage of a typed source node. Containment is not extraction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionArgumentCoverageV1<'a> {
    /// The node has no physical parameter, including nested zero-sized fields.
    Zero,
    /// A nonempty half-open range of signature slots in source traversal order.
    Components {
        /// First included signature slot.
        first: usize,
        /// First excluded signature slot.
        end: usize,
    },
    /// This node is represented by the complete physical parameter.
    Parameter(ProductionPhysicalArgumentV1<'a>),
    /// This field is contained in a carrier, without an independent KIR value.
    WithinAtomicParameter(ProductionPhysicalArgumentV1<'a>),
}

/// One postorder source node, borrowed only for the current visitor call.
#[derive(Debug)]
pub struct ProductionArgumentNodeV1<'a> {
    source: fe2o3_mir_model::SemanticSourceArgumentV1<'a>,
    adjusted: Option<fe2o3_mir_model::SemanticAdjustedArgumentV1<'a>>,
    ty: SemanticTypeIdV1,
    source_path: &'a [ProductionArgumentProjectionV1],
    local: Option<(SemanticLocalIdV1, &'a [ProductionArgumentProjectionV1])>,
    ignored: Option<&'a SemanticKirIgnoredParameterBindingV1>,
    coverage: ProductionArgumentCoverageV1<'a>,
}

impl ProductionArgumentNodeV1<'_> {
    /// Source signature argument ordinal, before RustCall tuple expansion.
    pub const fn source_argument(&self) -> u32 {
        self.source.ordinal()
    }
    /// Adjusted ABI ordinal, or none for a RustCall source tuple envelope.
    pub const fn adjusted_argument(&self) -> Option<u32> {
        match self.adjusted {
            Some(mapped) => Some(mapped.ordinal()),
            None => None,
        }
    }
    /// Whole-source binding and ownership, borrowed from the same logical map.
    pub const fn source(&self) -> fe2o3_mir_model::SemanticSourceArgumentV1<'_> {
        self.source
    }
    /// Exact original adjusted FnAbi mapping, absent only for source envelopes.
    pub const fn adjusted(&self) -> Option<fe2o3_mir_model::SemanticAdjustedArgumentV1<'_>> {
        self.adjusted
    }
    /// Applicable whole-local ignored row, not a fabricated row for this field.
    pub const fn ignored_local_binding(&self) -> Option<&SemanticKirIgnoredParameterBindingV1> {
        self.ignored
    }
    /// Exact semantic type of this structural node.
    pub const fn semantic_type(&self) -> SemanticTypeIdV1 {
        self.ty
    }
    /// Projection from the whole source argument, not from its MIR local.
    pub fn source_path(&self) -> &[ProductionArgumentProjectionV1] {
        self.source_path
    }
    /// Exact local and local-relative projection; expanded empty tuples have none.
    pub fn local_binding(&self) -> Option<(SemanticLocalIdV1, &[ProductionArgumentProjectionV1])> {
        self.local
    }
    /// Checked signature-slot coverage of this node.
    pub const fn coverage(&self) -> ProductionArgumentCoverageV1<'_> {
        self.coverage
    }
}

#[derive(Clone, Copy)]
struct AdjustedArgumentShapeV1 {
    first: usize,
    end: usize,
    atomic: bool,
}

struct ArgumentViewDataV1<'a> {
    semantic: &'a AdmittedInertSemanticMirV1,
    instance: &'a SemanticKirFunctionCorrespondenceV1,
    target: &'a Function,
    logical: &'a fe2o3_mir_model::SemanticLogicalArgumentMapV1<'a>,
    physical: &'a [IndexedArgumentTraceV1<'a>],
    ignored: &'a [Option<&'a SemanticKirIgnoredParameterBindingV1>],
    shapes: &'a [AdjustedArgumentShapeV1],
}

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
    data: ArgumentViewDataV1<'s>,
    budget: &'s mut ArgumentBudgetV1<'w>,
}

impl<'s, 'w> ProductionArgumentViewV1<'s, 'w> {
    /// Exact root, semantic function, physical function, and role association.
    pub fn association(&self) -> &'s SemanticKirFunctionCorrespondenceV1 {
        self.data.instance
    }
    /// Prepays a complete traversal of whole-source mappings, including empty tuples.
    pub fn source_arguments(
        &mut self,
    ) -> Result<
        impl ExactSizeIterator<Item = fe2o3_mir_model::SemanticSourceArgumentV1<'s>> + use<'s>,
        ProductionSemanticKirErrorV1,
    > {
        self.budget.charge_work(argument_sum_v1(&[
            1,
            argument_product_v1(self.data.logical.source_arguments().len(), 12)?,
        ])?)?;
        let logical: &'s _ = self.data.logical;
        Ok(logical.source_arguments())
    }
    /// Prepays a complete traversal of adjusted FnAbi rows, including ignored arguments.
    pub fn adjusted_arguments(
        &mut self,
    ) -> Result<
        impl ExactSizeIterator<Item = fe2o3_mir_model::SemanticAdjustedArgumentV1<'s>> + use<'s>,
        ProductionSemanticKirErrorV1,
    > {
        self.budget.charge_work(argument_sum_v1(&[
            1,
            argument_product_v1(self.data.logical.adjusted_arguments().len(), 12)?,
        ])?)?;
        let logical: &'s _ = self.data.logical;
        Ok(logical.adjusted_arguments())
    }
    /// Whole-local ignored row, never a fabricated row for a nested zero field.
    pub fn ignored_local(
        &mut self,
        local: SemanticLocalIdV1,
    ) -> Result<Option<&'s SemanticKirIgnoredParameterBindingV1>, ProductionSemanticKirErrorV1>
    {
        self.budget.charge_work(4)?;
        Ok(self
            .data
            .ignored
            .get(local.index() as usize)
            .copied()
            .flatten())
    }
    /// Looks up an actual signature slot with bounded, charged index work.
    pub fn physical(
        &mut self,
        slot: usize,
    ) -> Result<Option<ProductionPhysicalArgumentV1<'s>>, ProductionSemanticKirErrorV1> {
        self.data.physical(slot, self.budget)
    }
    /// Visits every structural node in postorder, including zero components.
    /// Pointees and active enum variants are not inferred from entry types.
    /// Repeated traversal accumulates work; visitor errors release traversal scratch.
    pub fn visit_nodes(
        &mut self,
        mut visit: impl for<'n> FnMut(
            ProductionArgumentNodeV1<'n>,
        ) -> Result<(), ProductionSemanticKirErrorV1>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let floor = self.budget.storage();
        let result = self.data.visit_nodes(self.budget, &mut visit);
        self.budget.release_storage(self.budget.storage() - floor)?;
        result
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

impl<'s> ArgumentViewDataV1<'s> {
    fn physical(
        &self,
        slot: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<ProductionPhysicalArgumentV1<'s>>, ProductionSemanticKirErrorV1> {
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
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let ty = self
            .target
            .signature
            .parameters
            .get(slot)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
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

    fn visit_nodes(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
        visit: &mut impl for<'n> FnMut(
            ProductionArgumentNodeV1<'n>,
        ) -> Result<(), ProductionSemanticKirErrorV1>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let function = &self.semantic.functions()[self.instance.semantic_function.index() as usize];
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
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
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
                visit(ProductionArgumentNodeV1 {
                    source,
                    adjusted: None,
                    ty: source.ty(),
                    source_path: &[],
                    local,
                    ignored: local.and_then(|(id, _)| {
                        self.ignored.get(id.index() as usize).copied().flatten()
                    }),
                    coverage: argument_composite_coverage_v1(first, slot),
                })?;
            }
        }
        if adjusted.next().is_some() {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        Ok(())
    }

    fn visit_adjusted(
        &self,
        source: fe2o3_mir_model::SemanticSourceArgumentV1<'_>,
        mapped: fe2o3_mir_model::SemanticAdjustedArgumentV1<'_>,
        shape: AdjustedArgumentShapeV1,
        budget: &mut ArgumentBudgetV1<'_>,
        visit: &mut impl for<'n> FnMut(
            ProductionArgumentNodeV1<'n>,
        ) -> Result<(), ProductionSemanticKirErrorV1>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let floor = budget.storage();
        let result = self.walk_adjusted(source, mapped, shape, budget, visit);
        budget.release_storage(budget.storage() - floor)?;
        result
    }

    fn walk_adjusted(
        &self,
        source: fe2o3_mir_model::SemanticSourceArgumentV1<'_>,
        mapped: fe2o3_mir_model::SemanticAdjustedArgumentV1<'_>,
        shape: AdjustedArgumentShapeV1,
        budget: &mut ArgumentBudgetV1<'_>,
        visit: &mut impl for<'n> FnMut(
            ProductionArgumentNodeV1<'n>,
        ) -> Result<(), ProductionSemanticKirErrorV1>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let prefix = mapped
            .tuple_field()
            .map(ProductionArgumentProjectionV1::Field);
        let mut emit = |node: ParameterStructureNodeV1<'_>, budget: &mut ArgumentBudgetV1<'_>| {
            budget.charge_work(1)?;
            let coverage = if node.physical.is_empty() {
                ProductionArgumentCoverageV1::Zero
            } else if shape.atomic
                || matches!(
                    self.semantic.types()[node.ty.index() as usize].shape(),
                    SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_)
                )
            {
                let slot = shape.first + node.physical.start;
                let physical = self
                    .physical(slot, budget)?
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
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
            visit(ProductionArgumentNodeV1 {
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
            })
        };
        if shape.atomic {
            visit_atomic_argument_structure_v1(
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
                &mut path,
                &mut output,
                &mut nodes,
                0,
                &mut |node| emit(node, budget),
            )?;
            if output.len() != shape.end - shape.first {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            Ok(())
        }
    }
}

fn argument_composite_coverage_v1(
    first: usize,
    end: usize,
) -> ProductionArgumentCoverageV1<'static> {
    if first == end {
        ProductionArgumentCoverageV1::Zero
    } else {
        ProductionArgumentCoverageV1::Components { first, end }
    }
}

struct AtomicArgumentFrameV1 {
    ty: SemanticTypeIdV1,
    next: u64,
    path_length: usize,
}

fn atomic_argument_child_v1(
    shape: &SemanticTypeShapeV1,
    index: u64,
) -> Result<Option<(SemanticTypeIdV1, ProductionArgumentProjectionV1)>, ArgumentResourceV1> {
    match shape {
        SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields) => {
            let offset = usize::try_from(index).map_err(|_| ArgumentResourceV1::Arithmetic)?;
            fields
                .fields()
                .get(offset)
                .copied()
                .map(|ty| {
                    let field = u32::try_from(index).map_err(|_| ArgumentResourceV1::Arithmetic)?;
                    Ok((ty, ProductionArgumentProjectionV1::Field(field)))
                })
                .transpose()
        }
        SemanticTypeShapeV1::Array { element, length } if index < *length => Ok(Some((
            *element,
            ProductionArgumentProjectionV1::ArrayIndex(index),
        ))),
        _ => Ok(None),
    }
}

fn argument_scratch_push_v1<T>(
    values: &mut Vec<T>,
    capacity: &mut usize,
    value: T,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ArgumentResourceV1> {
    budget.charge_work(1)?;
    if values.len() == *capacity {
        let next = argument_product_v1((*capacity).max(1), 2)?;
        budget.charge_work(*capacity)?;
        budget.reserve_storage(argument_product_v1(
            next - *capacity,
            std::mem::size_of::<T>(),
        )?)?;
        values
            .try_reserve_exact(next - values.len())
            .map_err(|_| ArgumentResourceV1::Allocation)?;
        *capacity = next;
    }
    values.push(value);
    Ok(())
}

// Atomic carriers are not scalarized. Their metadata tree can be deeper than
// the by-value component cap, so use a charged explicit stack and no pointee walk.
fn visit_atomic_argument_structure_v1(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    prefix: Option<ProductionArgumentProjectionV1>,
    budget: &mut ArgumentBudgetV1<'_>,
    visit: &mut impl FnMut(
        ParameterStructureNodeV1<'_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
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
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            let end = usize::from(declaration.layout().size_bytes() != Some(0));
            visit(
                ParameterStructureNodeV1 {
                    ty: frame.ty,
                    path: &path[..frame.path_length],
                    physical: 0..end,
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

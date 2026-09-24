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
    policy: ParameterLeafPolicyV1,
}

#[derive(Clone, Copy)]
struct ArgumentViewDataV1<'a> {
    work_ledger: ArgumentLedgerV1,
    budget_slot: usize,
    retained_floor: usize,
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
/// Queries require the original budget slot, work ledger, and retained floor.
///
/// Copied inert identities may leave a visit; borrowed nodes and paths may not.
pub struct ProductionArgumentViewV1<'s> {
    data: ArgumentViewDataV1<'s>,
}

impl<'s> ProductionArgumentViewV1<'s> {
    /// Lends the same immutable indices for a nested callback's shorter scope.
    pub fn reborrow_v1(&self) -> ProductionArgumentViewV1<'_> {
        ProductionArgumentViewV1 { data: self.data }
    }
    /// Original canonical function, in the same scope as the checked entry.
    pub const fn canonical_function(&self) -> &'s Function {
        self.data.target
    }
    /// Exact root, semantic function, physical function, and role association.
    pub fn association(&self) -> &'s SemanticKirFunctionCorrespondenceV1 {
        self.data.instance
    }
    /// Prepays a complete traversal of whole-source mappings, including empty tuples.
    pub fn source_arguments(
        &mut self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<
        impl ExactSizeIterator<Item = fe2o3_mir_model::SemanticSourceArgumentV1<'s>> + use<'s>,
        ProductionSourceArgumentErrorV1,
    > {
        self.data.check_query_v1(budget)?;
        budget.charge_work(argument_sum_v1(&[
            1,
            argument_product_v1(self.data.logical.source_arguments().len(), 12)?,
        ])?)?;
        let logical: &'s _ = self.data.logical;
        Ok(logical.source_arguments())
    }
    /// Prepays a complete traversal of adjusted FnAbi rows, including ignored arguments.
    pub fn adjusted_arguments(
        &mut self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<
        impl ExactSizeIterator<Item = fe2o3_mir_model::SemanticAdjustedArgumentV1<'s>> + use<'s>,
        ProductionSourceArgumentErrorV1,
    > {
        self.data.check_query_v1(budget)?;
        budget.charge_work(argument_sum_v1(&[
            1,
            argument_product_v1(self.data.logical.adjusted_arguments().len(), 12)?,
        ])?)?;
        let logical: &'s _ = self.data.logical;
        Ok(logical.adjusted_arguments())
    }
    /// Whole-local ignored row, never a fabricated row for a nested zero field.
    pub fn ignored_local(
        &mut self,
        budget: &mut ArgumentBudgetV1<'_>,
        local: SemanticLocalIdV1,
    ) -> Result<Option<&'s SemanticKirIgnoredParameterBindingV1>, ProductionSourceArgumentErrorV1>
    {
        self.data.check_query_v1(budget)?;
        budget.charge_work(4)?;
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
        budget: &mut ArgumentBudgetV1<'_>,
        slot: usize,
    ) -> Result<Option<ProductionPhysicalArgumentV1<'s>>, ProductionSourceArgumentErrorV1> {
        self.data.check_query_v1(budget)?;
        self.data.physical(slot, budget)
    }
    /// Visits every structural node in postorder, including zero components.
    /// Pointees and active enum variants are not inferred from entry types.
    /// Repeated traversal accumulates work; visitor errors release traversal scratch.
    pub fn visit_nodes(
        &mut self,
        budget: &mut ArgumentBudgetV1<'_>,
        mut visit: impl for<'n> FnMut(
            ProductionArgumentNodeV1<'n>,
        ) -> Result<(), ProductionSourceArgumentErrorV1>,
    ) -> Result<(), ProductionSourceArgumentErrorV1> {
        self.data.check_query_v1(budget)?;
        let floor = budget.storage();
        let result = self.data.visit_nodes(budget, &mut visit);
        budget.release_storage(
            budget
                .storage()
                .checked_sub(floor)
                .ok_or(ArgumentResourceV1::Accounting)?,
        )?;
        result
    }

    /// Visits a helper result's full pointer-free structure using the same
    /// representation walker as entry arguments. Paths are visitor-scoped;
    /// ordinary Result returns restore scratch, retaining work and peak history.
    pub fn visit_result_structure_v1(
        &self,
        ty: SemanticTypeIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
        mut visit: impl for<'n> FnMut(
            SemanticTypeIdV1,
            &'n [ProductionArgumentProjectionV1],
            std::ops::Range<usize>,
        ) -> Result<(), ProductionSourceArgumentErrorV1>,
    ) -> Result<(), ProductionSourceArgumentErrorV1> {
        self.data.check_query_v1(budget)?;
        let floor = budget.storage();
        let result = (|| {
            let types = self.data.semantic.types();
            prepay_typed_shape_v1(types, ty, 0, budget)?;
            append_parameter_structure_v1(
                types,
                ty,
                ParameterLeafPolicyV1::PointerFree,
                &mut Vec::new(),
                &mut Vec::new(),
                &mut 0,
                0,
                &mut |node| visit(node.ty, node.path, node.physical),
            )
        })();
        budget.release_storage(
            budget
                .storage()
                .checked_sub(floor)
                .ok_or(ArgumentResourceV1::Accounting)?,
        )?;
        result
    }
}

/// Copied coordinates only; authority remains in the private owner-bound binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionWholeSourceArgumentV1 {
    source: u32,
    adjusted: u32,
    local: SemanticLocalIdV1,
    ty: SemanticTypeIdV1,
}

impl ProductionWholeSourceArgumentV1 {
    pub const fn source_argument(self) -> u32 {
        self.source
    }
    pub const fn adjusted_argument(self) -> u32 {
        self.adjusted
    }
    pub const fn semantic_local(self) -> SemanticLocalIdV1 {
        self.local
    }
    pub const fn semantic_type(self) -> SemanticTypeIdV1 {
        self.ty
    }
}

impl ProductionArgumentViewV1<'_> {
    /// Resolve only a whole source argument, never a component or tuple envelope.
    pub fn whole_parameter_v1(
        &mut self,
        budget: &mut ArgumentBudgetV1<'_>,
        slot: usize,
        value: ValueId,
    ) -> Result<Option<ProductionWholeSourceArgumentV1>, ProductionSourceArgumentErrorV1> {
        let physical = self
            .physical(budget, slot)?
            .ok_or(ProductionSourceArgumentErrorV1::CorrespondenceMismatch)?;
        if physical.value() != value {
            return Err(ProductionSourceArgumentErrorV1::CorrespondenceMismatch);
        }
        let ProductionArgumentTraceV1::Direct(trace) = physical.trace() else {
            return Ok(None);
        };
        let mut argument = None;
        self.visit_nodes(budget, |node| {
            if !node.source_path().is_empty() {
                return Ok(());
            }
            let ProductionArgumentCoverageV1::Parameter(parameter) = node.coverage() else {
                return Ok(());
            };
            if parameter.slot() != slot || parameter.value() != value {
                return Ok(());
            }
            let Some((local, path)) = node.local_binding() else {
                return Err(ProductionSourceArgumentErrorV1::CorrespondenceMismatch);
            };
            if !path.is_empty() || local != trace.semantic_local() || argument.is_some() {
                return Err(ProductionSourceArgumentErrorV1::CorrespondenceMismatch);
            }
            argument = Some(ProductionWholeSourceArgumentV1 {
                source: node.source_argument(),
                adjusted: node
                    .adjusted_argument()
                    .ok_or(ProductionSourceArgumentErrorV1::CorrespondenceMismatch)?,
                local,
                ty: node.semantic_type(),
            });
            Ok(())
        })?;
        Ok(argument)
    }
}

impl<'s> ArgumentViewDataV1<'s> {
    fn require_live_v1(
        &self,
        budget: &ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSourceArgumentErrorV1> {
        if self.work_ledger != budget.work_ledger_identity_v1()
            || self.budget_slot != budget as *const ArgumentBudgetV1<'_> as usize
            || budget.storage() < self.retained_floor
        {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        Ok(())
    }

    fn check_query_v1(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSourceArgumentErrorV1> {
        budget.charge_work(5)?;
        self.require_live_v1(budget)
    }

    fn physical(
        &self,
        slot: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<ProductionPhysicalArgumentV1<'s>>, ProductionSourceArgumentErrorV1> {
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

    fn visit_nodes(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
        visit: &mut impl for<'n> FnMut(
            ProductionArgumentNodeV1<'n>,
        ) -> Result<(), ProductionSourceArgumentErrorV1>,
    ) -> Result<(), ProductionSourceArgumentErrorV1> {
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
            return Err(ProductionSourceArgumentErrorV1::CorrespondenceMismatch);
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
        ) -> Result<(), ProductionSourceArgumentErrorV1>,
    ) -> Result<(), ProductionSourceArgumentErrorV1> {
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
        ) -> Result<(), ProductionSourceArgumentErrorV1>,
    ) -> Result<(), ProductionSourceArgumentErrorV1> {
        let prefix = mapped
            .tuple_field()
            .map(ProductionArgumentProjectionV1::Field);
        let mut emit = |node: ParameterStructureNodeV1<'_>, budget: &mut ArgumentBudgetV1<'_>| {
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

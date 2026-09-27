// Source representation only. A source byte offset is not a kernarg offset.

/// One complete logical source argument, including a zero-sized argument.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionSourceAbiArgumentV1 {
    source: u32,
    local: SemanticLocalIdV1,
    ty: SemanticTypeIdV1,
    first: usize,
    end: usize,
}
impl ProductionSourceAbiArgumentV1 {
    /// Whole-source argument ordinal, not a physical parameter ordinal.
    pub const fn source_argument(self) -> u32 {
        self.source
    }
    /// Exact original MIR argument local.
    pub const fn local(self) -> SemanticLocalIdV1 {
        self.local
    }
    /// Exact captured source type, not a scalarized replacement type.
    pub const fn semantic_type(self) -> SemanticTypeIdV1 {
        self.ty
    }
    /// Dense physical signature coverage; an empty range explicitly means zero.
    pub fn physical_slots(self) -> std::ops::Range<usize> {
        self.first..self.end
    }
}

/// A checked source leaf paired with one actual physical KIR parameter.
/// This does not prescribe a device packing offset or authorize a host memcpy.
#[derive(Clone, Copy, Debug)]
pub struct ProductionSourceAbiComponentV1<'a> {
    source: u32,
    ty: SemanticTypeIdV1,
    source_offset: u64,
    physical: ProductionPhysicalArgumentV1<'a>,
}
impl<'a> ProductionSourceAbiComponentV1<'a> {
    /// Whole-source argument containing the leaf.
    pub const fn source_argument(self) -> u32 {
        self.source
    }
    /// Exact leaf type from the same admitted semantic owner.
    pub const fn semantic_type(self) -> SemanticTypeIdV1 {
        self.ty
    }
    /// Offset in the captured Rust source representation, never in kernarg bytes.
    pub const fn source_offset_bytes(self) -> u64 {
        self.source_offset
    }
    /// Actual signature slot, type, ValueId and immutable emission trace.
    pub const fn physical(self) -> ProductionPhysicalArgumentV1<'a> {
        self.physical
    }
}

#[derive(Clone, Copy)]
enum SourceAbiPlanFailureV1 {
    Mismatch,
    Resource(ArgumentResourceV1),
}
impl SourceAbiPlanFailureV1 {
    fn error(self) -> ProductionSemanticKirErrorV1 {
        match self {
            Self::Mismatch => ProductionSemanticKirErrorV1::CorrespondenceMismatch,
            Self::Resource(error) => error.into(),
        }
    }
}

/// Lexical, owner-bound source ABI plan for an authenticated kernel entry.
///
/// It preserves logical arguments, ignored arguments, exact rustc source-layout
/// offsets and the complete physical parameter census. It is not a descriptor,
/// optimized-output correspondence, host packer or artifact/launch authority.
/// The budget remains exclusively borrowed; no caller-supplied map is admitted.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionSourceAbiPlanV1;
/// fn duplicate(plan: ProductionSourceAbiPlanV1<'_, '_, '_>) { let _ = plan.clone(); }
/// ```
pub struct ProductionSourceAbiPlanV1<'scope, 'source, 'work> {
    semantic: &'source AdmittedInertSemanticMirV1,
    module: &'source Module,
    association: &'source SemanticKirFunctionCorrespondenceV1,
    arguments: &'scope [ProductionSourceAbiArgumentV1],
    components: &'scope [ProductionSourceAbiComponentV1<'source>],
    budget: &'scope mut ArgumentBudgetV1<'work>,
    required: usize,
    first: Option<SourceAbiPlanFailureV1>,
}
impl ProductionSourceAbiPlanV1<'_, '_, '_> {
    fn failed<T>(
        &mut self,
        failure: SourceAbiPlanFailureV1,
    ) -> Result<T, ProductionSemanticKirErrorV1> {
        let first = *self.first.get_or_insert(failure);
        Err(first.error())
    }
    fn check(&mut self) -> Result<(), ProductionSemanticKirErrorV1> {
        if let Some(first) = self.first {
            return Err(first.error());
        }
        if self.budget.storage() < self.required {
            return self.failed(SourceAbiPlanFailureV1::Resource(
                ArgumentResourceV1::Accounting,
            ));
        }
        if let Err(error) = self.budget.charge_work(1) {
            return self.failed(SourceAbiPlanFailureV1::Resource(error));
        }
        Ok(())
    }
    /// Rejects a same-shaped foreign source/module before charging query work.
    pub fn check_subject(
        &mut self,
        semantic: &AdmittedInertSemanticMirV1,
        module: &Module,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if !std::ptr::eq(self.semantic, semantic) || !std::ptr::eq(self.module, module) {
            return self.failed(SourceAbiPlanFailureV1::Mismatch);
        }
        self.check()
    }
    /// Exact root/body/physical-function association checked by the existing view.
    pub fn association(
        &mut self,
    ) -> Result<&SemanticKirFunctionCorrespondenceV1, ProductionSemanticKirErrorV1> {
        self.check()?;
        Ok(self.association)
    }
    /// Complete logical and physical counts, which need not be equal.
    pub fn counts(&mut self) -> Result<(usize, usize), ProductionSemanticKirErrorV1> {
        self.check()?;
        Ok((self.arguments.len(), self.components.len()))
    }
    /// Prepays consumer-side comparisons on this continuing ledger. The caller
    /// cannot obtain, replace or refund the exclusively borrowed budget.
    pub fn charge_work(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1> {
        self.check()?;
        match self.budget.charge_work(amount) {
            Ok(()) => Ok(()),
            Err(error) => self.failed(SourceAbiPlanFailureV1::Resource(error)),
        }
    }
    /// Exact logical row, including zero coverage and original nominal type.
    pub fn argument(
        &mut self,
        index: usize,
    ) -> Result<ProductionSourceAbiArgumentV1, ProductionSemanticKirErrorV1> {
        self.check()?;
        match self.arguments.get(index).copied() {
            Some(row) => Ok(row),
            None => self.failed(SourceAbiPlanFailureV1::Mismatch),
        }
    }
    /// Exact physical row. The borrowed trace/type cannot outlive this plan query.
    pub fn component(
        &mut self,
        index: usize,
    ) -> Result<ProductionSourceAbiComponentV1<'_>, ProductionSemanticKirErrorV1> {
        self.check()?;
        match self.components.get(index).copied() {
            Some(row) => Ok(row),
            None => self.failed(SourceAbiPlanFailureV1::Mismatch),
        }
    }
    /// Captured type/layout evidence for a whole source argument.
    pub fn argument_type(
        &mut self,
        index: usize,
    ) -> Result<&SemanticTypeDeclV1, ProductionSemanticKirErrorV1> {
        let row = self.argument(index)?;
        match self.semantic.types().get(row.ty.index() as usize) {
            Some(ty) => Ok(ty),
            None => self.failed(SourceAbiPlanFailureV1::Mismatch),
        }
    }
    /// This prerequisite never grants executable or publication authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

fn source_abi_plan_vector_v1<T>(
    count: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<T>, ProductionSemanticKirErrorV1> {
    let bytes = argument_product_v1(count, std::mem::size_of::<T>())?;
    budget.reserve_storage(argument_sum_v1(&[bytes, std::mem::size_of::<Vec<T>>()])?)?;
    budget.charge_work(1)?;
    let values = argument_vec_v1::<T>(count)?;
    let actual = argument_product_v1(values.capacity(), std::mem::size_of::<T>())?;
    budget.reserve_storage(
        actual
            .checked_sub(bytes)
            .ok_or(ArgumentResourceV1::Accounting)?,
    )?;
    Ok(values)
}

// This is storage accounting for the existing iterator, not a second iterator
// implementation. Its two closure captures are the original map and its ABI.
type SourceAbiIteratorFrameV1<'a> = std::iter::Map<
    std::iter::Enumerate<std::slice::Iter<'a, SemanticTypeIdV1>>,
    (
        &'a fe2o3_mir_model::semantic_mir_v1::SemanticFunctionAbiV1,
        &'a fe2o3_mir_model::SemanticLogicalArgumentMapV1<'a>,
    ),
>;

// The constructor closure captures this reference tuple and the user callback.
// Both payloads and both possible alignment paddings are prepaid before entry.
type SourceAbiConstructorRefsV1<'a, 'source, 'work> = (
    &'source AdmittedInertSemanticMirV1,
    &'source Module,
    &'a mut ProductionArgumentViewV1<'source, 'work>,
);

fn source_abi_plan_headers_v1<R>(
    capture: usize,
    alignment: usize,
) -> Result<usize, ArgumentResourceV1> {
    use std::mem::size_of;
    fn h<T>() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            size_of::<T>(),
            2 * size_of::<Result<T, source_arguments_v1::ProductionSourceArgumentErrorV1>>(),
            2 * size_of::<Result<T, ProductionSemanticKirErrorV1>>(),
            size_of::<Result<T, ArgumentResourceV1>>(),
        ])
    }
    argument_sum_v1(&[
        capture,
        argument_product_v1(alignment, 2)?,
        size_of::<SourceAbiConstructorRefsV1<'_, '_, '_>>(),
        2 * std::mem::align_of::<SourceAbiConstructorRefsV1<'_, '_, '_>>(),
        size_of::<ProductionSourceAbiPlanV1<'_, '_, '_>>(),
        h::<ProductionSourceAbiArgumentV1>()?,
        h::<Option<ProductionSourceAbiArgumentV1>>()?,
        h::<ProductionSourceAbiComponentV1<'_>>()?,
        h::<Option<ProductionSourceAbiComponentV1<'_>>>()?,
        h::<Option<&ProductionSourceAbiArgumentV1>>()?,
        h::<Option<&ProductionSourceAbiComponentV1<'_>>>()?,
        size_of::<std::ops::Range<usize>>(),
        h::<source_arguments_v1::KernelParameterShapeV1>()?,
        size_of::<&source_arguments_v1::KernelParameterShapeV1>(),
        size_of::<&Vec<source_arguments_v1::ByValueKernelParameterComponentV1>>(),
        size_of::<std::slice::Iter<'_, source_arguments_v1::ByValueKernelParameterComponentV1>>(),
        size_of::<&source_arguments_v1::ByValueKernelParameterComponentV1>(),
        h::<Option<&source_arguments_v1::ByValueKernelParameterComponentV1>>()?,
        h::<ProductionPhysicalArgumentV1<'_>>()?,
        h::<Option<ProductionPhysicalArgumentV1<'_>>>()?,
        h::<ProductionArgumentTraceV1<'_>>()?,
        size_of::<&SemanticKirParameterBindingV1>(),
        size_of::<&SemanticKirParameterComponentBindingV1>(),
        size_of::<&Vec<SemanticKirParameterProjectionV1>>(),
        size_of::<&[SemanticKirParameterProjectionV1]>(),
        size_of::<&SemanticTypeIdV1>(),
        size_of::<&Type>(),
        size_of::<&u64>(),
        h::<Option<&SemanticKirIgnoredParameterBindingV1>>()?,
        size_of::<&SemanticKirIgnoredParameterBindingV1>(),
        h::<&SemanticKirFunctionCorrespondenceV1>()?,
        h::<&SemanticFunctionDeclV1>()?,
        h::<Option<&SemanticFunctionDeclV1>>()?,
        h::<&fe2o3_kernel_ir::Function>()?,
        h::<Option<&fe2o3_kernel_ir::Function>>()?,
        size_of::<&[SemanticFunctionDeclV1]>(),
        size_of::<&[SemanticTypeDeclV1]>(),
        size_of::<&[ProductionSourceAbiArgumentV1]>(),
        size_of::<&[ProductionSourceAbiComponentV1<'_>]>(),
        h::<&SemanticTypeDeclV1>()?,
        h::<Option<&SemanticTypeDeclV1>>()?,
        h::<(usize, usize)>()?,
        h::<R>()?,
        h::<()>()?,
        h::<usize>()?,
        h::<Option<usize>>()?,
        h::<Vec<ProductionSourceAbiArgumentV1>>()?,
        h::<Vec<ProductionSourceAbiComponentV1<'_>>>()?,
        h::<SourceAbiIteratorFrameV1<'_>>()?,
        2 * std::mem::align_of::<SourceAbiIteratorFrameV1<'_>>(),
        h::<fe2o3_mir_model::SemanticSourceArgumentV1<'_>>()?,
        h::<Option<fe2o3_mir_model::SemanticSourceArgumentV1<'_>>>()?,
        size_of::<fe2o3_mir_model::SemanticSourceArgumentBindingV1<'_>>(),
        12 * size_of::<usize>(),
        4 * size_of::<u64>(),
        size_of::<&mut ProductionSourceAbiPlanV1<'_, '_, '_>>(),
        size_of::<&mut ArgumentBudgetV1<'_>>(),
        size_of::<SourceAbiPlanFailureV1>(),
        2 * size_of::<Option<SourceAbiPlanFailureV1>>(),
    ])
}

fn with_source_abi_plan_from_view_v1<'source, 'view: 'source, 'work, R>(
    semantic: &'source AdmittedInertSemanticMirV1,
    module: &'source Module,
    view: &'source mut ProductionArgumentViewV1<'view, 'work>,
    consume: impl for<'scope> FnOnce(
        &mut ProductionSourceAbiPlanV1<'scope, 'source, 'work>,
    ) -> Result<R, ProductionSemanticKirErrorV1>,
) -> Result<R, ProductionSemanticKirErrorV1> {
    use fe2o3_mir_model::SemanticSourceArgumentBindingV1 as Binding;
    use source_arguments_v1::KernelParameterShapeV1 as Shape;
    let floor = view.budget.storage();
    view.budget
        .reserve_storage(source_abi_plan_headers_v1::<R>(
            std::mem::size_of_val(&consume),
            std::mem::align_of_val(&consume),
        )?)?;
    let state = ((semantic, module, &mut *view), consume);
    let result = (move || {
        let ((semantic, module, view), consume) = state;
        let association = view.association();
        if association.role() != SemanticKirFunctionRoleV1::KernelEntry {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let function = semantic
            .functions()
            .get(association.semantic_function().index() as usize)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        view.budget.charge_work(argument_product_v1(
            module.functions.len(),
            association
                .kernel_ir_function()
                .as_str()
                .len()
                .checked_add(1)
                .ok_or(ArgumentResourceV1::Arithmetic)?,
        )?)?;
        let physical = module
            .function(association.kernel_ir_function())
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let count = physical.signature.parameters.len();
        let sources = view.source_arguments()?;
        if std::mem::size_of_val(&sources) > std::mem::size_of::<SourceAbiIteratorFrameV1<'_>>()
            || std::mem::align_of_val(&sources)
                > std::mem::align_of::<SourceAbiIteratorFrameV1<'_>>()
        {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        let mut arguments = source_abi_plan_vector_v1(sources.len(), view.budget)?;
        let mut components = source_abi_plan_vector_v1(count, view.budget)?;
        for source in sources {
            view.budget.charge_work(1)?;
            let Binding::Whole(local) = source.binding() else {
                // A kernel entry is not a RustCall closure ABI. Do not invent
                // whole-source offsets from expanded caller tuple fields.
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            };
            let first = components.len();
            let scratch = view.budget.storage();
            prepay_argument_shape_v1(semantic, source.ty(), view.budget)?;
            let shape = source_arguments_v1::kernel_parameter_shape_v1(
                semantic,
                function,
                source.ordinal(),
                source.ty(),
            )?;
            match &shape {
                Shape::Direct(expected) => {
                    let actual = view
                        .physical(components.len())?
                        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                    if actual.ty() != expected
                        || !matches!(actual.trace(), ProductionArgumentTraceV1::Direct(row)
                        if row.semantic_local() == local)
                    {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                    components.push(ProductionSourceAbiComponentV1 {
                        source: source.ordinal(),
                        ty: source.ty(),
                        source_offset: 0,
                        physical: actual,
                    });
                }
                Shape::Components(leaves) => {
                    view.budget.charge_work(leaves.len())?;
                    for (path, ty, expected, offset, _) in leaves {
                        view.budget.charge_work(path.len())?;
                        let actual = view
                            .physical(components.len())?
                            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                        if actual.ty() != expected
                            || !matches!(actual.trace(), ProductionArgumentTraceV1::Component(row)
                            if row.semantic_local() == local && row.semantic_component_type() == *ty
                                && row.projection() == path.as_slice())
                        {
                            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                        }
                        components.push(ProductionSourceAbiComponentV1 {
                            source: source.ordinal(),
                            ty: *ty,
                            source_offset: *offset,
                            physical: actual,
                        });
                    }
                    if leaves.is_empty()
                        && !view
                            .ignored_local(local)?
                            .is_some_and(|row| row.semantic_type() == source.ty())
                    {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                }
            }
            drop(shape);
            view.budget.release_storage(
                view.budget
                    .storage()
                    .checked_sub(scratch)
                    .ok_or(ArgumentResourceV1::Accounting)?,
            )?;
            arguments.push(ProductionSourceAbiArgumentV1 {
                source: source.ordinal(),
                local,
                ty: source.ty(),
                first,
                end: components.len(),
            });
        }
        if components.len() != count || view.physical(count)?.is_some() {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let required = view.budget.storage();
        let mut plan = ProductionSourceAbiPlanV1 {
            semantic,
            module,
            association,
            arguments: &arguments,
            components: &components,
            budget: view.budget,
            required,
            first: None,
        };
        let selected = consume(&mut plan);
        if let Some(first) = plan.first {
            return Err(first.error());
        }
        match selected {
            Ok(value) => {
                plan.check()?;
                Ok(value)
            }
            Err(error) => Err(error),
        }
    })();
    view.budget.release_storage(
        view.budget
            .storage()
            .checked_sub(floor)
            .ok_or(ArgumentResourceV1::Accounting)?,
    )?;
    result
}

fn source_abi_plan_wrapper_headers_v1<R>(
    capture: usize,
    alignment: usize,
) -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        capture,
        argument_product_v1(alignment, 2)?,
        std::mem::size_of::<Result<R, ProductionSemanticKirErrorV1>>(),
        std::mem::size_of::<Result<(), ArgumentResourceV1>>(),
        std::mem::size_of::<Result<usize, ArgumentResourceV1>>(),
        std::mem::size_of::<Option<usize>>(),
        std::mem::size_of::<&mut ArgumentBudgetV1<'_>>(),
        2 * std::mem::size_of::<SemanticFunctionIdV1>(),
        2 * std::mem::size_of::<usize>(),
    ])
}

impl ProductionSemanticKirOwnerV1 {
    /// Reuses complete checked argument correspondence and exact captured layouts.
    /// The callback cannot retain the plan, its backing rows or the live budget.
    ///
    /// ```compile_fail
    /// use fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1;
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// use fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1 as Id;
    /// fn escape(owner: &ProductionSemanticKirOwnerV1, budget: &mut Budget<'_>, id: Id) {
    ///     let saved = owner.with_checked_source_abi_plan_v1(id, id, budget, |plan| plan.component(0)).unwrap();
    ///     let _ = saved.physical();
    /// }
    /// ```
    pub fn with_checked_source_abi_plan_v1<'work, R>(
        &self,
        root: SemanticFunctionIdV1,
        function: SemanticFunctionIdV1,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: impl for<'scope, 'source> FnOnce(
            &mut ProductionSourceAbiPlanV1<'scope, 'source, 'work>,
        ) -> Result<R, ProductionSemanticKirErrorV1>,
    ) -> Result<R, ProductionSemanticKirErrorV1> {
        let callback = |view: &mut ProductionArgumentViewV1<'_, 'work>| {
            with_source_abi_plan_from_view_v1(
                self.semantic_ssa.source_semantic(),
                self.module(),
                view,
                consume,
            )
        };
        let floor = budget.storage();
        let headers = source_abi_plan_wrapper_headers_v1::<R>(
            std::mem::size_of_val(&callback),
            std::mem::align_of_val(&callback),
        )?;
        budget.reserve_storage(headers)?;
        let result = self.with_checked_arguments_v1(root, function, budget, callback);
        budget.release_storage(
            budget
                .storage()
                .checked_sub(floor)
                .ok_or(ArgumentResourceV1::Accounting)?,
        )?;
        result
    }
}

impl ProductionPreRankedKirOwnerV1 {
    /// The same source-only plan before ranked checks; no admission is added.
    pub fn with_checked_source_abi_plan_v1<'work, R>(
        &self,
        root: SemanticFunctionIdV1,
        function: SemanticFunctionIdV1,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: impl for<'scope, 'source> FnOnce(
            &mut ProductionSourceAbiPlanV1<'scope, 'source, 'work>,
        ) -> Result<R, ProductionSemanticKirErrorV1>,
    ) -> Result<R, ProductionSemanticKirErrorV1> {
        let callback = |view: &mut ProductionArgumentViewV1<'_, 'work>| {
            with_source_abi_plan_from_view_v1(
                self.semantic_ssa.source_semantic(),
                self.executable.module(),
                view,
                consume,
            )
        };
        let floor = budget.storage();
        let headers = source_abi_plan_wrapper_headers_v1::<R>(
            std::mem::size_of_val(&callback),
            std::mem::align_of_val(&callback),
        )?;
        budget.reserve_storage(headers)?;
        let result = self.with_checked_arguments_v1(root, function, budget, callback);
        budget.release_storage(
            budget
                .storage()
                .checked_sub(floor)
                .ok_or(ArgumentResourceV1::Accounting)?,
        )?;
        result
    }
}

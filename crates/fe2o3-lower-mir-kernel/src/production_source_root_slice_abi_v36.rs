// A descriptor-shaped source wrapper is not a Rust slice-shaped MIR type.
// Reconstruct it from the admitted descriptor and the original argument trace.
use fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1 as RootSliceDefinitionV36;

#[derive(Clone, Copy)]
struct RootSliceArgumentNodeV36 {
    local: SemanticLocalIdV1,
    slot: usize,
    value: ValueId,
}

/// Borrowed original entry reconstruction, not a memory or launch certificate.
///
/// The descriptor establishes a source wrapper's slice interpretation. The
/// argument trace establishes its exact original canonical parameter. Neither
/// an optimized parameter ordinal nor a matching source shape supplies this
/// relation. Component offsets belong to the captured kernarg proposal, not
/// to the source object's field layout.
pub struct ProductionSourceRootSliceAbiV36<'view, 'source> {
    owner: &'view ProductionSourceCorrespondenceV18<'source>,
    scope: &'view slice_view_v1::DescriptorRoleScopeV18,
    root: usize,
    local: SemanticLocalIdV1,
    parameter: RootSliceDefinitionV36,
    physical: &'view Type,
    abi: kernel_argument_abi_v18::SourceSliceEntryAbiV25<'view>,
}

impl ProductionSourceRootSliceAbiV36<'_, '_> {
    /// Revalidates the borrowed source owner with its original resource ledger.
    pub fn check(&self, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        self.owner.retain_query((|| {
            self.scope.observe(self.owner, budget)?;
            self.owner.query(budget)?;
            let (function, physical) = self.owner.source.root(self.root, budget)?;
            budget.charge_work(5)?;
            if function != self.abi.function()
                || !matches!(self.parameter, RootSliceDefinitionV36::FunctionArgument {
                    function, ..
                } if function.0 as usize == physical)
            {
                return self
                    .owner
                    .source
                    .missing("original root slice recipe owner differs");
            }
            Ok(())
        })())
    }

    /// Original semantic kernel function, before inlining or optimization.
    pub fn function(&self) -> SemanticFunctionIdV1 {
        self.abi.function()
    }
    /// Original source signature argument, not a physical component ordinal.
    pub fn argument(&self) -> u32 {
        self.abi.argument()
    }
    /// Exact original MIR argument local from the checked emission trace.
    pub fn local(&self) -> SemanticLocalIdV1 {
        self.local
    }
    /// Original wrapper type; its shape need not be a native Rust slice.
    pub fn source_type(&self) -> SemanticTypeIdV1 {
        self.abi.ty()
    }
    /// Nominal source type identity from the admitted descriptor.
    pub fn source_type_identity(&self) -> fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdentityV1 {
        self.abi.identity()
    }
    /// Exact original canonical slice carrier.
    pub fn parameter(&self) -> RootSliceDefinitionV36 {
        self.parameter
    }
    /// Borrowed type of the original canonical parameter, not an inferred type.
    pub fn parameter_type(&self) -> &Type {
        self.physical
    }
    /// Descriptor scalar element, independent of the wrapper's nominal shape.
    pub fn element(&self) -> ScalarType {
        self.abi.scalar()
    }
    /// The descriptor declares exclusive ownership; actual aliasing is not proved.
    pub fn is_exclusive_contract(&self) -> bool {
        self.abi.is_exclusive_contract()
    }
    /// Descriptor access contract, not current runtime access permission.
    pub fn allows_reads(&self) -> bool {
        self.abi.allows_reads()
    }
    /// Descriptor access contract, not current runtime access permission.
    pub fn allows_writes(&self) -> bool {
        self.abi.allows_writes()
    }
    /// Pointer then length: each (kernarg offset, byte width, byte alignment).
    pub fn components(&self) -> [(u32, u16, u16); 2] {
        self.abi.components_v36()
    }
    /// Captured slice length width; this is not the canonical INDEX width.
    pub fn metadata_bits(&self) -> u16 {
        self.components()[1].1 * 8
    }
    /// No runtime allocation, initialization, lifetime or launch grant is made.
    pub const fn grants_memory_or_launch_authority(&self) -> bool {
        false
    }
}

/// One source-owned root argument index. It cannot outlive its checked argument
/// walk, profile, resource ledger or original correspondence.
pub struct ProductionSourceRootSliceAbisV36<'view, 'source> {
    owner: &'view ProductionSourceCorrespondenceV18<'source>,
    root: usize,
    arguments: &'view ArgumentViewDataV18<'view>,
    profile: kernel_argument_abi_v18::SourceDescriptorRootAbiV29<'view>,
    rows: &'view [Option<RootSliceArgumentNodeV36>],
    scope: slice_view_v1::DescriptorRoleScopeV18,
}

impl ProductionSourceRootSliceAbisV36<'_, '_> {
    fn check(&self, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        self.owner.retain_query((|| {
            self.scope.observe(self.owner, budget)?;
            self.owner.query(budget)?;
            self.arguments
                .check(budget)
                .map_err(source_argument_error_v18)
        })())
    }

    /// Complete captured source signature length, including non-slice arguments.
    pub fn argument_count(&self, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<usize> {
        self.check(budget)?;
        Ok(self.rows.len())
    }

    /// Constant-time lookup after the one complete original argument walk.
    /// Non-slice arguments return None only after the exact local is checked.
    pub fn argument(
        &self,
        argument: u32,
        local: SemanticLocalIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<ProductionSourceRootSliceAbiV36<'_, '_>>> {
        self.owner.retain_query((|| {
            self.check(budget)?;
            budget.charge_work(12)?;
            let row = self.rows.get(argument as usize).ok_or(
                ProductionSourceOwnedViewErrorV18::Binding("original slice argument absent"),
            )?;
            let (function, physical_function) = self.owner.source.root(self.root, budget)?;
            let semantic = self.owner.source.source_semantic(budget)?;
            let declaration = semantic.functions().get(function.index() as usize).ok_or(
                ProductionSourceOwnedViewErrorV18::Binding("original slice function absent"),
            )?;
            let declaration_local = declaration.locals().get(local.index() as usize).ok_or(
                ProductionSourceOwnedViewErrorV18::Binding("original slice local absent"),
            )?;
            if declaration_local.role() != SemanticLocalRoleV1::Argument(argument)
                || declaration
                    .abi()
                    .source_input_types()
                    .get(argument as usize)
                    != Some(&declaration_local.ty())
            {
                return self.owner.source.missing("original slice local differs");
            }
            let Some(row) = row else { return Ok(None) };
            if row.local != local {
                return self
                    .owner
                    .source
                    .missing("original descriptor slice local differs");
            }
            let physical_function = self
                .owner
                .inventory
                .functions()
                .get(physical_function)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "original slice physical root absent",
                ))?;
            let physical = physical_function
                .function
                .signature
                .parameters
                .get(row.slot)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "original slice parameter absent",
                ))?;
            if physical_function
                .function
                .body
                .as_ref()
                .and_then(|body| body.parameters.get(row.slot))
                != Some(&row.value)
            {
                return self
                    .owner
                    .source
                    .missing("original descriptor slice value differs");
            }
            let abi = self
                .profile
                .slice_parameter_v26(argument, declaration_local.ty(), physical, budget)
                .map_err(source_argument_error_v18)?
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "original slice descriptor absent",
                ))?;
            if abi.function() != function {
                return self
                    .owner
                    .source
                    .missing("original slice descriptor function differs");
            }
            Ok(Some(ProductionSourceRootSliceAbiV36 {
                owner: self.owner,
                scope: &self.scope,
                root: self.root,
                local,
                parameter: RootSliceDefinitionV36::FunctionArgument {
                    function: physical_function.coordinate,
                    argument: u32::try_from(row.slot)
                        .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                },
                physical,
                abi,
            }))
        })())
    }
}

fn root_slice_argument_index_v36(
    data: &ArgumentViewDataV18<'_>,
    profile: kernel_argument_abi_v18::SourceDescriptorRootAbiV29<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<Option<RootSliceArgumentNodeV36>>, ProductionSemanticKirErrorV1> {
    #[cfg(test)]
    ROOT_SLICE_ABI_WALKS_V36.set(ROOT_SLICE_ABI_WALKS_V36.get() + 1);
    let count = profile.argument_count_v25(budget)?;
    let mut rows = source_reference_emission_vec_v29(count, budget)?;
    budget.charge_work(count)?;
    rows.resize(count, None);
    data.visit_nodes_scoped(budget, |node, budget| {
        budget.charge_work(6)?;
        if !node.source_path().is_empty() {
            return Ok(());
        }
        let argument = node.source_argument();
        let Some(ty) = profile.slice_parameter_type_v26(argument, budget)? else {
            return Ok(());
        };
        let Some((local, path)) = node.local_binding() else {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        };
        let ProductionArgumentCoverageV1::Parameter(parameter) = node.coverage() else {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        };
        let physical = data
            .physical(parameter.slot(), budget)?
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let same_trace = match (physical.trace(), parameter.trace()) {
            (
                source_arguments_v1::ProductionArgumentTraceV1::Direct(a),
                source_arguments_v1::ProductionArgumentTraceV1::Direct(b),
            ) => std::ptr::eq(a, b),
            (
                source_arguments_v1::ProductionArgumentTraceV1::Component(a),
                source_arguments_v1::ProductionArgumentTraceV1::Component(b),
            ) => std::ptr::eq(a, b),
            _ => false,
        };
        let row = rows
            .get_mut(argument as usize)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        if !path.is_empty()
            || node.semantic_type() != ty
            || physical.slot() != parameter.slot()
            || physical.value() != parameter.value()
            || !std::ptr::eq(physical.ty(), parameter.ty())
            || !same_trace
            || !matches!(parameter.ty(), Type::Slice(_))
            || row
                .replace(RootSliceArgumentNodeV36 {
                    local,
                    slot: parameter.slot(),
                    value: parameter.value(),
                })
                .is_some()
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        Ok(())
    })?;
    // No declaration is proved absent merely because the walk did not emit it.
    for (argument, row) in rows.iter().enumerate() {
        budget.charge_work(2)?;
        let argument = u32::try_from(argument).map_err(|_| ArgumentResourceV1::Arithmetic)?;
        if profile
            .slice_parameter_type_v26(argument, budget)?
            .is_some()
            != row.is_some()
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
    }
    Ok(rows)
}

#[cfg(test)]
std::thread_local! {
    static ROOT_SLICE_ABI_WALKS_V36: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

impl ProductionSourceCorrespondenceV18<'_> {
    /// Lends one complete source-owned root argument index in O(arguments +
    /// original argument nodes). Subsequent argument/local queries are O(1).
    ///
    /// The callback may use precharged output storage but may not retain this
    /// borrowed index or alter its resource floor. Missing profile/root or
    /// malformed source-to-entry correspondence fails, never guesses a shape.
    pub fn with_root_slice_abis_v36<'work>(
        &self,
        root: usize,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: impl for<'view> FnOnce(
            &ProductionSourceRootSliceAbisV36<'view, '_>,
            &mut ArgumentBudgetV1<'work>,
        ) -> SourceOwnedResultV18<()>,
    ) -> SourceOwnedResultV18<()> {
        slice_view_v1::shared_entry_consume_v18(self, budget, move |budget| {
            source_scalar_normalization_scratch_v18(
                self.source.cleanup,
                budget,
                root_slice_abi_headers_v36()?,
                |budget| {
                    let profile = self.source.descriptor_root_abi_v29(root, budget)?.ok_or(
                        ProductionSourceOwnedViewErrorV18::Binding(
                            "original slice ABI profile absent",
                        ),
                    )?;
                    let mut chosen = None;
                    let result = self.with_root_argument_data_v18(root, budget, |data, budget| {
                        let rows = root_slice_argument_index_v36(&data, profile, budget)?;
                        let view = ProductionSourceRootSliceAbisV36 {
                            owner: self,
                            root,
                            arguments: &data,
                            profile,
                            rows: &rows,
                            scope: slice_view_v1::DescriptorRoleScopeV18::new(budget),
                        };
                        let result =
                            slice_view_v1::shared_entry_consume_v18(self, budget, |budget| {
                                view.check(budget)?;
                                consume(&view, budget)
                            });
                        match result {
                            Ok(()) => Ok(()),
                            Err(error) => {
                                chosen = Some(error);
                                Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
                            }
                        }
                    });
                    match chosen {
                        Some(error) => Err(error),
                        None => result,
                    }
                },
            )
        })
    }
}

fn root_slice_abi_headers_v36() -> Result<usize, ArgumentResourceV1> {
    fn h<T>() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            size_of::<T>(),
            argument_product_v1(2, size_of::<Result<T, ProductionSemanticKirErrorV1>>())?,
            argument_product_v1(2, size_of::<SourceOwnedResultV18<T>>())?,
        ])
    }
    argument_sum_v1(&[
        h::<RootSliceArgumentNodeV36>()?,
        h::<Option<RootSliceArgumentNodeV36>>()?,
        h::<ProductionSourceRootSliceAbiV36<'_, '_>>()?,
        h::<Option<ProductionSourceRootSliceAbiV36<'_, '_>>>()?,
        h::<ProductionSourceRootSliceAbisV36<'_, '_>>()?,
        h::<Vec<Option<RootSliceArgumentNodeV36>>>()?,
        h::<slice_view_v1::DescriptorRoleScopeV18>()?,
        h::<&[Option<RootSliceArgumentNodeV36>]>()?,
        h::<ArgumentViewDataV18<'_>>()?,
        h::<ProductionArgumentNodeV1<'_>>()?,
        argument_product_v1(2, h::<ProductionPhysicalArgumentV1<'_>>()?)?,
        h::<Option<ProductionSourceOwnedViewErrorV18>>()?,
        h::<&ProductionSourceCorrespondenceV18<'_>>()?,
        h::<&fe2o3_mir_model::semantic_mir_v1::SemanticFunctionDeclV1>()?,
        h::<&fe2o3_mir_model::semantic_mir_v1::SemanticLocalDeclV1>()?,
        h::<std::iter::Enumerate<std::slice::Iter<'_, Option<RootSliceArgumentNodeV36>>>>()?,
        h::<kernel_argument_abi_v18::SourceDescriptorRootAbiV29<'_>>()?,
        h::<[usize; 6]>()?,
        h::<[bool; 4]>()?,
        h::<[u32; 2]>()?,
        h::<SemanticLocalIdV1>()?,
        h::<SemanticTypeIdV1>()?,
        h::<RootSliceDefinitionV36>()?,
        h::<&Type>()?,
        h::<[(u32, u16, u16); 2]>()?,
        kernel_argument_abi_v18::slice_entry_abi_headers_v25()?,
    ])
}

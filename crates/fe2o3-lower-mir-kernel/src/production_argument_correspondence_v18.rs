// Original source root adapter. Public argument identities remain owned by Pliron.

#[derive(Clone, Copy)]
struct ArgumentEntryV18<'a> {
    correspondence_owner: SemanticFunctionIdV1,
    semantic_function: SemanticFunctionIdV1,
    kernel_ir_function: &'a FunctionId,
    role: SemanticKirFunctionRoleV1,
    descriptor_root: Option<kernel_argument_abi_v18::SourceDescriptorRootAbiV29<'a>>,
}

fn source_argument_error_v18(
    error: ProductionSemanticKirErrorV1,
) -> ProductionSourceOwnedViewErrorV18 {
    match error {
        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error) => error.into(),
        ProductionSemanticKirErrorV1::CorrespondenceMismatch => {
            ProductionSourceOwnedViewErrorV18::Binding("original source argument correspondence")
        }
        error => ScopedModuleErrorV29::from(error).into(),
    }
}

impl ProductionSourceCorrespondenceV18<'_> {
    fn with_root_argument_data_v18<'work, R>(
        &self,
        root: usize,
        budget: &mut ArgumentBudgetV1<'work>,
        use_data: impl for<'s> FnOnce(
            ArgumentViewDataV18<'s>,
            &'s mut ArgumentBudgetV1<'work>,
        ) -> Result<R, ProductionSemanticKirErrorV1>,
    ) -> SourceOwnedResultV18<R> {
        self.retain_query((|| {
            self.query(budget)?;
            let source = self.source.source_semantic(budget)?;
            let (original_root, function_ordinal) = self.source.root(root, budget)?;
            let (instance_function, incoming) = self.source.instance(root, 0, budget)?;
            let sidecar = self.source.sidecar(root, 0, budget)?;
            budget.charge_work(3)?;
            let physical = self.inventory.functions().get(function_ordinal).ok_or(
                ProductionSourceOwnedViewErrorV18::Binding("original root physical function"),
            )?;
            if instance_function != original_root
                || incoming.is_some()
                || source.roots().binary_search(&original_root).is_err()
                || physical.coordinate.0 as usize != function_ordinal
                || self.source.root_row(root)?.coordinates.sources.rows[0].function != original_root
            {
                return self.source.missing("original root argument association");
            }
            with_parameter_data_v18(
                source,
                ArgumentEntryV18 {
                    correspondence_owner: original_root,
                    semantic_function: original_root,
                    kernel_ir_function: &physical.function.id,
                    role: SemanticKirFunctionRoleV1::KernelEntry,
                    descriptor_root: self.source.descriptor_root_abi_v29(root, budget)?,
                },
                physical.function,
                ArgumentTraceV1 {
                    direct: &sidecar.parameter_bindings,
                    components: &sidecar.parameter_component_bindings,
                    ignored: &sidecar.ignored_parameter_bindings,
                },
                self.source.cleanup,
                budget,
                use_data,
            )
            .map_err(source_argument_error_v18)
        })())
    }
}

fn with_argument_scratch_v18<'work, T>(
    cleanup: &ScopedSourceCleanupV29,
    budget: &mut ArgumentBudgetV1<'work>,
    run: impl FnOnce(&mut ArgumentBudgetV1<'work>) -> Result<T, ProductionSemanticKirErrorV1>,
) -> Result<T, ProductionSemanticKirErrorV1> {
    let floor = budget.storage();
    let value = scoped_source_attempt_v29(cleanup, budget, floor, run)?;
    // The run closure owns all scratch. Nested callback attempts have already
    // checked their larger live floors before any enclosing release can occur.
    let release = budget
        .storage()
        .checked_sub(floor)
        .ok_or(ArgumentResourceV1::Accounting)
        .and_then(|amount| budget.release_storage(amount));
    match release {
        Ok(()) => Ok(value),
        Err(error) => {
            cleanup.deny_refund();
            drop(value);
            Err(error.into())
        }
    }
}

struct ArgumentViewDataV18<'scope> {
    inner: source_arguments_v1::scoped_v18::ProductionArgumentViewV18<'scope>,
    cleanup: &'scope ScopedSourceCleanupV29,
}

impl<'scope> ArgumentViewDataV18<'scope> {
    fn check(&self, budget: &ArgumentBudgetV1<'_>) -> Result<(), ProductionSemanticKirErrorV1> {
        self.inner.check_custody(budget).map_err(Into::into)
    }

    fn semantic_function(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<SemanticFunctionIdV1, ProductionSemanticKirErrorV1> {
        self.inner.semantic_function(budget).map_err(Into::into)
    }

    fn physical(
        &self,
        slot: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<ProductionPhysicalArgumentV1<'scope>>, ProductionSemanticKirErrorV1> {
        self.inner.physical(slot, budget).map_err(Into::into)
    }

    fn visit_nodes_scoped<'work>(
        &self,
        budget: &mut ArgumentBudgetV1<'work>,
        mut visit: impl for<'node, 'borrow> FnMut(
            ProductionArgumentNodeV1<'node>,
            &'borrow mut ArgumentBudgetV1<'work>,
        ) -> Result<(), ProductionSemanticKirErrorV1>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        with_argument_scratch_v18(self.cleanup, budget, move |budget| {
            budget.reserve_storage(std::mem::size_of::<Option<ProductionSemanticKirErrorV1>>())?;
            let mut selected = None;
            let result = self.inner.visit_nodes_with_budget(budget, |node, budget| {
                visit(node, budget).map_err(|error| {
                    selected = Some(error);
                    source_arguments_v1::ProductionSourceArgumentErrorV1::Visitor
                })
            });
            argument_visitor_result_v1(result, selected)
        })
    }
}

fn with_parameter_data_v18<'work, R>(
    semantic: &AdmittedInertSemanticMirV1,
    entry: ArgumentEntryV18<'_>,
    target: &Function,
    trace: ArgumentTraceV1<'_>,
    cleanup: &ScopedSourceCleanupV29,
    budget: &mut ArgumentBudgetV1<'work>,
    use_data: impl for<'scope> FnOnce(
        ArgumentViewDataV18<'scope>,
        &'scope mut ArgumentBudgetV1<'work>,
    ) -> Result<R, ProductionSemanticKirErrorV1>,
) -> Result<R, ProductionSemanticKirErrorV1> {
    with_argument_scratch_v18(cleanup, budget, |budget| {
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of::<fe2o3_pliron::CanonicalAnalysisCleanupV1<'_>>(),
            2 * std::mem::size_of::<Option<ProductionSemanticKirErrorV1>>(),
            std::mem::size_of::<ArgumentViewDataV18<'_>>(),
        ])?)?;
        let linked = fe2o3_pliron::CanonicalAnalysisCleanupV1::linked(&cleanup.denied);
        let (mut profile_error, mut visitor_error) = (None, None);
        let descriptor = entry.descriptor_root;
        let mut shape_proposal = |argument, ty, budget: &mut ArgumentBudgetV1<'work>| {
            descriptor
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
                .shared_slice(argument, ty, budget)
        };
        let mut proposal = |argument, ty, budget: &mut ArgumentBudgetV1<'work>| {
            shape_proposal(argument, ty, budget).map_err(|error| {
                profile_error = Some(error);
                source_arguments_v1::ProductionSourceArgumentErrorV1::Visitor
            })
        };
        // Only this already-authenticated borrowed profile supplies production
        // shape answers. The shared view cannot replace final profile replay.
        let proposal: Option<
            &mut source_arguments_v1::scoped_v18::DescriptorSliceProposalV18<'_, 'work>,
        > = descriptor.map(|_| &mut proposal as _);
        let result = source_arguments_v1::scoped_v18::with_parameter_correspondence_v18(
            semantic,
            source_arguments_v1::scoped_v18::ArgumentEntryV18 {
                correspondence_owner: entry.correspondence_owner,
                semantic_function: entry.semantic_function,
                kernel_ir_function: entry.kernel_ir_function,
                role: entry.role,
            },
            target,
            trace,
            &linked,
            budget,
            proposal,
            |inner, budget| {
                use_data(ArgumentViewDataV18 { inner, cleanup }, budget).map_err(|error| {
                    visitor_error = Some(error);
                    source_arguments_v1::ProductionSourceArgumentErrorV1::Visitor
                })
            },
        );
        argument_visitor_result_v1(result, visitor_error.or(profile_error))
    })
}

#[cfg(test)]
#[path = "production_argument_correspondence_v18_tests.rs"]
mod original_argument_v18_tests;

include!("production_source_root_slice_abi_v36.rs");

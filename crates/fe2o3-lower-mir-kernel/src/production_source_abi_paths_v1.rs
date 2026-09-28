// Paths come only from the existing checked ArgumentView traversal. They are
// structural coordinates, not byte-copy permissions or independent authority.
type SourceAbiPathRunV1<'a, 'source, 'work, F> = (
    &'a source_arguments_v1::ProductionArgumentViewV1<'source>,
    &'a mut ArgumentBudgetV1<'work>,
    F,
    &'a mut Option<ProductionSemanticKirErrorV1>,
    &'a mut Option<SourceAbiPlanFailureV1>,
    usize,
    usize,
);

fn source_abi_paths_headers_v1<F>() -> Result<usize, ArgumentResourceV1> {
    use std::mem::{align_of, size_of};
    type Error = ProductionSemanticKirErrorV1;
    type SourceError = source_arguments_v1::ProductionSourceArgumentErrorV1;
    argument_sum_v1(&[
        size_of::<F>(),
        align_of::<F>(),
        size_of::<(&mut F, &mut Option<Error>)>(),
        align_of::<(&mut F, &mut Option<Error>)>(),
        size_of::<SourceAbiPathRunV1<'_, '_, '_, F>>(),
        align_of::<SourceAbiPathRunV1<'_, '_, '_, F>>(),
        size_of::<std::panic::AssertUnwindSafe<SourceAbiPathRunV1<'_, '_, '_, F>>>(),
        size_of::<ProductionArgumentNodeV1<'_>>(),
        size_of::<Option<Error>>(),
        size_of::<&Option<Error>>(),
        size_of::<&ArgumentResourceV1>(),
        size_of::<&mut ArgumentResourceV1>(),
        size_of::<&SourceError>(),
        size_of::<&mut Option<SourceAbiPlanFailureV1>>(),
        size_of::<&mut SourceAbiPlanFailureV1>(),
        size_of::<&Result<Result<(), SourceError>, Box<dyn std::any::Any + Send>>>(),
        size_of::<Error>(),
        3 * size_of::<Result<(), Error>>(),
        2 * size_of::<Result<(), SourceError>>(),
        size_of::<Result<Result<(), SourceError>, Box<dyn std::any::Any + Send>>>(),
        size_of::<Result<(), ArgumentResourceV1>>(),
        size_of::<Result<usize, ArgumentResourceV1>>(),
        size_of::<Option<usize>>(),
        6 * size_of::<usize>(),
        size_of::<bool>(),
        size_of::<SourceAbiPlanFailureV1>(),
        size_of::<Option<SourceAbiPlanFailureV1>>(),
    ])
}

impl ProductionSourceAbiPlanV1<'_, '_, '_> {
    /// Visits the complete checked source structure in postorder, including
    /// nested zero-sized fields and array elements. Coverage distinguishes a
    /// physical parameter from containment inside one atomic parameter.
    ///
    /// Paths and nodes are borrowed for one callback only. Repeated visits pay
    /// the existing traversal again; callback work is the consumer's obligation.
    /// No second type/layout walker or detached source map is constructed.
    ///
    /// ```compile_fail
    /// use fe2o3_lower_mir_kernel::ProductionSourceAbiPlanV1;
    /// fn escape(plan: &mut ProductionSourceAbiPlanV1<'_, '_, '_>) {
    ///     let mut saved = None;
    ///     plan.visit_nodes(|node| { saved = Some(node.source_path()); Ok(()) }).unwrap();
    ///     assert!(saved.is_some());
    /// }
    /// ```
    pub fn visit_nodes<F>(&mut self, visit: F) -> Result<(), ProductionSemanticKirErrorV1>
    where
        F: for<'node> FnMut(
            ProductionArgumentNodeV1<'node>,
        ) -> Result<(), ProductionSemanticKirErrorV1>,
    {
        self.visit_nodes_with_work(0, 0, visit)
    }

    /// Prepays logical consumer work at each existing traversal node before
    /// invoking the visitor. Arithmetic overflow and denied work skip that
    /// callback and remain sticky. No budget/refund capability is exposed.
    pub fn visit_nodes_with_work<F>(
        &mut self,
        per_node: usize,
        per_projection: usize,
        visit: F,
    ) -> Result<(), ProductionSemanticKirErrorV1>
    where
        F: for<'node> FnMut(
            ProductionArgumentNodeV1<'node>,
        ) -> Result<(), ProductionSemanticKirErrorV1>,
    {
        self.check()?;
        let floor = self.budget.storage();
        let bytes = match source_abi_paths_headers_v1::<F>() {
            Ok(bytes) => bytes,
            Err(error) => return self.failed(SourceAbiPlanFailureV1::Resource(error)),
        };
        if let Err(error) = self.budget.reserve_storage(bytes) {
            return self.failed(SourceAbiPlanFailureV1::Resource(error));
        }
        let required = self.budget.storage();
        let mut selected = None;
        let state = (
            self.source_view,
            &mut *self.budget,
            visit,
            &mut selected,
            &mut self.first,
            per_node,
            per_projection,
        );
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let (data, budget, mut visit, selected, first, per_node, per_projection) = state;
            let callback = |node: ProductionArgumentNodeV1<'_>| {
                visit(node).map_err(|error| {
                    *selected = Some(error);
                    source_arguments_v1::ProductionSourceArgumentErrorV1::Visitor
                })
            };
            let result = if per_node == 0 && per_projection == 0 {
                data.visit_nodes(budget, callback)
            } else {
                data.visit_nodes_with_work(budget, per_node, per_projection, callback)
            };
            // Retain a walker refusal before owned-visitor destruction can
            // replace this Result with an unwind.
            if let Some(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error)) =
                selected
            {
                first.get_or_insert(SourceAbiPlanFailureV1::Resource(*error));
            }
            if let Err(source_arguments_v1::ProductionSourceArgumentErrorV1::ArgumentCorrespondenceResource(error)) = &result {
                first.get_or_insert(SourceAbiPlanFailureV1::Resource(*error));
            }
            drop(visit);
            result
        }));
        if let Some(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error)) = &selected
        {
            self.first
                .get_or_insert(SourceAbiPlanFailureV1::Resource(*error));
        }
        // The visitor has no budget access. All new traversal storage is scratch;
        // unwind drops its paths before this continuing-ledger refund.
        if let Ok(Err(
            source_arguments_v1::ProductionSourceArgumentErrorV1::ArgumentCorrespondenceResource(
                error,
            ),
        )) = &result
        {
            self.first
                .get_or_insert(SourceAbiPlanFailureV1::Resource(*error));
        }
        let valid = self.budget.storage() >= required;
        if valid {
            if let Err(error) = self.budget.release_storage(self.budget.storage() - floor) {
                self.first
                    .get_or_insert(SourceAbiPlanFailureV1::Resource(error));
            }
        } else {
            self.first.get_or_insert(SourceAbiPlanFailureV1::Resource(
                ArgumentResourceV1::Accounting,
            ));
        }
        let result = match result {
            Err(payload) => std::panic::resume_unwind(payload),
            Ok(result) => argument_visitor_result_v1(result, selected),
        };
        if let Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error)) = &result {
            self.first
                .get_or_insert(SourceAbiPlanFailureV1::Resource(*error));
        }
        match self.first {
            Some(first) => Err(first.error()),
            None => result,
        }
    }
}

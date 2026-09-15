//! The original source session remains owned until the root closes its reads.
use super::*;
use crate::production_ranked_projection_v1::ProductionRankedProjectionErrorV1 as P;

impl AuthenticatedProductionKernelContextsV1 {
    pub(crate) fn prepare_ranked_bf16_events<'native, 'source>(
        &'native self,
        owner: &'source fe2o3_pliron::ProductionSemanticSsaOwnerV1,
        root: SemanticFunctionIdV1,
        launch: &LaunchContract,
        entry: Option<&fe2o3_lower_mir_kernel::ProductionKernelContextEntrySsaRelationV1<'source>>,
        max_work: usize,
    ) -> std::result::Result<Option<GuardedBf16SourceEventsV1<'native, 'source>>, P> {
        self.checked_ranked_bf16_source(owner, root, launch, entry, max_work)?
            .map(|session| self.capture_ranked_bf16_inputs(session)
                .and_then(CapturedGlobalBf16SourceInputsV1::prepare_ranked_root_events)
                .map_err(P::StructuralValidation))
            .transpose()
    }
}

impl<'native, 'source> CapturedGlobalBf16SourceInputsV1<'native, 'source> {
    pub(crate) fn prepare_ranked_root_events(mut self) -> Result<GuardedBf16SourceEventsV1<'native, 'source>> {
        for index in 0..self.len() {
            self.checked_constructor_lane(index)?;
        }
        self.prepare_guarded_volatile_events()
    }
}

impl<'source> GuardedBf16SourceEventsV1<'_, 'source> {
    pub(crate) fn require_ranked_root(
        &self,
        owner: &fe2o3_pliron::ProductionSemanticSsaOwnerV1,
        root: SemanticFunctionIdV1,
        body: &SemanticFunctionDeclV1,
    ) -> std::result::Result<(), P> {
        if !std::ptr::eq(self.inputs.batch.owner(), owner)
            || self.inputs.batch.view().root() != root
            || !std::ptr::eq(self.inputs.batch.view().body(), body)
            || owner.execution_view_for_root(root)
                .is_none_or(|view| !std::ptr::eq(view, self.inputs.batch.view()))
            || self.rows.is_empty()
            || self.rows.len() != self.inputs.len()
        {
            return Err(P::StructuralValidation(E::CorrespondenceMismatch));
        }
        Ok(())
    }

    pub(crate) fn matrix_uses_while_reads_pending(
        &mut self,
    ) -> Result<fe2o3_lower_mir_kernel::ProductionScopedMatrixUseRelationV1<'source>> {
        self.inputs.session.matrix_uses_while_bf16_pending()
    }

    /// Visits the complete original census using the original session's charger.
    /// The callback binds root tables only; it cannot mark a read consumed.
    pub(crate) fn with_ranked_root_inputs(
        &mut self,
        function: &SemanticFunctionDeclV1,
        types: &[SemanticTypeDeclV1],
        mut inspect: impl FnMut(
            ProductionGlobalBf16SourceRowV1<'_, '_>,
            &root_physical::RootPhysicalV1<'_>,
            ProductionScopedBf16LaneUseV1,
            &mut dyn FnMut(usize) -> Result<()>,
        ) -> std::result::Result<(), P>,
    ) -> std::result::Result<(), P> {
        if !std::ptr::eq(function, self.inputs.batch.view().body())
            || !std::ptr::eq(types, self.inputs.batch.owner().source_semantic().types())
            || self.rows.len() != self.inputs.len()
        {
            return Err(P::StructuralValidation(E::CorrespondenceMismatch));
        }
        for (index, event) in self.rows.iter().enumerate() {
            let row = self.inputs.batch.row(index)
                .ok_or(P::StructuralValidation(E::CorrespondenceMismatch))?;
            self.inputs.session.with_checked_global_bf16_read_source(row, |row, lane, charge| {
                if source_key(row, lane, charge)? != event.key {
                    return Err(E::CorrespondenceMismatch);
                }
                with_formula_budget(charge, |work| {
                    let expected = formula::reference(
                        lane.contract().operand().role == SemanticMfmaOperandRoleV1::B, work,
                    )?;
                    event.graph.verify(&expected, work)
                })?;
                let physical = root_physical::project(row, charge)?;
                Ok(inspect(row, &physical, lane, charge))
            }).map_err(P::StructuralValidation)??;
        }
        Ok(())
    }

    /// Source bindings and matrix custody are not read consumption. Until the
    /// guarded materializer supplies the real consumer, the original gate must
    /// reject every nonempty batch before construction or compilation succeeds.
    pub(crate) fn finish_ranked_root(self) -> std::result::Result<(), P> {
        self.inputs.session.finish(&[]).map(|_| ()).map_err(P::StructuralValidation)
    }
}

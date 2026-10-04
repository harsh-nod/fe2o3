//! Complete constructor-output preservation for context-bearing requests.
//!
//! These records do not identify trusted workgroup providers or authorize
//! execution. The separate source receipts and scope checks retain those roles.

use super::*;
#[cfg(test)]
mod tests;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticFunctionCanonicalCommitmentV1, SemanticMirWireVersionV1,
    canonical_function_commitment_v1,
};

pub(crate) struct ExpectedFunctionCommitmentV29<'tcx> {
    instance: Instance<'tcx>,
    function: SemanticFunctionIdV1,
    identities: ProductionSemanticFunctionIdentitiesV1,
    role: SemanticFunctionRoleV1,
    source: SemanticSourceProvenanceV1,
}

impl<'tcx> ExpectedFunctionCommitmentV29<'tcx> {
    pub(crate) fn new(
        instance: Instance<'tcx>,
        function: SemanticFunctionIdV1,
        identities: ProductionSemanticFunctionIdentitiesV1,
        role: SemanticFunctionRoleV1,
        source: SemanticSourceProvenanceV1,
    ) -> Self {
        Self {
            instance,
            function,
            identities,
            role,
            source,
        }
    }

    fn matches_output(&self, function: &SemanticFunctionDeclV1) -> bool {
        self.identities
            == ProductionSemanticFunctionIdentitiesV1::new(
                function.identity(),
                function.item_definition_identity(),
                function.monomorphization_identity(),
                function.generic_type_arguments_identity(),
                function.const_generic_arguments_identity(),
            )
            && self.role == function.role()
            && self.source == function.source()
    }
}

struct FunctionCommitmentRowV29<'tcx> {
    source: ExpectedFunctionCommitmentV29<'tcx>,
    captured: Option<SemanticFunctionCanonicalCommitmentV1>,
}

pub(super) struct PendingFunctionCommitmentsV29<'tcx> {
    rows: Vec<FunctionCommitmentRowV29<'tcx>>,
    completed: usize,
}

pub(super) struct PreparedFunctionCommitmentV29<'a> {
    slot: &'a mut Option<SemanticFunctionCanonicalCommitmentV1>,
    completed: &'a mut usize,
    commitment: SemanticFunctionCanonicalCommitmentV1,
}

impl PreparedFunctionCommitmentV29<'_> {
    pub(super) fn publish(self) {
        *self.slot = Some(self.commitment);
        *self.completed += 1;
    }
}

impl<'tcx> ProductionSemanticBodyRequestOwnerV1<'tcx> {
    pub(crate) fn enable_function_commitments_v29(
        &mut self,
        expected_count: usize,
        sources: impl Iterator<Item = ExpectedFunctionCommitmentV29<'tcx>>,
    ) -> Result<(), ProductionSemanticBodyErrorV1> {
        if self.function_commitments.is_some()
            || self.totals.functions != 0
            || !self.context_entries.is_empty()
            || expected_count == 0
            || expected_count != self.defined_functions
        {
            return Err(table("function commitment initialization"));
        }
        let maximum = self.limits.limit(SemanticMirResourceV1::Functions);
        if u64::try_from(expected_count).unwrap_or(u64::MAX) > maximum {
            return Err(ProductionSemanticBodyErrorV1::LimitExceeded {
                resource: SemanticMirResourceV1::Functions,
                actual: u64::try_from(expected_count).unwrap_or(u64::MAX),
                maximum,
            });
        }
        self.charge(SemanticMirResourceV1::ValidationWork, expected_count)?;
        let mut rows = Vec::new();
        rows.try_reserve_exact(expected_count)
            .map_err(|_| allocation(SemanticMirResourceV1::Functions))?;
        for (index, source) in sources.enumerate() {
            self.charge(SemanticMirResourceV1::ValidationWork, 1)?;
            if index >= expected_count
                || source.function.index() as usize != index
                || self.defined_callable(source.instance)?.index() != source.function.index()
            {
                return Err(table("function commitment preflight roster"));
            }
            rows.push(FunctionCommitmentRowV29 {
                source,
                captured: None,
            });
        }
        if rows.len() != expected_count {
            return Err(table("function commitment preflight completeness"));
        }
        self.function_commitments = Some(PendingFunctionCommitmentsV29 { rows, completed: 0 });
        Ok(())
    }

    pub(super) fn begin_function_commitment_v29(
        &mut self,
        input: &ProductionSemanticBodyInputV1<'_, 'tcx>,
    ) -> Result<(), ProductionSemanticBodyErrorV1> {
        let Some(pending) = &self.function_commitments else {
            return Ok(());
        };
        self.totals
            .charge(SemanticMirResourceV1::ValidationWork, 1, self.limits)?;
        let row = pending
            .rows
            .get(pending.completed)
            .ok_or_else(|| table("function commitment construction order"))?;
        if row.captured.is_some()
            || row.source.instance != input.instance
            || row.source.function != input.function
            || row.source.identities != input.identities
            || row.source.role != input.role
            || row.source.source != input.source
        {
            return Err(table("function commitment source binding"));
        }
        Ok(())
    }

    pub(super) fn capture_function_commitment_v29(
        &mut self,
        function: SemanticFunctionIdV1,
        output: &SemanticFunctionDeclV1,
    ) -> Result<Option<SemanticFunctionCanonicalCommitmentV1>, ProductionSemanticBodyErrorV1> {
        let Some(pending) = &self.function_commitments else {
            return Ok(None);
        };
        self.totals
            .charge(SemanticMirResourceV1::ValidationWork, 1, self.limits)?;
        let row = pending
            .rows
            .get(pending.completed)
            .ok_or_else(|| table("function commitment construction order"))?;
        if row.source.function != function
            || row.captured.is_some()
            || !row.source.matches_output(output)
        {
            return Err(table("function commitment constructor output"));
        }
        self.totals
            .function_commitment_v29(output, self.limits)
            .map(Some)
    }
}

impl PendingFunctionCommitmentsV29<'_> {
    pub(super) fn source_identity(
        &self,
        callable: SemanticCallableIdV1,
    ) -> Result<SemanticFunctionIdentityV1, ProductionSemanticBodyErrorV1> {
        let row = self
            .rows
            .get(callable.index() as usize)
            .ok_or_else(|| table("scope provider source identity"))?;
        if row.source.function.index() != callable.index() {
            return Err(table("scope provider source function"));
        }
        Ok(row.source.identities.identity)
    }

    pub(super) fn prepare(
        &mut self,
        commitment: SemanticFunctionCanonicalCommitmentV1,
    ) -> Result<PreparedFunctionCommitmentV29<'_>, ProductionSemanticBodyErrorV1> {
        let row = self
            .rows
            .get_mut(self.completed)
            .ok_or_else(|| table("function commitment construction order"))?;
        if row.captured.is_some() {
            return Err(table("function commitment duplicate"));
        }
        Ok(PreparedFunctionCommitmentV29 {
            slot: &mut row.captured,
            completed: &mut self.completed,
            commitment,
        })
    }

    pub(super) fn verify(
        self,
        semantic: &AdmittedInertSemanticMirV1,
        totals: &mut ConstructionTotalsV1,
        limits: SemanticMirLimitsV1,
    ) -> Result<(), ProductionSemanticBodyErrorV1> {
        totals.charge(SemanticMirResourceV1::ValidationWork, 1, limits)?;
        if semantic.wire_version() != SemanticMirWireVersionV1::V29
            || self.completed != self.rows.len()
            || semantic.functions().len() != self.rows.len()
        {
            return Err(table("function commitment seal completeness"));
        }
        // This bounded roster walk precedes the existing issuance census.
        // Both borrow this same immutable admitted request and share its ledger.
        for (row, function) in self.rows.into_iter().zip(semantic.functions()) {
            totals.charge(SemanticMirResourceV1::ValidationWork, 1, limits)?;
            let captured = row
                .captured
                .ok_or_else(|| table("function commitment missing body"))?;
            if totals.function_commitment_v29(function, limits)? != captured {
                return Err(table("function commitment changed body"));
            }
        }
        Ok(())
    }
}

impl ConstructionTotalsV1 {
    pub(super) fn declaration_tables_commitment_v29(
        &mut self,
        types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
        callables: &[fe2o3_mir_model::semantic_mir_v1::SemanticCallableDeclV1],
        limits: SemanticMirLimitsV1,
    ) -> Result<
        fe2o3_mir_model::semantic_mir_v1::SemanticDeclarationTablesCommitmentV1,
        ProductionSemanticBodyErrorV1,
    > {
        fe2o3_mir_model::semantic_mir_v1::canonical_declaration_tables_commitment_v1(
            types,
            callables,
            SemanticMirWireVersionV1::V29,
            limits,
            &mut |amount| {
                charge_construction_total_v1(
                    &mut self.validation_work,
                    SemanticMirResourceV1::ValidationWork,
                    amount,
                    limits.limit(SemanticMirResourceV1::ValidationWork),
                )
            },
        )
        .map_err(construction_resource_error_v1)
    }

    fn function_commitment_v29(
        &mut self,
        function: &SemanticFunctionDeclV1,
        limits: SemanticMirLimitsV1,
    ) -> Result<SemanticFunctionCanonicalCommitmentV1, ProductionSemanticBodyErrorV1> {
        canonical_function_commitment_v1(
            function,
            SemanticMirWireVersionV1::V29,
            limits,
            &mut |amount| {
                charge_construction_total_v1(
                    &mut self.validation_work,
                    SemanticMirResourceV1::ValidationWork,
                    amount,
                    limits.limit(SemanticMirResourceV1::ValidationWork),
                )
            },
        )
        .map_err(construction_resource_error_v1)
    }
}

// Both error surfaces use this same counter update; no shadow work budget.
pub(super) fn charge_construction_total_v1(
    slot: &mut u64,
    resource: SemanticMirResourceV1,
    amount: usize,
    maximum: u64,
) -> Result<(), SemanticMirErrorV1> {
    let amount = u64::try_from(amount).unwrap_or(u64::MAX);
    *slot = slot
        .checked_add(amount)
        .ok_or(SemanticMirErrorV1::LimitExceeded {
            resource,
            actual: u64::MAX,
            max: maximum,
        })?;
    if *slot > maximum {
        return Err(SemanticMirErrorV1::LimitExceeded {
            resource,
            actual: *slot,
            max: maximum,
        });
    }
    Ok(())
}

pub(super) fn construction_resource_error_v1(
    error: SemanticMirErrorV1,
) -> ProductionSemanticBodyErrorV1 {
    match error {
        SemanticMirErrorV1::LimitExceeded {
            resource,
            actual,
            max,
        } => ProductionSemanticBodyErrorV1::LimitExceeded {
            resource,
            actual,
            maximum: max,
        },
        error => ProductionSemanticBodyErrorV1::Schema(error),
    }
}

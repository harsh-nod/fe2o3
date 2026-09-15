mod carrier_borrow_observation {
    use super::*;
    include!("carrier_borrow_observation.rs");
}

impl WorkgroupSourceResolverV1<'_, '_> {
    fn carrier_borrow(
        &mut self,
        site: Site,
        projections: &[SemanticProjectionV1],
        contract: SemanticExecutionCapabilityContractV1,
        depth: usize,
    ) -> Result<Origin> {
        use fe2o3_pliron::{
            ProductionSemanticSsaSourceQueryErrorV1 as QueryError,
            ProductionSemanticSsaSourceSiteV1,
        };
        // Retrieve the original node for the pointer-identical source-use query.
        // An equal clone is not an authenticated source occurrence.
        let view = self.view;
        let statement = site
            .statement
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let Some(SemanticStatementKindV1::Assign(assignment)) = view
            .body()
            .blocks()
            .get(site.block as usize)
            .and_then(|block| block.statements().get(statement as usize))
            .map(|statement| statement.kind())
        else {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        };
        let SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Shared,
            place,
        } = assignment.value().kind()
        else {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        };
        let destination = view
            .body()
            .locals()
            .get(site.local as usize)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
            .ty();
        if assignment.destination().local().index() != site.local
            || !assignment.destination().projections().is_empty()
            || assignment.destination().ty() != destination
            || assignment.value().result_type() != destination
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let source_type = view
            .body()
            .locals()
            .get(place.local().index() as usize)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
            .ty();
        self.graph.charge(borrow_route::route_work(
            place.projections().len(),
            projections.len(),
        )?)?;
        let route = borrow_route::borrow_route(
            self.owner.source_semantic().types(),
            source_type,
            place,
            destination,
            projections,
        )?;
        let query = self
            .owner
            .source_query_for_root(view.root(), view.body())
            .map_err(|_| ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let mut work_error = None;
        let result = query.borrow_place_use(
            ProductionSemanticSsaSourceSiteV1::new(
                SemanticBlockIdV1::from_index(site.block),
                site.statement,
            ),
            place,
            &mut || match self.graph.charge(1) {
                Ok(()) => true,
                Err(error) => {
                    work_error = Some(error);
                    false
                }
            },
        );
        if let Some(error) = work_error {
            return Err(error);
        }
        let source = result.map_err(|error| match error {
            QueryError::NoPromotedUse => {
                carrier_borrow_observation::emit(self, &query, site, place, destination, projections);
                reject("Workgroup carrier borrow has no exact SSA use")
            }
            QueryError::DisagreeingUses => {
                reject("Workgroup carrier borrow has disagreeing SSA uses")
            }
            _ => ProductionSemanticKirErrorV1::CorrespondenceMismatch,
        })?;
        let mut origin = self.resolve(source, &route.path, contract, depth + 1)?;
        if route.storage_loan {
            let words = origin
                .loans
                .len()
                .checked_add(1)
                .and_then(|n| {
                    n.checked_mul(
                        std::mem::size_of::<Loan>().div_ceil(std::mem::size_of::<usize>()),
                    )
                })
                .and_then(|n| n.checked_add(3))
                .ok_or(ProductionSemanticKirErrorV1::AllocationFailure {
                    resource: ProductionSemanticKirResourceV1::AnalysisWork,
                })?;
            self.graph.charge(words)?;
            origin.loans.try_reserve_exact(1).map_err(|_| {
                ProductionSemanticKirErrorV1::AllocationFailure {
                    resource: ProductionSemanticKirResourceV1::AnalysisWork,
                }
            })?;
            origin.loans.push(Loan {
                borrow: site,
                owner_value: source,
                owner_local: place.local().index(),
            });
        }
        Ok(origin)
    }
}

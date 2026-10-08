// Original owned type/projection facts for an eventual atomic-only cast.
// This is NOT pointer, allocation, loan, or AtomicRmw authority. Callers must
// additionally authenticate the original operation and every current origin.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceAtomicStoragePathV41 {
    wrapper: SemanticTypeIdV1,
    children: [SemanticTypeIdV1; 3],
    depth: u8,
}
impl SourceAtomicStoragePathV41 {
    fn current_type(self) -> SemanticTypeIdV1 {
        if self.depth == 0 {
            self.wrapper
        } else {
            self.children[usize::from(self.depth - 1)]
        }
    }

    // One real zero-offset field edge only: never reverse, skip, or infer a
    // wrapper from a scalar/UnsafeCell/Align4 found independently.
    fn next_field(self, output: SemanticTypeIdV1) -> Option<SemanticProjectionV1> {
        (self.children.get(usize::from(self.depth)).copied() == Some(output))
            .then(|| SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), output).ok())
            .flatten()
    }

    fn is_scalar_leaf(self) -> bool {
        self.depth == 3
    }
}

// A typed Cast may traverse several committed transparent field-zero edges.
// This returns only inert path algebra. The caller must already own the exact
// original Cast, pointer holder, allocation, generation and live atomic custody.
// It never invents a SemanticProjection or permits a skipped projected access.
fn source_atomic_cast_descendant_v41(
    types: &[SemanticTypeDeclV1],
    mut path: SourceAtomicStoragePathV41,
    output: SemanticTypeIdV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Option<SourceAtomicStoragePathV41>, ProductionSemanticKirErrorV1> {
    source_reference_emission_prepay_v29::<Option<SourceAtomicStoragePathV41>>(budget)?;
    source_reference_emission_prepay_v29::<
        Result<Option<SourceAtomicStoragePathV41>, ProductionSemanticKirErrorV1>,
    >(budget)?;
    source_reference_emission_prepay_v29::<Option<[SemanticTypeIdV1; 3]>>(budget)?;
    budget.charge_work(4)?;
    if path.depth > 3 {
        return Ok(None);
    }
    // Recheck every intermediate field identity and exact storage layout from
    // the same original type table, not the diagnostic record or target shape.
    budget.charge_work(40)?;
    let Some(children) =
        fe2o3_mir_model::semantic_mir_v1::semantic_atomic_storage_chain_v41(types, path.wrapper)
    else {
        return Ok(None);
    };
    budget.charge_work(3)?;
    if children != path.children {
        return Ok(None);
    }
    if output == path.current_type() {
        return Ok(Some(path));
    }
    for _ in path.depth..3 {
        budget.charge_work(4)?;
        path.depth += 1; // The closed range bounds this at three.
        if path.current_type() == output {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

// Pure private path algebra; its result alone cannot authorize an access.
fn source_atomic_storage_path_shape_v41(
    types: &[SemanticTypeDeclV1],
    root: SemanticTypeIdV1,
    path: &[SemanticProjectionV1],
    expected: SemanticTypeIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<SourceAtomicStoragePathV41>, ProductionSemanticKirErrorV1> {
    source_reference_emission_prepay_v29::<Option<SourceAtomicStoragePathV41>>(budget)?;
    source_reference_emission_prepay_v29::<
        Result<Option<SourceAtomicStoragePathV41>, ProductionSemanticKirErrorV1>,
    >(budget)?;
    source_reference_emission_prepay_v29::<(SemanticTypeIdV1, usize, Option<&SemanticTypeDeclV1>)>(
        budget,
    )?;
    let mut current = root;
    let mut observed = None;
    for position in 0..=path.len() {
        budget.charge_work(4)?;
        let Some(declaration) = types.get(current.index() as usize) else {
            return Ok(None);
        };
        if observed.is_none()
            && matches!(
                declaration.rust_type_kind(),
                fe2o3_mir_model::semantic_mir_v1::SemanticRustTypeKindV1::AtomicI32
                    | fe2o3_mir_model::semantic_mir_v1::SemanticRustTypeKindV1::AtomicU32
            )
        {
            budget.charge_work(40)?;
            let Some(children) =
                fe2o3_mir_model::semantic_mir_v1::semantic_atomic_storage_chain_v41(types, current)
            else {
                return Ok(None);
            };
            observed = Some(SourceAtomicStoragePathV41 {
                wrapper: current,
                children,
                depth: 0,
            });
        }
        if position == path.len() {
            break;
        }
        let projection = path[position];
        let SemanticProjectionKindV1::Field(field) = projection.kind() else {
            return Ok(None);
        };
        let (SemanticTypeShapeV1::Aggregate(fields) | SemanticTypeShapeV1::Tuple(fields)) =
            declaration.shape()
        else {
            return Ok(None);
        };
        if fields.fields().get(field as usize).copied() != Some(projection.result_type()) {
            return Ok(None);
        }
        if let Some(fact) = &mut observed {
            if field != 0 || fact.next_field(projection.result_type()) != Some(projection) {
                return Ok(None);
            }
            fact.depth += 1; // At most three; next_field has checked the fixed array.
        }
        current = projection.result_type();
    }
    if current != expected || observed.is_some_and(|fact| fact.current_type() != current) {
        return Ok(None);
    }
    Ok(observed)
}

impl SourceReferencePlanV29<'_, '_> {
    fn atomic_storage_path_v41(
        &self,
        instance: ProductionCallInstanceIdV1,
        local: SemanticLocalIdV1,
        path: std::ops::Range<usize>,
        pointee: SemanticTypeIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<SourceAtomicStoragePathV41>, ProductionSemanticKirErrorV1> {
        let result = (|| {
            self.check_owner(self.instances, budget)?;
            self.charge(4, budget)?;
            let original = self.instances.owner().source_semantic();
            if original.wire_version()
                != fe2o3_mir_model::semantic_mir_v1::SemanticMirWireVersionV1::V41
            {
                return Ok(None);
            }
            let root = self
                .instances
                .instance(instance)
                .and_then(|row| row.declaration().locals().get(local.index() as usize))
                .ok_or(ArgumentResourceV1::Accounting)?
                .ty();
            let projections = self
                .projections
                .get(path)
                .ok_or(ArgumentResourceV1::Accounting)?;
            source_atomic_storage_path_shape_v41(
                original.types(),
                root,
                projections,
                pointee,
                budget,
            )
        })();
        result.inspect_err(|error| source_reference_record_failure_v29(self, error))
    }
}

#[cfg(test)]
#[path = "production_source_atomic_storage_path_v41_tests.rs"]
mod source_atomic_storage_path_v41_tests;

/// Interpretation obligation attached to one checked source-reference carrier.
/// Neither variant grants allocation, dereference, or source-value authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSourceReferenceCarrierV38 {
    /// An actual canonical pointer, with independently required memory semantics.
    MemoryPointer,
    /// A scalar payload of a shared stable referent, not an address or pointer.
    StableScalar,
}

/// Borrowed original SingleLoan coordinates retained with the typed SSA archive.
/// Static generation and call-instance coordinates are not dynamic allocation IDs.
pub struct ProductionSourceReferenceEndpointV38<'a, 'source> {
    owner: &'a ProductionSourceCorrespondenceV18<'source>,
    loan: &'a SourceSsaLoanV36,
}

impl ProductionSourceReferenceEndpointV38<'_, '_> {
    /// Returns the checked representation class without interpreting its value.
    pub fn carrier(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionSourceReferenceCarrierV38> {
        self.owner.retain_query((|| {
            self.owner.query(budget)?;
            budget.charge_work(1)?;
            Ok(self.loan.carrier)
        })())
    }

    /// Returns the root-relative original invocation containing the referent.
    pub fn origin_instance(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<usize> {
        self.owner.retain_query((|| {
            self.owner.query(budget)?;
            budget.charge_work(1)?;
            Ok(self.loan.origin_instance.index())
        })())
    }

    /// Returns the original semantic function declaring the referent.
    pub fn origin_function(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<SemanticFunctionIdV1> {
        self.owner.retain_query((|| {
            self.owner.query(budget)?;
            budget.charge_work(1)?;
            Ok(self.loan.origin_function)
        })())
    }

    /// Returns the original local at the root of the referent place. StableScalar
    /// has no projections; MemoryPointer may still require a projected-place join.
    pub fn origin_local(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<SemanticLocalIdV1> {
        self.owner.retain_query((|| {
            self.owner.query(budget)?;
            budget.charge_work(1)?;
            Ok(self.loan.origin_local)
        })())
    }

    /// Returns the original source referent type, not the canonical carrier type.
    pub fn origin_type(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<SemanticTypeIdV1> {
        self.owner.retain_query((|| {
            self.owner.query(budget)?;
            budget.charge_work(1)?;
            Ok(self.loan.origin_type)
        })())
    }

    /// Returns the original static local-generation coordinate, not a runtime epoch.
    pub fn origin_generation(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<u32> {
        self.owner.retain_query((|| {
            self.owner.query(budget)?;
            budget.charge_work(1)?;
            Ok(self.loan.origin_generation)
        })())
    }

    /// Returns the original loan site as invocation, source block and optional statement.
    pub fn borrow_site(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<(usize, SemanticBlockIdV1, Option<usize>)> {
        self.owner.retain_query((|| {
            self.owner.query(budget)?;
            budget.charge_work(1)?;
            Ok((
                self.loan.site.instance.index(),
                self.loan.site.block,
                self.loan.site.statement,
            ))
        })())
    }
}

impl<'a, 'source> ProductionSourceSsaEndpointV36<'a, 'source> {
    /// Borrows this leaf's checked reference provenance. Ordinary values and Unit
    /// return `None`; aggregate callers must select an exact component first.
    pub fn reference(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<ProductionSourceReferenceEndpointV38<'a, 'source>>> {
        self.owner.retain_query((|| {
            self.owner.query(budget)?;
            budget.charge_work(2)?;
            match self.physical {
                SourceSsaPhysicalV36::Value {
                    loan: Some(loan), ..
                } => Ok(Some(ProductionSourceReferenceEndpointV38 {
                    owner: self.owner,
                    loan,
                })),
                SourceSsaPhysicalV36::Value { loan: None, .. } | SourceSsaPhysicalV36::Unit => {
                    Ok(None)
                }
                SourceSsaPhysicalV36::Aggregate { .. } | SourceSsaPhysicalV36::Unmodeled => self
                    .owner
                    .source
                    .missing("source reference query requires an exact modeled leaf"),
            }
        })())
    }
}

fn source_reference_endpoint_headers_v38() -> Result<usize, ArgumentResourceV1> {
    fn h<T>() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            size_of::<T>(),
            argument_product_v1(2, size_of::<Result<T, ProductionSemanticKirErrorV1>>())?,
            argument_product_v1(2, size_of::<SourceOwnedResultV18<T>>())?,
        ])
    }
    argument_sum_v1(&[
        h::<ProductionSourceReferenceCarrierV38>()?,
        h::<ProductionSourceReferenceEndpointV38<'_, '_>>()?,
        h::<Option<ProductionSourceReferenceEndpointV38<'_, '_>>>()?,
        h::<(usize, SemanticBlockIdV1, Option<usize>)>()?,
        h::<SourceReferenceRepresentationV29>()?,
        h::<SemanticBorrowKindV1>()?,
        h::<u32>()?,
        h::<&SourceSsaLoanV36>()?,
    ])
}

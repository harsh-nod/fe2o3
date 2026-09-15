use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct SsaUse {
    pub(super) site: ProductionSemanticSsaSourceSiteV1,
    pub(super) ty: SemanticTypeIdV1,
    pub(super) variable: u32,
    pub(super) value: SsaValueV1,
    pub(super) event_range: [usize; 2],
    pub(super) agreeing_uses: usize,
}

impl SsaUse {
    pub(super) fn scalar(self) -> SourceValue {
        SourceValue::Ssa {
            variable: self.variable,
            value: self.value,
            event_range: self.event_range,
            agreeing_uses: self.agreeing_uses,
        }
    }
}

pub(super) fn ssa_use<'source>(
    query: &ProductionSemanticSsaSourceQueryV1<'source>,
    site: ProductionSemanticSsaSourceSiteV1,
    operand: &'source SemanticOperandV1,
    charge: &mut dyn FnMut(usize) -> Result<()>,
) -> Result<SsaUse> {
    charge(1)?;
    if !matches!(
        operand,
        SemanticOperandV1::Copy(_) | SemanticOperandV1::Move(_)
    ) {
        return Err(error(formula::Error::Source));
    }
    let mut failure = None;
    let used = query.operand_use(site, operand, &mut || match charge(1) {
        Ok(()) => true,
        Err(e) => {
            failure = Some(e);
            false
        }
    });
    if let Some(e) = failure {
        return Err(e);
    }
    let used = used.map_err(|_| error(formula::Error::Source))?;
    let range = used.event_range();
    Ok(SsaUse {
        site,
        ty: operand.ty(),
        variable: used.variable().get(),
        value: used.value(),
        event_range: [range.start, range.end],
        agreeing_uses: used.agreeing_uses(),
    })
}

fn exact_shared_edge(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    pointee: SemanticTypeIdV1,
    metadata: SemanticPointerMetadataV1,
) -> bool {
    matches!(types.get(ty.index() as usize).map(SemanticTypeDeclV1::shape),
        Some(SemanticTypeShapeV1::Pointer(p))
        if p.kind() == SemanticPointerKindV1::Reference
        && p.mutability() == SemanticMutabilityV1::Immutable
        && p.address_space() == 0 && p.pointer_width_bits() == 64
        && p.pointee() == pointee && p.metadata() == metadata
        && types.get(pointee.index() as usize).is_some())
}

pub(super) fn reference_use<'source>(
    types: &[SemanticTypeDeclV1],
    query: &ProductionSemanticSsaSourceQueryV1<'source>,
    operand: ProductionGlobalBf16SourceOperandV1<'source>,
    pointee: SemanticTypeIdV1,
    metadata: SemanticPointerMetadataV1,
    charge: &mut dyn FnMut(usize) -> Result<()>,
) -> Result<SsaUse> {
    charge(2)?;
    if !exact_shared_edge(types, operand.operand().ty(), pointee, metadata) {
        return Err(E::CorrespondenceMismatch);
    }
    ssa_use(query, operand.site(), operand.operand(), charge)
}

#[cfg(test)]
#[path = "reference_bindings_tests.rs"]
mod tests;

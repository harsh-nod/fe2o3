use super::*;
use fe2o3_pliron::{
    ProductionSemanticSsaIncomingValuesV1, ProductionSemanticSsaSourceQueryV1,
    ProductionSemanticSsaSourceSiteV1, ProductionSemanticSsaValueOriginV1,
    ProductionSemanticSsaValueV1,
};
use fe2o3_mir_model::{SsaBlockIdV1, SsaValueV1};
use crate::production_reference_effect_join_v2::{
    RankedGpuWriteV2, scalar_guard_v1::ScalarWriteGuardV1,
};

mod diamond;
#[cfg(test)]
mod tests;
type Site = ProductionSemanticSsaSourceSiteV1;
pub(super) const AMBIGUOUS: &str =
    "GPU semantic scalar local has multiple definitions; select/phi normalization is incomplete";
const CLOSED: &str = "GPU scalar SSA value is outside the closed two-way source selection";
const CUSTODY: &str = "GPU scalar SSA normalization lost its exact source owner or use";

fn charge(work: &mut usize) -> bool {
    let Some(next) = work.checked_add(1) else {
        return false;
    };
    *work = next;
    next <= fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2
}
fn step(work: &mut usize) -> Result<(), &'static str> {
    charge(work)
        .then_some(())
        .ok_or("GPU semantic expression exceeds its bounded node budget")
}

fn reserve_slot<T>(values: &mut Vec<T>, work: &mut usize) -> Result<(), &'static str> {
    if values.len() < values.capacity() {
        return Ok(());
    }
    let next = values.len().checked_add(1).ok_or(CUSTODY)?;
    // Charge old plus replacement payload and both Vec headers before growth.
    // No storage or work owner is created by this optional precision path.
    let bytes = next
        .checked_mul(std::mem::size_of::<T>())
        .and_then(|bytes| bytes.checked_mul(2))
        .and_then(|bytes| bytes.checked_add(2 * std::mem::size_of::<Vec<T>>()))
        .ok_or(CUSTODY)?;
    for _ in 0..bytes.div_ceil(std::mem::size_of::<usize>()) {
        step(work)?;
    }
    values.try_reserve_exact(1).map_err(|_| CUSTODY)?;
    if values.capacity() != next {
        return Err(CUSTODY);
    }
    Ok(())
}

fn contains<T: PartialEq>(values: &[T], value: &T, work: &mut usize) -> Result<bool, &'static str> {
    for candidate in values {
        step(work)?;
        if candidate == value {
            return Ok(true);
        }
    }
    Ok(false)
}
fn site(block: usize, statement: Option<usize>) -> Result<Site, &'static str> {
    Ok(Site::new(
        SemanticBlockIdV1::from_index(u32::try_from(block).map_err(|_| CUSTODY)?),
        statement
            .map(u32::try_from)
            .transpose()
            .map_err(|_| CUSTODY)?,
    ))
}

pub(super) struct SourceNormalizationV1<'a> {
    query: ProductionSemanticSsaSourceQueryV1<'a>,
    active_site: Option<Site>,
    proved_some: Vec<(u32, u32)>,
    visiting: Vec<(u32, SsaValueV1)>,
}

impl<'a> SourceNormalizationV1<'a> {
    fn new(
        query: ProductionSemanticSsaSourceQueryV1<'a>,
        function: &'a SemanticFunctionDeclV1,
        work: &mut usize,
    ) -> Result<Self, &'static str> {
        step(work)?;
        if !std::ptr::eq(query.function(), function) {
            return Err(CUSTODY);
        }
        Ok(Self {
            query,
            active_site: None,
            proved_some: Vec::new(),
            visiting: Vec::new(),
        })
    }

    fn use_value(
        &self,
        operand: &'a SemanticOperandV1,
        work: &mut usize,
    ) -> Result<ProductionSemanticSsaValueV1<'a>, &'static str> {
        step(work)?;
        let site = self.active_site.ok_or(CUSTODY)?;
        self.query
            .operand_use(site, operand, &mut || charge(work))
            .map(|value| value.retained_value())
            .map_err(|_| CUSTODY)
    }
}

/// Only the production projection constructs this deferred resolver. It keeps
/// the same borrowed source owner and the existing scalar/load/version engine.
pub(crate) struct DeferredSourceValuesV1<'a> {
    resolver: GpuSemanticExpressionResolverV2<'a>,
    query: Option<ProductionSemanticSsaSourceQueryV1<'a>>,
    intrinsic: &'a IntrinsicProjectionV1,
    callables: &'a [SemanticCallableDeclV1],
    sources: &'a [ProjectedAccessSourceV1],
}

impl<'a> DeferredSourceValuesV1<'a> {
    pub(super) fn new(
        resolver: GpuSemanticExpressionResolverV2<'a>,
        query: ProductionSemanticSsaSourceQueryV1<'a>,
        intrinsic: &'a IntrinsicProjectionV1,
        callables: &'a [SemanticCallableDeclV1],
        sources: &'a [ProjectedAccessSourceV1],
    ) -> Self {
        Self {
            resolver,
            query: Some(query),
            intrinsic,
            callables,
            sources,
        }
    }

    pub(crate) fn resolve(
        &mut self,
        write: &RankedGpuWriteV2,
        guard: &mut ScalarWriteGuardV1<'_>,
    ) -> Result<ProductionSemanticExpressionV2, &'static str> {
        if write.value.as_ref().err().copied() != Some(AMBIGUOUS) {
            return write.value.clone();
        }
        if !guard.is_write(write) {
            return Err(CUSTODY);
        }
        self.resolver.charge_v2()?;
        let mut matching = None;
        for source in self.sources {
            self.resolver.charge_v2()?;
            if source.block == write.block
                && source.operation == write.operation
                && source.access.writes_memory()
                && matching.replace(source).is_some()
            {
                return Err(CUSTODY);
            }
        }
        let source = matching.ok_or(CUSTODY)?;
        let source_site = source.semantic_site.ok_or(CUSTODY)?;
        if let Some(query) = self.query.take() {
            if !std::ptr::eq(query.function(), self.resolver.function) {
                return Err(CUSTODY);
            }
            self.resolver.source_ssa = Some(SourceNormalizationV1::new(
                query,
                self.resolver.function,
                &mut self.resolver.work,
            )?);
        }
        let state = self.resolver.source_ssa.as_mut().ok_or(CUSTODY)?;
        state.proved_some.clear();
        if !state.visiting.is_empty() {
            return Err(CUSTODY);
        }
        // These facts apply only to this complete write path. Keep the guard's
        // original work owner, and never retain a previous write's successful fact.
        for (&(block, statement), producer) in &self.intrinsic.global_uses.discriminants {
            step(&mut self.resolver.work)?;
            let Some(effect) = self
                .intrinsic
                .direct_read_effects
                .get(*producer)
                .and_then(Option::as_ref)
            else {
                return Err(CUSTODY);
            };
            if effect.checked_success.is_some()
                || effect.access != AccessKindAttr::Read
                || effect.memory_space != MemorySpaceAttr::Global
                || effect.comparisons.is_empty()
            {
                continue;
            }
            let mut proven = true;
            for &(lhs, rhs) in &effect.comparisons {
                step(&mut self.resolver.work)?;
                proven &= guard.proves_less_than(write, lhs, rhs)?;
            }
            if proven {
                let site = site(block, Some(statement))?;
                reserve_slot(&mut state.proved_some, &mut self.resolver.work)?;
                state
                    .proved_some
                    .push((site.block().index(), site.statement().ok_or(CUSTODY)?));
            }
        }
        let intrinsic = self.intrinsic;
        let callables = self.callables;
        let expression = self.resolver.with_source_site_v1(
            site(source_site.block, source_site.statement)?,
            |resolver| {
                let body = resolver
                    .function
                    .blocks()
                    .get(source_site.block)
                    .ok_or(CUSTODY)?;
                if let Some(statement) = source_site.statement {
                    let statement = body.statements().get(statement).ok_or(CUSTODY)?;
                    if statement.source() != source.source {
                        return Err(CUSTODY);
                    }
                    resolver.resolve_store_v2(statement.kind())
                } else {
                    let (call, contract, bound) = typed_global_source_call_v1(
                        resolver.function,
                        callables,
                        intrinsic,
                        source,
                        write.view,
                        &write.indices,
                        source.access,
                    )?;
                    if bound.allocation.allocation_origin != write.allocation_origin {
                        return Err(CUSTODY);
                    }
                    resolver
                        .resolve_operand_v2(contract.shape.value_operand(call).ok_or(CUSTODY)?, 0)
                }
            },
        )?;
        self.resolver.validate_source_expression_v1(expression)
    }
}

impl<'a> GpuSemanticExpressionResolverV2<'a> {
    pub(super) fn resolve_reaching_store_rhs_v1(
        &mut self,
        semantic_block: usize,
        depth: usize,
    ) -> Result<ProductionSemanticExpressionV2, &'static str> {
        let operand = self
            .function
            .blocks()
            .get(semantic_block)
            .and_then(|block| match block.terminator().kind() {
                SemanticTerminatorKindV1::Call(call) => call.arguments().get(2),
                _ => None,
            })
            .ok_or("mutable global reaching store lost its source call")?;
        if self.source_ssa.is_some() {
            // The memory-version owner selected this original store. Its RHS
            // belongs to that terminator, not the later write being normalized.
            self.with_source_site_v1(site(semantic_block, None)?, |resolver| {
                resolver.resolve_operand_v2(operand, depth)
            })
        } else {
            self.resolve_operand_v2(operand, depth)
        }
    }

    fn validate_source_expression_v1(
        &mut self,
        expression: ProductionSemanticExpressionV2,
    ) -> Result<ProductionSemanticExpressionV2, &'static str> {
        fn nodes(
            expression: &ProductionSemanticExpressionV2,
            depth: usize,
            work: &mut usize,
        ) -> Result<usize, &'static str> {
            step(work)?;
            if depth > fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 {
                return Err("scalar SSA selection exceeds its bounded validation depth");
            }
            use ProductionSemanticExpressionV2::*;
            let children = match expression {
                Symbol { .. } | Constant { .. } | Load(_) => 0,
                Unary { operand, .. } | Cast { operand, .. } => nodes(operand, depth + 1, work)?,
                Binary { lhs, rhs, .. } | Compare { lhs, rhs, .. } => nodes(lhs, depth + 1, work)?
                    .checked_add(nodes(rhs, depth + 1, work)?)
                    .ok_or(CUSTODY)?,
                Select {
                    condition,
                    when_true,
                    when_false,
                    ..
                } => {
                    let condition = nodes(condition, depth + 1, work)?;
                    let when_true = nodes(when_true, depth + 1, work)?;
                    let when_false = nodes(when_false, depth + 1, work)?;
                    condition
                        .checked_add(when_true)
                        .and_then(|sum| sum.checked_add(when_false))
                        .ok_or(CUSTODY)?
                }
            };
            children.checked_add(1).ok_or(CUSTODY)
        }
        let count = nodes(&expression, 1, &mut self.work)?;
        // Validation/statistics/domain walks plus worst-case repeated constant
        // folds. This reserves work only; no second arithmetic proof is made.
        let validation_work = count
            .checked_mul(count)
            .and_then(|square| square.checked_mul(2))
            .and_then(|folds| {
                count
                    .checked_mul(3)
                    .and_then(|walks| folds.checked_add(walks))
            })
            .ok_or(CUSTODY)?;
        for _ in 0..validation_work {
            self.charge_v2()?;
        }
        expression
            .validate()
            .map_err(|_| "scalar SSA selection is not a valid typed expression")?;
        expression
            .validate_static_domains()
            .map_err(|_| "scalar SSA selection has a partial arithmetic domain")?;
        Ok(expression)
    }
    fn with_source_site_v1<T>(
        &mut self,
        site: Site,
        f: impl FnOnce(&mut Self) -> Result<T, &'static str>,
    ) -> Result<T, &'static str> {
        let previous = self
            .source_ssa
            .as_mut()
            .ok_or(CUSTODY)?
            .active_site
            .replace(site);
        let result = f(self);
        self.source_ssa.as_mut().ok_or(CUSTODY)?.active_site = previous;
        result
    }
    pub(super) fn resolve_source_operand_v1(
        &mut self,
        operand: &'a SemanticOperandV1,
        depth: usize,
    ) -> Result<ProductionSemanticExpressionV2, &'static str> {
        let (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = operand else {
            return Err(CUSTODY);
        };
        // The original use-specific load path precedes promotion and preserves
        // all existing mutable-memory/version checks, including projected payloads.
        if self.place_loads.contains_key(&(place as *const _)) {
            return self.resolve_place_v2(place, depth);
        }
        if !place.projections().is_empty()
            || self
                .address_escaped
                .contains(&(place.local().index() as usize))
        {
            return Err(CUSTODY);
        }
        let value = self
            .source_ssa
            .as_ref()
            .ok_or(CUSTODY)?
            .use_value(operand, &mut self.work)?;
        let scalar = self.scalar_v2(place.ty())?;
        self.resolve_source_value_v1(value, scalar, depth)
    }

    fn resolve_source_value_v1(
        &mut self,
        value: ProductionSemanticSsaValueV1<'a>,
        scalar: ProductionSemanticScalarTypeV2,
        depth: usize,
    ) -> Result<ProductionSemanticExpressionV2, &'static str> {
        Self::require_depth_v2(depth)?;
        self.charge_v2()?;
        let key = (value.variable().get(), value.value());
        let state = self.source_ssa.as_mut().ok_or(CUSTODY)?;
        if contains(&state.visiting, &key, &mut self.work)? {
            return Err("GPU scalar SSA selection has a cyclic value");
        }
        reserve_slot(&mut state.visiting, &mut self.work)?;
        state.visiting.push(key);
        let resolved = (|| {
            let state = self.source_ssa.as_ref().ok_or(CUSTODY)?;
            let origin = state
                .query
                .value_origin(&value, &mut || charge(&mut self.work))
                .map_err(|_| CUSTODY)?;
            let local = value.variable().get();
            let declaration = self.function.locals().get(local as usize).ok_or(CUSTODY)?;
            if self.scalar_v2(declaration.ty())? != scalar {
                return Err(CUSTODY);
            }
            let expression = match origin {
                ProductionSemanticSsaValueOriginV1::Entry { .. } => {
                    let SemanticLocalRoleV1::Argument(argument) = declaration.role() else {
                        return Err(CLOSED);
                    };
                    let symbol = crate::reference_effect_v1::kernel_scalar_symbol_v2(argument)
                        .ok_or(CUSTODY)?;
                    ProductionSemanticExpressionV2::Symbol { symbol, scalar }
                }
                ProductionSemanticSsaValueOriginV1::Event { site, .. } => {
                    let SemanticStatementKindV1::Assign(assignment) = self
                        .function
                        .blocks()
                        .get(site.block().index() as usize)
                        .and_then(|block| {
                            site.statement()
                                .and_then(|statement| block.statements().get(statement as usize))
                        })
                        .ok_or(CUSTODY)?
                        .kind()
                    else {
                        return Err(CLOSED);
                    };
                    if assignment.destination().local().index() != local
                        || !assignment.destination().projections().is_empty()
                        || assignment.destination().ty() != declaration.ty()
                        || assignment.value().result_type() != declaration.ty()
                    {
                        return Err(CUSTODY);
                    }
                    self.with_source_site_v1(site, |resolver| {
                        resolver.resolve_rvalue_inner_v2(assignment.value(), depth + 1)
                    })?
                }
                ProductionSemanticSsaValueOriginV1::Edge { .. } => return Err(CLOSED),
                ProductionSemanticSsaValueOriginV1::BlockArgument(incoming) => {
                    let selection =
                        diamond::two_way(&state.query, &incoming, &mut || charge(&mut self.work))?;
                    let condition = self.with_source_site_v1(selection.site, |resolver| {
                        resolver.source_condition_v1(selection.condition, depth + 1)
                    })?;
                    let when_true =
                        self.resolve_source_value_v1(selection.when_true, scalar, depth + 1)?;
                    let when_false =
                        self.resolve_source_value_v1(selection.when_false, scalar, depth + 1)?;
                    ProductionSemanticExpressionV2::Select {
                        scalar,
                        condition: Box::new(condition),
                        when_true: Box::new(when_true),
                        when_false: Box::new(when_false),
                    }
                }
            };
            if expression.scalar() != scalar {
                return Err(CUSTODY);
            }
            Ok(expression)
        })();
        if self.source_ssa.as_mut().ok_or(CUSTODY)?.visiting.pop() != Some(key) {
            return Err(CUSTODY);
        }
        resolved
    }

    fn source_condition_v1(
        &mut self,
        operand: &'a SemanticOperandV1,
        depth: usize,
    ) -> Result<ProductionSemanticExpressionV2, &'static str> {
        self.charge_v2()?;
        let state = self.source_ssa.as_ref().ok_or(CUSTODY)?;
        if matches!(
            operand,
            SemanticOperandV1::Copy(_) | SemanticOperandV1::Move(_)
        ) {
            let value = state.use_value(operand, &mut self.work)?;
            if let ProductionSemanticSsaValueOriginV1::Event { site, .. } = state
                .query
                .value_origin(&value, &mut || charge(&mut self.work))
                .map_err(|_| CUSTODY)?
            {
                if let Some(statement) = site.statement()
                    && contains(
                        &state.proved_some,
                        &(site.block().index(), statement),
                        &mut self.work,
                    )?
                {
                    let SemanticStatementKindV1::Assign(assignment) = self.function.blocks()
                        [site.block().index() as usize]
                        .statements()[site.statement().ok_or(CUSTODY)? as usize]
                        .kind()
                    else {
                        return Err(CUSTODY);
                    };
                    if assignment.destination().local().index() != value.variable().get()
                        || !matches!(
                            assignment.value().kind(),
                            SemanticRvalueKindV1::Discriminant(_)
                        )
                    {
                        return Err(CUSTODY);
                    }
                    return Ok(ProductionSemanticExpressionV2::Constant {
                        scalar: ProductionSemanticScalarTypeV2::Bool,
                        bits: 1,
                    });
                }
            }
        }
        if self.scalar_v2(operand.ty())? != ProductionSemanticScalarTypeV2::Bool {
            return Err(
                "scalar SSA selector lacks an exact Boolean or proved typed-global condition",
            );
        }
        self.resolve_operand_v2(operand, depth)
    }
}

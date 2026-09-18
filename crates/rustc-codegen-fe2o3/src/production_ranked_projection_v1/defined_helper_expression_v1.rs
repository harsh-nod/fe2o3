//! Private source-value expansion. Calls and their effects remain in the IR.
use super::helper_value_template_v1::{Ledger, Meter};
use super::source_helper_value_context_v1::with_source_helper_values;
use super::*;

struct SourceValueMeter<'a> {
    facts: &'a mut dyn ProjectedAssertionFactsV1,
    failure: Option<ProductionRankedProjectionErrorV1>,
}

impl SourceValueMeter<'_> {
    fn capture<T>(
        &mut self,
        result: Result<T, ProductionRankedProjectionErrorV1>,
    ) -> Result<T, &'static str> {
        result.map_err(|error| {
            self.failure.get_or_insert(error);
            "helper value caller resource ledger refused the request"
        })
    }
}

impl Meter for SourceValueMeter<'_> {
    fn work(&mut self, amount: usize) -> Result<(), &'static str> {
        let result = self.facts.charge_private_array_work(amount);
        self.capture(result)
    }
    fn reserve(&mut self, amount: usize) -> Result<(), &'static str> {
        let result = self.facts.reserve_scalar_private_storage_v1(amount);
        self.capture(result)
    }
    fn release(&mut self, amount: usize) -> Result<(), &'static str> {
        let result = self.facts.release_scalar_private_storage_v1(amount);
        self.capture(result)
    }
    fn exhausted(&self) -> bool {
        self.failure.is_some()
    }
    fn storage(&self) -> Result<usize, &'static str> {
        self.facts
            .scalar_private_storage_v1()
            .map_err(|_| "helper value storage ledger unavailable")
    }
    fn identity(&mut self) -> Result<Ledger, &'static str> {
        let result = self.facts.helper_value_ledger_v1();
        self.capture(result)
            .map(|(slot, work)| Ledger { slot, work })
    }
}

/// The returned byte count covers additional instantiated expression trees.
/// The caller drops the writes before releasing it, including error paths via
/// its enclosing canonical scratch transaction. Existing root resolver/index
/// allocations retain their independent historical bounded domain.
pub(super) fn projected_reference_gpu_writes_with_helpers_v1(
    semantic: &AdmittedInertSemanticMirV1,
    function: &SemanticFunctionDeclV1,
    blocks: &[ProductionRankedBlockV1],
    sources: &[ProjectedAccessSourceV1],
    facts: &mut dyn ProjectedAssertionFactsV1,
) -> Result<
    (
        Vec<crate::production_reference_effect_join_v2::RankedGpuWriteV2>,
        usize,
    ),
    ProductionRankedProjectionErrorV1,
> {
    facts.charge_private_array_work(function.blocks().len())?;
    let needed = function.blocks().iter().any(|block| {
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            return false;
        };
        matches!(
            semantic.callables().get(call.callee().index() as usize),
            Some(SemanticCallableDeclV1::Defined { .. })
        )
    });
    if !needed {
        return projected_reference_gpu_writes_v2(
            semantic.types(),
            function,
            semantic.callables(),
            blocks,
            sources,
        )
        .map(|writes| (writes, 0));
    }
    facts.charge_private_array_work(semantic.functions().len())?;
    let root = semantic
        .functions()
        .iter()
        .position(|candidate| std::ptr::eq(candidate, function))
        .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
            "helper value root is foreign to source",
        ))?;
    let mut meter = SourceValueMeter {
        facts,
        failure: None,
    };
    let mut projection_error = None;
    let result = with_source_helper_values(semantic, root, &mut meter, |context, meter| {
        let mut expressions = GpuSemanticExpressionResolverV2::with_ranked_reads(
            semantic.types(),
            function,
            blocks,
            sources,
        )
        .map_err(|error| {
            projection_error = Some(error);
            "helper value root resolver could not be constructed"
        })?;
        expressions.helper_semantic = Some(semantic);
        expressions.helper_values = Some(context);
        expressions.helper_meter = Some(&mut *meter);
        let index_bytes = function
            .locals()
            .len()
            .checked_mul(std::mem::size_of::<Option<(usize, &SemanticDirectCallV1)>>())
            .ok_or("helper scalar-call index storage overflow")?;
        let index_work = function
            .blocks()
            .len()
            .checked_mul(2)
            .and_then(|n| n.checked_add(function.locals().len()))
            .and_then(|n| n.checked_mul(8))
            .ok_or("helper scalar-call index work overflow")?;
        expressions
            .helper_meter
            .as_deref_mut()
            .ok_or("helper meter unavailable")?
            .work(index_work)?;
        expressions
            .helper_meter
            .as_deref_mut()
            .ok_or("helper meter unavailable")?
            .reserve(index_bytes)?;
        let mut expressions = expressions
            .with_scalar_callables_v1(semantic.callables())
            .map_err(|error| {
                projection_error = Some(error);
                "helper value scalar call index could not be constructed"
            })?;
        let actual_index_bytes = expressions
            .scalar_calls
            .capacity()
            .checked_mul(std::mem::size_of::<Option<(usize, &SemanticDirectCallV1)>>())
            .ok_or("helper scalar-call actual index storage overflow")?;
        if actual_index_bytes > index_bytes {
            expressions
                .helper_meter
                .as_deref_mut()
                .ok_or("helper meter unavailable")?
                .reserve(actual_index_bytes - index_bytes)?;
        }
        let writes = projected_reference_gpu_writes_inner_v2(
            function,
            semantic.callables(),
            blocks,
            sources,
            &mut expressions,
        )
        .map_err(|error| {
            projection_error = Some(error);
            "helper value ranked writes could not be reconstructed"
        })?;
        let bytes = expressions.helper_reserved;
        drop(expressions);
        meter.release(actual_index_bytes.max(index_bytes))?;
        if meter.exhausted() {
            return Err("helper value caller resource ledger exhausted");
        }
        Ok((writes, bytes))
    });
    if let Some(error) = meter.failure {
        return Err(error);
    }
    if let Some(error) = projection_error {
        return Err(error);
    }
    result.map_err(ProductionRankedProjectionErrorV1::Incomplete)
}

impl<'a> GpuSemanticExpressionResolverV2<'a> {
    pub(super) fn resolve_defined_call_v1(
        &mut self,
        block: usize,
        call: &'a SemanticDirectCallV1,
        depth: usize,
    ) -> Result<ProductionSemanticExpressionV2, &'static str> {
        Self::require_depth_v2(depth)?;
        let semantic = self
            .helper_semantic
            .ok_or("unresolved helper value recipe")?;
        let values = self.helper_values.ok_or("unresolved helper value recipe")?;
        let template = values.call(
            semantic,
            self.function,
            block,
            call,
            self.helper_meter
                .as_deref_mut()
                .ok_or("helper value caller meter unavailable")?,
        )?;
        let arguments = call.arguments();
        // Existing root resolution can clone authenticated load index lists.
        // Reserve a conservative bounded argument-tree workspace before calling
        // it. Template output is charged separately using its exact expansion.
        self.helper_meter
            .as_deref_mut()
            .ok_or("helper meter unavailable")?
            .work(
                self.loads
                    .len()
                    .checked_add(self.place_loads.len())
                    .ok_or("helper argument load census overflow")?,
            )?;
        let index_bytes = self
            .loads
            .values()
            .chain(self.place_loads.values())
            .map(|load| {
                load.indices
                    .len()
                    .checked_mul(std::mem::size_of::<ProductionRankedValueV1>())
            })
            .try_fold(0usize, |largest, bytes| {
                bytes.map(|bytes| largest.max(bytes))
            })
            .ok_or("helper argument load index storage overflow")?;
        let nodes = arguments
            .len()
            .checked_mul(fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2)
            .ok_or("helper argument node count overflow")?;
        let tree_bytes = nodes
            .checked_mul(
                std::mem::size_of::<ProductionSemanticExpressionV2>()
                    .checked_add(index_bytes)
                    .ok_or("helper argument node size overflow")?,
            )
            .ok_or("helper argument tree storage overflow")?;
        let meter = self
            .helper_meter
            .as_deref_mut()
            .ok_or("helper value caller meter unavailable")?;
        meter.work(nodes)?;
        meter.reserve(tree_bytes)?;
        let (mut resolved, vector_bytes) =
            match helper_value_template_v1::vector(arguments.len(), meter) {
                Ok(value) => value,
                Err(error) => {
                    meter.release(tree_bytes)?;
                    return Err(error);
                }
            };
        let result = (|| {
            for (ordinal, argument) in arguments.iter().enumerate() {
                // Preserve ordered move invalidation even though the root
                // recipe resolver itself does not execute mutable local state.
                if let SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) = argument {
                    for previous in &arguments[..ordinal] {
                        self.helper_meter
                            .as_deref_mut()
                            .ok_or("helper meter unavailable")?
                            .work(1)?;
                        if matches!(previous, SemanticOperandV1::Move(old) if old.local() == place.local())
                        {
                            return Err("helper argument reads a moved source local");
                        }
                    }
                }
                resolved.push(self.resolve_operand_v2(argument, depth + 1)?);
            }
            let meter = self
                .helper_meter
                .as_deref_mut()
                .ok_or("helper meter unavailable")?;
            let (expression, bytes) = template.instantiate(&resolved, meter)?;
            match self.helper_reserved.checked_add(bytes) {
                Some(total) => {
                    self.helper_reserved = total;
                    Ok(expression)
                }
                None => {
                    drop(expression);
                    meter.release(bytes)?;
                    Err("helper output reservation overflow")
                }
            }
        })();
        drop(resolved);
        let meter = self
            .helper_meter
            .as_deref_mut()
            .ok_or("helper meter unavailable")?;
        meter.release(vector_bytes)?;
        meter.release(tree_bytes)?;
        result
    }
}

//! Failure-only observations of existing enum facts; never a proof producer.
use super::*;

const MAX_ROWS: usize = 24;
const MAX_VISITS: usize = 8192;

struct Report {
    rows: Vec<String>,
    remaining: usize,
    complete: bool,
}
impl Report {
    fn new() -> Self {
        Self {
            rows: Vec::with_capacity(MAX_ROWS + 1),
            remaining: MAX_VISITS,
            complete: true,
        }
    }
    fn visit(&mut self) -> bool {
        if self.remaining == 0 || self.rows.len() == MAX_ROWS {
            self.complete = false;
            false
        } else {
            self.remaining -= 1;
            true
        }
    }
    fn finish(mut self) -> Vec<String> {
        self.rows.push(format!(
            "enum-alias observation complete={} visits={} rows={}",
            self.complete,
            MAX_VISITS - self.remaining,
            self.rows.len()
        ));
        self.rows
    }
}

impl SemanticFunctionLoweringV1<'_> {
    pub(super) fn enum_alias_failure_evidence_v1(
        &self,
        block: SemanticBlockIdV1,
        local: SemanticLocalIdV1,
        variant: u32,
        field: u32,
    ) -> Vec<String> {
        let mut report = Report::new();
        report.rows.push(format!("enum-alias plan initialized={} selected_owner={:?} local={} block={} variant={} field={}",
            self.enum_payload_aliases.is_some(), self.enum_payload_aliases.as_ref().and_then(|p| p.get(&local.index())).map(|proof| proof.source()),
            local.index(), block.index(), variant, field));
        // At most the failing carrier and its original whole-local source.
        let mut source = None;
        let mut alias_definitions = 0;
        let mut carrier_writes = 0;
        for (b, body) in self.function.blocks().iter().enumerate() {
            if !report.visit() {
                break;
            }
            for (s, statement) in body.statements().iter().enumerate() {
                if !report.visit() {
                    break;
                }
                if let SemanticStatementKindV1::Assign(a) = statement.kind()
                    && a.destination().local() == local
                {
                    carrier_writes += 1;
                    if a.destination().projections().is_empty()
                        && let SemanticRvalueKindV1::Use(
                            SemanticOperandV1::Copy(p) | SemanticOperandV1::Move(p),
                        ) = a.value().kind()
                        && p.projections().is_empty()
                    {
                        alias_definitions += 1;
                        source = Some(p.local());
                        report.rows.push(format!("enum-alias candidate local={} source={} block={} statement={} destination_type={} operand_type={} result_type={}",
                            local.index(), p.local().index(), b, s, a.destination().ty().index(), p.ty().index(), a.value().result_type().index()));
                    }
                }
            }
            if !report.complete {
                break;
            }
        }
        if report.visit() {
            report.rows.push(format!(
                "enum-alias carrier assignments={} whole_aliases={}",
                carrier_writes, alias_definitions
            ));
        }
        if alias_definitions != 1 {
            source = None;
        }
        for observed in [Some(local), source].into_iter().flatten() {
            if !report.visit() {
                break;
            }
            let id = observed.index();
            let promoted = self.control_flow_ssa.promoted.get(&id);
            let declaration = self.function.locals().get(id as usize);
            let payload = match self.locals.get(id as usize).and_then(Option::as_ref) {
                Some(SemanticValueBindingV1::Enum {
                    variant: known,
                    payloads,
                    ..
                }) => Some((
                    *known,
                    payloads.len(),
                    payloads.get(&variant).map_or(0, Vec::len),
                )),
                _ => None,
            };
            report.rows.push(format!("enum-alias state local={} ssa={} promoted={} structural={} semantic_type={:?} transport_type={:?} compiler_bound={} variant_available={} slot_components={:?} binding={:?}",
                id, self.control_flow_ssa.ssa_value_locals.contains(&id), promoted.is_some(),
                promoted.is_some_and(|p| p.transport.uses_structural_enum_transport()),
                promoted.map(|p| p.semantic_type.index()), promoted.map(|p| p.transport_semantic_type.index()),
                declaration.is_some_and(|d| self.control_flow_ssa.compiler_issued_bindings.contains_key(&d.ty())),
                self.enum_variant_is_available_v1(observed, variant, block),
                self.enum_payload_storage.get(&(id, variant, field)).map(|s| s.components.len()), payload));
            if let Some(SemanticTypeShapeV1::Enum { variants, .. }) = declaration
                .and_then(|d| self.types.get(d.ty().index() as usize))
                .map(|t| t.shape())
            {
                for (v, definition) in variants.iter().enumerate() {
                    if !report.visit() {
                        break;
                    }
                    for (f, ty) in definition.fields().fields().iter().enumerate() {
                        if !report.visit() {
                            break;
                        }
                        let scalar = matches!(
                            self.types.get(ty.index() as usize).map(|t| t.shape()),
                            Some(SemanticTypeShapeV1::Scalar(
                                SemanticScalarTypeV1::Bool
                                    | SemanticScalarTypeV1::Integer {
                                        bits: 8 | 16 | 32 | 64,
                                        ..
                                    }
                            ))
                        );
                        report.rows.push(format!("enum-alias field local={} variant={} field={} type={} scalar={} compiler_bound={}",
                            id, v, f, ty.index(), scalar, self.control_flow_ssa.compiler_issued_bindings.contains_key(ty)));
                    }
                    if !report.complete {
                        break;
                    }
                }
            }
        }
        // Report the complete tracked write/escape roster, or explicitly mark
        // it partial. No Debug formatting of an unbounded source expression.
        for (b, body) in self.function.blocks().iter().enumerate() {
            if !report.visit() {
                break;
            }
            for (s, statement) in body.statements().iter().enumerate() {
                if !report.visit() {
                    break;
                }
                let event = match statement.kind() {
                    SemanticStatementKindV1::Assign(a) => {
                        if a.destination().local() == local
                            || Some(a.destination().local()) == source
                        {
                            Some((
                                a.destination().local(),
                                if !a.destination().projections().is_empty() {
                                    "projected-write"
                                } else if matches!(a.value().kind(), SemanticRvalueKindV1::Aggregate(a) if matches!(a.kind(), SemanticAggregateKindV1::EnumVariant(_)))
                                {
                                    "constructor"
                                } else {
                                    "assignment"
                                },
                            ))
                        } else {
                            match a.value().kind() {
                                SemanticRvalueKindV1::Borrow { place, .. } => {
                                    Some((place.local(), "borrow"))
                                }
                                SemanticRvalueKindV1::AddressOf { place, .. } => {
                                    Some((place.local(), "address"))
                                }
                                SemanticRvalueKindV1::Load(load) => {
                                    Some((load.source().local(), "load"))
                                }
                                _ => None,
                            }
                        }
                    }
                    SemanticStatementKindV1::Store(store) => {
                        Some((store.destination().local(), "store"))
                    }
                    SemanticStatementKindV1::SetDiscriminant { place, .. } => {
                        Some((place.local(), "set-discriminant"))
                    }
                    SemanticStatementKindV1::Deinitialize(place) => {
                        Some((place.local(), "deinitialize"))
                    }
                    SemanticStatementKindV1::StorageDead(l) => Some((*l, "storage-dead")),
                    SemanticStatementKindV1::AtomicRmw(_)
                    | SemanticStatementKindV1::AtomicCompareExchange(_) => {
                        // Atomic source roles are not silently summarized as clean.
                        report.rows.push(format!(
                            "enum-alias atomic site block={} statement={} requires-role-inspection",
                            b, s
                        ));
                        None
                    }
                    _ => None,
                };
                if let Some((l, kind)) = event
                    && (l == local || Some(l) == source)
                {
                    report.rows.push(format!(
                        "enum-alias event local={} block={} statement={} kind={}",
                        l.index(),
                        b,
                        s,
                        kind
                    ));
                }
            }
            if !report.complete || !report.visit() {
                break;
            }
            let operands = match body.terminator().kind() {
                SemanticTerminatorKindV1::Call(c) => {
                    if let Some(d) = c.destination()
                        && (d.place().local() == local || Some(d.place().local()) == source)
                    {
                        report.rows.push(format!(
                            "enum-alias event local={} block={} kind=call-destination",
                            d.place().local().index(),
                            b
                        ));
                    }
                    Some(c.arguments())
                }
                SemanticTerminatorKindV1::TailCall(c) => Some(c.arguments()),
                SemanticTerminatorKindV1::Drop { place, .. } => {
                    if place.local() == local || Some(place.local()) == source {
                        report.rows.push(format!(
                            "enum-alias event local={} block={} kind=drop",
                            place.local().index(),
                            b
                        ));
                    }
                    None
                }
                _ => None,
            };
            if let Some(operands) = operands {
                for (argument, operand) in operands.iter().enumerate() {
                    if !report.visit() {
                        break;
                    }
                    if let SemanticOperandV1::Copy(p) | SemanticOperandV1::Move(p) = operand
                        && (p.local() == local || Some(p.local()) == source)
                    {
                        report.rows.push(format!(
                            "enum-alias event local={} block={} argument={} kind=call-escape",
                            p.local().index(),
                            b,
                            argument
                        ));
                    }
                }
            }
            if !report.complete {
                break;
            }
        }
        report.finish()
    }
}

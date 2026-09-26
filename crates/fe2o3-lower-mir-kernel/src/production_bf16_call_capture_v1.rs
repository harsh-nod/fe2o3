// Sparse rows copied from the real emitter's live source-SSA maps. All arrays
// are inline and included in the prepayment; neither a map nor owner escapes.
const BF16_EMISSION_ROLES_V1: [Bf16CallInstanceRoleV1; 7] = [
    Bf16CallInstanceRoleV1::Context,
    Bf16CallInstanceRoleV1::Lane,
    Bf16CallInstanceRoleV1::Lhs,
    Bf16CallInstanceRoleV1::Rhs,
    Bf16CallInstanceRoleV1::Zero,
    Bf16CallInstanceRoleV1::Result,
    Bf16CallInstanceRoleV1::Values,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Bf16CallEmissionCaptureV1 {
    seen: [bool; 2],
    producers: [[ValueId; 4]; 7],
    arguments: [[ValueId; 4]; 4],
    formals: [[ValueId; 4]; 4],
    call_result: [ValueId; 4],
}
impl Bf16CallEmissionCaptureV1 {
    const fn new() -> Self {
        Self {
            seen: [false; 2],
            producers: [[ValueId(0); 4]; 7],
            arguments: [[ValueId(0); 4]; 4],
            formals: [[ValueId(0); 4]; 4],
            call_result: [ValueId(0); 4],
        }
    }
    fn record(
        &mut self,
        source: &CheckedBf16CallInstanceV1<'_>,
        plan: &LoweredFunctionPlanV1,
        lowering: &SemanticFunctionLoweringV1<'_>,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(32)?;
        let slot = if plan.semantic_function == source.root() {
            0
        } else if plan.semantic_function == source.helper() {
            1
        } else {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        };
        if self.seen[slot]
            || plan.correspondence_owner != source.root()
            || lowering.semantic_function != plan.semantic_function
            || lowering.execution.is_some()
            || lowering.emission_placement != SemanticEmissionPlacementV1::default()
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let mut component = |value, role| {
            budget.charge_work(2 + usize::BITS as usize)?;
            let binding = lowering
                .semantic_ssa_bindings
                .get(&value)
                .ok_or_else(|| bf16_emission_refusal_v1("BF16 exact live SSA binding absent"))?;
            bf16_live_components_v1(binding, role)
        };
        for (index, role) in BF16_EMISSION_ROLES_V1.into_iter().enumerate() {
            let row = source.producer(role);
            if row.function() == plan.semantic_function {
                self.producers[index] = component(row.value(), role)?;
            }
        }
        for (index, role) in [
            Bf16CallInstanceRoleV1::Context,
            Bf16CallInstanceRoleV1::Lhs,
            Bf16CallInstanceRoleV1::Rhs,
            Bf16CallInstanceRoleV1::Zero,
        ]
        .into_iter()
        .enumerate()
        {
            let value = if slot == 0 {
                source.call_argument_ssa(index)
            } else {
                source.formal_ssa(index)
            }
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            let values = component(value, role)?;
            if slot == 0 {
                self.arguments[index] = values;
            } else {
                self.formals[index] = values;
            }
        }
        // Last closure use above ends its ledger borrow before this scan.
        if slot == 0 {
            let value = bf16_call_result_definition_v1(source, budget)?;
            budget.charge_work(usize::BITS as usize + 2)?;
            let binding = lowering
                .semantic_ssa_bindings
                .get(&value)
                .ok_or_else(|| bf16_emission_refusal_v1("BF16 Call result live SSA binding"))?;
            self.call_result = bf16_live_components_v1(binding, Bf16CallInstanceRoleV1::Values)?;
        }
        self.seen[slot] = true;
        Ok(())
    }
}

fn bf16_live_components_v1(
    binding: &SemanticValueBindingV1,
    role: Bf16CallInstanceRoleV1,
) -> Result<[ValueId; 4], ProductionSemanticKirErrorV1> {
    use Bf16CallInstanceRoleV1 as R;
    let mut result = [ValueId(0); 4];
    let values = match (role, binding) {
        (R::Context, SemanticValueBindingV1::MatrixContext) => return Ok(result),
        (R::Lane, SemanticValueBindingV1::WaveLane { value, wave }) if wave.width == 64 => {
            result[0] = *value; return Ok(result);
        }
        (R::Lhs | R::Rhs, SemanticValueBindingV1::MatrixFragment {
            values, contract, storage_layout, wave,
        }) if *storage_layout == SemanticMfmaStorageLayoutV1::RowMajor
            && contract.profile == SemanticMfmaProfileV1::Bf16F32M16N16K16
            && contract.wave_width == 64 && wave.width == 64
            && contract.register_distribution == SemanticMfmaRegisterDistributionV1::Tile16x16
            && contract.role == if role == R::Lhs { SemanticMfmaOperandRoleV1::A } else { SemanticMfmaOperandRoleV1::B } => values,
        (R::Zero | R::Result, SemanticValueBindingV1::AccumulatorFragment {
            values, contract, wave,
        }) if contract.profile == SemanticMfmaProfileV1::Bf16F32M16N16K16
            && contract.wave_width == 64 && wave.width == 64
            && contract.distribution == fe2o3_mir_model::semantic_mir_v1::SemanticMfmaAccumulatorDistributionV1::RowMajor => values,
        (R::Values, SemanticValueBindingV1::Aggregate(parts)) if parts.len() == 4 => {
            for (index, part) in parts.iter().enumerate() {
                let SemanticValueBindingV1::Value { id, ty } = part else {
                    return Err(bf16_emission_refusal_v1("BF16 values array component kind"));
                };
                if *ty != Type::Scalar(ScalarType::F32) {
                    return Err(bf16_emission_refusal_v1("BF16 values array component type"));
                }
                result[index] = *id;
            }
            return Ok(result);
        }
        _ => return Err(bf16_emission_refusal_v1("BF16 live role/layout/wave differs")),
    };
    if values.len() != 4 {
        return Err(bf16_emission_refusal_v1("BF16 live component width"));
    }
    let ty = if matches!(role, R::Lhs | R::Rhs) {
        ScalarType::Bf16
    } else {
        ScalarType::F32
    };
    for (index, (id, actual)) in values.iter().enumerate() {
        if *actual != Type::Scalar(ty) || result[..index].contains(id) {
            return Err(bf16_emission_refusal_v1(
                "BF16 component type or repeated identity",
            ));
        }
        result[index] = *id;
    }
    Ok(result)
}

// The selected call must preserve the typed roles BEFORE converting components
// to the ordinary Call operand vector. Only the private planner can set this.
fn bf16_check_call_bindings_v1(
    bindings: &[SemanticValueBindingV1],
) -> Result<(), ProductionSemanticKirErrorV1> {
    if bindings.len() != 4 {
        return Err(bf16_emission_refusal_v1("BF16 call source argument count"));
    }
    for (binding, role) in bindings.iter().zip([
        Bf16CallInstanceRoleV1::Context,
        Bf16CallInstanceRoleV1::Lhs,
        Bf16CallInstanceRoleV1::Rhs,
        Bf16CallInstanceRoleV1::Zero,
    ]) {
        bf16_live_components_v1(binding, role)?;
    }
    Ok(())
}

fn bf16_call_result_definition_v1(
    source: &CheckedBf16CallInstanceV1<'_>,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<SsaValueV1, ProductionSemanticKirErrorV1> {
    use fe2o3_pliron::{
        ProductionSemanticSsaEventRoleV1 as Role, ProductionSemanticSsaOccurrenceSiteV1 as Site,
    };
    let local = source
        .source_call()
        .destination()
        .filter(|d| d.place().projections().is_empty())
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
        .place()
        .local()
        .index();
    let rows = source
        .owner()
        .occurrences_v1()
        .and_then(|r| r.function(source.root()))
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    let mut found = None;
    for edge in rows.edge_definitions() {
        budget.charge_work(1)?;
        if edge.edge().source().get() == source.call_block().index()
            && edge.variable().get() == local
        {
            if edge.edge().ordinal() != 0
                || !edge.is_reachable()
                || !edge.is_promoted()
                || found
                    .replace(
                        edge.value()
                            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?,
                    )
                    .is_some()
            {
                return Err(bf16_emission_refusal_v1(
                    "BF16 actual Call result edge identity",
                ));
            }
        }
    }
    let site = Site::Terminator {
        block: SsaBlockIdV1::new(source.call_block().index()),
    };
    for event in rows.events() {
        budget.charge_work(1)?;
        if event.site() == site
            && event.role() == Role::DestinationDefine
            && let Some(SsaResolvedEventV1::Define { variable, value }) = event.resolved()
            && variable.get() == local
        {
            if !event.is_reachable() || !event.is_promoted() || found.replace(value).is_some() {
                return Err(bf16_emission_refusal_v1(
                    "BF16 actual Call result definition identity",
                ));
            }
        }
    }
    found.ok_or_else(|| bf16_emission_refusal_v1("BF16 actual Call result absent"))
}

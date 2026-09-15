//! Original selected-constructor custody for flat, non-capability data only.
use super::mixed_lineage::queried;
use super::*;
use fe2o3_pliron::{
    ProductionSemanticSsaSourceSiteV1 as Site, ProductionSemanticSsaValueOriginV1 as Origin,
};

pub(in super::super) struct AggregateCustodyPlan<'a> {
    lineage: MixedAliasPlan<'a>,
    originals: BTreeMap<(u32, u32), Box<SemanticValueBindingV1>>,
}

// This is an eligibility check, not a constructor. Empty fields must still be
// present in the original source binding retained by `retain` below.
pub(super) fn payload_type(
    types: &[SemanticTypeDeclV1],
    transport: &SemanticControlFlowSsaPlanV1,
    ty: SemanticTypeIdV1,
    budget: &mut SemanticEnumAnalysisBudgetV1,
) -> Result<Option<(u32, SemanticTypeIdV1)>, ProductionSemanticKirErrorV1> {
    budget.charge_work(1 + lookup_work(transport.compiler_issued_bindings.len()))?;
    if transport.compiler_issued_bindings.contains_key(&ty) {
        return Ok(None);
    }
    let Some(SemanticTypeShapeV1::Enum { variants, .. }) =
        types.get(ty.index() as usize).map(|t| t.shape())
    else {
        return Ok(None);
    };
    let mut selected = None;
    for (v, variant) in variants.iter().enumerate() {
        budget.charge_work(1)?;
        let fields = variant.fields().fields();
        if fields.is_empty() {
            continue;
        }
        let [field] = fields else {
            return Ok(None);
        };
        if selected.is_some() || !plain_type(types, transport, *field, budget)? {
            return Ok(None);
        }
        selected = Some((v as u32, *field));
    }
    Ok(selected)
}

fn plain_type(
    types: &[SemanticTypeDeclV1],
    transport: &SemanticControlFlowSsaPlanV1,
    ty: SemanticTypeIdV1,
    budget: &mut SemanticEnumAnalysisBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.charge_work(1 + lookup_work(transport.compiler_issued_bindings.len()))?;
    if transport.compiler_issued_bindings.contains_key(&ty) {
        return Ok(false);
    }
    let Some(decl) = types.get(ty.index() as usize) else {
        return Ok(false);
    };
    let SemanticTypeShapeV1::Aggregate(fields) = decl.shape() else {
        return Ok(false);
    };
    if decl.layout().is_uninhabited() || fields.fields().len() > MAX_SSA_VALUE_COMPONENTS_V1 {
        return Ok(false);
    }
    let mut has_scalar = false;
    for field in fields.fields() {
        budget.charge_work(1 + lookup_work(transport.compiler_issued_bindings.len()))?;
        if transport.compiler_issued_bindings.contains_key(field) {
            return Ok(false);
        }
        let Some(decl) = types.get(field.index() as usize) else {
            return Ok(false);
        };
        if scalar_payload(types, *field) {
            has_scalar = true;
            continue;
        }
        if decl.layout().is_uninhabited() || decl.layout().size_bytes() != Some(0) {
            return Ok(false);
        }
        match decl.shape() {
            SemanticTypeShapeV1::Unit => (),
            SemanticTypeShapeV1::Aggregate(fields) if fields.fields().is_empty() => (),
            _ => return Ok(false),
        }
    }
    Ok(has_scalar)
}

fn same_original(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    a: &SemanticValueBindingV1,
    b: &SemanticValueBindingV1,
    budget: &mut SemanticEnumAnalysisBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.charge_work(1)?;
    let (
        Some(SemanticTypeShapeV1::Aggregate(shape)),
        SemanticValueBindingV1::Aggregate(a),
        SemanticValueBindingV1::Aggregate(b),
    ) = (types.get(ty.index() as usize).map(|t| t.shape()), a, b)
    else {
        return Ok(false);
    };
    if shape.fields().len() != a.len()
        || a.len() != b.len()
        || a.len() > MAX_SSA_VALUE_COMPONENTS_V1
    {
        return Ok(false);
    }
    for ((ty, a), b) in shape.fields().iter().zip(a).zip(b) {
        budget.charge_work(1)?;
        let equal = match (types[ty.index() as usize].shape(), a, b) {
            (
                SemanticTypeShapeV1::Scalar(_),
                SemanticValueBindingV1::Value { id: a, ty: x },
                SemanticValueBindingV1::Value { id: b, ty: y },
            ) => {
                scalar_payload(types, *ty)
                    && a == b
                    && x == y
                    && *x == lower_scalar_type(types, *ty)?
            }
            (
                SemanticTypeShapeV1::Unit,
                SemanticValueBindingV1::Unit,
                SemanticValueBindingV1::Unit,
            ) => true,
            (
                SemanticTypeShapeV1::Aggregate(shape),
                SemanticValueBindingV1::Aggregate(a),
                SemanticValueBindingV1::Aggregate(b),
            ) => shape.fields().is_empty() && a.is_empty() && b.is_empty(),
            _ => false,
        };
        if !equal {
            return Ok(false);
        }
    }
    Ok(true)
}

impl<'a> SemanticFunctionLoweringV1<'a> {
    fn aggregate_query(&self) -> Option<fe2o3_pliron::ProductionSemanticSsaSourceQueryV1<'a>> {
        self.enum_alias_source
            .or_else(|| self.enum_mixed_aliases.as_ref().map(MixedAliasPlan::query))
    }

    fn ensure_aggregate_plan(&mut self) -> Result<bool, ProductionSemanticKirErrorV1> {
        if self.enum_aggregate_custody.is_none() {
            let Some(query) = self.aggregate_query() else {
                return Ok(false);
            };
            if !std::ptr::eq(query.function(), self.function) {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            self.enum_analysis_budget.charge_storage(8)?;
            self.enum_aggregate_custody = Some(AggregateCustodyPlan {
                lineage: MixedAliasPlan::new_aggregate(
                    query,
                    self.types,
                    &self.control_flow_ssa,
                    &mut self.enum_analysis_budget,
                )?,
                originals: BTreeMap::new(),
            });
        }
        Ok(true)
    }

    pub(in super::super::super) fn retain_plain_enum_constructor(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        local: SemanticLocalIdV1,
        variant: u32,
        field: usize,
        binding: &SemanticValueBindingV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        // Scalar/capability payloads never initialize this optional analysis.
        if field != 0 || !matches!(binding, SemanticValueBindingV1::Aggregate(_)) {
            return Ok(());
        }
        self.enum_analysis_budget
            .charge_work(lookup_work(self.enum_payload_storage.len()))?;
        if !self
            .enum_payload_storage
            .contains_key(&(local.index(), variant, 0))
        {
            return Ok(());
        }
        let Some(s) = statement else {
            return Ok(());
        };
        let Some(ty) = self
            .function
            .locals()
            .get(local.index() as usize)
            .map(|l| l.ty())
        else {
            return Ok(());
        };
        let Some((selected, payload)) = payload_type(
            self.types,
            &self.control_flow_ssa,
            ty,
            &mut self.enum_analysis_budget,
        )?
        else {
            return Ok(());
        };
        if selected != variant || !self.ensure_aggregate_plan()? {
            return Ok(());
        }
        let plan = self.enum_aggregate_custody.as_mut().unwrap();
        let site = Site::new(block, statement);
        if plan
            .lineage
            .constructor(local.index(), variant, &mut self.enum_analysis_budget)?
            != Some(site)
        {
            return Ok(());
        }
        let query = plan.lineage.query();
        let Some(SemanticStatementKindV1::Assign(a)) = query
            .function()
            .blocks()
            .get(block.index() as usize)
            .and_then(|b| b.statements().get(s as usize))
            .map(|s| s.kind())
        else {
            return Ok(());
        };
        if a.destination().local() != local
            || !a.destination().projections().is_empty()
            || a.destination().ty() != ty
            || a.value().result_type() != ty
        {
            return Ok(());
        }
        let SemanticRvalueKindV1::Aggregate(a) = a.value().kind() else {
            return Ok(());
        };
        if a.kind() != &SemanticAggregateKindV1::EnumVariant(variant) {
            return Ok(());
        }
        let [operand] = a.operands() else {
            return Ok(());
        };
        let Some(place) = whole_operand(operand).filter(|p| {
            p.ty() == payload
                && self
                    .function
                    .locals()
                    .get(p.local().index() as usize)
                    .is_some_and(|l| {
                        l.ty() == payload && l.role() == SemanticLocalRoleV1::Temporary
                    })
        }) else {
            return Ok(());
        };
        // Query the pointer-identical original constructor operand, not a
        // reconstructed Copy or a local-number lookup at the later alias use.
        let Some(source) = queried(&mut self.enum_analysis_budget, |c| {
            query.operand_use(site, operand, &mut || c())
        })?
        else {
            return Ok(());
        };
        if source.variable().get() != place.local().index() || !source.belongs_to(&query) {
            return Ok(());
        }
        // A phi or a reconstructed aggregate alias is not marker custody. Keep
        // the original nominal constructor, including its explicit ZST operands.
        let Some(Origin::Event {
            site: source_site, ..
        }) = queried(&mut self.enum_analysis_budget, |c| {
            query.value_origin(&source.retained_value(), &mut || c())
        })?
        else {
            return Ok(());
        };
        let Some(source_statement) = source_site.statement() else {
            return Ok(());
        };
        self.enum_analysis_budget.charge_work(1)?;
        let Some(SemanticStatementKindV1::Assign(definition)) = query
            .function()
            .blocks()
            .get(source_site.block().index() as usize)
            .and_then(|body| body.statements().get(source_statement as usize))
            .map(|item| item.kind())
        else {
            return Ok(());
        };
        if definition.destination().local() != place.local()
            || !definition.destination().projections().is_empty()
            || definition.destination().ty() != payload
            || definition.value().result_type() != payload
        {
            return Ok(());
        }
        let SemanticRvalueKindV1::Aggregate(definition) = definition.value().kind() else {
            return Ok(());
        };
        let SemanticTypeShapeV1::Aggregate(shape) = self.types[payload.index() as usize].shape()
        else {
            return Ok(());
        };
        if definition.kind() != &SemanticAggregateKindV1::Aggregate
            || definition.operands().len() != shape.fields().len()
        {
            return Ok(());
        }
        for (field, operand) in shape.fields().iter().zip(definition.operands()) {
            self.enum_analysis_budget.charge_work(1)?;
            if operand.ty() != *field
                || (!scalar_payload(self.types, *field)
                    && !matches!(operand, SemanticOperandV1::Constant(c)
                        if c.value() == &SemanticConstantValueV1::ZeroSized))
            {
                return Ok(());
            }
        }
        self.enum_analysis_budget
            .charge_work(lookup_work(self.semantic_ssa_bindings.len()))?;
        let Some(original) = self.semantic_ssa_bindings.get(&source.value()) else {
            return Ok(());
        };
        if !same_original(
            self.types,
            payload,
            binding,
            original,
            &mut self.enum_analysis_budget,
        )? {
            return Ok(());
        }
        let SemanticValueBindingV1::Aggregate(fields) = binding else {
            return Ok(());
        };
        self.enum_analysis_budget
            .charge_work(2 * lookup_work(plan.originals.len().saturating_add(1)))?;
        // Covers the BTree entry, binding/vector allocation and bounded clone.
        self.enum_analysis_budget.charge_work(fields.len())?;
        self.enum_analysis_budget.charge_storage(
            32 + std::mem::size_of::<SemanticValueBindingV1>()
                .div_ceil(std::mem::size_of::<usize>())
                + fields.len().saturating_mul(
                    std::mem::size_of::<SemanticValueBindingV1>()
                        .div_ceil(std::mem::size_of::<usize>())
                        + 2,
                ),
        )?;
        if plan
            .originals
            .insert((local.index(), variant), Box::new(binding.clone()))
            .is_some()
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        Ok(())
    }

    pub(in super::super::super) fn metered_enum_variant_is_available_v1(
        &mut self,
        local: SemanticLocalIdV1,
        variant: u32,
        block: SemanticBlockIdV1,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        // Precharge local upper bounds before the unchanged guard, including
        // its short-circuited fallback. Metadata reads are charged separately.
        self.enum_analysis_budget.charge_work(1)?;
        if block == self.function.entry() {
            self.enum_analysis_budget
                .charge_work(lookup_work(self.control_flow_ssa.entry_definitions.len()))?;
        } else {
            self.enum_analysis_budget
                .charge_work(lookup_work(self.control_flow_ssa.live_in.len()))?;
            let live_count = self.control_flow_ssa.live_in(block.index()).len();
            self.enum_analysis_budget.charge_work(
                lookup_work(self.control_flow_ssa.live_in.len())
                    .saturating_add(live_count)
                    .saturating_add(lookup_work(self.control_flow_ssa.block_entry_values.len())),
            )?;
        }
        self.enum_analysis_budget
            .charge_work(lookup_work(self.promoted_enum_variant_by_value.len()) + 1)?;
        let dominance_work = self
            .enum_payload_dominance
            .availability_lookup_work_units(local);
        self.enum_analysis_budget
            .charge_work(dominance_work.saturating_add(1))?;
        Ok(self.enum_variant_is_available_v1(local, variant, block))
    }

    pub(in super::super::super) fn refine_plain_enum_alias(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        local: u32,
        operations: &mut Vec<Operation>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        // A proof cannot create missing constructor custody while visiting a
        // later consumer. If no original producer ran, remain unsupported.
        if self.enum_aggregate_custody.is_none() {
            return Ok(());
        }
        let Some(SemanticValueBindingV1::Enum {
            semantic_type,
            payloads,
            ..
        }) = self.locals.get(local as usize).and_then(Option::as_ref)
        else {
            return Ok(());
        };
        let ty = *semantic_type;
        if !payloads.is_empty()
            || self
                .function
                .locals()
                .get(local as usize)
                .is_none_or(|l| l.ty() != ty)
        {
            return Ok(());
        }
        let Some((variant, payload)) = payload_type(
            self.types,
            &self.control_flow_ssa,
            ty,
            &mut self.enum_analysis_budget,
        )?
        else {
            return Ok(());
        };
        if !self.metered_enum_variant_is_available_v1(
            SemanticLocalIdV1::from_index(local),
            variant,
            block,
        )? {
            return Ok(());
        }
        let plan = self.enum_aggregate_custody.as_mut().unwrap();
        let Some(source) = plan.lineage.storage_owner(
            self.types,
            &self.control_flow_ssa,
            block,
            statement,
            local,
            variant,
            &mut self.enum_analysis_budget,
        )?
        else {
            return Ok(());
        };
        self.enum_analysis_budget.charge_work(
            lookup_work(plan.originals.len()) + lookup_work(self.enum_payload_storage.len()),
        )?;
        let Some(original) = plan.originals.get(&(source, variant)) else {
            return Ok(());
        };
        let Some(storage) = self.enum_payload_storage.get(&(source, variant, 0)) else {
            return Ok(());
        };
        if storage.semantic_type != payload
            || storage.exact_enum_variant.is_some()
            || storage.compiler_issued_binding.is_some()
        {
            return Ok(());
        }
        if !same_original(
            self.types,
            payload,
            original,
            original,
            &mut self.enum_analysis_budget,
        )? {
            return Ok(());
        }
        let SemanticValueBindingV1::Aggregate(fields) = original.as_ref() else {
            return Ok(());
        };
        self.enum_analysis_budget.charge_work(fields.len())?;
        let scalar_count = fields
            .iter()
            .filter(|f| matches!(f, SemanticValueBindingV1::Value { .. }))
            .count();
        if scalar_count != storage.components.len() {
            return Ok(());
        }
        // Validate every component before allocating output or emitting loads.
        for (value, component) in fields
            .iter()
            .filter_map(|f| f.value().ok())
            .zip(storage.components.iter())
        {
            self.enum_analysis_budget.charge_work(1)?;
            if value.1 != component.kernel_type {
                return Ok(());
            }
        }
        self.enum_analysis_budget.charge_storage(
            64 + std::mem::size_of::<SemanticValueBindingV1>()
                .div_ceil(std::mem::size_of::<usize>())
                + fields.len().saturating_mul(
                    std::mem::size_of::<SemanticValueBindingV1>()
                        .div_ceil(std::mem::size_of::<usize>())
                        + 2,
                )
                + storage.components.len().saturating_mul(
                    std::mem::size_of::<SemanticEnumPayloadComponentStorageV1>()
                        .div_ceil(std::mem::size_of::<usize>())
                        + 2,
                ),
        )?;
        self.enum_analysis_budget
            .charge_work(fields.len() + storage.components.len())?;
        let mut fields = fields.clone();
        let components = storage.components.clone();
        let mut next = components.iter();
        for field in &mut fields {
            if matches!(field, SemanticValueBindingV1::Value { .. }) {
                let component = next
                    .next()
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                *field = self.emit(
                    operations,
                    component.kernel_type.clone(),
                    OperationKind::Load {
                        pointer: component.pointer,
                        access: MemoryAccess::new(AddressSpace::Private, component.alignment),
                    },
                )?;
            }
        }
        // Empty fields above are copies of retained original bindings, never
        // manufactured by binding_from_value_defs or a zero-sized default.
        let Some(SemanticValueBindingV1::Enum {
            variant: selected,
            payloads,
            ..
        }) = self.locals.get_mut(local as usize).and_then(Option::as_mut)
        else {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        };
        *selected = Some(variant);
        payloads.insert(variant, vec![SemanticValueBindingV1::Aggregate(fields)]);
        Ok(())
    }
}

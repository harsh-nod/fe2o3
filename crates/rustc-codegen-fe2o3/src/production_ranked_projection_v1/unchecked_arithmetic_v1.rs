//! Normalization facts borrowed from one exact retained execution function.
//!
//! Source admission is not a transferable flag: execution expansion can change
//! sites, and this resolver also has callers without a semantic owner. Analyze
//! the actual borrowed body once and keep pointer identities live with that borrow.
//! Production supplies the replay-checked SSA owner's execution-view body. These
//! arithmetic facts do not grant source-safety authority or waive shift/FP checks.

use super::*;
use super::lossless_csr_v1::{CsrWorkV1, LosslessCsrV1};
use fe2o3_mir_model::{
    MAX_SEMANTIC_OPTION_DOMINANCE_WORK_V1, semantic_unchecked_arithmetic_violation_with_types_v1,
};

const UNPROVEN: &str = "GPU unchecked arithmetic requires a retained dominating no-overflow proof";
const WORK_LIMIT: &str = "GPU unchecked arithmetic custody exceeds its bounded work budget";
const STORAGE: &str = "GPU unchecked arithmetic custody storage cannot be reserved";
type SiteV1 = (usize, usize);

pub(super) struct UncheckedArithmeticV1<'a> {
    proof: Result<RetainedProofV1<'a>, &'static str>,
    unproved_inputs: usize,
    normalizing: bool,
}

pub(super) struct OperandFrameV1(usize);

impl<'a> UncheckedArithmeticV1<'a> {
    pub(super) fn new(
        types: &'a [SemanticTypeDeclV1],
        function: &'a SemanticFunctionDeclV1,
    ) -> Self {
        Self {
            proof: RetainedProofV1::analyze(types, function),
            unproved_inputs: 0,
            normalizing: false,
        }
    }

    pub(super) fn enter_operand(
        &mut self,
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
        operand: &SemanticOperandV1,
    ) -> Result<OperandFrameV1, &'static str> {
        let proved = self.proof.as_ref().is_ok_and(|proof| {
            proof.is_owner(types, function) && proof.operands.contains(&(operand as *const _))
        });
        if self.normalizing && !proved {
            return Err(UNPROVEN);
        }
        let frame = OperandFrameV1(self.unproved_inputs);
        self.unproved_inputs = self
            .unproved_inputs
            .checked_add(usize::from(!proved))
            .ok_or(WORK_LIMIT)?;
        Ok(frame)
    }

    pub(super) fn leave_operand(&mut self, frame: OperandFrameV1) {
        self.unproved_inputs = frame.0;
    }

    fn begin(
        &mut self,
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
        value: &SemanticRvalueV1,
    ) -> Result<bool, &'static str> {
        if self.unproved_inputs != 0 {
            return Err(UNPROVEN);
        }
        let proof = self.proof.as_ref().map_err(|detail| *detail)?;
        let &(block, statement) = proof.sites.get(&(value as *const _)).ok_or(UNPROVEN)?;
        if !proof.is_owner(types, function) {
            return Err(UNPROVEN);
        }
        let Some(SemanticStatementKindV1::Assign(assignment)) = function
            .blocks()
            .get(block)
            .and_then(|block| block.statements().get(statement))
            .map(|statement| statement.kind())
        else {
            return Err(UNPROVEN);
        };
        if !std::ptr::eq(assignment.value(), value) {
            return Err(UNPROVEN);
        }
        Ok(std::mem::replace(&mut self.normalizing, true))
    }
}

impl<'a> GpuSemanticExpressionResolverV2<'a> {
    pub(super) fn resolve_unchecked_binary_v2(
        &mut self,
        value: &'a SemanticRvalueV1,
        depth: usize,
    ) -> Result<ProductionSemanticExpressionV2, &'static str> {
        let SemanticRvalueKindV1::UncheckedBinary(unchecked) = value.kind() else {
            return Err(UNPROVEN);
        };
        let operation = match unchecked.operation() {
            SemanticUncheckedBinaryOpV1::Add => ProductionSemanticBinaryOpV2::Add,
            SemanticUncheckedBinaryOpV1::Subtract => ProductionSemanticBinaryOpV2::Subtract,
            SemanticUncheckedBinaryOpV1::Multiply => ProductionSemanticBinaryOpV2::Multiply,
        };
        let scalar = self.scalar_v2(value.result_type())?;
        let previous = self.unchecked.begin(self.types, self.function, value)?;
        // Under this site's retained zero-overflow edge, ordinary bit-vector
        // arithmetic has exactly the unchecked operation's defined result.
        // The source node, guard, and source-to-KIR correspondence remain intact.
        let resolved = self.binary_expression_v2(
            operation,
            ProductionOverflowContractV2::Wrapping,
            scalar,
            unchecked.left(),
            unchecked.right(),
            depth,
        );
        self.unchecked.normalizing = previous;
        resolved
    }
}

struct RetainedProofV1<'a> {
    types: &'a [SemanticTypeDeclV1],
    function: &'a SemanticFunctionDeclV1,
    sites: HashMap<*const SemanticRvalueV1, SiteV1>,
    operands: HashSet<*const SemanticOperandV1>,
}

impl<'a> RetainedProofV1<'a> {
    fn is_owner(&self, types: &[SemanticTypeDeclV1], function: &SemanticFunctionDeclV1) -> bool {
        std::ptr::eq(self.types, types) && std::ptr::eq(self.function, function)
    }

    fn analyze(
        types: &'a [SemanticTypeDeclV1],
        function: &'a SemanticFunctionDeclV1,
    ) -> Result<Self, &'static str> {
        let mut budget = BudgetV1::default();
        budget.charge(
            function
                .locals()
                .len()
                .saturating_add(function.blocks().len()),
        )?;
        let mut sites = HashMap::new();
        for (block_index, block) in function.blocks().iter().enumerate() {
            budget.charge(
                block
                    .statements()
                    .len()
                    .saturating_add(block.terminator().kind().edge_count()),
            )?;
            for (statement_index, statement) in block.statements().iter().enumerate() {
                let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                    continue;
                };
                let SemanticRvalueKindV1::UncheckedBinary(unchecked) = assignment.value().kind()
                else {
                    continue;
                };
                let ty = assignment.value().result_type();
                if assignment.destination().ty() != ty
                    || unchecked.left().ty() != ty
                    || unchecked.right().ty() != ty
                    || !matches!(
                        types
                            .get(ty.index() as usize)
                            .map(SemanticTypeDeclV1::shape),
                        Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                            bits: 8 | 16 | 32 | 64,
                            ..
                        }))
                    )
                {
                    return Err(UNPROVEN);
                }
                sites.try_reserve(1).map_err(|_| STORAGE)?;
                sites.insert(
                    assignment.value() as *const _,
                    (block_index, statement_index),
                );
            }
        }
        let mut proof = Self {
            types,
            function,
            sites,
            operands: HashSet::new(),
        };
        if proof.sites.is_empty() {
            return Ok(proof);
        }
        // This is the sole overflow analysis, including for execution views
        // whose original source owner was already admitted.
        if semantic_unchecked_arithmetic_violation_with_types_v1(types, function)
            .map_err(|_| UNPROVEN)?
            .is_some()
        {
            return Err(UNPROVEN);
        }
        let mut bindings = BindingAuditV1::new(function, budget)?;
        for (block_index, block) in function.blocks().iter().enumerate() {
            for (statement_index, statement) in block.statements().iter().enumerate() {
                let site = (block_index, statement_index);
                visit_statement_operands(statement.kind(), |operand| {
                    proof.record_operand(operand, site, &mut bindings)
                })?;
            }
            let site = (block_index, block.statements().len());
            let mut record = |operand| proof.record_operand(operand, site, &mut bindings);
            match block.terminator().kind() {
                SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => record(discriminant)?,
                SemanticTerminatorKindV1::Assert { condition, .. } => record(condition)?,
                SemanticTerminatorKindV1::Call(call) => {
                    for operand in call.arguments() {
                        record(operand)?;
                    }
                }
                SemanticTerminatorKindV1::TailCall(call) => {
                    for operand in call.arguments() {
                        record(operand)?;
                    }
                }
                _ => {}
            }
        }
        Ok(proof)
    }

    fn record_operand(
        &mut self,
        operand: &SemanticOperandV1,
        site: SiteV1,
        bindings: &mut BindingAuditV1<'_>,
    ) -> Result<(), &'static str> {
        bindings.budget.charge(1)?;
        if bindings.matches_resolver(self.types, operand, site)? {
            self.operands.try_reserve(1).map_err(|_| STORAGE)?;
            self.operands.insert(operand as *const _);
        }
        Ok(())
    }
}

// Overflow equality and scalar expression reconstruction are distinct facts.
// The latter resolver uses original argument symbols and globally unique local
// assignments. Check those bindings at each retained operand use before caching
// permission; a proof of equality after an overwrite cannot justify an old symbol.
struct BindingAuditV1<'a> {
    function: &'a SemanticFunctionDeclV1,
    graph: LosslessCsrV1<'a>,
    inventory: AssertionDefinitionInventoryV1,
    cache: HashMap<(usize, SiteV1), bool>,
    budget: BudgetV1,
}

impl<'a> BindingAuditV1<'a> {
    fn new(
        function: &'a SemanticFunctionDeclV1,
        mut budget: BudgetV1,
    ) -> Result<Self, &'static str> {
        let graph = LosslessCsrV1::build(function,
            CsrWorkV1::new(&mut budget.used, MAX_SEMANTIC_OPTION_DOMINANCE_WORK_V1))
            .map_err(|_| UNPROVEN)?;
        let mut inventory = assertion_definition_inventory(function).map_err(|_| UNPROVEN)?;
        for block in function.blocks() {
            budget.charge(block.statements().len().saturating_add(1))?;
            for statement in block.statements() {
                if let SemanticStatementKindV1::Assign(assignment) = statement.kind()
                    && let SemanticRvalueKindV1::Borrow { place, .. }
                    | SemanticRvalueKindV1::AddressOf { place, .. } = assignment.value().kind()
                {
                    *inventory
                        .address_escaped
                        .get_mut(place.local().index() as usize)
                        .ok_or(UNPROVEN)? = true;
                }
            }
        }
        Ok(Self {
            function,
            graph,
            inventory,
            cache: HashMap::new(),
            budget,
        })
    }

    fn matches_resolver(
        &mut self,
        types: &[SemanticTypeDeclV1],
        operand: &SemanticOperandV1,
        use_site: SiteV1,
    ) -> Result<bool, &'static str> {
        let Some(bytes) = scalar_bytes(types, operand.ty()) else {
            return Ok(false);
        };
        let place = match operand {
            SemanticOperandV1::Constant(constant) => {
                return Ok(
                    matches!(constant.value(), SemanticConstantValueV1::Scalar(value) if value.size_bytes() == bytes),
                );
            }
            SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => place,
        };
        if !place.projections().is_empty() {
            return Ok(false);
        }
        let local = place.local().index() as usize;
        let declaration = self.function.locals().get(local).ok_or(UNPROVEN)?;
        if declaration.ty() != place.ty()
            || self.inventory.address_escaped.get(local) != Some(&false)
        {
            return Ok(false);
        }
        if let Some(result) = self.cache.get(&(local, use_site)) {
            return Ok(*result);
        }
        let expected = if matches!(declaration.role(), SemanticLocalRoleV1::Argument(_)) {
            if self.inventory.counts[local] != 0 {
                return Ok(false);
            }
            None
        } else {
            let Some(assignment) = self.inventory.assignments[local] else {
                return Ok(false);
            };
            if self.inventory.counts[local] != 1 {
                return Ok(false);
            }
            Some((assignment.block, assignment.statement))
        };
        let result = self.exact_binding_reaches(local, declaration.ty(), expected, use_site)?;
        self.cache.try_reserve(1).map_err(|_| STORAGE)?;
        self.cache.insert((local, use_site), result);
        Ok(result)
    }

    fn exact_binding_reaches(
        &mut self,
        local: usize,
        ty: SemanticTypeIdV1,
        expected: Option<SiteV1>,
        use_site: SiteV1,
    ) -> Result<bool, &'static str> {
        self.graph.query(
            CsrWorkV1::new(&mut self.budget.used, MAX_SEMANTIC_OPTION_DOMINANCE_WORK_V1),
            |mut query| query.exact_binding_reaches(local, ty, expected, use_site),
        ).map_err(|_| WORK_LIMIT)
    }

}

fn scalar_bytes(types: &[SemanticTypeDeclV1], ty: SemanticTypeIdV1) -> Option<u8> {
    match types.get(ty.index() as usize)?.shape() {
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool) => Some(1),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { bits, .. })
            if matches!(bits, 8 | 16 | 32 | 64) =>
        {
            Some((*bits / 8) as u8)
        }
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 32 }) => Some(4),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 64 }) => Some(8),
        _ => None,
    }
}

fn moves_local(operand: &SemanticOperandV1, local: usize) -> bool {
    matches!(operand, SemanticOperandV1::Move(place) if place.local().index() as usize == local)
}

fn visit_statement_operands(
    kind: &SemanticStatementKindV1,
    mut visit: impl FnMut(&SemanticOperandV1) -> Result<(), &'static str>,
) -> Result<(), &'static str> {
    match kind {
        SemanticStatementKindV1::Assign(assignment) => {
            assignment.value().kind().try_visit_operands(visit)
        }
        SemanticStatementKindV1::Store(store) => visit(store.value()),
        SemanticStatementKindV1::AtomicRmw(atomic) => visit(atomic.value()),
        SemanticStatementKindV1::AtomicCompareExchange(atomic) => {
            visit(atomic.expected())?;
            visit(atomic.replacement())
        }
        SemanticStatementKindV1::Assume(operand) => visit(operand),
        _ => Ok(()),
    }
}

#[derive(Default)]
struct BudgetV1 {
    used: usize,
}

impl BudgetV1 {
    fn charge(&mut self, amount: usize) -> Result<(), &'static str> {
        self.used = self.used.checked_add(amount).ok_or(WORK_LIMIT)?;
        if self.used > MAX_SEMANTIC_OPTION_DOMINANCE_WORK_V1 {
            return Err(WORK_LIMIT);
        }
        Ok(())
    }

    #[cfg(test)]
    fn push<T>(&mut self, values: &mut Vec<T>, value: T) -> Result<(), &'static str> {
        self.charge(1)?;
        values.try_reserve(1).map_err(|_| STORAGE)?;
        values.push(value);
        Ok(())
    }
}

#[cfg(test)]
mod tests;

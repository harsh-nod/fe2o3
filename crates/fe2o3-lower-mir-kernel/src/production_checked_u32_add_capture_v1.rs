//! Opt-in, owner-bound capture from the actual legacy V8 lowering invocation.
//! This records emission custody, not expression equivalence or launch authority.

use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_mir_model::{SsaEventV1, SsaVariableIdV1};
use fe2o3_pliron::{
    ProductionSemanticSsaEventRoleV1 as EventRole, ProductionSemanticSsaOccurrenceErrorV1,
    ProductionSemanticSsaOccurrenceSiteV1 as Site,
    ProductionSemanticSsaOperandRoleV1 as OperandRole,
};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

/// Exact caller-selected source statement, not caller-supplied evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionCheckedU32AddCaptureRequestV1 {
    root: SemanticFunctionIdV1,
    function: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
    statement: u32,
}

impl ProductionCheckedU32AddCaptureRequestV1 {
    /// Selects one source site; the genuine lowerer must capture it exactly once.
    pub const fn new(
        root: SemanticFunctionIdV1,
        function: SemanticFunctionIdV1,
        block: SemanticBlockIdV1,
        statement: u32,
    ) -> Self {
        Self {
            root,
            function,
            block,
            statement,
        }
    }
    /// Exact semantic root in the retained source owner.
    pub const fn root(self) -> SemanticFunctionIdV1 {
        self.root
    }
    /// Exact source function, not its emitted ordinal.
    pub const fn function(self) -> SemanticFunctionIdV1 {
        self.function
    }
    /// Exact original source block.
    pub const fn block(self) -> SemanticBlockIdV1 {
        self.block
    }
    /// Original statement ordinal.
    pub const fn statement(self) -> u32 {
        self.statement
    }
}

/// Capture admission errors do not grant a partial emission relation.
#[derive(Debug)]
pub enum ProductionCheckedU32AddCaptureErrorV1 {
    /// Existing source/SSA/lowering or exact capture replay rejection.
    Lowering(ProductionSemanticKirErrorV1),
    /// The genuine source occurrence capture rejected its replay or resources.
    Occurrences(ProductionSemanticSsaOccurrenceErrorV1),
    /// The caller's capture-only resource ledger rejected a reservation.
    Resource(Resource),
    /// The genuine lowerer produced a different canonical wire version.
    UnsupportedCanonicalVersion,
}

impl fmt::Display for ProductionCheckedU32AddCaptureErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "checked-u32 V8 emission capture: {self:?}")
    }
}
impl std::error::Error for ProductionCheckedU32AddCaptureErrorV1 {}
impl From<ProductionSemanticKirErrorV1> for ProductionCheckedU32AddCaptureErrorV1 {
    fn from(error: ProductionSemanticKirErrorV1) -> Self {
        Self::Lowering(error)
    }
}
impl From<Resource> for ProductionCheckedU32AddCaptureErrorV1 {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Source {
    request: ProductionCheckedU32AddCaptureRequestV1,
    lhs_local: SemanticLocalIdV1,
    tuple_local: SemanticLocalIdV1,
    lhs_ssa: SsaValueV1,
    tuple_ssa: SsaValueV1,
    use_event: u32,
    define_event: u32,
    literal: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Captured {
    source: Source,
    block: BlockId,
    operation: u32,
    operand: ValueId,
    value: ValueId,
    overflow: ValueId,
}

impl Captured {
    pub(super) const fn request(self) -> ProductionCheckedU32AddCaptureRequestV1 {
        self.source.request
    }
}

/// A borrowed fact from one actual source/SSA/KIR owner, never a detached row.
/// Both source-to-SSA value equality and machine entry equality remain unproved.
///
/// ```compile_fail,E0505
/// use fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1;
/// fn cannot_detach(owner: ProductionSemanticKirOwnerV1) {
///     let fact = owner.checked_u32_add_capture_v1().unwrap();
///     drop(owner);
///     let _ = fact.value();
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{ProductionCheckedU32AddCaptureV1, ProductionSemanticKirOwnerV1};
/// fn cannot_forge(owner: &ProductionSemanticKirOwnerV1) {
///     let _ = ProductionCheckedU32AddCaptureV1 { owner, capture: todo!() };
/// }
/// ```
#[derive(Clone, Copy, Debug)]
pub struct ProductionCheckedU32AddCaptureV1<'a> {
    owner: &'a ProductionSemanticKirOwnerV1,
    capture: &'a Captured,
}

impl<'a> ProductionCheckedU32AddCaptureV1<'a> {
    /// The same move-only owner that emitted and retains the captured values.
    pub const fn owner(self) -> &'a ProductionSemanticKirOwnerV1 {
        self.owner
    }
    /// Actual captured source site.
    pub const fn request(self) -> ProductionCheckedU32AddCaptureRequestV1 {
        self.capture.source.request
    }
    /// Original unprojected copied input local.
    pub const fn lhs_local(self) -> SemanticLocalIdV1 {
        self.capture.source.lhs_local
    }
    /// Original unprojected checked-result tuple local.
    pub const fn tuple_local(self) -> SemanticLocalIdV1 {
        self.capture.source.tuple_local
    }
    /// Resolution of the actual adapter's input Use event.
    pub const fn lhs_ssa(self) -> SsaValueV1 {
        self.capture.source.lhs_ssa
    }
    /// Resolution of the actual adapter's tuple Define event.
    pub const fn tuple_ssa(self) -> SsaValueV1 {
        self.capture.source.tuple_ssa
    }
    /// Input event ordinal in the original block's complete event sequence.
    pub const fn use_event(self) -> u32 {
        self.capture.source.use_event
    }
    /// Definition event ordinal in that same sequence.
    pub const fn define_event(self) -> u32 {
        self.capture.source.define_event
    }
    /// Exact emitted function associated with this root/function pair.
    pub fn kernel_ir_function(self) -> &'a str {
        self.owner
            .correspondence
            .lowered_functions()
            .iter()
            .find(|row| {
                row.correspondence_owner() == self.request().root
                    && row.semantic_function() == self.request().function
            })
            .expect("captured function correspondence was checked during construction")
            .kernel_ir_function()
            .as_str()
    }
    /// Actual emitted checked-add block.
    pub const fn block(self) -> BlockId {
        self.capture.block
    }
    /// Actual checked-add operation ordinal, not a raw value ID.
    pub const fn operation(self) -> u32 {
        self.capture.operation
    }
    /// Actual lowering-map value consumed as the left operand.
    pub const fn operand(self) -> ValueId {
        self.capture.operand
    }
    /// Actual first checked-add result.
    pub const fn value(self) -> ValueId {
        self.capture.value
    }
    /// Actual second checked-add result.
    pub const fn overflow(self) -> ValueId {
        self.capture.overflow
    }
    /// Exact source constant and emitted u32 constant.
    pub const fn literal(self) -> u32 {
        self.capture.source.literal
    }
    /// Emission custody is not an expression-equivalence theorem.
    pub const fn proves_source_value_equivalence(self) -> bool {
        false
    }
    /// This capture never grants artifact or launch authority.
    pub const fn grants_artifact_or_launch_authority(self) -> bool {
        false
    }
}

impl ProductionSemanticKirOwnerV1 {
    /// Captures one exact `Copy(u32 local) + u32 literal` checked assignment
    /// during the same genuine lowering call. Only the unchanged V8 wire is
    /// supported. The ordinary constructors remain capture-free.
    ///
    /// The ledger covers upstream occurrence-capture work/storage and the added
    /// fixed retained capture. Exact-site searches and emission-map joins use
    /// existing source/lowering limits but are not charged to this ledger, nor
    /// are legacy source/SSA construction, lowering or replay allocations. This
    /// is not whole-admission metering or an allocator/RSS guarantee. On
    /// return or unwind the entry floor is restored after failed owners drop.
    /// Success transfers an unreserved owner; reserve its capture storage before
    /// any further controlled allocation while it lives.
    pub fn try_lower_with_checked_u32_add_capture_v1(
        semantic: ProductionSemanticMirOwnerV1,
        limits: ProductionSemanticKirLimitsV1,
        request: ProductionCheckedU32AddCaptureRequestV1,
        budget: &mut Budget<'_>,
    ) -> Result<Self, ProductionCheckedU32AddCaptureErrorV1> {
        let floor = budget.storage();
        let result = catch_unwind(AssertUnwindSafe(|| {
            budget.reserve_storage(std::mem::size_of::<Option<Captured>>())?;
            let mut semantic_ssa = ProductionSemanticSsaOwnerV1::try_new(
                semantic,
                ProductionSemanticSsaLimitsV1::default(),
            )
            .map_err(ProductionSemanticKirErrorV1::SemanticSsa)?;
            let receipt = semantic_ssa
                .try_capture_occurrences_with_budget_v1(budget)
                .map_err(ProductionCheckedU32AddCaptureErrorV1::Occurrences)?;
            budget.reserve_storage(receipt.retained_storage())?;
            let mut capture = Pending::prepare(&semantic_ssa, request)?;
            let (module, correspondence) =
                lower_module_with_capture_v1(&semantic_ssa, limits, None, Some(&mut capture))?;
            let canonical_kernel_ir = ProductionCanonicalKernelIrV1::from_module(module.clone())?;
            if !matches!(canonical_kernel_ir, ProductionCanonicalKernelIrV1::V8(_)) {
                return Err(ProductionCheckedU32AddCaptureErrorV1::UnsupportedCanonicalVersion);
            }
            let owner = Self {
                semantic_ssa,
                module,
                canonical_kernel_ir,
                correspondence,
                limits,
                launch_roots: None,
                generic_checks: Vec::new().into_boxed_slice(),
                checked_u32_add_capture: Some(capture.finish()?),
            };
            owner.verify_equivalence()?;
            Ok(owner)
        }));
        let cleanup = budget
            .storage()
            .checked_sub(floor)
            .ok_or(Resource::Accounting)
            .and_then(|amount| budget.release_storage(amount));
        match result {
            Ok(result) => {
                cleanup?;
                result
            }
            Err(panic) => resume_unwind(panic),
        }
    }

    /// Borrows the optional actual emission attachment without reconstructing it.
    pub fn checked_u32_add_capture_v1(&self) -> Option<ProductionCheckedU32AddCaptureV1<'_>> {
        self.checked_u32_add_capture
            .as_ref()
            .map(|capture| ProductionCheckedU32AddCaptureV1 {
                owner: self,
                capture,
            })
    }

    /// Capture-only retained bytes transferred unreserved by the opt-in constructor.
    /// Existing source, SSA, module and correspondence storage stays excluded.
    pub fn checked_u32_add_capture_storage_v1(&self) -> Option<usize> {
        self.checked_u32_add_capture.as_ref()?;
        self.semantic_ssa
            .occurrence_storage()?
            .retained_storage()
            .checked_add(std::mem::size_of::<Option<Captured>>())
    }
}

pub(super) struct Pending {
    source: Source,
    captured: Option<Captured>,
}

impl Pending {
    pub(super) fn prepare(
        owner: &ProductionSemanticSsaOwnerV1,
        request: ProductionCheckedU32AddCaptureRequestV1,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        let error = || {
            unsupported(
                request.function.index(),
                Some(request.block.index()),
                Some(request.statement),
                "exact captured checked-u32 source profile unavailable",
            )
        };
        let semantic = owner.source_semantic();
        if !semantic.roots().contains(&request.root) {
            return Err(error());
        }
        let function = semantic
            .functions()
            .get(request.function.index() as usize)
            .ok_or_else(error)?;
        let statement = function
            .blocks()
            .get(request.block.index() as usize)
            .and_then(|block| block.statements().get(request.statement as usize))
            .ok_or_else(error)?;
        let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
            return Err(error());
        };
        let SemanticRvalueKindV1::CheckedBinary(add) = assignment.value().kind() else {
            return Err(error());
        };
        let SemanticOperandV1::Copy(lhs) = add.left() else {
            return Err(error());
        };
        let SemanticOperandV1::Constant(constant) = add.right() else {
            return Err(error());
        };
        let is_u32 = |ty: SemanticTypeIdV1| {
            matches!(
                semantic
                    .types()
                    .get(ty.index() as usize)
                    .map(|decl| decl.shape()),
                Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32
                }))
            )
        };
        if add.operation() != SemanticCheckedBinaryOpV1::Add
            || !lhs.projections().is_empty()
            || !assignment.destination().projections().is_empty()
            || !is_u32(lhs.ty())
            || !is_u32(constant.ty())
            || function
                .locals()
                .get(lhs.local().index() as usize)
                .is_none_or(|local| local.ty() != lhs.ty())
            || function
                .locals()
                .get(assignment.destination().local().index() as usize)
                .is_none_or(|local| local.ty() != assignment.destination().ty())
            || assignment.value().result_type() != assignment.destination().ty()
        {
            return Err(error());
        }
        let Some(SemanticTypeShapeV1::Tuple(tuple)) = semantic
            .types()
            .get(assignment.destination().ty().index() as usize)
            .map(|decl| decl.shape())
        else {
            return Err(error());
        };
        let [value_ty, overflow_ty] = tuple.fields() else {
            return Err(error());
        };
        if !is_u32(*value_ty)
            || !matches!(
                semantic
                    .types()
                    .get(overflow_ty.index() as usize)
                    .map(|decl| decl.shape()),
                Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool))
            )
        {
            return Err(error());
        }
        let SemanticConstantValueV1::Scalar(literal) = constant.value() else {
            return Err(error());
        };
        let literal = u32::try_from(literal.bits())
            .ok()
            .filter(|_| literal.size_bytes() == 4)
            .ok_or_else(error)?;
        let occurrences = owner.occurrences_v1().ok_or_else(error)?;
        let rows = occurrences.function(request.function).ok_or_else(error)?;
        if !std::ptr::eq(rows.owner(), owner) {
            return Err(error());
        }
        let site = Site::Statement {
            block: SsaBlockIdV1::new(request.block.index()),
            statement: request.statement,
        };
        let mut events = rows.events().iter().filter(|row| row.site() == site);
        let use_row = events.next().ok_or_else(error)?;
        let define_row = events.next().ok_or_else(error)?;
        if events.next().is_some()
            || !use_row.is_reachable()
            || !use_row.is_promoted()
            || !define_row.is_reachable()
            || !define_row.is_promoted()
            || use_row.role() != EventRole::BaseUse
            || use_row.operand() != OperandRole::RvalueOperand(0)
            || define_row.role() != EventRole::DestinationDefine
            || define_row.operand() != OperandRole::Destination
            || use_row.ordinal().checked_add(1) != Some(define_row.ordinal())
            || use_row.event() != SsaEventV1::Use(SsaVariableIdV1::new(lhs.local().index()))
            || define_row.event()
                != SsaEventV1::Define(SsaVariableIdV1::new(
                    assignment.destination().local().index(),
                ))
        {
            return Err(error());
        }
        let Some(SsaResolvedEventV1::Use {
            variable: lhs_variable,
            value: lhs_ssa,
        }) = use_row.resolved()
        else {
            return Err(error());
        };
        let Some(SsaResolvedEventV1::Define {
            variable: tuple_variable,
            value: tuple_ssa,
        }) = define_row.resolved()
        else {
            return Err(error());
        };
        let plan = owner
            .plan_for_function(request.function)
            .ok_or_else(error)?
            .plan();
        if lhs_variable.get() != lhs.local().index()
            || tuple_variable.get() != assignment.destination().local().index()
            || plan
                .resolved_event(SsaBlockIdV1::new(request.block.index()), use_row.ordinal())
                .copied()
                != use_row.resolved()
            || plan
                .resolved_event(
                    SsaBlockIdV1::new(request.block.index()),
                    define_row.ordinal(),
                )
                .copied()
                != define_row.resolved()
        {
            return Err(error());
        }
        let mut constants = rows.constants().iter().filter(|row| row.site() == site);
        let literal_row = constants.next().ok_or_else(error)?;
        if constants.next().is_some()
            || literal_row.operand() != OperandRole::RvalueOperand(1)
            || literal_row.ty() != constant.ty()
            || literal_row.next_event() != define_row.ordinal()
        {
            return Err(error());
        }
        Ok(Self {
            source: Source {
                request,
                lhs_local: lhs.local(),
                tuple_local: assignment.destination().local(),
                lhs_ssa,
                tuple_ssa,
                use_event: use_row.ordinal(),
                define_event: define_row.ordinal(),
                literal,
            },
            captured: None,
        })
    }

    pub(super) fn record_function(
        &mut self,
        plan: &LoweredFunctionPlanV1,
        lowering: &SemanticFunctionLoweringV1<'_>,
        blocks: &[BasicBlock],
        spans: &[SemanticKirStatementOperationSpanV1],
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let request = self.source.request;
        if plan.correspondence_owner != request.root || plan.semantic_function != request.function {
            return Ok(());
        }
        let error = || ProductionSemanticKirErrorV1::CorrespondenceMismatch;
        if self.captured.is_some() {
            return Err(error());
        }
        let scalar = |value, ty| match lowering.semantic_ssa_bindings.get(&value) {
            Some(SemanticValueBindingV1::Value { id, ty: actual })
                if *actual == Type::Scalar(ty) =>
            {
                Ok(*id)
            }
            _ => Err(error()),
        };
        let operand = scalar(self.source.lhs_ssa, ScalarType::U32)?;
        let Some(SemanticValueBindingV1::Aggregate(tuple)) =
            lowering.semantic_ssa_bindings.get(&self.source.tuple_ssa)
        else {
            return Err(error());
        };
        let [
            SemanticValueBindingV1::Value {
                id: value,
                ty: value_ty,
            },
            SemanticValueBindingV1::Value {
                id: overflow,
                ty: overflow_ty,
            },
        ] = tuple.as_slice()
        else {
            return Err(error());
        };
        if *value_ty != Type::Scalar(ScalarType::U32)
            || *overflow_ty != Type::Scalar(ScalarType::Bool)
        {
            return Err(error());
        }
        let mut matching = spans.iter().filter(|span| {
            span.correspondence_owner() == request.root
                && span.semantic_function() == request.function
                && span.semantic_block() == request.block
                && span.statement_ordinal() == request.statement
        });
        let span = matching.next().ok_or_else(error)?;
        if matching.next().is_some() || span.operation_count() != 2 {
            return Err(error());
        }
        let block = blocks
            .iter()
            .find(|block| block.id == span.kernel_ir_block())
            .ok_or_else(error)?;
        let operation = span
            .first_operation_ordinal()
            .checked_add(1)
            .ok_or_else(error)?;
        let constant = block
            .operations
            .get(span.first_operation_ordinal() as usize)
            .ok_or_else(error)?;
        let add = block.operations.get(operation as usize).ok_or_else(error)?;
        let [rhs] = constant.results.as_slice() else {
            return Err(error());
        };
        if constant.kind != OperationKind::Constant(Constant::U32(self.source.literal))
            || rhs.ty != Type::Scalar(ScalarType::U32)
            || add.kind
                != (OperationKind::Binary {
                    op: BinaryOp::Checked(CheckedBinaryOperator::Add),
                    lhs: operand,
                    rhs: rhs.id,
                })
            || add.results.as_slice()
                != [
                    ValueDef::new(*value, Type::Scalar(ScalarType::U32)),
                    ValueDef::new(*overflow, Type::Scalar(ScalarType::Bool)),
                ]
        {
            return Err(error());
        }
        self.captured = Some(Captured {
            source: self.source,
            block: block.id,
            operation,
            operand,
            value: *value,
            overflow: *overflow,
        });
        Ok(())
    }

    pub(super) fn finish(self) -> Result<Captured, ProductionSemanticKirErrorV1> {
        self.captured
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    }
}

#[cfg(test)]
#[path = "production_checked_u32_add_capture_v1_tests.rs"]
mod tests;

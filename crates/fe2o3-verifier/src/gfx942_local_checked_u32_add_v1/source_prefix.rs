//! Conditional entry-prefix value correspondence, not machine or launch authority.

use super::*;
use fe2o3_kernel_analysis::{Gfx942U32AddResultV1, gfx942_add_u32_v1};
use fe2o3_lower_mir_kernel::{
    ProductionCheckedU32AddCaptureV1, SemanticKirCorrespondenceV1,
    SemanticKirStatementOperationSpanV1,
};
use fe2o3_mir_model::semantic_mir_v1::{
    AdmittedInertSemanticMirV1 as AdmittedSemanticMirV1, SemanticAbiArgumentRoleV1,
    SemanticAbiPassModeV1, SemanticFunctionDeclV1, SemanticLocalRoleV1, SemanticPlaceV1,
    SemanticStatementV1,
};
use std::collections::BTreeMap;

mod assemble;
mod basis;
mod fold;
mod normalize;
use fold::{Origin, PrefixInput, PrefixStep};

const MAX_STATEMENTS: usize = 256;
const MAX_LOCALS: usize = 4096;
const MAX_ARGUMENTS: usize = 128;

/// Symbolic value in the exact checked function-input basis, not a live value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckedU32PrefixOriginV1 {
    Argument(usize),
    Constant(u32),
}

/// These source and KIR inputs must have equal u32 values at function entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedU32PrefixArgumentV1 {
    argument: usize,
    semantic_local: u32,
    kernel_ir_value: ValueId,
}

impl CheckedU32PrefixArgumentV1 {
    pub const fn argument(self) -> usize {
        self.argument
    }

    pub const fn semantic_local(self) -> u32 {
        self.semantic_local
    }

    pub const fn kernel_ir_value(self) -> ValueId {
        self.kernel_ir_value
    }
}

/// A checked terminal value relation borrowing its original compiler owner.
///
/// Equal input values in `arguments()` imply equal captured operands and equal
/// checked-add value/overflow results on the first traversal from function
/// entry. Later backedges and repeated block visits are not covered. This is
/// not a claim about machine entry, continuation, compiler origin or authority.
/// ABI discovery and complete KIR assembly remain separate from the shared
/// source-normalization, argument-basis and fold theorems.
#[derive(Debug)]
pub struct CapturedCheckedU32PrefixV1<'a> {
    capture: ProductionCheckedU32AddCaptureV1<'a>,
    arguments: Vec<CheckedU32PrefixArgumentV1>,
    origin: CheckedU32PrefixOriginV1,
}

impl<'a> CapturedCheckedU32PrefixV1<'a> {
    pub const fn capture(&self) -> ProductionCheckedU32AddCaptureV1<'a> {
        self.capture
    }

    pub fn arguments(&self) -> &[CheckedU32PrefixArgumentV1] {
        &self.arguments
    }

    pub const fn operand_origin(&self) -> CheckedU32PrefixOriginV1 {
        self.origin
    }

    /// Evaluate the conditional result for a supplied common input vector.
    /// This does not authenticate an execution's actual argument values.
    pub fn evaluate(
        &self,
        arguments: &[u32],
    ) -> Result<Gfx942U32AddResultV1, CheckedU32PrefixErrorV1> {
        if arguments.len() != self.arguments.len() {
            return Err(CheckedU32PrefixErrorV1::Arguments);
        }
        let lhs = match self.origin {
            CheckedU32PrefixOriginV1::Argument(index) => arguments[index],
            CheckedU32PrefixOriginV1::Constant(value) => value,
        };
        Ok(gfx942_add_u32_v1(lhs, self.capture.literal()))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CheckedU32PrefixErrorV1 {
    Version,
    Capacity,
    Entry,
    Arguments,
    Source,
    Span,
    Kernel,
    Uninitialized,
    ValueMismatch,
}

impl fmt::Display for CheckedU32PrefixErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "unsupported checked-u32 entry prefix: {self:?}")
    }
}
impl Error for CheckedU32PrefixErrorV1 {}

/// Independently check the actual retained MIR and V8 KIR entry prefixes.
///
/// At most 256 statements (including the checked add), 4096 source locals and
/// 128 direct u32 arguments are supported. Source constants/copies/Nops and KIR
/// constants precede one captured checked add. All other prefix effects reject.
/// No arbitrary state, replacement module, detached map or receipt is accepted.
/// The result cannot outlive the compiler owner that supplied the capture:
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1;
/// use fe2o3_verifier::{CapturedCheckedU32PrefixV1, check_captured_checked_u32_prefix_v1};
/// fn detached(owner: ProductionSemanticKirOwnerV1) -> CapturedCheckedU32PrefixV1<'static> {
///     check_captured_checked_u32_prefix_v1(owner.checked_u32_add_capture_v1().unwrap()).unwrap()
/// }
/// ```
///
/// ```compile_fail,E0505
/// use fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1;
/// use fe2o3_verifier::check_captured_checked_u32_prefix_v1;
/// fn drop_before_evaluation(owner: ProductionSemanticKirOwnerV1) {
///     let relation = check_captured_checked_u32_prefix_v1(
///         owner.checked_u32_add_capture_v1().unwrap()).unwrap();
///     drop(owner);
///     let _ = relation.evaluate(&[]);
/// }
/// ```
pub fn check_captured_checked_u32_prefix_v1(
    capture: ProductionCheckedU32AddCaptureV1<'_>,
) -> Result<CapturedCheckedU32PrefixV1<'_>, CheckedU32PrefixErrorV1> {
    let owner = capture.owner();
    owner
        .canonical_kernel_ir_v8()
        .ok_or(CheckedU32PrefixErrorV1::Version)?;
    let (arguments, origin) = check_parts(
        capture,
        owner.semantic().semantic(),
        owner.module(),
        owner.correspondence(),
    )?;
    Ok(CapturedCheckedU32PrefixV1 {
        capture,
        arguments,
        origin,
    })
}

fn is_u32(source: &AdmittedSemanticMirV1, ty: SemanticTypeIdV1) -> bool {
    normalize::is_u32(source.types(), ty)
}

fn scalar_local(
    source: &AdmittedSemanticMirV1,
    function: &SemanticFunctionDeclV1,
    place: &SemanticPlaceV1,
) -> Result<usize, CheckedU32PrefixErrorV1> {
    normalize::scalar_local(source.types(), function.locals(), place)
}

fn scalar_constant(source: &AdmittedSemanticMirV1, operand: &SemanticOperandV1) -> Option<u32> {
    normalize::scalar_constant(source.types(), operand)
}

fn check_arguments(
    source: &AdmittedSemanticMirV1,
    function: &SemanticFunctionDeclV1,
    kernel: &Function,
    capture: ProductionCheckedU32AddCaptureV1<'_>,
    correspondence: &SemanticKirCorrespondenceV1,
) -> Result<Vec<CheckedU32PrefixArgumentV1>, CheckedU32PrefixErrorV1> {
    use CheckedU32PrefixErrorV1 as E;
    let request = capture.request();
    let abi = function.abi();
    let body = kernel.body.as_ref().ok_or(E::Entry)?;
    let count = abi.source_input_types().len();
    if count > MAX_ARGUMENTS {
        return Err(E::Capacity);
    }
    if abi.can_unwind()
        || abi.c_variadic()
        || !abi.hidden_arguments().is_empty()
        || abi.adjusted_arguments().len() != count
        || abi.fixed_count() as usize != count
        || body.parameters.len() != count
        || kernel.signature.parameters.len() != count
        || correspondence
            .parameter_component_bindings()
            .iter()
            .any(|binding| {
                binding.correspondence_owner() == request.root()
                    && binding.semantic_function() == request.function()
            })
        || correspondence
            .ignored_parameter_bindings()
            .iter()
            .any(|binding| {
                binding.correspondence_owner() == request.root()
                    && binding.semantic_function() == request.function()
            })
    {
        return Err(E::Arguments);
    }
    let mut locals = vec![None; count];
    for (local, declaration) in function.locals().iter().enumerate() {
        if let SemanticLocalRoleV1::Argument(argument) = declaration.role() {
            let entry = locals.get_mut(argument as usize).ok_or(E::Arguments)?;
            if entry.replace(local).is_some() {
                return Err(E::Arguments);
            }
        }
    }
    let bindings: Vec<_> = correspondence
        .parameter_bindings()
        .iter()
        .filter(|binding| {
            binding.correspondence_owner() == request.root()
                && binding.semantic_function() == request.function()
        })
        .collect();
    if bindings.len() != count {
        return Err(E::Arguments);
    }
    let mut arguments = Vec::with_capacity(count);
    for (argument, local) in locals.into_iter().enumerate() {
        let local = local.ok_or(E::Arguments)?;
        let ty = function.locals()[local].ty();
        let adjusted = &abi.adjusted_arguments()[argument];
        let value = body.parameters[argument];
        if !is_u32(source, ty)
            || abi.source_input_types()[argument] != ty
            || adjusted.role() != SemanticAbiArgumentRoleV1::Source
            || adjusted.ty() != ty
            || adjusted.value().adjusted().is_some()
            || adjusted.value().pointee_override().is_some()
            || !matches!(adjusted.mode(), SemanticAbiPassModeV1::Direct(_))
            || kernel.signature.parameters[argument] != Type::Scalar(ScalarType::U32)
            || bindings
                .iter()
                .filter(|binding| {
                    binding.semantic_local().index() as usize == local
                        && binding.kernel_ir_value() == value
                })
                .count()
                != 1
            || body.parameters[..argument].contains(&value)
        {
            return Err(E::Arguments);
        }
        arguments.push(CheckedU32PrefixArgumentV1 {
            argument,
            semantic_local: local as u32,
            kernel_ir_value: value,
        });
    }
    Ok(arguments)
}

fn check_parts(
    capture: ProductionCheckedU32AddCaptureV1<'_>,
    source: &AdmittedSemanticMirV1,
    module: &Module,
    correspondence: &SemanticKirCorrespondenceV1,
) -> Result<(Vec<CheckedU32PrefixArgumentV1>, CheckedU32PrefixOriginV1), CheckedU32PrefixErrorV1> {
    use CheckedU32PrefixErrorV1 as E;
    let request = capture.request();
    let statement_count = (request.statement() as usize)
        .checked_add(1)
        .ok_or(E::Capacity)?;
    let function = source
        .functions()
        .get(request.function().index() as usize)
        .ok_or(E::Entry)?;
    if statement_count > MAX_STATEMENTS || function.locals().len() > MAX_LOCALS {
        return Err(E::Capacity);
    }
    if function.entry() != request.block() {
        return Err(E::Entry);
    }
    let block = function
        .blocks()
        .get(request.block().index() as usize)
        .ok_or(E::Entry)?;
    let prefix = block.statements().get(..statement_count).ok_or(E::Entry)?;
    let kernel_function = capture.kernel_ir_function();
    let mut functions = module
        .functions
        .iter()
        .filter(|function| function.id.as_str() == kernel_function);
    let kernel = functions.next().ok_or(E::Entry)?;
    if functions.next().is_some() {
        return Err(E::Entry);
    }
    let body = kernel.body.as_ref().ok_or(E::Entry)?;
    let entry = body.blocks.first().ok_or(E::Entry)?;
    if entry.id != capture.block() || !entry.parameters.is_empty() {
        return Err(E::Entry);
    }
    let arguments = check_arguments(source, function, kernel, capture, correspondence)?;
    let (source_steps, next_operation) = assemble::source_prefix(
        source,
        function,
        prefix,
        correspondence.statement_operation_spans(),
        capture,
        entry.id,
    )?;
    let mut source_state = vec![Origin::Uninitialized; function.locals().len()];
    let mut kernel_state = vec![Origin::Uninitialized; arguments.len()];
    if !basis::initialize_argument_basis(&arguments, &mut source_state, &mut kernel_state) {
        return Err(E::Arguments);
    }
    if !fold::fold(&mut source_state, &source_steps) {
        return Err(E::Uninitialized);
    }
    let SemanticStatementKindV1::Assign(assign) = prefix.last().ok_or(E::Source)?.kind() else {
        return Err(E::Source);
    };
    let SemanticRvalueKindV1::CheckedBinary(add) = assign.value().kind() else {
        return Err(E::Source);
    };
    let SemanticOperandV1::Copy(lhs) = add.left() else {
        return Err(E::Source);
    };
    let source_local = scalar_local(source, function, lhs)?;
    if add.operation() != SemanticCheckedBinaryOpV1::Add
        || lhs.local() != capture.lhs_local()
        || assign.destination().local() != capture.tuple_local()
        || !assign.destination().projections().is_empty()
        || assign.value().result_type() != assign.destination().ty()
        || scalar_constant(source, add.right()) != Some(capture.literal())
    {
        return Err(E::Source);
    }
    let Some(SemanticTypeShapeV1::Tuple(tuple)) = source
        .types()
        .get(assign.destination().ty().index() as usize)
        .map(|ty| ty.shape())
    else {
        return Err(E::Source);
    };
    let [value_ty, overflow_ty] = tuple.fields() else {
        return Err(E::Source);
    };
    if !is_u32(source, *value_ty)
        || !matches!(
            source
                .types()
                .get(overflow_ty.index() as usize)
                .map(|ty| ty.shape()),
            Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool))
        )
        || function
            .locals()
            .get(capture.tuple_local().index() as usize)
            .is_none_or(|local| local.ty() != assign.destination().ty())
    {
        return Err(E::Source);
    }
    let operations = entry
        .operations
        .get(..next_operation as usize)
        .ok_or(E::Kernel)?;
    let mut slots = BTreeMap::new();
    kernel_state.reserve(operations.len() + 1);
    for (slot, binding) in arguments.iter().enumerate() {
        if slots.insert(binding.kernel_ir_value, slot).is_some() {
            return Err(E::Kernel);
        }
    }
    let mut kernel_steps = Vec::with_capacity(operations.len());
    for operation in &operations[..operations.len().checked_sub(1).ok_or(E::Kernel)?] {
        let OperationKind::Constant(Constant::U32(value)) = operation.kind else {
            return Err(E::Kernel);
        };
        let [result] = operation.results.as_slice() else {
            return Err(E::Kernel);
        };
        let destination = kernel_state.len();
        if result.ty != Type::Scalar(ScalarType::U32)
            || slots.insert(result.id, destination).is_some()
        {
            return Err(E::Kernel);
        }
        kernel_state.push(Origin::Uninitialized);
        kernel_steps.push(PrefixStep {
            destination,
            input: PrefixInput::Constant(value),
        });
    }
    if !fold::fold(&mut kernel_state, &kernel_steps) {
        return Err(E::Kernel);
    }
    let terminal = operations.last().ok_or(E::Kernel)?;
    let OperationKind::Binary {
        op: BinaryOp::Checked(CheckedBinaryOperator::Add),
        lhs,
        rhs,
    } = terminal.kind
    else {
        return Err(E::Kernel);
    };
    let [value, overflow] = terminal.results.as_slice() else {
        return Err(E::Kernel);
    };
    if lhs != capture.operand()
        || value.id != capture.value()
        || overflow.id != capture.overflow()
        || value.id == overflow.id
        || slots.contains_key(&value.id)
        || slots.contains_key(&overflow.id)
        || value.ty != Type::Scalar(ScalarType::U32)
        || overflow.ty != Type::Scalar(ScalarType::Bool)
        || slots
            .get(&rhs)
            .is_none_or(|slot| kernel_state[*slot] != Origin::Constant(capture.literal()))
        || operations[operations.len() - 2]
            .results
            .first()
            .is_none_or(|result| result.id != rhs)
    {
        return Err(E::Kernel);
    }
    let origin = source_state[source_local];
    if slots
        .get(&lhs)
        .is_none_or(|slot| kernel_state[*slot] != origin)
    {
        return Err(E::ValueMismatch);
    }
    let origin = match origin {
        Origin::Uninitialized => return Err(E::Uninitialized),
        Origin::Argument(index) => CheckedU32PrefixOriginV1::Argument(index),
        Origin::Constant(value) => CheckedU32PrefixOriginV1::Constant(value),
    };
    Ok((arguments, origin))
}

#[cfg(test)]
mod tests;

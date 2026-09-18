//! Source-only derivation. Native KIR templates must be rebuilt independently.
use super::helper_value_template_v1::{Kind, Meter, Template, Value, vector};
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticCanonAbiV1, SemanticExternAbiV1, SemanticFunctionAbiV1, SemanticSaturatingIntegerOpV1,
};

type Error = &'static str;
type Scalar = ProductionSemanticScalarTypeV2;

include!("source_helper_constant_shift_v1.rs");

#[derive(Clone, Copy)]
enum Slot {
    Uninitialized,
    Scalar(Value),
    // Checked arithmetic is total, but this first template domain cannot
    // represent its overflow flag. Only the exact numeric component is usable.
    CheckedValue(Value),
}

pub(super) fn scalar(types: &[SemanticTypeDeclV1], ty: SemanticTypeIdV1) -> Result<Scalar, Error> {
    match types
        .get(ty.index() as usize)
        .map(SemanticTypeDeclV1::shape)
    {
        Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool)) => Ok(Scalar::Bool),
        Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed, bits }))
            if matches!(bits, 8 | 16 | 32 | 64) =>
        {
            Ok(Scalar::Integer {
                signed: *signed,
                bits: *bits,
            })
        }
        Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 32 | 64 })) => {
            let SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits }) =
                types[ty.index() as usize].shape()
            else {
                unreachable!()
            };
            Ok(Scalar::Float { bits: *bits })
        }
        _ => Err("helper value requires an ordinary fixed scalar"),
    }
}

fn checked_type(types: &[SemanticTypeDeclV1], ty: SemanticTypeIdV1) -> Option<SemanticTypeIdV1> {
    let SemanticTypeShapeV1::Tuple(fields) = types.get(ty.index() as usize)?.shape() else {
        return None;
    };
    let [value, flag] = fields.fields() else {
        return None;
    };
    (scalar(types, *value).ok()?.is_integer() && scalar(types, *flag).ok()? == Scalar::Bool)
        .then_some(*value)
}

pub(super) fn abi(
    types: &[SemanticTypeDeclV1],
    abi: &SemanticFunctionAbiV1,
    meter: &mut dyn Meter,
) -> Result<(), Error> {
    meter.work(12)?;
    if abi.canon_abi() != SemanticCanonAbiV1::Rust
        || abi.extern_abi() != SemanticExternAbiV1::Rust
        || abi.can_unwind()
        || abi.c_variadic()
        || !abi.hidden_arguments().is_empty()
        || abi.arguments().len() != abi.source_input_types().len()
        || abi.arguments().len() != abi.source_argument_ownership().len()
        || abi.fixed_count() as usize != abi.arguments().len()
    {
        return Err("helper value requires exact nounwind Rust scalar ABI");
    }
    for ((argument, ty), ownership) in abi
        .arguments()
        .iter()
        .zip(abi.source_input_types())
        .zip(abi.source_argument_ownership())
    {
        meter.work(8)?;
        if !argument.is_source()
            || argument.ty() != *ty
            || argument.value().adjusted().is_some()
            || argument.value().pointee_override().is_some()
            || !matches!(argument.mode(), SemanticAbiPassModeV1::Direct(_))
            || *ownership != SemanticSourceArgumentOwnershipV1::ByValue
        {
            return Err("helper value argument has nontrivial ABI transport");
        }
        scalar(types, *ty)?;
    }
    let returned = abi.return_value();
    scalar(types, abi.source_output_type())?;
    if returned.source_ty() != abi.source_output_type()
        || returned.adjusted().is_some()
        || returned.pointee_override().is_some()
        || !matches!(returned.mode(), SemanticAbiPassModeV1::Direct(_))
    {
        return Err("helper value result has nontrivial ABI transport");
    }
    Ok(())
}

struct Frame<'a, 't, 'm> {
    types: &'a [SemanticTypeDeclV1],
    function: &'a SemanticFunctionDeclV1,
    functions: &'a [SemanticFunctionDeclV1],
    callables: &'a [SemanticCallableDeclV1],
    templates: &'t [Option<Template>],
    output: Template,
    locals: Vec<Slot>,
    live: Vec<bool>,
    meter: &'m mut dyn Meter,
}

impl Frame<'_, '_, '_> {
    fn read(&mut self, place: &SemanticPlaceV1) -> Result<Value, Error> {
        self.meter.work(4)?;
        let index = place.local().index() as usize;
        let declaration = self
            .function
            .locals()
            .get(index)
            .ok_or("helper local outside function")?;
        if self.live.get(index).copied() != Some(true) {
            return Err("helper use outside storage lifetime");
        }
        let value = match (self.locals.get(index), place.projections()) {
            (Some(Slot::Scalar(value)), []) if place.ty() == declaration.ty() => *value,
            (Some(Slot::CheckedValue(value)), [projection])
                if matches!(projection.kind(), SemanticProjectionKindV1::Field(0))
                    && checked_type(self.types, declaration.ty()) == Some(place.ty())
                    && projection.result_type() == place.ty() =>
            {
                *value
            }
            _ => return Err("helper value is uninitialized, projected, or an overflow flag"),
        };
        if self.output.scalar(value)? != scalar(self.types, place.ty())? {
            return Err("helper place scalar mismatch");
        }
        Ok(value)
    }

    fn operand(&mut self, operand: &SemanticOperandV1) -> Result<Value, Error> {
        self.meter.work(1)?;
        match operand {
            SemanticOperandV1::Constant(constant) => {
                let SemanticConstantValueV1::Scalar(value) = constant.value() else {
                    return Err("non-scalar helper constant");
                };
                let bits = u64::try_from(value.bits())
                    .map_err(|_| "helper constant exceeds scalar width")?;
                self.output.push(
                    scalar(self.types, constant.ty())?,
                    Kind::Constant(bits),
                    self.meter,
                )
            }
            SemanticOperandV1::Copy(place) => self.read(place),
            SemanticOperandV1::Move(place) => {
                // Primitive values are Copy at the Rust type level, but retain
                // exact MIR move/init state instead of silently reading again.
                let value = self.read(place)?;
                self.locals[place.local().index() as usize] = Slot::Uninitialized;
                Ok(value)
            }
        }
    }

    fn write(&mut self, place: &SemanticPlaceV1, value: Slot) -> Result<(), Error> {
        self.meter.work(4)?;
        let index = place.local().index() as usize;
        let local = self
            .function
            .locals()
            .get(index)
            .ok_or("helper destination outside function")?;
        if !place.projections().is_empty()
            || place.ty() != local.ty()
            || self.live.get(index).copied() != Some(true)
        {
            return Err("helper destination is projected, mistyped or outside lifetime");
        }
        let expected = match value {
            Slot::Scalar(_) => scalar(self.types, place.ty())?,
            Slot::CheckedValue(_) => scalar(
                self.types,
                checked_type(self.types, place.ty()).ok_or("helper checked tuple type mismatch")?,
            )?,
            Slot::Uninitialized => return Err("missing helper destination value"),
        };
        let (Slot::Scalar(value_id) | Slot::CheckedValue(value_id)) = value else {
            unreachable!()
        };
        if self.output.scalar(value_id)? != expected {
            return Err("helper assignment changes scalar type");
        }
        self.locals[index] = value;
        Ok(())
    }

    fn rvalue(&mut self, value: &SemanticRvalueV1) -> Result<Slot, Error> {
        self.meter.work(3)?;
        if let SemanticRvalueKindV1::CheckedBinary(binary) = value.kind() {
            let ty = checked_type(self.types, value.result_type())
                .ok_or("invalid helper checked tuple")?;
            if binary.left().ty() != ty || binary.right().ty() != ty {
                return Err("helper checked operand type mismatch");
            }
            let left = self.operand(binary.left())?;
            let right = self.operand(binary.right())?;
            let operation = match binary.operation() {
                SemanticCheckedBinaryOpV1::Add => ProductionSemanticBinaryOpV2::Add,
                SemanticCheckedBinaryOpV1::Subtract => ProductionSemanticBinaryOpV2::Subtract,
                SemanticCheckedBinaryOpV1::Multiply => ProductionSemanticBinaryOpV2::Multiply,
            };
            let result = self.output.push(
                scalar(self.types, ty)?,
                Kind::Binary(
                    operation,
                    ProductionOverflowContractV2::Wrapping,
                    left,
                    right,
                ),
                self.meter,
            )?;
            return Ok(Slot::CheckedValue(result));
        }
        let result_scalar = scalar(self.types, value.result_type())?;
        let result = match value.kind() {
            SemanticRvalueKindV1::Use(operand) if operand.ty() == value.result_type() => {
                self.operand(operand)?
            }
            SemanticRvalueKindV1::Unary { operation, operand }
                if operand.ty() == value.result_type() =>
            {
                let operation = match (operation, result_scalar) {
                    (SemanticUnaryOpV1::Not, Scalar::Bool | Scalar::Integer { .. }) => {
                        ProductionSemanticUnaryOpV2::Not
                    }
                    (SemanticUnaryOpV1::Negate, Scalar::Float { bits: 32 }) => {
                        ProductionSemanticUnaryOpV2::Negate
                    }
                    _ => return Err("helper unary operation has no closed total scalar rule"),
                };
                let operand = self.operand(operand)?;
                self.output
                    .push(result_scalar, Kind::Unary(operation, operand), self.meter)?
            }
            SemanticRvalueKindV1::Binary {
                operation,
                left,
                right,
            } if matches!(
                operation,
                SemanticBinaryOpV1::ShiftLeft | SemanticBinaryOpV1::ShiftRight
            ) =>
            {
                self.meter.work(8)?;
                if left.ty() != value.result_type()
                    || !source_helper_constant_shift_v1(self.types, right, result_scalar)
                {
                    return Err("helper shift requires an exact in-range integer literal");
                }
                let lhs = self.operand(left)?;
                let rhs = self.operand(right)?;
                let operation = match operation {
                    SemanticBinaryOpV1::ShiftLeft => ProductionSemanticBinaryOpV2::ShiftLeft,
                    SemanticBinaryOpV1::ShiftRight => ProductionSemanticBinaryOpV2::ShiftRight,
                    _ => unreachable!(),
                };
                self.output.push(
                    result_scalar,
                    Kind::Binary(operation, ProductionOverflowContractV2::Wrapping, lhs, rhs),
                    self.meter,
                )?
            }
            SemanticRvalueKindV1::Binary {
                operation,
                left,
                right,
            } => {
                if left.ty() != right.ty() {
                    return Err("helper binary operand type mismatch");
                }
                let input = scalar(self.types, left.ty())?;
                let lhs = self.operand(left)?;
                let rhs = self.operand(right)?;
                let comparison = match operation {
                    SemanticBinaryOpV1::Equal => Some(ProductionSemanticComparisonV2::Equal),
                    SemanticBinaryOpV1::NotEqual => Some(ProductionSemanticComparisonV2::NotEqual),
                    SemanticBinaryOpV1::LessThan => Some(ProductionSemanticComparisonV2::LessThan),
                    SemanticBinaryOpV1::LessOrEqual => {
                        Some(ProductionSemanticComparisonV2::LessOrEqual)
                    }
                    SemanticBinaryOpV1::GreaterThan => {
                        Some(ProductionSemanticComparisonV2::GreaterThan)
                    }
                    SemanticBinaryOpV1::GreaterOrEqual => {
                        Some(ProductionSemanticComparisonV2::GreaterOrEqual)
                    }
                    _ => None,
                };
                if let Some(operation) = comparison {
                    if result_scalar != Scalar::Bool {
                        return Err("helper comparison result is not bool");
                    }
                    self.output.push(
                        result_scalar,
                        Kind::Compare(operation, input, lhs, rhs),
                        self.meter,
                    )?
                } else {
                    if result_scalar != input {
                        return Err("helper binary result type mismatch");
                    }
                    let operation = match (operation, input) {
                        (
                            SemanticBinaryOpV1::Add,
                            Scalar::Integer { .. } | Scalar::Float { .. },
                        ) => ProductionSemanticBinaryOpV2::Add,
                        (
                            SemanticBinaryOpV1::Subtract,
                            Scalar::Integer { .. } | Scalar::Float { .. },
                        ) => ProductionSemanticBinaryOpV2::Subtract,
                        (
                            SemanticBinaryOpV1::Multiply,
                            Scalar::Integer { .. } | Scalar::Float { .. },
                        ) => ProductionSemanticBinaryOpV2::Multiply,
                        (SemanticBinaryOpV1::Divide, Scalar::Float { bits: 32 }) => {
                            ProductionSemanticBinaryOpV2::Divide
                        }
                        (SemanticBinaryOpV1::BitAnd, Scalar::Bool | Scalar::Integer { .. }) => {
                            ProductionSemanticBinaryOpV2::BitAnd
                        }
                        (SemanticBinaryOpV1::BitOr, Scalar::Bool | Scalar::Integer { .. }) => {
                            ProductionSemanticBinaryOpV2::BitOr
                        }
                        (SemanticBinaryOpV1::BitXor, Scalar::Bool | Scalar::Integer { .. }) => {
                            ProductionSemanticBinaryOpV2::BitXor
                        }
                        _ => return Err("helper binary operation has no closed total scalar rule"),
                    };
                    self.output.push(
                        result_scalar,
                        Kind::Binary(operation, ProductionOverflowContractV2::Wrapping, lhs, rhs),
                        self.meter,
                    )?
                }
            }
            SemanticRvalueKindV1::Cast {
                kind: SemanticCastKindV1::Integer,
                operand,
            } if result_scalar.is_integer() && scalar(self.types, operand.ty())?.is_integer() => {
                let source = scalar(self.types, operand.ty())?;
                let operand = self.operand(operand)?;
                if source == result_scalar {
                    operand
                } else {
                    self.output.push(
                        result_scalar,
                        Kind::Cast(ProductionSemanticCastV2::Integer, source, operand),
                        self.meter,
                    )?
                }
            }
            _ => return Err("helper rvalue has memory, unchecked or unsupported scalar semantics"),
        };
        Ok(Slot::Scalar(result))
    }

    fn call(&mut self, call: &SemanticDirectCallV1) -> Result<SemanticBlockIdV1, Error> {
        self.meter.work(6)?;
        let destination = call
            .destination()
            .ok_or("helper value call has no return")?;
        if !call.variadic_argument_abis().is_empty()
            || !matches!(
                call.unwind(),
                SemanticUnwindActionV1::Continue | SemanticUnwindActionV1::Unreachable
            )
            || destination.edge().role() != SemanticEdgeRoleV1::CallReturn
        {
            return Err("helper value call has exceptional or variadic transport");
        }
        let declaration = self
            .callables
            .get(call.callee().index() as usize)
            .ok_or("missing helper callable")?;
        let (signature, template, saturation) = match declaration {
            SemanticCallableDeclV1::Defined { function } => {
                let index = function.index() as usize;
                (
                    self.functions
                        .get(index)
                        .ok_or("missing helper function")?
                        .abi(),
                    Some(
                        self.templates
                            .get(index)
                            .and_then(Option::as_ref)
                            .ok_or("unresolved or recursive helper return")?,
                    ),
                    None,
                )
            }
            SemanticCallableDeclV1::CompilerIntrinsic {
                binding,
                operation: SemanticCompilerIntrinsicOperationV1::SaturatingInteger(operation),
                ..
            } => (binding.abi(), None, Some(*operation)),
            _ => return Err("helper has an unknown or non-value call"),
        };
        abi(self.types, signature, self.meter)?;
        if call.arguments().len() != signature.source_input_types().len()
            || destination.place().ty() != signature.source_output_type()
        {
            return Err("helper call result or arity mismatch");
        }
        let (mut arguments, storage) = vector(call.arguments().len(), self.meter)?;
        let result = (|| {
            // Evaluate operands in source order before assigning the destination.
            for (operand, expected) in call.arguments().iter().zip(signature.source_input_types()) {
                if operand.ty() != *expected {
                    return Err("helper call argument type mismatch");
                }
                arguments.push(self.operand(operand)?);
            }
            let value = if let Some(template) = template {
                self.output.append_call(template, &arguments, self.meter)?
            } else {
                let operation = saturation.ok_or("unresolved helper call")?;
                self.saturating(
                    operation,
                    scalar(self.types, signature.source_output_type())?,
                    &arguments,
                )?
            };
            self.write(destination.place(), Slot::Scalar(value))?;
            Ok(destination.edge().target())
        })();
        drop(arguments);
        self.meter.release(storage)?;
        result
    }

    fn saturating(
        &mut self,
        operation: SemanticSaturatingIntegerOpV1,
        scalar: Scalar,
        arguments: &[Value],
    ) -> Result<Value, Error> {
        let [left, right] = arguments else {
            return Err("saturation arity mismatch");
        };
        let Scalar::Integer { signed, bits } = scalar else {
            return Err("saturation scalar mismatch");
        };
        if self.output.scalar(*left)? != scalar || self.output.scalar(*right)? != scalar {
            return Err("saturation argument mismatch");
        }
        let binary = |this: &mut Self, operation, left, right| {
            this.output.push(
                scalar,
                Kind::Binary(
                    operation,
                    ProductionOverflowContractV2::Wrapping,
                    left,
                    right,
                ),
                this.meter,
            )
        };
        let raw = binary(
            self,
            match operation {
                SemanticSaturatingIntegerOpV1::Add => ProductionSemanticBinaryOpV2::Add,
                SemanticSaturatingIntegerOpV1::Subtract => ProductionSemanticBinaryOpV2::Subtract,
            },
            *left,
            *right,
        )?;
        let less = |this: &mut Self, left, right| {
            this.output.push(
                Scalar::Bool,
                Kind::Compare(
                    ProductionSemanticComparisonV2::LessThan,
                    scalar,
                    left,
                    right,
                ),
                this.meter,
            )
        };
        let (condition, clamp) = if signed {
            let first = binary(self, ProductionSemanticBinaryOpV2::BitXor, *left, raw)?;
            let second = match operation {
                SemanticSaturatingIntegerOpV1::Add => {
                    binary(self, ProductionSemanticBinaryOpV2::BitXor, *right, raw)?
                }
                SemanticSaturatingIntegerOpV1::Subtract => {
                    binary(self, ProductionSemanticBinaryOpV2::BitXor, *left, *right)?
                }
            };
            let changed = binary(self, ProductionSemanticBinaryOpV2::BitAnd, first, second)?;
            let zero = self.output.push(scalar, Kind::Constant(0), self.meter)?;
            let overflow = less(self, changed, zero)?;
            let negative = less(self, *left, zero)?;
            let minimum =
                self.output
                    .push(scalar, Kind::Constant(1u64 << (bits - 1)), self.meter)?;
            let maximum =
                self.output
                    .push(scalar, Kind::Constant((1u64 << (bits - 1)) - 1), self.meter)?;
            let clamp =
                self.output
                    .push(scalar, Kind::Select(negative, minimum, maximum), self.meter)?;
            (overflow, clamp)
        } else {
            match operation {
                SemanticSaturatingIntegerOpV1::Add => (
                    less(self, raw, *left)?,
                    self.output.push(
                        scalar,
                        Kind::Constant(((1u128 << bits) - 1) as u64),
                        self.meter,
                    )?,
                ),
                SemanticSaturatingIntegerOpV1::Subtract => (
                    less(self, *left, *right)?,
                    self.output.push(scalar, Kind::Constant(0), self.meter)?,
                ),
            }
        };
        self.output
            .push(scalar, Kind::Select(condition, clamp, raw), self.meter)
    }
}

pub(super) fn derive(
    semantic: &AdmittedInertSemanticMirV1,
    function_index: usize,
    templates: &[Option<Template>],
    meter: &mut dyn Meter,
) -> Result<Template, Error> {
    let function = semantic
        .functions()
        .get(function_index)
        .ok_or("helper function outside owner")?;
    meter.work(5)?;
    if function.role() != SemanticFunctionRoleV1::InternalHelper || function.export().is_some() {
        return Err("helper value target is not a private defined function");
    }
    abi(semantic.types(), function.abi(), meter)?;
    let (mut parameters, parameter_storage) =
        vector(function.abi().source_input_types().len(), meter)?;
    let output = (|| {
        for ty in function.abi().source_input_types() {
            parameters.push(scalar(semantic.types(), *ty)?);
        }
        Template::new(&parameters, meter)
    })();
    drop(parameters);
    meter.release(parameter_storage)?;
    let mut output = output?;
    let result = derive_inner(semantic, function, templates, &mut output, meter);
    if let Err(error) = result {
        output.destroy(meter)?;
        return Err(error);
    }
    Ok(output)
}

fn derive_inner(
    semantic: &AdmittedInertSemanticMirV1,
    function: &SemanticFunctionDeclV1,
    templates: &[Option<Template>],
    output: &mut Template,
    meter: &mut dyn Meter,
) -> Result<(), Error> {
    let (mut locals, local_storage) = vector(function.locals().len(), meter)?;
    locals.resize(function.locals().len(), Slot::Uninitialized);
    let (mut live, live_storage) = match vector(function.locals().len(), meter) {
        Ok(result) => result,
        Err(error) => {
            drop(locals);
            meter.release(local_storage)?;
            return Err(error);
        }
    };
    live.resize(function.locals().len(), true);
    let (mut seen, seen_storage) = match vector(function.blocks().len(), meter) {
        Ok(result) => result,
        Err(error) => {
            drop(locals);
            drop(live);
            meter.release(local_storage)?;
            meter.release(live_storage)?;
            return Err(error);
        }
    };
    seen.resize(function.blocks().len(), false);
    // Placeholder ownership swap only; its empty vectors allocate nothing.
    let empty = match Template::new(&[], meter) {
        Ok(result) => result,
        Err(error) => {
            drop(locals);
            drop(live);
            drop(seen);
            meter.release(local_storage)?;
            meter.release(live_storage)?;
            meter.release(seen_storage)?;
            return Err(error);
        }
    };
    let owned = std::mem::replace(output, empty);
    let mut frame = Frame {
        types: semantic.types(),
        function,
        functions: semantic.functions(),
        callables: semantic.callables(),
        templates,
        output: owned,
        locals,
        live,
        meter,
    };
    let result = (|| {
        let mut returned = None;
        let mut arguments = 0usize;
        for (index, local) in function.locals().iter().enumerate() {
            frame.meter.work(3)?;
            match local.role() {
                SemanticLocalRoleV1::Argument(argument) => {
                    if function
                        .abi()
                        .source_input_types()
                        .get(argument as usize)
                        .copied()
                        != Some(local.ty())
                    {
                        return Err("helper argument local does not match ABI");
                    }
                    arguments = arguments
                        .checked_add(1)
                        .ok_or("helper argument count overflow")?;
                    let value = frame.output.push(
                        scalar(frame.types, local.ty())?,
                        Kind::Parameter(argument as usize),
                        frame.meter,
                    )?;
                    frame.locals[index] = Slot::Scalar(value);
                }
                SemanticLocalRoleV1::Return => {
                    let previous_return = returned.replace(index);
                    if previous_return.is_some()
                        || local.ty() != function.abi().source_output_type()
                    {
                        return Err("helper return local is ambiguous or mistyped");
                    }
                }
                _ => {}
            }
            if scalar(frame.types, local.ty()).is_err()
                && checked_type(frame.types, local.ty()).is_none()
            {
                return Err("helper local has pointer, aggregate or unsupported layout");
            }
        }
        if arguments != function.abi().source_input_types().len() {
            return Err("helper argument local roster is incomplete");
        }
        let returned = returned.ok_or("missing helper return local")?;
        // Presence of StorageLive means the slot is not live at entry. A
        // complete prepass prevents an early use from inheriting default-live.
        for block in function.blocks() {
            frame.meter.work(1)?;
            for statement in block.statements() {
                frame.meter.work(1)?;
                if let SemanticStatementKindV1::StorageLive(local) = statement.kind() {
                    let index = local.index() as usize;
                    if !matches!(
                        function.locals().get(index).map(|l| l.role()),
                        Some(SemanticLocalRoleV1::Temporary)
                    ) {
                        return Err("helper StorageLive rewrites an entry or return slot");
                    }
                    *frame
                        .live
                        .get_mut(index)
                        .ok_or("helper lifetime local outside function")? = false;
                }
            }
        }
        let mut block = function.entry().index() as usize;
        let mut visited = 0;
        loop {
            frame.meter.work(4)?;
            if std::mem::replace(
                seen.get_mut(block).ok_or("helper edge outside function")?,
                true,
            ) {
                return Err("helper control flow is cyclic");
            }
            visited += 1;
            let body = &function.blocks()[block];
            for statement in body.statements() {
                frame.meter.work(2)?;
                match statement.kind() {
                    SemanticStatementKindV1::Assign(assignment) => {
                        let value = frame.rvalue(assignment.value())?;
                        if assignment.destination().ty() != assignment.value().result_type() {
                            return Err("helper assignment type mismatch");
                        }
                        frame.write(assignment.destination(), value)?;
                    }
                    SemanticStatementKindV1::StorageLive(local)
                    | SemanticStatementKindV1::StorageDead(local) => {
                        let index = local.index() as usize;
                        *frame
                            .locals
                            .get_mut(index)
                            .ok_or("helper lifetime local outside function")? = Slot::Uninitialized;
                        frame.live[index] =
                            matches!(statement.kind(), SemanticStatementKindV1::StorageLive(_));
                    }
                    SemanticStatementKindV1::Nop => {}
                    _ => {
                        return Err(
                            "helper contains memory, assumption or unsupported statement effects",
                        );
                    }
                }
            }
            match body.terminator().kind() {
                SemanticTerminatorKindV1::Goto(edge) if edge.role() == SemanticEdgeRoleV1::Goto => {
                    block = edge.target().index() as usize
                }
                SemanticTerminatorKindV1::Call(call) => block = frame.call(call)?.index() as usize,
                SemanticTerminatorKindV1::Return => {
                    if visited != function.blocks().len() {
                        return Err("helper has unvisited or extra return blocks");
                    }
                    let Some(Slot::Scalar(value)) = frame.locals.get(returned).copied() else {
                        return Err("helper return is uninitialized or non-scalar");
                    };
                    if !frame.live[returned] {
                        return Err("helper return is outside storage lifetime");
                    }
                    frame.output.finish(
                        value,
                        scalar(frame.types, function.abi().source_output_type())?,
                    )?;
                    return Ok(());
                }
                _ => {
                    return Err(
                        "helper control has a branch, assertion, unwind or non-returning exit",
                    );
                }
            }
        }
    })();
    let Frame {
        output: owned,
        locals,
        live,
        meter,
        ..
    } = frame;
    *output = owned;
    drop(locals);
    drop(live);
    drop(seen);
    meter.release(local_storage)?;
    meter.release(live_storage)?;
    meter.release(seen_storage)?;
    result
}

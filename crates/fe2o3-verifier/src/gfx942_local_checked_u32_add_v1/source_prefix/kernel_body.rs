macro_rules! checked_u32_prefix_kernel_constant_body_v1 {
    ($exec:ident, $operation:ident) => {
        $exec!({
            let OperationKind::Constant(Constant::U32(value)) = $operation.kind else {
                return None;
            };
            if $operation.results.len() != 1 {
                return None;
            }
            let result = &$operation.results[0];
            if !matches!(result.ty, Type::Scalar(ScalarType::U32)) {
                return None;
            }
            Some((result.id.0, value))
        })
    };
}

macro_rules! checked_u32_prefix_kernel_terminal_body_v1 {
    ($exec:ident, $terminal:ident, $previous:ident, $origins:ident,
     $operand:ident, $value:ident, $overflow:ident, $literal:ident) => {
        $exec!({
            let OperationKind::Binary {
                op: BinaryOp::Checked(CheckedBinaryOperator::Add),
                lhs,
                rhs,
            } = $terminal.kind
            else {
                return Err(CheckedU32PrefixErrorV1::Kernel);
            };
            if $terminal.results.len() != 2 {
                return Err(CheckedU32PrefixErrorV1::Kernel);
            }
            let value = &$terminal.results[0];
            let overflow = &$terminal.results[1];
            if lhs.0 != $operand
                || value.id.0 != $value
                || overflow.id.0 != $overflow
                || value.id.0 == overflow.id.0
                || $origins.contains_key(&value.id.0)
                || $origins.contains_key(&overflow.id.0)
                || !matches!(value.ty, Type::Scalar(ScalarType::U32))
                || !matches!(overflow.ty, Type::Scalar(ScalarType::Bool))
                || constant_binding($previous) != Some((rhs.0, $literal))
            {
                return Err(CheckedU32PrefixErrorV1::Kernel);
            }
            match $origins.get(&rhs.0) {
                Some(Origin::Constant(value)) if *value == $literal => {}
                _ => return Err(CheckedU32PrefixErrorV1::Kernel),
            }
            match $origins.get(&lhs.0) {
                Some(origin) => Ok(*origin),
                None => Err(CheckedU32PrefixErrorV1::ValueMismatch),
            }
        })
    };
}

macro_rules! checked_u32_prefix_kernel_assemble_body_v1 {
    ($exec:ident, $arguments:ident, $operations:ident,
     $operand:ident, $value:ident, $overflow:ident, $literal:ident,
     $origins:ident, $argument:ident, [$($argument_invariants:tt)*], [$($argument_proof:tt)*],
     $index:ident, [$($constant_invariants:tt)*], [$($constant_proof:tt)*]) => {
        $exec!({
            if $arguments.len() > 128 || $operations.len() < 2 || $operations.len() > 257 {
                return Err(CheckedU32PrefixErrorV1::Kernel);
            }
            let mut $origins: BTreeMap<u32, Origin> = BTreeMap::new();
            let mut $argument = 0usize;
            while $argument < $arguments.len()
                $($argument_invariants)*
            {
                $($argument_proof)*
                let binding = &$arguments[$argument];
                if binding.argument != $argument
                    || $origins.insert(binding.kernel_ir_value.0, Origin::Argument($argument)).is_some()
                {
                    return Err(CheckedU32PrefixErrorV1::Kernel);
                }
                $argument += 1;
            }
            let mut $index = 0usize;
            while $index < $operations.len() - 1
                $($constant_invariants)*
            {
                $($constant_proof)*
                let Some((id, value)) = constant_binding(&$operations[$index]) else {
                    return Err(CheckedU32PrefixErrorV1::Kernel);
                };
                if $origins.insert(id, Origin::Constant(value)).is_some() {
                    return Err(CheckedU32PrefixErrorV1::Kernel);
                }
                $index += 1;
            }
            terminal_origin(&$operations[$operations.len() - 1], &$operations[$operations.len() - 2],
                &$origins, $operand, $value, $overflow, $literal)
        })
    };
}

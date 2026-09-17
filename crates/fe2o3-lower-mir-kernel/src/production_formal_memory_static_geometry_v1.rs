//! Independent, bounded KIR geometry replay for the closed publication protocol.
//! Unknown values add possible paths. They never stand in for an acquired READY.

use std::collections::{HashMap, VecDeque};
use fe2o3_kernel_ir as kir;

const MAX_BLOCKS: usize = 1024;
const MAX_OPERATIONS: usize = 8192;
const MAX_VALUES: usize = 16384;
const MAX_REPLAY_WORK: usize = 1_048_576;

fn charge(work: &mut usize, amount: usize) -> Result<(), &'static str> {
    *work = work.checked_add(amount).ok_or("geometry replay work overflow")?;
    if *work > MAX_REPLAY_WORK { return Err("geometry replay work limit"); }
    Ok(())
}

/// The caller must bind this KIR entry to the retained source launch's maximum
/// grid of two complete WG128 groups; dynamic KIR extent alone is not a bound.
pub(super) fn prove_geometry(
    kernel: &kir::Kernel,
    function: &kir::Function,
    producer: kir::FunctionOperationLocation,
    consumer: kir::FunctionOperationLocation,
    producer_index: kir::ValueId,
    consumer_index: kir::ValueId,
) -> Result<(), &'static str> {
    let body = function.body.as_ref().ok_or("publication entry has no body")?;
    if kernel.workgroup_size != Some(kir::WorkgroupSize::new(128, 1, 1))
        || body.blocks.is_empty() || body.blocks.len() > MAX_BLOCKS
        || !matches!(kernel.domain, kir::LaunchDomain::D1 {
            x: kir::LaunchExtent::Dynamic | kir::LaunchExtent::Static(128 | 256)
        })
    { return Err("publication requires bounded one-dimensional WG128 geometry"); }
    if body.parameters.len() > MAX_VALUES || body.parameters.len() != function.signature.parameters.len() {
        return Err("parameter limit or arity mismatch");
    }
    let mut operation_count = 0_usize;
    let mut types = HashMap::new();
    for (value, ty) in body.parameters.iter().zip(&function.signature.parameters) {
        if types.insert(*value, ty).is_some() { return Err("duplicate parameter identity"); }
    }
    for block in &body.blocks {
        operation_count = operation_count.checked_add(block.operations.len()).ok_or("operation limit")?;
        if operation_count > MAX_OPERATIONS { return Err("operation limit"); }
        for definition in block.parameters.iter().chain(block.operations.iter().flat_map(|op| &op.results)) {
            if types.len() == MAX_VALUES { return Err("value limit"); }
            if types.insert(definition.id, &definition.ty).is_some() { return Err("duplicate SSA value"); }
        }
    }
    let blocks = body.blocks.iter().enumerate().map(|(index, block)| (block.id, index)).collect::<HashMap<_,_>>();
    if blocks.len() != body.blocks.len() { return Err("duplicate block identity"); }
    let producer_block = *blocks.get(&producer.block).ok_or("producer block missing")?;
    let consumer_block = *blocks.get(&consumer.block).ok_or("consumer block missing")?;
    if body.blocks[producer_block].operations.get(producer.operation_index).is_none()
        || body.blocks[consumer_block].operations.get(consumer.operation_index).is_none()
    { return Err("publication effect location is missing"); }
    if producer_block == consumer_block { return Err("publication roles share a block"); }
    let mut successors = Vec::with_capacity(body.blocks.len());
    let mut indegrees = vec![0_usize; body.blocks.len()];
    let mut edge_arguments = 0_usize;
    for block in &body.blocks {
        let terminator = block.terminator.as_ref().ok_or("unterminated block")?;
        let (successor_count, argument_count) = edge_counts(terminator)?;
        if successor_count > 16 { return Err("successor limit"); }
        edge_arguments = edge_arguments.checked_add(argument_count).ok_or("edge argument overflow")?;
        if edge_arguments > MAX_VALUES { return Err("edge argument limit"); }
        let targets = terminator.successors();
        if targets.len() > 16 { return Err("successor limit"); }
        let mut resolved = Vec::with_capacity(targets.len());
        for target in targets {
            let index = *blocks.get(&target).ok_or("unknown successor")?;
            indegrees[index] += 1;
            resolved.push(index);
        }
        successors.push(resolved);
    }
    let mut queue = (0..body.blocks.len()).filter(|block| indegrees[*block] == 0).collect::<VecDeque<_>>();
    let mut order = Vec::with_capacity(body.blocks.len());
    while let Some(block) = queue.pop_front() {
        order.push(block);
        for target in &successors[block] {
            indegrees[*target] -= 1;
            if indegrees[*target] == 0 { queue.push_back(*target); }
        }
    }
    if order.len() != body.blocks.len() { return Err("publication entry contains a cycle"); }
    // The authenticated source launch permits one or two complete WG128 groups.
    // Replay both sizes; smaller launches need only be safe, not make progress.
    let mut replay_work = 0_usize;
    for extent in [128_u64, 256] {
        for global in 0..extent {
            let mut values = HashMap::new();
            let mut reachable = vec![false; body.blocks.len()];
            reachable[0] = true;
            for block_index in order.iter().copied() {
                charge(&mut replay_work, 1)?;
                if !reachable[block_index] { continue; }
                let block = &body.blocks[block_index];
                for (operation_index, operation) in block.operations.iter().enumerate() {
                    charge(&mut replay_work, 1)?;
                    if (block_index == producer_block && operation_index == producer.operation_index)
                        || (block_index == consumer_block && operation_index == consumer.operation_index)
                    {
                        let publishing = block_index == producer_block;
                        let index = if publishing { producer_index } else { consumer_index };
                        if publishing != (global < 128) || values.get(&index) != Some(&(global % 128)) {
                            return Err("KIR publication role or cell is not exact");
                        }
                    }
                    evaluate_operation(operation, &types, &mut values, global, extent);
                }
                let edges = possible_edges(block.terminator.as_ref().ok_or("unterminated block")?, &values)?;
                for (target, arguments) in edges {
                    charge(&mut replay_work, arguments.len().saturating_add(1))?;
                    let target = *blocks.get(&target).ok_or("unknown edge")?;
                    let parameters = &body.blocks[target].parameters;
                    if arguments.len() != parameters.len() { return Err("edge arity changed"); }
                    let incoming = arguments.iter().map(|value| values.get(value).copied()).collect::<Vec<_>>();
                    for (parameter, incoming) in parameters.iter().zip(incoming) {
                        let merged = if reachable[target] {
                            values.get(&parameter.id).copied().filter(|old| Some(*old) == incoming)
                        } else { incoming };
                        match merged {
                            Some(value) => { values.insert(parameter.id, value); }
                            None => { values.remove(&parameter.id); }
                        }
                    }
                    reachable[target] = true;
                }
            }
            if reachable[producer_block] && global >= 128 || reachable[consumer_block] && global < 128 {
                return Err("KIR permits a wrong-role publication");
            }
            if extent == 256 && (reachable[producer_block] != (global < 128)
                || reachable[consumer_block] != (global >= 128))
            { return Err("KIR full-domain publication coverage is incomplete"); }
        }
    }
    Ok(())
}

fn unsigned_width(ty: &kir::Type) -> Option<u32> {
    match ty {
        kir::Type::Scalar(kir::ScalarType::Bool) => Some(1),
        kir::Type::Scalar(kir::ScalarType::U8) => Some(8),
        kir::Type::Scalar(kir::ScalarType::U16) => Some(16),
        kir::Type::Scalar(kir::ScalarType::U32) => Some(32),
        kir::Type::Scalar(kir::ScalarType::U64 | kir::ScalarType::Index) => Some(64),
        _ => None,
    }
}

fn mask(width: u32) -> u64 { u64::MAX >> (64 - width) }

fn constant(value: &kir::Constant) -> Option<u64> {
    match value {
        kir::Constant::Bool(value) => Some(u64::from(*value)),
        kir::Constant::U8(value) => Some(u64::from(*value)),
        kir::Constant::U16(value) => Some(u64::from(*value)),
        kir::Constant::U32(value) => Some(u64::from(*value)),
        kir::Constant::U64(value) | kir::Constant::Index(value) => Some(*value),
        _ => None,
    }
}

fn evaluate_operation(
    operation: &kir::Operation,
    types: &HashMap<kir::ValueId, &kir::Type>,
    values: &mut HashMap<kir::ValueId, u64>,
    global: u64,
    extent: u64,
) {
    let [result] = operation.results.as_slice() else { return; };
    let Some(width) = unsigned_width(&result.ty) else { return; };
    let get = |value: &kir::ValueId| values.get(value).copied();
    let value = match &operation.kind {
        kir::OperationKind::Constant(value) => constant(value),
        kir::OperationKind::Intrinsic(intrinsic) => {
            let x = intrinsic.kind.axis() == kir::Axis::X;
            match intrinsic.kind {
                kir::IntrinsicKind::LaunchExtent { .. } => Some(if x { extent } else { 1 }),
                kir::IntrinsicKind::InvocationIndex { kind, .. } => Some(match kind {
                    kir::IndexKind::Global => if x { global } else { 0 },
                    kir::IndexKind::Local => if x { global % 128 } else { 0 },
                    kir::IndexKind::Workgroup => if x { global / 128 } else { 0 },
                    kir::IndexKind::WorkgroupSize => if x { 128 } else { 1 },
                    kir::IndexKind::WorkgroupCount => if x { extent / 128 } else { 1 },
                }),
            }
        }
        kir::OperationKind::Cast { kind: kir::CastKind::ZeroExtend | kir::CastKind::Truncate | kir::CastKind::Bitcast, value, .. }
            if types.get(value).and_then(|ty| unsigned_width(ty)).is_some() => get(value),
        kir::OperationKind::Binary { op, lhs, rhs } => get(lhs).zip(get(rhs)).and_then(|(a,b)| {
            if types.get(lhs).and_then(|ty| unsigned_width(ty)).is_none()
                || types.get(rhs).and_then(|ty| unsigned_width(ty)).is_none() { return None; }
            match op {
                kir::BinaryOp::Add => Some(a.wrapping_add(b)),
                kir::BinaryOp::Subtract => Some(a.wrapping_sub(b)),
                kir::BinaryOp::Multiply => Some(a.wrapping_mul(b)),
                kir::BinaryOp::Divide => a.checked_div(b),
                kir::BinaryOp::Remainder => a.checked_rem(b),
                kir::BinaryOp::BitAnd => Some(a & b),
                kir::BinaryOp::BitOr => Some(a | b),
                kir::BinaryOp::BitXor => Some(a ^ b),
                kir::BinaryOp::ShiftLeft if b < u64::from(width) => Some(a << b),
                kir::BinaryOp::ShiftRight if b < u64::from(width) => Some(a >> b),
                _ => None,
            }
        }),
        kir::OperationKind::Compare { predicate, lhs, rhs } => get(lhs).zip(get(rhs)).and_then(|(a,b)| {
            if types.get(lhs).and_then(|ty| unsigned_width(ty)).is_none()
                || types.get(rhs).and_then(|ty| unsigned_width(ty)).is_none() { return None; }
            Some(u64::from(match predicate {
                kir::ComparePredicate::Equal => a == b,
                kir::ComparePredicate::NotEqual => a != b,
                kir::ComparePredicate::LessThan => a < b,
                kir::ComparePredicate::LessThanOrEqual => a <= b,
                kir::ComparePredicate::GreaterThan => a > b,
                kir::ComparePredicate::GreaterThanOrEqual => a >= b,
            }))
        }),
        kir::OperationKind::Unary { op: kir::UnaryOp::Not, operand } => get(operand).map(|value| !value),
        kir::OperationKind::Select { condition, true_value, false_value } => match get(condition) {
            Some(0) => get(false_value),
            Some(1) => get(true_value),
            _ => get(true_value).filter(|value| Some(*value) == get(false_value)),
        },
        _ => None,
    };
    if let Some(value) = value { values.insert(result.id, value & mask(width)); }
}

fn edge_counts(terminator: &kir::Terminator) -> Result<(usize, usize), &'static str> {
    let add = |a: usize, b: usize| a.checked_add(b).ok_or("edge argument overflow");
    match terminator {
        kir::Terminator::Branch { arguments, .. } => Ok((1, arguments.len())),
        kir::Terminator::ConditionalBranch { then_arguments, else_arguments, .. } =>
            Ok((2, add(then_arguments.len(), else_arguments.len())?)),
        kir::Terminator::Switch { cases, default_arguments, .. } => {
            if cases.len() >= 16 { return Err("successor limit"); }
            let count = cases.iter().try_fold(default_arguments.len(), |sum, case| add(sum, case.arguments.len()))?;
            Ok((cases.len() + 1, count))
        }
        kir::Terminator::IntegerSwitch { cases, default_arguments, .. } => {
            if cases.len() >= 16 { return Err("successor limit"); }
            let count = cases.iter().try_fold(default_arguments.len(), |sum, case| add(sum, case.arguments.len()))?;
            Ok((cases.len() + 1, count))
        }
        kir::Terminator::Return { .. } | kir::Terminator::Unreachable => Ok((0, 0)),
    }
}

fn possible_edges<'a>(
    terminator: &'a kir::Terminator,
    values: &HashMap<kir::ValueId, u64>,
) -> Result<Vec<(kir::BlockId, &'a [kir::ValueId])>, &'static str> {
    let mut edges = Vec::new();
    match terminator {
        kir::Terminator::Branch { target, arguments } => edges.push((*target, arguments.as_slice())),
        kir::Terminator::ConditionalBranch { condition, then_target, then_arguments, else_target, else_arguments } => {
            let condition = values.get(condition).copied();
            if condition != Some(0) { edges.push((*then_target, then_arguments.as_slice())); }
            if condition != Some(1) { edges.push((*else_target, else_arguments.as_slice())); }
        }
        kir::Terminator::Switch { selector, cases, default_target, default_arguments } => {
            let selector = values.get(selector).copied();
            let mut matched = false;
            for case in cases {
                if selector.is_none() || selector == Some(case.value) {
                    edges.push((case.target, case.arguments.as_slice()));
                    matched = selector.is_some();
                }
            }
            if !matched { edges.push((*default_target, default_arguments.as_slice())); }
        }
        kir::Terminator::IntegerSwitch { selector, cases, default_target, default_arguments } => {
            let selector = values.get(selector).copied();
            let mut matched = false;
            for case in cases {
                let candidate = constant(&case.value);
                if candidate.is_none() { return Err("signed integer switch is not in publication geometry subset"); }
                if selector.is_none() || selector == candidate {
                    edges.push((case.target, case.arguments.as_slice()));
                    matched = selector.is_some();
                }
            }
            if !matched { edges.push((*default_target, default_arguments.as_slice())); }
        }
        kir::Terminator::Return { .. } | kir::Terminator::Unreachable => (),
    }
    if edges.len() > 16 { return Err("edge limit"); }
    Ok(edges)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kir::{BasicBlock, BlockId, Constant, FunctionOperationLocation as Site, Operation, OperationKind as O, ScalarType as S, Terminator as T, Type, ValueDef, ValueId as V};

    fn fixture() -> (kir::Kernel, kir::Function) {
        let op = |id, ty, kind| Operation::effect_free(ValueDef::new(V(id), ty), kind);
        let mut entry = BasicBlock::new(BlockId(0));
        entry.operations = vec![
            op(0, Type::INDEX, O::Intrinsic(kir::IntrinsicOperation::global_id_1d())),
            op(1, Type::INDEX, O::Constant(Constant::Index(128))),
            op(2, Type::INDEX, O::Binary { op: kir::BinaryOp::Remainder, lhs: V(0), rhs: V(1) }),
            op(3, Type::Scalar(S::Bool), O::Compare { predicate: kir::ComparePredicate::LessThan, lhs: V(0), rhs: V(1) }),
        ];
        entry.terminator = Some(T::ConditionalBranch { condition: V(3), then_target: BlockId(1), then_arguments: vec![], else_target: BlockId(2), else_arguments: vec![] });
        let mut producer = BasicBlock::new(BlockId(1));
        producer.operations = vec![op(4, Type::Scalar(S::F32), O::Constant(Constant::F32Bits(0)))];
        producer.terminator = Some(T::Return { values: vec![] });
        let mut consumer = BasicBlock::new(BlockId(2));
        consumer.operations = vec![op(5, Type::Scalar(S::F32), O::Constant(Constant::F32Bits(0)))];
        consumer.terminator = Some(T::Return { values: vec![] });
        let function = kir::Function::kernel_entry("publication", kir::Signature::new(vec![], vec![]), vec![], vec![entry, producer, consumer]);
        let mut kernel = kir::Kernel::new("publication", "publication", kir::LaunchDomain::D1 { x: kir::LaunchExtent::Dynamic });
        kernel.workgroup_size = Some(kir::WorkgroupSize::new(128, 1, 1));
        (kernel, function)
    }

    fn prove(kernel: &kir::Kernel, function: &kir::Function) -> Result<(), &'static str> {
        prove_geometry(kernel, function, Site::new(BlockId(1), 0), Site::new(BlockId(2), 0), V(2), V(2))
    }

    #[test]
    fn publication_geometry_replays_both_allowed_launch_sizes() {
        let (kernel, function) = fixture();
        prove(&kernel, &function).unwrap();
    }

    #[test]
    fn publication_geometry_rejects_wrong_roles_cells_and_cycles() {
        for mutant in 0..6 {
            let (mut kernel, mut function) = fixture();
            let body = function.body.as_mut().unwrap();
            match mutant {
                0 => body.blocks[0].operations[1].kind = O::Constant(Constant::Index(64)),
                1 => body.blocks[0].operations[2].kind = O::Constant(Constant::Index(0)),
                2 => body.blocks[0].operations[3].kind = O::Constant(Constant::Bool(true)),
                3 => body.blocks[1].terminator = Some(T::Branch { target: BlockId(0), arguments: vec![] }),
                4 => kernel.workgroup_size = Some(kir::WorkgroupSize::new(64, 1, 1)),
                5 => kernel.domain = kir::LaunchDomain::D1 { x: kir::LaunchExtent::Static(512) },
                _ => unreachable!(),
            }
            assert!(prove(&kernel, &function).is_err(), "mutant {mutant}");
        }
    }

    #[test]
    fn publication_geometry_never_substitutes_unknown_for_ready_or_an_index() {
        let (kernel, mut function) = fixture();
        let body = function.body.as_mut().unwrap();
        body.blocks[0].operations[2].kind = O::SliceLength { slice: V(99) };
        assert!(prove(&kernel, &function).is_err());
        let (_, function) = fixture();
        assert!(prove_geometry(&kernel, &function, Site::new(BlockId(1), 99), Site::new(BlockId(2), 0), V(2), V(2)).is_err());
    }

    #[test]
    fn publication_geometry_preserves_unsigned_width_and_checked_shift_limits() {
        let definition = ValueDef::new(V(2), Type::Scalar(S::U8));
        let types = HashMap::from([(V(0), &definition.ty), (V(1), &definition.ty)]);
        let mut values = HashMap::from([(V(0), 255), (V(1), 1)]);
        let operation = Operation::effect_free(definition.clone(), O::Binary { op: kir::BinaryOp::Add, lhs: V(0), rhs: V(1) });
        evaluate_operation(&operation, &types, &mut values, 0, 256);
        assert_eq!(values.get(&V(2)), Some(&0));
        values.remove(&V(2));
        values.insert(V(1), 8);
        let operation = Operation::effect_free(definition.clone(), O::Binary { op: kir::BinaryOp::ShiftLeft, lhs: V(0), rhs: V(1) });
        evaluate_operation(&operation, &types, &mut values, 0, 256);
        assert_eq!(values.get(&V(2)), None);
    }
}

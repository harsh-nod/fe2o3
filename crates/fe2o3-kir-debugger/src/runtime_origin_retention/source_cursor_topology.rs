//! Narrow actual-owner topology gate. Synthetic tests must not become source evidence.
use fe2o3_kernel_ir::*;
use fe2o3_kir_sim::SimulationDebugSiteV1;

#[path = "source_cursor_topology_tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug)]
pub(super) struct Selected {
    pub call: SimulationDebugSiteV1,
    pub helper: SimulationDebugSiteV1,
}

fn pure(function: &Function) -> bool {
    let scalar = Type::Scalar(ScalarType::U32);
    if function.role != FunctionRole::InternalHelper
        || function.signature.parameters != [scalar.clone(), scalar.clone()]
        || function.signature.results != [scalar]
    {
        return false;
    }
    let Some(body) = &function.body else {
        return false;
    };
    if body.blocks.len() != 1 {
        return false;
    }
    let block = &body.blocks[0];
    let (mut masks, mut xors, mut ands) = (0, 0, 0);
    for operation in &block.operations {
        if operation.results.len() != 1 || operation.results[0].ty != Type::Scalar(ScalarType::U32)
        {
            return false;
        }
        match operation.kind {
            OperationKind::Constant(Constant::U32(0xffff)) => masks += 1,
            OperationKind::Binary {
                op: BinaryOp::BitXor,
                ..
            } => xors += 1,
            OperationKind::Binary {
                op: BinaryOp::BitAnd,
                ..
            } => ands += 1,
            _ => return false,
        }
    }
    masks == 1
        && xors == 1
        && ands == 1
        && matches!(&block.terminator, Some(Terminator::Return { values }) if values.len() == 1)
}

pub(super) fn select(module: &Module) -> Result<Selected, &'static str> {
    if module.kernels.len() != 1
        || module.kernels[0].id.as_str() != "loop_helper"
        || module.functions.is_empty()
        || module.functions.len() > 8
    {
        return Err("one bounded actual-source kernel required");
    }
    let mut block_count = 0usize;
    let mut operation_count = 0usize;
    for function in &module.functions {
        if let Some(body) = &function.body {
            block_count += body.blocks.len();
            for block in &body.blocks {
                operation_count += block.operations.len();
            }
        }
    }
    if block_count > 64 || operation_count > 256 {
        return Err("graph cap");
    }
    let entry = module
        .functions
        .iter()
        .position(|f| f.id == module.kernels[0].entry)
        .ok_or("entry absent")?;
    let function = &module.functions[entry];
    let body = function.body.as_ref().ok_or("entry body absent")?;
    if function.role != FunctionRole::KernelEntry || body.blocks.is_empty() {
        return Err("entry role");
    }
    let mut reach = [[false; 64]; 64];
    let mut edges = 0usize;
    for (index, block) in body.blocks.iter().enumerate() {
        let term = block.terminator.as_ref().ok_or("missing terminator")?;
        let count = match term {
            Terminator::Branch { .. } => 1,
            Terminator::ConditionalBranch { .. } => 2,
            Terminator::Switch { cases, .. } => {
                if cases.len() > 64 {
                    return Err("case cap");
                }
                cases.len() + 1
            }
            Terminator::IntegerSwitch { cases, .. } => {
                if cases.len() > 64 {
                    return Err("case cap");
                }
                cases.len() + 1
            }
            Terminator::Return { .. } | Terminator::Unreachable => 0,
        };
        edges += count;
        if edges > 256 {
            return Err("edge cap");
        }
        for target in term.successors() {
            let next = body
                .blocks
                .iter()
                .position(|b| b.id == target)
                .ok_or("successor absent")?;
            reach[index][next] = true;
        }
    }
    for k in 0..body.blocks.len() {
        for i in 0..body.blocks.len() {
            for j in 0..body.blocks.len() {
                let through = reach[i][k] && reach[k][j];
                reach[i][j] |= through;
            }
        }
    }
    let mut selected = None;
    for (index, block) in body.blocks.iter().enumerate() {
        if !(index == 0 || reach[0][index]) || !reach[index][index] {
            continue;
        }
        for (op, operation) in block.operations.iter().enumerate() {
            let OperationKind::Call { callee, arguments } = &operation.kind else {
                continue;
            };
            let helper = module
                .functions
                .iter()
                .position(|f| f.id == *callee)
                .ok_or("callee absent")?;
            if helper == entry || !pure(&module.functions[helper]) {
                continue;
            }
            if arguments.len() != 2
                || operation.results.len() != 1
                || operation.results[0].ty != Type::Scalar(ScalarType::U32)
            {
                return Err("call ABI");
            }
            if selected.is_some() {
                return Err("ambiguous actual cyclic helper call");
            }
            let helper_block = &module.functions[helper].body.as_ref().unwrap().blocks[0];
            selected = Some(Selected {
                call: SimulationDebugSiteV1 {
                    function_ordinal: entry,
                    block: block.id,
                    operation: op as u32,
                },
                helper: SimulationDebugSiteV1 {
                    function_ordinal: helper,
                    block: helper_block.id,
                    operation: 0,
                },
            });
        }
    }
    selected.ok_or("actual reachable cyclic helper call required")
}

#[test]
fn missing_or_acyclic_fixture_is_not_source_loop_qualification() {
    let empty = Module::new("synthetic-negative");
    assert!(select(&empty).is_err());
    let (admitted, _) = super::fixtures::memory();
    let mut synthetic = admitted.module().clone();
    synthetic.kernels[0].id = "loop_helper".into();
    assert!(select(&synthetic).is_err());
}

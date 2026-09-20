//! Bounded topology gate over the exact decoded canonical owner.
use fe2o3_kernel_ir::*;
use serde_json::{Value, json};

const MAX_SWITCH_CASES: usize = 64;
const MAX_CFG_EDGES: usize = 256;

fn successor_count(terminator: &Terminator) -> Result<usize, String> {
    let cases = match terminator {
        Terminator::Branch { .. } => return Ok(1),
        Terminator::ConditionalBranch { .. } => return Ok(2),
        Terminator::Switch { cases, .. } => cases.len(),
        Terminator::IntegerSwitch { cases, .. } => cases.len(),
        Terminator::Return { .. } | Terminator::Unreachable => return Ok(0),
    };
    if cases > MAX_SWITCH_CASES {
        return Err("source topology: switch case cap".into());
    }
    // The default is a real successor even with no cases or duplicate targets.
    Ok(cases + 1)
}

#[derive(Clone, Debug)]
pub(super) struct Topology {
    pub entry: usize,
    pub helper: usize,
    pub call: [u32; 3],
    pub call_authoring_coordinate: [u32; 3],
    pub cycle: Vec<u32>,
    pub helper_sites: Vec<[u32; 3]>,
    pub helper_authoring_coordinates: Vec<[u32; 3]>,
}

impl Topology {
    pub fn json(&self) -> Value {
        json!({"entry":self.entry,"helper":self.helper,"call":self.call,
            "call_authoring_coordinate":self.call_authoring_coordinate,
            "cycle":self.cycle,"helper_sites":self.helper_sites,
            "helper_authoring_coordinates":self.helper_authoring_coordinates})
    }
}

fn pure_helper(function: &Function) -> bool {
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
    let mut xor = 0;
    let mut and = 0;
    let mut mask = 0;
    for operation in &block.operations {
        if operation.results.len() != 1 || operation.results[0].ty != Type::Scalar(ScalarType::U32)
        {
            return false;
        }
        match operation.kind {
            OperationKind::Binary {
                op: BinaryOp::BitXor,
                ..
            } => xor += 1,
            OperationKind::Binary {
                op: BinaryOp::BitAnd,
                ..
            } => and += 1,
            OperationKind::Constant(Constant::U32(0xffff)) => mask += 1,
            _ => return false,
        }
    }
    xor == 1
        && and == 1
        && mask == 1
        && matches!(&block.terminator, Some(Terminator::Return { values }) if values.len() == 1)
}

pub(super) fn select(module: &Module) -> Result<Topology, String> {
    let fail = |message: &str| Err(message.to_owned());
    if module.kernels.len() != 1
        || module.kernels[0].id.as_str() != "loop_helper"
        || module.functions.is_empty()
        || module.functions.len() > 8
    {
        return fail("source topology: bounded one-kernel roster");
    }
    let mut blocks = 0usize;
    let mut operations = 0usize;
    for function in &module.functions {
        if function.id.as_str().len() > 256 {
            return fail("source topology: identifier cap");
        }
        if let Some(body) = &function.body {
            blocks += body.blocks.len();
            let mut values = body.parameters.len();
            for block in &body.blocks {
                operations += block.operations.len();
                values += block.parameters.len();
                values += block
                    .operations
                    .iter()
                    .map(|op| op.results.len())
                    .sum::<usize>();
            }
            if values > 256 {
                return fail("source topology: SSA cap");
            }
        }
    }
    if blocks > 64 || operations > 256 {
        return fail("source topology: graph cap");
    }
    let entry = module
        .functions
        .iter()
        .position(|f| f.id == module.kernels[0].entry)
        .ok_or("source topology: entry absent")?;
    let body = module.functions[entry]
        .body
        .as_ref()
        .ok_or("source topology: entry body")?;
    if module.functions[entry].role != FunctionRole::KernelEntry || body.blocks.is_empty() {
        return fail("source topology: actual entry role/body");
    }
    let mut reach = vec![vec![false; body.blocks.len()]; body.blocks.len()];
    let mut edges = 0usize;
    for (i, block) in body.blocks.iter().enumerate() {
        let terminator = block
            .terminator
            .as_ref()
            .ok_or("source topology: missing fixture terminator")?;
        edges = edges
            .checked_add(successor_count(terminator)?)
            .filter(|count| *count <= MAX_CFG_EDGES)
            .ok_or("source topology: CFG edge cap")?;
        // Use the canonical IR's exhaustive cases-plus-default semantics.
        // Count/cap first: successors allocates at most 65 bounded BlockIds.
        for successor in terminator.successors() {
            let j = body
                .blocks
                .iter()
                .position(|b| b.id == successor)
                .ok_or("source topology: missing successor")?;
            reach[i][j] = true;
        }
    }
    // <=64^3 transitions; identity diagonal is not added, so a cycle needs edges.
    for k in 0..reach.len() {
        for i in 0..reach.len() {
            for j in 0..reach.len() {
                let through = reach[i][k] && reach[k][j];
                reach[i][j] |= through;
            }
        }
    }
    let mut selected = None;
    for (i, block) in body.blocks.iter().enumerate() {
        if !(i == 0 || reach[0][i]) || !reach[i][i] {
            continue;
        }
        for (op_index, operation) in block.operations.iter().enumerate() {
            let OperationKind::Call { callee, arguments } = &operation.kind else {
                continue;
            };
            let helper = module
                .functions
                .iter()
                .position(|f| f.id == *callee)
                .ok_or("source topology: retained callee absent")?;
            if helper == entry || !pure_helper(&module.functions[helper]) {
                continue;
            }
            if arguments.len() != 2
                || operation.results.len() != 1
                || operation.results[0].ty != Type::Scalar(ScalarType::U32)
            {
                return fail("source topology: call ABI");
            }
            if selected.is_some() {
                return fail("source topology: ambiguous cyclic helper call");
            }
            let cycle = body
                .blocks
                .iter()
                .enumerate()
                .filter(|(j, _)| reach[i][*j] && reach[*j][i])
                .map(|(_, block)| block.id.0)
                .collect();
            let helper_blocks = &module.functions[helper].body.as_ref().unwrap().blocks;
            let helper_sites = helper_blocks
                .iter()
                .flat_map(|block| {
                    (0..block.operations.len()).map(move |n| [helper as u32, block.id.0, n as u32])
                })
                .collect();
            // Authoring coordinates index the canonical block roster; runtime
            // sites use the actual BlockId. Derive both from this same owner.
            let helper_authoring_coordinates = helper_blocks
                .iter()
                .enumerate()
                .flat_map(|(block_index, block)| {
                    (0..block.operations.len())
                        .map(move |n| [helper as u32, block_index as u32, n as u32])
                })
                .collect();
            selected = Some(Topology {
                entry,
                helper,
                call: [entry as u32, block.id.0, op_index as u32],
                call_authoring_coordinate: [entry as u32, i as u32, op_index as u32],
                cycle,
                helper_sites,
                helper_authoring_coordinates,
            });
        }
    }
    selected.ok_or("source topology: actual cyclic retained helper call unavailable".into())
}

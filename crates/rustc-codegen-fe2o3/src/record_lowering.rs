use crate::mir_import::{MirModule, MirTerminatorKind};
use crate::trusted_device_items::TrustedDeviceItem;
use dialect_mir::{MirAttrValue, MirOp, MirOpRecord};
use std::collections::{HashMap, HashSet};
use std::fmt::Write;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordLoweringPlan {
    pub functions: Vec<RecordLoweringFunction>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordLoweringFunction {
    pub symbol: String,
    pub kind: String,
    pub arg_count: usize,
    pub local_count: usize,
    pub block_count: usize,
    pub locals: Vec<RecordLoweringLocal>,
    pub ops: Vec<RecordLoweringOp>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordLoweringLocal {
    pub index: usize,
    pub role: String,
    pub ty: String,
    pub rust_ty: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordLoweringOp {
    pub op: MirOp,
    pub block: Option<usize>,
    pub statement: Option<usize>,
    pub source: Option<String>,
    pub operation: Option<String>,
    pub callee: Option<String>,
    trusted_callee: Option<TrustedDeviceItem>,
    pub target: Option<usize>,
    pub targets: Option<usize>,
    pub destination_local: Option<usize>,
    pub destination: Option<String>,
    pub operand_count: Option<usize>,
    pub operands: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordPlaceRef {
    pub local: usize,
    pub projection: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordAccessSketch {
    pub loads: Vec<RecordMemoryAccess>,
    pub stores: Vec<RecordMemoryAccess>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordMemoryAccess {
    pub place: RecordPlaceRef,
    pub index_local: Option<usize>,
    pub operation: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordIndexSketch {
    pub thread_index_local: Option<usize>,
    pub bindings: Vec<RecordIndexBinding>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordIndexBinding {
    pub local: usize,
    pub index: RecordLinearIndex,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecordLinearIndex {
    pub stride: i64,
    pub offset: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordSliceAccessSketch {
    pub reads: Vec<RecordSliceAccess>,
    pub writes: Vec<RecordSliceAccess>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordSliceAccess {
    pub arg_index: usize,
    pub local: usize,
    pub index: RecordLinearIndex,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordExpressionSketch {
    pub local_bindings: Vec<RecordExpressionBinding>,
    pub stores: Vec<RecordStoreExpression>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordExpressionBinding {
    pub local: usize,
    pub expr: RecordExpression,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordStoreExpression {
    pub destination: RecordPlaceRef,
    pub expr: RecordExpression,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecordExpression {
    Use(RecordExpressionOperand),
    Binary {
        lhs: RecordExpressionOperand,
        rhs: RecordExpressionOperand,
        op: RecordBinaryOp,
    },
    Unary {
        operand: RecordExpressionOperand,
        op: RecordUnaryOp,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecordExpressionOperand {
    Local(usize),
    ScalarArg { arg_index: usize, local: usize },
    SliceElement(RecordSliceAccess),
    Constant { ty: String, value: String },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecordBinaryOp {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecordUnaryOp {
    Neg,
}

pub fn plan_from_module(module: &MirModule) -> RecordLoweringPlan {
    let mut plan = plan_from_records(&module.dialect_records());

    for function in &module.functions {
        let Some(target) = plan
            .functions
            .iter_mut()
            .find(|candidate| candidate.symbol == function.export_name)
        else {
            continue;
        };

        for block in &function.blocks {
            let Some(MirTerminatorKind::Call {
                callee: Some(callee),
                ..
            }) = block.terminator.as_ref().map(|terminator| &terminator.kind)
            else {
                continue;
            };
            let Some(trusted_item) = callee.trusted_item() else {
                continue;
            };
            let mut matches = target
                .ops
                .iter()
                .enumerate()
                .filter(|(_, op)| op.op == MirOp::Call && op.block == Some(block.index))
                .map(|(index, _)| index);
            let (Some(index), None) = (matches.next(), matches.next()) else {
                continue;
            };
            target.ops[index].trusted_callee = Some(trusted_item);
        }
    }

    plan
}

pub fn plan_from_records(records: &[MirOpRecord]) -> RecordLoweringPlan {
    let mut functions = Vec::new();

    for record in records {
        if record.op == MirOp::Func {
            functions.push(RecordLoweringFunction {
                symbol: string_attr(record, "symbol")
                    .unwrap_or("<anonymous>")
                    .to_string(),
                kind: string_attr(record, "kind").unwrap_or("device").to_string(),
                arg_count: usize_attr(record, "args").unwrap_or(0),
                local_count: usize_attr(record, "locals").unwrap_or(0),
                block_count: usize_attr(record, "blocks").unwrap_or(0),
                locals: Vec::new(),
                ops: Vec::new(),
            });
            continue;
        }

        if matches!(record.op, MirOp::Arg | MirOp::Local) {
            let Some(function) = string_attr(record, "function") else {
                continue;
            };
            let Some(target) = functions
                .iter_mut()
                .find(|candidate| candidate.symbol == function)
            else {
                continue;
            };
            target.locals.push(RecordLoweringLocal {
                index: usize_attr(record, "index").unwrap_or(0),
                role: string_attr(record, "role").unwrap_or("temp").to_string(),
                ty: string_attr(record, "type")
                    .unwrap_or("mir.unknown")
                    .to_string(),
                rust_ty: string_attr(record, "rust_type")
                    .unwrap_or("<unknown>")
                    .to_string(),
            });
            continue;
        }

        if !is_lowering_op(record.op) {
            continue;
        }
        let Some(function) = string_attr(record, "function") else {
            continue;
        };
        let Some(target) = functions
            .iter_mut()
            .find(|candidate| candidate.symbol == function)
        else {
            continue;
        };
        target.ops.push(RecordLoweringOp {
            op: record.op,
            block: usize_attr(record, "block"),
            statement: usize_attr(record, "statement"),
            source: string_attr(record, "source").map(str::to_string),
            operation: string_attr(record, "operation").map(str::to_string),
            callee: string_attr(record, "callee").map(str::to_string),
            trusted_callee: None,
            target: usize_attr(record, "target"),
            targets: usize_attr(record, "targets"),
            destination_local: usize_attr(record, "destination_local"),
            destination: string_attr(record, "destination").map(str::to_string),
            operand_count: usize_attr(record, "operand_count"),
            operands: string_attr(record, "operands").map(str::to_string),
        });
    }

    RecordLoweringPlan { functions }
}

impl RecordLoweringPlan {
    pub fn function(&self, symbol: &str) -> Option<&RecordLoweringFunction> {
        self.functions
            .iter()
            .find(|function| function.symbol == symbol)
    }

    pub fn summary(&self) -> String {
        let op_count = self
            .functions
            .iter()
            .map(|function| function.ops.len())
            .sum::<usize>();
        let mut output = format!(
            "\n=== fe2o3 MIR record lowering plan ({} function(s), {op_count} lowering op(s)) ===\n",
            self.functions.len()
        );

        for function in &self.functions {
            let _ = writeln!(
                output,
                "  [{}] {}: {} bb, {} locals, {} args, {} lowering op(s)",
                function.kind,
                function.symbol,
                function.block_count,
                function.local_count,
                function.arg_count,
                function.ops.len()
            );
            let counts = function.op_counts();
            if !counts.is_empty() {
                let _ = writeln!(output, "      ops: {}", counts.join(", "));
            }
        }

        output.push_str("===================================\n");
        output
    }
}

impl RecordLoweringFunction {
    pub fn args(&self) -> Vec<&RecordLoweringLocal> {
        let mut args = self
            .locals
            .iter()
            .filter(|local| local.role == "arg")
            .collect::<Vec<_>>();
        args.sort_by_key(|local| local.index);
        args
    }

    pub fn has_op(&self, op: MirOp) -> bool {
        self.op_count(op) > 0
    }

    pub fn op_count(&self, op: MirOp) -> usize {
        self.ops
            .iter()
            .filter(|candidate| candidate.op == op)
            .count()
    }

    pub fn ops_by(&self, op: MirOp) -> Vec<&RecordLoweringOp> {
        self.ops
            .iter()
            .filter(|candidate| candidate.op == op)
            .collect()
    }

    pub fn has_trusted_call(&self, item: TrustedDeviceItem) -> bool {
        self.ops_by(MirOp::Call)
            .into_iter()
            .any(|candidate| candidate.trusted_callee == Some(item))
    }

    pub fn access_sketch(&self) -> RecordAccessSketch {
        let loads = self
            .ops_by(MirOp::Load)
            .into_iter()
            .flat_map(|op| {
                op.operand_places()
                    .into_iter()
                    .map(|place| RecordMemoryAccess {
                        index_local: place.index_local(),
                        place,
                        operation: op.operation.clone(),
                    })
            })
            .collect();
        let stores = self
            .ops_by(MirOp::Store)
            .into_iter()
            .filter_map(|op| {
                op.destination_place().map(|place| RecordMemoryAccess {
                    index_local: place.index_local(),
                    place,
                    operation: op.operation.clone(),
                })
            })
            .collect();

        RecordAccessSketch { loads, stores }
    }

    pub fn index_sketch(&self) -> RecordIndexSketch {
        let mut thread_index_local = None;
        let mut thread_index_aliases = HashSet::new();
        let mut bindings = HashMap::new();

        for op in &self.ops {
            if op.op == MirOp::Call {
                bind_call_index(
                    op,
                    &mut thread_index_local,
                    &mut thread_index_aliases,
                    &mut bindings,
                );
                continue;
            }

            bind_thread_index_alias(op, &mut thread_index_aliases);
            bind_operation_index(op, &mut bindings);
        }

        let mut bindings = bindings
            .into_iter()
            .map(|(local, index)| RecordIndexBinding { local, index })
            .collect::<Vec<_>>();
        bindings.sort_by_key(|binding| binding.local);

        RecordIndexSketch {
            thread_index_local,
            bindings,
        }
    }

    pub fn slice_access_sketch(&self) -> RecordSliceAccessSketch {
        let args = self.args();
        let access_sketch = self.access_sketch();
        let index_sketch = self.index_sketch();
        let reads = access_sketch
            .loads
            .iter()
            .filter_map(|access| slice_access_from_memory(&args, access, &index_sketch))
            .collect();
        let writes = access_sketch
            .stores
            .iter()
            .filter_map(|access| slice_access_from_memory(&args, access, &index_sketch))
            .collect();

        RecordSliceAccessSketch { reads, writes }
    }

    pub fn expression_sketch(&self) -> RecordExpressionSketch {
        let args = self.args();
        let index_sketch = self.index_sketch();
        let mut local_expressions = HashMap::new();
        let mut stores = Vec::new();

        for op in &self.ops {
            if op.op == MirOp::Load {
                let Some(destination) = op.destination_local else {
                    continue;
                };
                let Some(access) = op.operand_places().into_iter().find_map(|place| {
                    let access = RecordMemoryAccess {
                        index_local: place.index_local(),
                        place,
                        operation: op.operation.clone(),
                    };
                    slice_access_from_memory(&args, &access, &index_sketch)
                }) else {
                    continue;
                };

                local_expressions.insert(
                    destination,
                    RecordExpression::Use(RecordExpressionOperand::SliceElement(access)),
                );
                continue;
            }

            let Some(expr) = record_expression_from_op(op, &args) else {
                continue;
            };
            if op.op == MirOp::Store {
                if let Some(destination) = op.destination_place() {
                    stores.push(RecordStoreExpression { destination, expr });
                }
            } else if let Some(destination) = op.destination_local {
                local_expressions.insert(destination, expr);
            }
        }

        let mut local_bindings = local_expressions
            .into_iter()
            .map(|(local, expr)| RecordExpressionBinding { local, expr })
            .collect::<Vec<_>>();
        local_bindings.sort_by_key(|binding| binding.local);

        RecordExpressionSketch {
            local_bindings,
            stores,
        }
    }

    fn op_counts(&self) -> Vec<String> {
        LOWERING_OPS
            .iter()
            .filter_map(|op| {
                let count = self
                    .ops
                    .iter()
                    .filter(|candidate| candidate.op == *op)
                    .count();
                (count > 0).then(|| format!("{}={count}", op.name()))
            })
            .collect()
    }
}

impl RecordIndexSketch {
    pub fn get(&self, local: usize) -> Option<RecordLinearIndex> {
        self.bindings
            .iter()
            .find(|binding| binding.local == local)
            .map(|binding| binding.index)
    }
}

impl RecordExpressionSketch {
    pub fn binary_op_count(&self, op: RecordBinaryOp) -> usize {
        self.local_bindings
            .iter()
            .filter(|binding| matches!(&binding.expr, RecordExpression::Binary { op: actual, .. } if *actual == op))
            .count()
            + self
                .stores
                .iter()
                .filter(|store| matches!(&store.expr, RecordExpression::Binary { op: actual, .. } if *actual == op))
                .count()
    }

    pub fn unary_op_count(&self, op: RecordUnaryOp) -> usize {
        self.local_bindings
            .iter()
            .filter(|binding| matches!(&binding.expr, RecordExpression::Unary { op: actual, .. } if *actual == op))
            .count()
            + self
                .stores
                .iter()
                .filter(|store| matches!(&store.expr, RecordExpression::Unary { op: actual, .. } if *actual == op))
                .count()
    }

    pub fn uses_scalar_arg(&self, arg_index: usize) -> bool {
        self.local_bindings.iter().any(|binding| {
            self.expression_uses_scalar_arg(&binding.expr, arg_index, &mut HashSet::new())
        }) || self.stores.iter().any(|store| {
            self.expression_uses_scalar_arg(&store.expr, arg_index, &mut HashSet::new())
        })
    }

    pub fn uses_constant(&self, ty: &str, value_fragment: &str) -> bool {
        self.local_bindings.iter().any(|binding| {
            self.expression_uses_constant(&binding.expr, ty, value_fragment, &mut HashSet::new())
        }) || self.stores.iter().any(|store| {
            self.expression_uses_constant(&store.expr, ty, value_fragment, &mut HashSet::new())
        })
    }

    fn expression_uses_scalar_arg(
        &self,
        expr: &RecordExpression,
        arg_index: usize,
        visited: &mut HashSet<usize>,
    ) -> bool {
        match expr {
            RecordExpression::Use(operand) => {
                self.operand_uses_scalar_arg(operand, arg_index, visited)
            }
            RecordExpression::Binary { lhs, rhs, .. } => {
                self.operand_uses_scalar_arg(lhs, arg_index, visited)
                    || self.operand_uses_scalar_arg(rhs, arg_index, visited)
            }
            RecordExpression::Unary { operand, .. } => {
                self.operand_uses_scalar_arg(operand, arg_index, visited)
            }
        }
    }

    fn operand_uses_scalar_arg(
        &self,
        operand: &RecordExpressionOperand,
        arg_index: usize,
        visited: &mut HashSet<usize>,
    ) -> bool {
        match operand {
            RecordExpressionOperand::ScalarArg {
                arg_index: actual, ..
            } => *actual == arg_index,
            RecordExpressionOperand::Local(local) => self
                .local_expression(*local, visited)
                .is_some_and(|expr| self.expression_uses_scalar_arg(expr, arg_index, visited)),
            _ => false,
        }
    }

    fn expression_uses_constant(
        &self,
        expr: &RecordExpression,
        ty: &str,
        value_fragment: &str,
        visited: &mut HashSet<usize>,
    ) -> bool {
        match expr {
            RecordExpression::Use(operand) => {
                self.operand_uses_constant(operand, ty, value_fragment, visited)
            }
            RecordExpression::Binary { lhs, rhs, .. } => {
                self.operand_uses_constant(lhs, ty, value_fragment, visited)
                    || self.operand_uses_constant(rhs, ty, value_fragment, visited)
            }
            RecordExpression::Unary { operand, .. } => {
                self.operand_uses_constant(operand, ty, value_fragment, visited)
            }
        }
    }

    fn operand_uses_constant(
        &self,
        operand: &RecordExpressionOperand,
        ty: &str,
        value_fragment: &str,
        visited: &mut HashSet<usize>,
    ) -> bool {
        match operand {
            RecordExpressionOperand::Constant {
                ty: actual_ty,
                value,
            } => actual_ty == ty && value.contains(value_fragment),
            RecordExpressionOperand::Local(local) => {
                self.local_expression(*local, visited).is_some_and(|expr| {
                    self.expression_uses_constant(expr, ty, value_fragment, visited)
                })
            }
            _ => false,
        }
    }

    fn local_expression(
        &self,
        local: usize,
        visited: &mut HashSet<usize>,
    ) -> Option<&RecordExpression> {
        if !visited.insert(local) {
            return None;
        }
        self.local_bindings
            .iter()
            .find(|binding| binding.local == local)
            .map(|binding| &binding.expr)
    }
}

impl RecordLoweringOp {
    #[cfg(test)]
    pub(crate) fn new_for_test(op: MirOp) -> Self {
        Self {
            op,
            block: Some(0),
            statement: None,
            source: None,
            operation: None,
            callee: None,
            trusted_callee: None,
            target: None,
            targets: None,
            destination_local: None,
            destination: None,
            operand_count: None,
            operands: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn set_trusted_callee_for_test(&mut self, item: TrustedDeviceItem) {
        self.trusted_callee = Some(item);
    }

    pub fn destination_place(&self) -> Option<RecordPlaceRef> {
        self.destination.as_deref().and_then(RecordPlaceRef::parse)
    }

    pub fn operand_places(&self) -> Vec<RecordPlaceRef> {
        self.operands
            .as_deref()
            .into_iter()
            .flat_map(split_record_operands)
            .filter_map(|operand| RecordPlaceRef::parse(&operand))
            .collect()
    }

    fn operand_labels(&self) -> Vec<String> {
        self.operands
            .as_deref()
            .into_iter()
            .flat_map(split_record_operands)
            .collect()
    }
}

impl RecordPlaceRef {
    pub fn parse(label: &str) -> Option<Self> {
        let label = label.strip_prefix("local")?;
        let local_len = label.bytes().take_while(u8::is_ascii_digit).count();
        if local_len == 0 {
            return None;
        }

        let local = label[..local_len].parse().ok()?;
        let rest = &label[local_len..];
        let projection = rest
            .strip_prefix('.')
            .into_iter()
            .flat_map(|projection| projection.split('.'))
            .filter(|projection| !projection.is_empty())
            .map(str::to_string)
            .collect();

        Some(Self { local, projection })
    }

    pub fn index_local(&self) -> Option<usize> {
        self.projection.iter().find_map(|projection| {
            projection
                .strip_prefix("index_local")
                .and_then(|index| index.parse().ok())
        })
    }
}

impl RecordLinearIndex {
    pub fn thread() -> Self {
        Self {
            stride: 1,
            offset: 0,
        }
    }

    fn offset(self, offset: i64) -> Option<Self> {
        Some(Self {
            stride: self.stride,
            offset: self.offset.checked_add(offset)?,
        })
    }

    fn scale(self, factor: i64) -> Option<Self> {
        Some(Self {
            stride: self.stride.checked_mul(factor)?,
            offset: self.offset.checked_mul(factor)?,
        })
    }

    fn add(self, rhs: Self) -> Option<Self> {
        Some(Self {
            stride: self.stride.checked_add(rhs.stride)?,
            offset: self.offset.checked_add(rhs.offset)?,
        })
    }

    fn sub(self, rhs: Self) -> Option<Self> {
        Some(Self {
            stride: self.stride.checked_sub(rhs.stride)?,
            offset: self.offset.checked_sub(rhs.offset)?,
        })
    }
}

fn bind_call_index(
    op: &RecordLoweringOp,
    thread_index_local: &mut Option<usize>,
    thread_index_aliases: &mut HashSet<usize>,
    bindings: &mut HashMap<usize, RecordLinearIndex>,
) {
    let Some(callee) = op.trusted_callee else {
        return;
    };
    let Some(destination) = op.destination_local else {
        return;
    };

    if callee == TrustedDeviceItem::ThreadIndex1d {
        *thread_index_local = Some(destination);
        thread_index_aliases.clear();
        thread_index_aliases.insert(destination);
        return;
    }

    thread_index_aliases.remove(&destination);
    if thread_index_local.is_none() {
        return;
    }
    let operands = op.operand_labels();
    let Some(receiver) = operands
        .first()
        .and_then(|operand| RecordPlaceRef::parse(operand))
    else {
        return;
    };
    if !receiver.projection.is_empty() || !thread_index_aliases.contains(&receiver.local) {
        return;
    }

    let index = if callee == TrustedDeviceItem::ThreadIndexGet {
        Some(RecordLinearIndex::thread())
    } else if callee == TrustedDeviceItem::ThreadIndexOffset {
        operands
            .get(1)
            .and_then(|operand| parse_record_unsigned_const(operand))
            .and_then(|offset| RecordLinearIndex::thread().offset(offset))
    } else if callee == TrustedDeviceItem::ThreadIndexOffsetSigned {
        operands
            .get(1)
            .and_then(|operand| parse_record_signed_const(operand))
            .and_then(|offset| RecordLinearIndex::thread().offset(offset))
    } else if callee == TrustedDeviceItem::ThreadIndexStride {
        operands
            .get(1)
            .and_then(|operand| parse_record_unsigned_const(operand))
            .map(|stride| RecordLinearIndex { stride, offset: 0 })
    } else if callee == TrustedDeviceItem::ThreadIndexStrideOffset {
        operands
            .get(1)
            .and_then(|operand| parse_record_unsigned_const(operand))
            .zip(
                operands
                    .get(2)
                    .and_then(|operand| parse_record_signed_const(operand)),
            )
            .map(|(stride, offset)| RecordLinearIndex { stride, offset })
    } else {
        None
    };

    if let Some(index) = index {
        bindings.insert(destination, index);
    }
}

fn bind_thread_index_alias(op: &RecordLoweringOp, aliases: &mut HashSet<usize>) {
    if op.op != MirOp::Assign {
        return;
    }
    let Some(destination) = op.destination_local else {
        return;
    };
    aliases.remove(&destination);
    if op.operation.as_deref() != Some("ref") {
        return;
    }
    let Some(source) = op
        .operand_labels()
        .first()
        .and_then(|operand| RecordPlaceRef::parse(operand))
    else {
        return;
    };
    if source.projection.is_empty() && aliases.contains(&source.local) {
        aliases.insert(destination);
    }
}

fn bind_operation_index(op: &RecordLoweringOp, bindings: &mut HashMap<usize, RecordLinearIndex>) {
    let Some(destination) = op.destination_local else {
        return;
    };
    let operands = op.operand_labels();

    if op.op == MirOp::Assign && op.operation.as_deref() == Some("use") {
        if let Some(index) = operands
            .first()
            .and_then(|operand| place_index(operand, bindings))
        {
            bindings.insert(destination, index);
        }
        return;
    }

    let Some(operation) = op.operation.as_deref() else {
        return;
    };
    let Some(index) = index_from_binary_operation(operation, &operands, bindings) else {
        return;
    };
    bindings.insert(destination, index);
}

fn index_from_binary_operation(
    operation: &str,
    operands: &[String],
    bindings: &HashMap<usize, RecordLinearIndex>,
) -> Option<RecordLinearIndex> {
    let [lhs, rhs] = operands else {
        return None;
    };
    let lhs_index = place_index(lhs, bindings);
    let rhs_index = place_index(rhs, bindings);
    let lhs_const = parse_record_integer_const(lhs);
    let rhs_const = parse_record_integer_const(rhs);

    match operation {
        "add" | "add_unchecked" | "add_with_overflow" => {
            match (lhs_index, rhs_index, lhs_const, rhs_const) {
                (Some(lhs), Some(rhs), _, _) => lhs.add(rhs),
                (Some(index), None, _, Some(offset)) => index.offset(offset),
                (None, Some(index), Some(offset), _) => index.offset(offset),
                _ => None,
            }
        }
        "sub" | "sub_unchecked" | "sub_with_overflow" => {
            match (lhs_index, rhs_index, lhs_const, rhs_const) {
                (Some(lhs), Some(rhs), _, _) => lhs.sub(rhs),
                (Some(index), None, _, Some(offset)) => index.offset(offset.checked_neg()?),
                (None, Some(index), Some(offset), _) => {
                    RecordLinearIndex { stride: 0, offset }.sub(index)
                }
                _ => None,
            }
        }
        "mul" | "mul_unchecked" | "mul_with_overflow" => {
            match (lhs_index, rhs_index, lhs_const, rhs_const) {
                (Some(index), None, _, Some(factor)) => index.scale(factor),
                (None, Some(index), Some(factor), _) => index.scale(factor),
                _ => None,
            }
        }
        _ => None,
    }
}

fn place_index(
    operand: &str,
    bindings: &HashMap<usize, RecordLinearIndex>,
) -> Option<RecordLinearIndex> {
    let place = RecordPlaceRef::parse(operand)?;
    if place.projection.is_empty() {
        return bindings.get(&place.local).copied();
    }
    match place.projection.as_slice() {
        [field] if field == "field0" => bindings.get(&place.local).copied(),
        _ => None,
    }
}

fn parse_record_integer_const(operand: &str) -> Option<i64> {
    parse_record_unsigned_const(operand).or_else(|| parse_record_signed_const(operand))
}

fn parse_record_unsigned_const(operand: &str) -> Option<i64> {
    if !(operand.starts_with("const:mir.usize=") || operand.contains(", usize)")) {
        return None;
    }
    if let Some(value) = parse_record_eval_u64(operand) {
        return i64::try_from(value).ok();
    }
    let raw = parse_record_const_hex(operand)?;
    i64::try_from(raw).ok()
}

fn parse_record_signed_const(operand: &str) -> Option<i64> {
    if !(operand.starts_with("const:mir.isize=") || operand.contains(", isize)")) {
        return None;
    }
    if let Some(value) = parse_record_eval_i64(operand) {
        return Some(value);
    }
    let raw = parse_record_const_hex(operand)?;
    Some(i64::from_ne_bytes(raw.to_ne_bytes()))
}

fn parse_record_eval_u64(operand: &str) -> Option<u64> {
    parse_record_eval_attr(operand, "eval_u64=")?.parse().ok()
}

fn parse_record_eval_i64(operand: &str) -> Option<i64> {
    parse_record_eval_attr(operand, "eval_i64=")?.parse().ok()
}

fn parse_record_eval_attr<'a>(operand: &'a str, prefix: &str) -> Option<&'a str> {
    let start = operand.find(prefix)? + prefix.len();
    operand[start..]
        .split(|ch: char| !(ch == '-' || ch.is_ascii_digit()))
        .next()
}

fn parse_record_const_hex(operand: &str) -> Option<u64> {
    let start = operand.find("0x")? + 2;
    let hex = operand[start..]
        .chars()
        .take_while(|ch| ch.is_ascii_hexdigit())
        .collect::<String>();
    u64::from_str_radix(&hex, 16).ok()
}

fn split_record_operands(operands: &str) -> Vec<String> {
    let mut split = Vec::new();
    let mut start = 0;
    let mut depth = 0usize;

    for (index, ch) in operands.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                let operand = operands[start..index].trim();
                if !operand.is_empty() {
                    split.push(operand.to_string());
                }
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }

    let operand = operands[start..].trim();
    if !operand.is_empty() {
        split.push(operand.to_string());
    }
    split
}

fn slice_access_from_memory(
    args: &[&RecordLoweringLocal],
    access: &RecordMemoryAccess,
    index_sketch: &RecordIndexSketch,
) -> Option<RecordSliceAccess> {
    let (arg_index, arg) = args
        .iter()
        .enumerate()
        .find(|(_, arg)| arg.index == access.place.local)?;
    if arg.ty != "mir.slice" {
        return None;
    }

    Some(RecordSliceAccess {
        arg_index,
        local: arg.index,
        index: index_sketch.get(access.index_local?)?,
    })
}

fn record_expression_from_op(
    op: &RecordLoweringOp,
    args: &[&RecordLoweringLocal],
) -> Option<RecordExpression> {
    let operation = op.operation.as_deref()?;
    let operands = op.operand_labels();

    if operation == "use" {
        return operands
            .first()
            .and_then(|operand| record_expression_operand(operand, args))
            .map(RecordExpression::Use);
    }

    if let Some(op) = record_binary_op(operation) {
        let [lhs, rhs] = operands.as_slice() else {
            return None;
        };
        return record_expression_operand(lhs, args)
            .zip(record_expression_operand(rhs, args))
            .map(|(lhs, rhs)| RecordExpression::Binary { lhs, rhs, op });
    }

    if let Some(op) = record_unary_op(operation) {
        let [operand] = operands.as_slice() else {
            return None;
        };
        return record_expression_operand(operand, args)
            .map(|operand| RecordExpression::Unary { operand, op });
    }

    None
}

fn record_expression_operand(
    operand: &str,
    args: &[&RecordLoweringLocal],
) -> Option<RecordExpressionOperand> {
    if let Some((ty, value)) = parse_record_const_operand(operand) {
        return Some(RecordExpressionOperand::Constant { ty, value });
    }

    let place = RecordPlaceRef::parse(operand)?;
    if !place.projection.is_empty() {
        return (place.projection.as_slice() == ["field0"])
            .then_some(RecordExpressionOperand::Local(place.local));
    }

    if let Some((arg_index, arg)) = args
        .iter()
        .enumerate()
        .find(|(_, arg)| arg.index == place.local && is_scalar_arg(arg))
    {
        return Some(RecordExpressionOperand::ScalarArg {
            arg_index,
            local: arg.index,
        });
    }

    Some(RecordExpressionOperand::Local(place.local))
}

fn parse_record_const_operand(operand: &str) -> Option<(String, String)> {
    let rest = operand.strip_prefix("const:")?;
    let (ty, value) = rest.split_once('=')?;
    Some((ty.to_string(), value.to_string()))
}

fn record_binary_op(operation: &str) -> Option<RecordBinaryOp> {
    match operation {
        "add" => Some(RecordBinaryOp::Add),
        "sub" => Some(RecordBinaryOp::Sub),
        "mul" => Some(RecordBinaryOp::Mul),
        "div" => Some(RecordBinaryOp::Div),
        _ => None,
    }
}

fn record_unary_op(operation: &str) -> Option<RecordUnaryOp> {
    match operation {
        "neg" => Some(RecordUnaryOp::Neg),
        _ => None,
    }
}

fn is_scalar_arg(arg: &RecordLoweringLocal) -> bool {
    matches!(arg.ty.as_str(), "mir.f32" | "mir.f64")
}

fn is_lowering_op(op: MirOp) -> bool {
    LOWERING_OPS.contains(&op)
}

const LOWERING_OPS: &[MirOp] = &[
    MirOp::Assign,
    MirOp::Add,
    MirOp::Sub,
    MirOp::Mul,
    MirOp::Div,
    MirOp::Eq,
    MirOp::Lt,
    MirOp::Le,
    MirOp::Ne,
    MirOp::Ge,
    MirOp::Gt,
    MirOp::Cmp,
    MirOp::Cast,
    MirOp::Load,
    MirOp::Store,
    MirOp::Gep,
    MirOp::SliceLen,
    MirOp::SlicePtr,
    MirOp::ThreadIndex1d,
    MirOp::Branch,
    MirOp::CondBranch,
    MirOp::Switch,
    MirOp::Call,
    MirOp::Return,
    MirOp::Assert,
    MirOp::Unreachable,
];

fn string_attr<'a>(record: &'a MirOpRecord, name: &'static str) -> Option<&'a str> {
    record.attrs.iter().find_map(|attr| {
        if attr.name == name
            && let MirAttrValue::String(value) = &attr.value
        {
            return Some(value.as_str());
        }
        None
    })
}

fn usize_attr(record: &MirOpRecord, name: &'static str) -> Option<usize> {
    record.attrs.iter().find_map(|attr| {
        if attr.name == name
            && let MirAttrValue::Usize(value) = &attr.value
        {
            return Some(*value);
        }
        None
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use dialect_mir::MirAttr;

    fn trust_calls_for_test(
        plan: &mut RecordLoweringPlan,
        function: &str,
        items: &[TrustedDeviceItem],
    ) {
        let function = plan
            .functions
            .iter_mut()
            .find(|candidate| candidate.symbol == function)
            .expect("test function");
        let calls = function
            .ops
            .iter_mut()
            .filter(|op| op.op == MirOp::Call)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), items.len());
        for (call, item) in calls.into_iter().zip(items) {
            call.trusted_callee = Some(*item);
        }
    }

    #[test]
    fn groups_lowering_ops_by_function() {
        let records = vec![
            MirOpRecord::new(MirOp::Module).with_attr(MirAttr::usize("functions", 1)),
            MirOpRecord::new(MirOp::Func)
                .with_attr(MirAttr::string("symbol", "copy"))
                .with_attr(MirAttr::string("kind", "kernel"))
                .with_attr(MirAttr::usize("args", 2))
                .with_attr(MirAttr::usize("locals", 5))
                .with_attr(MirAttr::usize("blocks", 2)),
            MirOpRecord::new(MirOp::Local)
                .with_attr(MirAttr::string("function", "copy"))
                .with_attr(MirAttr::usize("index", 0))
                .with_attr(MirAttr::string("role", "return"))
                .with_attr(MirAttr::string("type", "mir.unit"))
                .with_attr(MirAttr::string("rust_type", "()")),
            MirOpRecord::new(MirOp::Arg)
                .with_attr(MirAttr::string("function", "copy"))
                .with_attr(MirAttr::usize("index", 1))
                .with_attr(MirAttr::string("role", "arg"))
                .with_attr(MirAttr::string("type", "mir.slice"))
                .with_attr(MirAttr::string("rust_type", "&[f32]")),
            MirOpRecord::new(MirOp::Load)
                .with_attr(MirAttr::string("function", "copy"))
                .with_attr(MirAttr::usize("block", 0))
                .with_attr(MirAttr::usize("statement", 1))
                .with_attr(MirAttr::string("source", "mir.assign"))
                .with_attr(MirAttr::string("operation", "use"))
                .with_attr(MirAttr::usize("destination_local", 3))
                .with_attr(MirAttr::string("destination", "local3"))
                .with_attr(MirAttr::usize("operand_count", 1))
                .with_attr(MirAttr::string("operands", "local1.deref.index_local2")),
            MirOpRecord::new(MirOp::Call)
                .with_attr(MirAttr::string("function", "copy"))
                .with_attr(MirAttr::usize("block", 0))
                .with_attr(MirAttr::string("callee", "fe2o3_device::thread::index_1d"))
                .with_attr(MirAttr::usize("target", 1))
                .with_attr(MirAttr::usize("destination_local", 4))
                .with_attr(MirAttr::string("destination", "local4")),
            MirOpRecord::new(MirOp::Return)
                .with_attr(MirAttr::string("function", "copy"))
                .with_attr(MirAttr::usize("block", 1)),
        ];

        let mut plan = plan_from_records(&records);
        trust_calls_for_test(&mut plan, "copy", &[TrustedDeviceItem::ThreadIndex1d]);

        assert_eq!(plan.functions.len(), 1);
        assert_eq!(plan.functions[0].symbol, "copy");
        assert_eq!(plan.functions[0].kind, "kernel");
        assert_eq!(plan.functions[0].arg_count, 2);
        assert_eq!(plan.functions[0].local_count, 5);
        assert_eq!(plan.functions[0].block_count, 2);
        assert_eq!(plan.functions[0].args().len(), 1);
        assert_eq!(plan.functions[0].args()[0].ty, "mir.slice");
        assert_eq!(plan.functions[0].ops.len(), 3);
        assert_eq!(plan.functions[0].ops[0].op, MirOp::Load);
        assert_eq!(plan.functions[0].ops[0].statement, Some(1));
        assert_eq!(
            plan.functions[0].ops[0].source.as_deref(),
            Some("mir.assign")
        );
        assert_eq!(plan.functions[0].ops[0].operation.as_deref(), Some("use"));
        assert_eq!(plan.functions[0].ops[0].destination_local, Some(3));
        assert_eq!(plan.functions[0].ops[0].operand_count, Some(1));
        assert_eq!(plan.functions[0].ops_by(MirOp::Call).len(), 1);
        assert!(plan.functions[0].has_trusted_call(TrustedDeviceItem::ThreadIndex1d));
        assert_eq!(plan.functions[0].op_count(MirOp::Return), 1);
        let access = plan.functions[0].access_sketch();
        assert_eq!(access.loads.len(), 1);
        assert_eq!(access.loads[0].place.local, 1);
        assert_eq!(access.loads[0].index_local, Some(2));
        assert!(plan.function("copy").is_some());
        assert!(plan.functions[0].has_op(MirOp::Return));
    }

    #[test]
    fn parses_record_place_labels() {
        let place = RecordPlaceRef::parse("local12.deref.index_local7").unwrap();

        assert_eq!(place.local, 12);
        assert_eq!(place.projection, vec!["deref", "index_local7"]);
        assert_eq!(place.index_local(), Some(7));
        assert!(RecordPlaceRef::parse("const:mir.usize=1").is_none());
    }

    #[test]
    fn sketches_thread_index_helper_calls() {
        let records = vec![
            MirOpRecord::new(MirOp::Func)
                .with_attr(MirAttr::string("symbol", "gather_odd"))
                .with_attr(MirAttr::string("kind", "kernel")),
            MirOpRecord::new(MirOp::Call)
                .with_attr(MirAttr::string("function", "gather_odd"))
                .with_attr(MirAttr::string(
                    "callee",
                    "fe2o3_device::thread::index_1d",
                ))
                .with_attr(MirAttr::usize("destination_local", 3))
                .with_attr(MirAttr::string("destination", "local3")),
            MirOpRecord::new(MirOp::Call)
                .with_attr(MirAttr::string("function", "gather_odd"))
                .with_attr(MirAttr::string(
                    "callee",
                    "fe2o3_device::ThreadIndex::stride_offset",
                ))
                .with_attr(MirAttr::usize("destination_local", 4))
                .with_attr(MirAttr::string("destination", "local4"))
                .with_attr(MirAttr::usize("operand_count", 3))
                .with_attr(MirAttr::string(
                    "operands",
                    "local3, const:mir.usize=Val(Scalar(0x0000000000000002), usize), const:mir.isize=Val(Scalar(0x0000000000000001), isize)",
                )),
        ];

        let mut plan = plan_from_records(&records);
        trust_calls_for_test(
            &mut plan,
            "gather_odd",
            &[
                TrustedDeviceItem::ThreadIndex1d,
                TrustedDeviceItem::ThreadIndexStrideOffset,
            ],
        );
        let function = plan.function("gather_odd").unwrap();
        let sketch = function.index_sketch();

        assert_eq!(sketch.thread_index_local, Some(3));
        assert_eq!(
            sketch.get(4),
            Some(RecordLinearIndex {
                stride: 2,
                offset: 1
            })
        );
    }

    #[test]
    fn sketches_thread_index_helpers_through_shared_reference_temporaries() {
        let records = vec![
            MirOpRecord::new(MirOp::Func)
                .with_attr(MirAttr::string("symbol", "borrowed_index"))
                .with_attr(MirAttr::string("kind", "kernel")),
            MirOpRecord::new(MirOp::Call)
                .with_attr(MirAttr::string("function", "borrowed_index"))
                .with_attr(MirAttr::string("callee", "fe2o3_device::thread::index_1d"))
                .with_attr(MirAttr::usize("destination_local", 3)),
            MirOpRecord::new(MirOp::Assign)
                .with_attr(MirAttr::string("function", "borrowed_index"))
                .with_attr(MirAttr::usize("destination_local", 4))
                .with_attr(MirAttr::string("operation", "ref"))
                .with_attr(MirAttr::string("operands", "local3")),
            MirOpRecord::new(MirOp::Call)
                .with_attr(MirAttr::string("function", "borrowed_index"))
                .with_attr(MirAttr::string("callee", "fe2o3_device::ThreadIndex::get"))
                .with_attr(MirAttr::usize("destination_local", 5))
                .with_attr(MirAttr::string("operands", "local4")),
        ];

        let mut plan = plan_from_records(&records);
        trust_calls_for_test(
            &mut plan,
            "borrowed_index",
            &[
                TrustedDeviceItem::ThreadIndex1d,
                TrustedDeviceItem::ThreadIndexGet,
            ],
        );
        let sketch = plan.function("borrowed_index").unwrap().index_sketch();

        assert_eq!(sketch.thread_index_local, Some(3));
        assert_eq!(sketch.get(5), Some(RecordLinearIndex::thread()));
    }

    #[test]
    fn overwritten_thread_index_reference_is_not_treated_as_an_alias() {
        let records = vec![
            MirOpRecord::new(MirOp::Func)
                .with_attr(MirAttr::string("symbol", "overwritten_index"))
                .with_attr(MirAttr::string("kind", "kernel")),
            MirOpRecord::new(MirOp::Call)
                .with_attr(MirAttr::string("function", "overwritten_index"))
                .with_attr(MirAttr::usize("destination_local", 3)),
            MirOpRecord::new(MirOp::Assign)
                .with_attr(MirAttr::string("function", "overwritten_index"))
                .with_attr(MirAttr::usize("destination_local", 4))
                .with_attr(MirAttr::string("operation", "ref"))
                .with_attr(MirAttr::string("operands", "local3")),
            MirOpRecord::new(MirOp::Assign)
                .with_attr(MirAttr::string("function", "overwritten_index"))
                .with_attr(MirAttr::usize("destination_local", 4))
                .with_attr(MirAttr::string("operation", "use"))
                .with_attr(MirAttr::string("operands", "local8")),
            MirOpRecord::new(MirOp::Call)
                .with_attr(MirAttr::string("function", "overwritten_index"))
                .with_attr(MirAttr::usize("destination_local", 5))
                .with_attr(MirAttr::string("operands", "local4")),
        ];

        let mut plan = plan_from_records(&records);
        trust_calls_for_test(
            &mut plan,
            "overwritten_index",
            &[
                TrustedDeviceItem::ThreadIndex1d,
                TrustedDeviceItem::ThreadIndexGet,
            ],
        );
        let sketch = plan.function("overwritten_index").unwrap().index_sketch();

        assert_eq!(sketch.thread_index_local, Some(3));
        assert_eq!(sketch.get(5), None);
    }

    #[test]
    fn diagnostic_path_and_forged_tag_cannot_bind_index() {
        let records = vec![
            MirOpRecord::new(MirOp::Func)
                .with_attr(MirAttr::string("symbol", "lookalike"))
                .with_attr(MirAttr::string("kind", "kernel")),
            MirOpRecord::new(MirOp::Call)
                .with_attr(MirAttr::string("function", "lookalike"))
                .with_attr(MirAttr::string("callee", "fe2o3_device::thread::index_1d"))
                .with_attr(MirAttr::string("trusted_callee", "thread_index_1d"))
                .with_attr(MirAttr::usize("destination_local", 3)),
        ];

        let plan = plan_from_records(&records);
        let function = plan.function("lookalike").unwrap();

        assert!(!function.has_trusted_call(TrustedDeviceItem::ThreadIndex1d));
        assert_eq!(function.index_sketch().thread_index_local, None);
    }

    #[test]
    fn sketches_raw_index_arithmetic_and_overflow_field_projection() {
        let records = vec![
            MirOpRecord::new(MirOp::Func)
                .with_attr(MirAttr::string("symbol", "raw_output_shift"))
                .with_attr(MirAttr::string("kind", "kernel")),
            MirOpRecord::new(MirOp::Call)
                .with_attr(MirAttr::string("function", "raw_output_shift"))
                .with_attr(MirAttr::string("callee", "fe2o3_device::thread::index_1d"))
                .with_attr(MirAttr::usize("destination_local", 3)),
            MirOpRecord::new(MirOp::Call)
                .with_attr(MirAttr::string("function", "raw_output_shift"))
                .with_attr(MirAttr::string("callee", "fe2o3_device::ThreadIndex::get"))
                .with_attr(MirAttr::usize("destination_local", 4))
                .with_attr(MirAttr::string("operands", "local3")),
            MirOpRecord::new(MirOp::Add)
                .with_attr(MirAttr::string("function", "raw_output_shift"))
                .with_attr(MirAttr::usize("destination_local", 6))
                .with_attr(MirAttr::string("operation", "add_with_overflow"))
                .with_attr(MirAttr::string(
                    "operands",
                    "local4, const:mir.usize=Val(Scalar(0x0000000000000001), usize)",
                )),
            MirOpRecord::new(MirOp::Assign)
                .with_attr(MirAttr::string("function", "raw_output_shift"))
                .with_attr(MirAttr::usize("destination_local", 5))
                .with_attr(MirAttr::string("operation", "use"))
                .with_attr(MirAttr::string("operands", "local6.field0")),
        ];

        let mut plan = plan_from_records(&records);
        trust_calls_for_test(
            &mut plan,
            "raw_output_shift",
            &[
                TrustedDeviceItem::ThreadIndex1d,
                TrustedDeviceItem::ThreadIndexGet,
            ],
        );
        let function = plan.function("raw_output_shift").unwrap();
        let sketch = function.index_sketch();

        assert_eq!(sketch.get(4), Some(RecordLinearIndex::thread()));
        assert_eq!(
            sketch.get(5),
            Some(RecordLinearIndex {
                stride: 1,
                offset: 1
            })
        );
    }

    #[test]
    fn sketches_record_slice_accesses_from_memory_and_indexes() {
        let records = vec![
            MirOpRecord::new(MirOp::Func)
                .with_attr(MirAttr::string("symbol", "raw_output_shift"))
                .with_attr(MirAttr::string("kind", "kernel")),
            MirOpRecord::new(MirOp::Arg)
                .with_attr(MirAttr::string("function", "raw_output_shift"))
                .with_attr(MirAttr::usize("index", 1))
                .with_attr(MirAttr::string("role", "arg"))
                .with_attr(MirAttr::string("type", "mir.slice"))
                .with_attr(MirAttr::string("rust_type", "&[f32]")),
            MirOpRecord::new(MirOp::Arg)
                .with_attr(MirAttr::string("function", "raw_output_shift"))
                .with_attr(MirAttr::usize("index", 2))
                .with_attr(MirAttr::string("role", "arg"))
                .with_attr(MirAttr::string("type", "mir.slice"))
                .with_attr(MirAttr::string("rust_type", "&mut [f32]")),
            MirOpRecord::new(MirOp::Call)
                .with_attr(MirAttr::string("function", "raw_output_shift"))
                .with_attr(MirAttr::string("callee", "fe2o3_device::thread::index_1d"))
                .with_attr(MirAttr::usize("destination_local", 3)),
            MirOpRecord::new(MirOp::Call)
                .with_attr(MirAttr::string("function", "raw_output_shift"))
                .with_attr(MirAttr::string("callee", "fe2o3_device::ThreadIndex::get"))
                .with_attr(MirAttr::usize("destination_local", 4))
                .with_attr(MirAttr::string("operands", "local3")),
            MirOpRecord::new(MirOp::Add)
                .with_attr(MirAttr::string("function", "raw_output_shift"))
                .with_attr(MirAttr::usize("destination_local", 6))
                .with_attr(MirAttr::string("operation", "add_with_overflow"))
                .with_attr(MirAttr::string(
                    "operands",
                    "local4, const:mir.usize=Val(Scalar(0x0000000000000001), usize)",
                )),
            MirOpRecord::new(MirOp::Assign)
                .with_attr(MirAttr::string("function", "raw_output_shift"))
                .with_attr(MirAttr::usize("destination_local", 5))
                .with_attr(MirAttr::string("operation", "use"))
                .with_attr(MirAttr::string("operands", "local6.field0")),
            MirOpRecord::new(MirOp::Load)
                .with_attr(MirAttr::string("function", "raw_output_shift"))
                .with_attr(MirAttr::string("operation", "use"))
                .with_attr(MirAttr::usize("destination_local", 7))
                .with_attr(MirAttr::string("destination", "local7"))
                .with_attr(MirAttr::usize("operand_count", 1))
                .with_attr(MirAttr::string("operands", "local1.deref.index_local4")),
            MirOpRecord::new(MirOp::Store)
                .with_attr(MirAttr::string("function", "raw_output_shift"))
                .with_attr(MirAttr::string("operation", "use"))
                .with_attr(MirAttr::string("destination", "local2.deref.index_local5"))
                .with_attr(MirAttr::usize("operand_count", 1))
                .with_attr(MirAttr::string("operands", "local7")),
        ];

        let mut plan = plan_from_records(&records);
        trust_calls_for_test(
            &mut plan,
            "raw_output_shift",
            &[
                TrustedDeviceItem::ThreadIndex1d,
                TrustedDeviceItem::ThreadIndexGet,
            ],
        );
        let function = plan.function("raw_output_shift").unwrap();
        let sketch = function.slice_access_sketch();

        assert_eq!(
            sketch.reads,
            vec![RecordSliceAccess {
                arg_index: 0,
                local: 1,
                index: RecordLinearIndex {
                    stride: 1,
                    offset: 0
                }
            }]
        );
        assert_eq!(
            sketch.writes,
            vec![RecordSliceAccess {
                arg_index: 1,
                local: 2,
                index: RecordLinearIndex {
                    stride: 1,
                    offset: 1
                }
            }]
        );
    }

    #[test]
    fn sketches_record_expressions_from_loads_args_literals_and_stores() {
        let records = vec![
            MirOpRecord::new(MirOp::Func)
                .with_attr(MirAttr::string("symbol", "expr"))
                .with_attr(MirAttr::string("kind", "kernel")),
            MirOpRecord::new(MirOp::Arg)
                .with_attr(MirAttr::string("function", "expr"))
                .with_attr(MirAttr::usize("index", 1))
                .with_attr(MirAttr::string("role", "arg"))
                .with_attr(MirAttr::string("type", "mir.f32"))
                .with_attr(MirAttr::string("rust_type", "f32")),
            MirOpRecord::new(MirOp::Arg)
                .with_attr(MirAttr::string("function", "expr"))
                .with_attr(MirAttr::usize("index", 2))
                .with_attr(MirAttr::string("role", "arg"))
                .with_attr(MirAttr::string("type", "mir.slice"))
                .with_attr(MirAttr::string("rust_type", "&[f32]")),
            MirOpRecord::new(MirOp::Arg)
                .with_attr(MirAttr::string("function", "expr"))
                .with_attr(MirAttr::usize("index", 3))
                .with_attr(MirAttr::string("role", "arg"))
                .with_attr(MirAttr::string("type", "mir.disjoint_slice"))
                .with_attr(MirAttr::string(
                    "rust_type",
                    "fe2o3_device::DisjointSlice<f32>",
                )),
            MirOpRecord::new(MirOp::Call)
                .with_attr(MirAttr::string("function", "expr"))
                .with_attr(MirAttr::string("callee", "fe2o3_device::thread::index_1d"))
                .with_attr(MirAttr::usize("destination_local", 4)),
            MirOpRecord::new(MirOp::Call)
                .with_attr(MirAttr::string("function", "expr"))
                .with_attr(MirAttr::string("callee", "fe2o3_device::ThreadIndex::get"))
                .with_attr(MirAttr::usize("destination_local", 5))
                .with_attr(MirAttr::string("operands", "local4")),
            MirOpRecord::new(MirOp::Load)
                .with_attr(MirAttr::string("function", "expr"))
                .with_attr(MirAttr::string("operation", "use"))
                .with_attr(MirAttr::usize("destination_local", 6))
                .with_attr(MirAttr::string("destination", "local6"))
                .with_attr(MirAttr::usize("operand_count", 1))
                .with_attr(MirAttr::string("operands", "local2.deref.index_local5")),
            MirOpRecord::new(MirOp::Mul)
                .with_attr(MirAttr::string("function", "expr"))
                .with_attr(MirAttr::usize("destination_local", 7))
                .with_attr(MirAttr::string("operation", "mul"))
                .with_attr(MirAttr::string("operands", "local1, local6")),
            MirOpRecord::new(MirOp::Sub)
                .with_attr(MirAttr::string("function", "expr"))
                .with_attr(MirAttr::usize("destination_local", 8))
                .with_attr(MirAttr::string("operation", "sub"))
                .with_attr(MirAttr::string(
                    "operands",
                    "local7, const:mir.f32=Val(Scalar(0x3fc00000), f32)",
                )),
            MirOpRecord::new(MirOp::Store)
                .with_attr(MirAttr::string("function", "expr"))
                .with_attr(MirAttr::string("operation", "div"))
                .with_attr(MirAttr::string("destination", "local9.deref"))
                .with_attr(MirAttr::string(
                    "operands",
                    "local8, const:mir.f32=Val(Scalar(0x40000000), f32)",
                )),
            MirOpRecord::new(MirOp::Store)
                .with_attr(MirAttr::string("function", "expr"))
                .with_attr(MirAttr::string("operation", "neg"))
                .with_attr(MirAttr::string("destination", "local10.deref"))
                .with_attr(MirAttr::string("operands", "local6")),
        ];

        let mut plan = plan_from_records(&records);
        trust_calls_for_test(
            &mut plan,
            "expr",
            &[
                TrustedDeviceItem::ThreadIndex1d,
                TrustedDeviceItem::ThreadIndexGet,
            ],
        );
        let function = plan.function("expr").unwrap();
        let sketch = function.expression_sketch();

        assert_eq!(sketch.binary_op_count(RecordBinaryOp::Mul), 1);
        assert_eq!(sketch.binary_op_count(RecordBinaryOp::Sub), 1);
        assert_eq!(sketch.binary_op_count(RecordBinaryOp::Div), 1);
        assert_eq!(sketch.unary_op_count(RecordUnaryOp::Neg), 1);
        assert!(sketch.uses_scalar_arg(0));
        assert!(sketch.uses_constant("mir.f32", "0x3fc00000"));
        assert!(sketch.uses_constant("mir.f32", "0x40000000"));
        assert_eq!(sketch.stores.len(), 2);
        assert!(matches!(
            &sketch.local_bindings[0].expr,
            RecordExpression::Use(RecordExpressionOperand::SliceElement(access))
                if access.arg_index == 1 && access.local == 2
        ));
    }

    #[test]
    fn parses_signed_record_constants() {
        assert_eq!(
            parse_record_signed_const("const:mir.isize=Val(Scalar(0xffffffffffffffff), isize)"),
            Some(-1)
        );
        assert_eq!(
            parse_record_unsigned_const("const:mir.usize=Unevaluated(..., usize);eval_u64=1023"),
            Some(1023)
        );
    }

    #[test]
    fn splits_record_operands_without_breaking_constant_values() {
        assert_eq!(
            split_record_operands("local4, const:mir.usize=Val(Scalar(0x0000000000000001), usize)"),
            vec![
                "local4".to_string(),
                "const:mir.usize=Val(Scalar(0x0000000000000001), usize)".to_string(),
            ]
        );
    }

    #[test]
    fn summary_reports_deterministic_op_counts() {
        let records = vec![
            MirOpRecord::new(MirOp::Func)
                .with_attr(MirAttr::string("symbol", "copy"))
                .with_attr(MirAttr::string("kind", "kernel"))
                .with_attr(MirAttr::usize("args", 2))
                .with_attr(MirAttr::usize("locals", 5))
                .with_attr(MirAttr::usize("blocks", 1)),
            MirOpRecord::new(MirOp::Load).with_attr(MirAttr::string("function", "copy")),
            MirOpRecord::new(MirOp::Store).with_attr(MirAttr::string("function", "copy")),
            MirOpRecord::new(MirOp::Return).with_attr(MirAttr::string("function", "copy")),
        ];

        let summary = plan_from_records(&records).summary();

        assert!(summary.contains("[kernel] copy: 1 bb, 5 locals, 2 args, 3 lowering op(s)"));
        assert!(summary.contains("mir.load=1, mir.store=1, mir.return=1"));
    }
}

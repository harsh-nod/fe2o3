use dialect_mir::{MirAttrValue, MirOp, MirOpRecord};
use std::collections::HashMap;
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

    pub fn has_call_suffix(&self, suffix: &str) -> bool {
        self.ops_by(MirOp::Call).into_iter().any(|candidate| {
            candidate
                .callee
                .as_deref()
                .is_some_and(|callee| callee.ends_with(suffix))
        })
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
        let mut bindings = HashMap::new();

        for op in &self.ops {
            if op.op == MirOp::Call {
                bind_call_index(op, &mut thread_index_local, &mut bindings);
                continue;
            }

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

impl RecordLoweringOp {
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
    bindings: &mut HashMap<usize, RecordLinearIndex>,
) {
    let Some(callee) = op.callee.as_deref() else {
        return;
    };
    let Some(destination) = op.destination_local else {
        return;
    };

    if callee.ends_with("fe2o3_device::thread::index_1d") {
        *thread_index_local = Some(destination);
        return;
    }

    let Some(thread_index_local) = *thread_index_local else {
        return;
    };
    let operands = op.operand_labels();
    let Some(receiver) = operands
        .first()
        .and_then(|operand| RecordPlaceRef::parse(operand))
    else {
        return;
    };
    if receiver.local != thread_index_local {
        return;
    }

    let index = if callee.ends_with("fe2o3_device::ThreadIndex::get") {
        Some(RecordLinearIndex::thread())
    } else if callee.ends_with("fe2o3_device::ThreadIndex::offset") {
        operands
            .get(1)
            .and_then(|operand| parse_record_unsigned_const(operand))
            .and_then(|offset| RecordLinearIndex::thread().offset(offset))
    } else if callee.ends_with("fe2o3_device::ThreadIndex::offset_signed") {
        operands
            .get(1)
            .and_then(|operand| parse_record_signed_const(operand))
            .and_then(|offset| RecordLinearIndex::thread().offset(offset))
    } else if callee.ends_with("fe2o3_device::ThreadIndex::stride") {
        operands
            .get(1)
            .and_then(|operand| parse_record_unsigned_const(operand))
            .map(|stride| RecordLinearIndex { stride, offset: 0 })
    } else if callee.ends_with("fe2o3_device::ThreadIndex::stride_offset") {
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
    Some(
        operand[start..]
            .split(|ch: char| !(ch == '-' || ch.is_ascii_digit()))
            .next()?,
    )
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
        if attr.name == name {
            if let MirAttrValue::String(value) = &attr.value {
                return Some(value.as_str());
            }
        }
        None
    })
}

fn usize_attr(record: &MirOpRecord, name: &'static str) -> Option<usize> {
    record.attrs.iter().find_map(|attr| {
        if attr.name == name {
            if let MirAttrValue::Usize(value) = &attr.value {
                return Some(*value);
            }
        }
        None
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use dialect_mir::MirAttr;

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

        let plan = plan_from_records(&records);

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
        assert!(plan.functions[0].has_call_suffix("thread::index_1d"));
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

        let plan = plan_from_records(&records);
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

        let plan = plan_from_records(&records);
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

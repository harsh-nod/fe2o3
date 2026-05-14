use dialect_mir::{MirAttrValue, MirOp, MirOpRecord};
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
        self.ops.iter().any(|candidate| candidate.op == op)
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

fn is_lowering_op(op: MirOp) -> bool {
    LOWERING_OPS.contains(&op)
}

const LOWERING_OPS: &[MirOp] = &[
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
        assert!(plan.function("copy").is_some());
        assert!(plan.functions[0].has_op(MirOp::Return));
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

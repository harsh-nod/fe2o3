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
    pub block_count: usize,
    pub ops: Vec<RecordLoweringOp>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordLoweringOp {
    pub op: MirOp,
    pub block: Option<usize>,
    pub statement: Option<usize>,
    pub destination: Option<String>,
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
                block_count: usize_attr(record, "blocks").unwrap_or(0),
                ops: Vec::new(),
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
            destination: string_attr(record, "destination").map(str::to_string),
            operands: string_attr(record, "operands").map(str::to_string),
        });
    }

    RecordLoweringPlan { functions }
}

impl RecordLoweringPlan {
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
                "  [{}] {}: {} bb, {} lowering op(s)",
                function.kind,
                function.symbol,
                function.block_count,
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
                .with_attr(MirAttr::usize("blocks", 2)),
            MirOpRecord::new(MirOp::Load)
                .with_attr(MirAttr::string("function", "copy"))
                .with_attr(MirAttr::usize("block", 0))
                .with_attr(MirAttr::usize("statement", 1))
                .with_attr(MirAttr::string("destination", "local3"))
                .with_attr(MirAttr::string("operands", "local1.deref.index_local2")),
            MirOpRecord::new(MirOp::Return)
                .with_attr(MirAttr::string("function", "copy"))
                .with_attr(MirAttr::usize("block", 1)),
        ];

        let plan = plan_from_records(&records);

        assert_eq!(plan.functions.len(), 1);
        assert_eq!(plan.functions[0].symbol, "copy");
        assert_eq!(plan.functions[0].kind, "kernel");
        assert_eq!(plan.functions[0].block_count, 2);
        assert_eq!(plan.functions[0].ops.len(), 2);
        assert_eq!(plan.functions[0].ops[0].op, MirOp::Load);
        assert_eq!(plan.functions[0].ops[0].statement, Some(1));
    }

    #[test]
    fn summary_reports_deterministic_op_counts() {
        let records = vec![
            MirOpRecord::new(MirOp::Func)
                .with_attr(MirAttr::string("symbol", "copy"))
                .with_attr(MirAttr::string("kind", "kernel"))
                .with_attr(MirAttr::usize("blocks", 1)),
            MirOpRecord::new(MirOp::Load).with_attr(MirAttr::string("function", "copy")),
            MirOpRecord::new(MirOp::Store).with_attr(MirAttr::string("function", "copy")),
            MirOpRecord::new(MirOp::Return).with_attr(MirAttr::string("function", "copy")),
        ];

        let summary = plan_from_records(&records).summary();

        assert!(summary.contains("[kernel] copy: 1 bb, 3 lowering op(s)"));
        assert!(summary.contains("mir.load=1, mir.store=1, mir.return=1"));
    }
}

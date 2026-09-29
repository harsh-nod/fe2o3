//! Temporary, opt-in observation of the unchanged uniformity analysis.

use super::Analyzer;
use crate::{AnalysisReport, Diagnostic};
use fe2o3_kernel_ir::{
    BlockId, FunctionBody, Module, OperationKind, Terminator, Type, ValueDef, ValueId,
};
use std::fmt::{self, Write as _};
use std::io::{self, Write};
use std::sync::atomic::{AtomicUsize, Ordering};

const MAX_DUMPS: usize = 8;
const MAX_BLOCKS: usize = 512;
const MAX_OPERATIONS: usize = 8192;
const MAX_ROWS: usize = 32768;
const MAX_BYTES: usize = 4 * 1024 * 1024;
const MAX_ITEMS: usize = 64;
const MAX_FUNCTIONS: usize = 1024;
const MAX_NAME_BYTES: usize = 96;
static ORIGINAL_DUMPS: AtomicUsize = AtomicUsize::new(0);
static ANALYZED_DUMPS: AtomicUsize = AtomicUsize::new(0);

#[path = "diagnostic_selector_slice_v1794.rs"]
mod selector_slice;

fn enabled() -> bool {
    std::env::var_os("FE2O3_DIAG_UNIFORMITY_V1794").as_deref() == Some(std::ffi::OsStr::new("1"))
}

fn reserve(counter: &AtomicUsize, phase: &str) -> Option<usize> {
    match counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
        (n <= MAX_DUMPS).then_some(n + 1)
    }) {
        Ok(n) if n < MAX_DUMPS => Some(n),
        Ok(_) => {
            let _ = writeln!(
                io::stderr().lock(),
                "UNIFORMITY_V1794 OMIT phase={phase} reason=dump_limit limit={MAX_DUMPS}"
            );
            None
        }
        Err(_) => None,
    }
}

struct Name<'a>(&'a str);
impl fmt::Display for Name<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0.as_bytes().iter().take(MAX_NAME_BYTES) {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

struct Line {
    bytes: [u8; 2048],
    len: usize,
}
impl fmt::Write for Line {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let Some(end) = self
            .len
            .checked_add(text.len())
            .filter(|n| *n <= self.bytes.len())
        else {
            return Err(fmt::Error);
        };
        self.bytes[self.len..end].copy_from_slice(text.as_bytes());
        self.len = end;
        Ok(())
    }
}

struct Trace<W> {
    writer: W,
    rows: usize,
    bytes: usize,
    truncated: bool,
    output_error: bool,
    unknown_operations: usize,
    row_limit: usize,
    byte_limit: usize,
}

impl<W: Write> Trace<W> {
    fn new(writer: W) -> Self {
        Self {
            writer,
            rows: 0,
            bytes: 0,
            truncated: false,
            output_error: false,
            unknown_operations: 0,
            row_limit: MAX_ROWS,
            byte_limit: MAX_BYTES,
        }
    }

    fn stopped(&self) -> bool {
        self.output_error || self.rows >= self.row_limit || self.bytes >= self.byte_limit
    }

    fn row(&mut self, args: fmt::Arguments<'_>) {
        if self.output_error {
            return;
        }
        let mut line = Line {
            bytes: [0; 2048],
            len: 0,
        };
        if line.write_str("UNIFORMITY_V1794 ").is_err()
            || line.write_fmt(args).is_err()
            || line.write_char('\n').is_err()
            || self.rows >= self.row_limit
            || line.len > self.byte_limit.saturating_sub(self.bytes)
        {
            self.truncated = true;
            return;
        }
        self.rows += 1;
        self.bytes += line.len;
        if self.writer.write_all(&line.bytes[..line.len]).is_err() {
            self.output_error = true;
        }
    }

    fn finish(&mut self, phase: &str, dump: usize) {
        // The bounded final frame is outside the data allowance. An I/O failure
        // can prevent delivery; a missing END never means complete observation.
        let _ = writeln!(
            self.writer,
            "UNIFORMITY_V1794 END phase={phase} dump={dump} rows={} bytes={} truncated={} output_error={} unknown_operations={}",
            self.rows,
            self.bytes,
            usize::from(self.truncated),
            usize::from(self.output_error),
            self.unknown_operations
        );
    }

    fn name(&mut self, role: &str, name: &str) {
        self.row(format_args!(
            "NAME role={role} bytes={} prefix={} truncated={}",
            name.len(),
            Name(name),
            usize::from(name.len() > MAX_NAME_BYTES)
        ));
        self.truncated |= name.len() > MAX_NAME_BYTES;
    }

    fn values(&mut self, role: &str, values: &[ValueId]) {
        self.row(format_args!("VALUES role={role} count={}", values.len()));
        for (ordinal, value) in values.iter().take(MAX_ITEMS).enumerate() {
            self.row(format_args!(
                "VALUE role={role} ordinal={ordinal} id={}",
                value.0
            ));
        }
        self.truncated |= values.len() > MAX_ITEMS;
    }

    fn definitions(&mut self, role: &str, values: &[ValueDef], report: Option<&AnalysisReport>) {
        for (ordinal, value) in values.iter().take(MAX_ITEMS).enumerate() {
            self.row(format_args!(
                "DEF role={role} ordinal={ordinal} id={} scalar={:?} variation={:?}",
                value.id.0,
                value.ty.as_scalar(),
                report.map(|r| r.value(value.id))
            ));
        }
        self.truncated |= values.len() > MAX_ITEMS;
    }

    fn edge(&mut self, role: &str, target: BlockId, arguments: &[ValueId]) {
        self.row(format_args!("EDGE role={role} target={}", target.0));
        self.values(role, arguments);
    }

    fn terminator(&mut self, terminator: Option<&Terminator>) {
        match terminator {
            Some(Terminator::Branch { target, arguments }) => {
                self.edge("branch", *target, arguments)
            }
            Some(Terminator::ConditionalBranch {
                condition,
                then_target,
                then_arguments,
                else_target,
                else_arguments,
            }) => {
                self.row(format_args!(
                    "TERMINATOR kind=conditional selector={}",
                    condition.0
                ));
                self.edge("then", *then_target, then_arguments);
                self.edge("else", *else_target, else_arguments);
            }
            Some(Terminator::Switch {
                selector,
                cases,
                default_target,
                default_arguments,
            }) => {
                self.row(format_args!(
                    "TERMINATOR kind=switch selector={} cases={}",
                    selector.0,
                    cases.len()
                ));
                for case in cases.iter().take(MAX_ITEMS) {
                    self.row(format_args!("CASE value={}", case.value));
                    self.edge("case", case.target, &case.arguments);
                }
                self.truncated |= cases.len() > MAX_ITEMS;
                self.edge("default", *default_target, default_arguments);
            }
            Some(Terminator::IntegerSwitch {
                selector,
                cases,
                default_target,
                default_arguments,
            }) => {
                self.row(format_args!(
                    "TERMINATOR kind=integer_switch selector={} cases={}",
                    selector.0,
                    cases.len()
                ));
                for case in cases.iter().take(MAX_ITEMS) {
                    self.row(format_args!("CASE value={:?}", case.value));
                    self.edge("case", case.target, &case.arguments);
                }
                self.truncated |= cases.len() > MAX_ITEMS;
                self.edge("default", *default_target, default_arguments);
            }
            Some(Terminator::Return { values }) => self.values("return", values),
            Some(Terminator::Unreachable) => self.row(format_args!("TERMINATOR kind=unreachable")),
            None => self.row(format_args!("TERMINATOR kind=missing")),
        }
    }

    fn operation(&mut self, kind: &OperationKind) {
        match kind {
            OperationKind::Constant(value) => {
                self.row(format_args!("OP kind=constant value={value:?}"))
            }
            OperationKind::Intrinsic(value) => {
                self.row(format_args!("OP kind=intrinsic value={:?}", value.kind))
            }
            OperationKind::Unary { op, operand } => self.row(format_args!(
                "OP kind=unary op={op:?} operand={}",
                operand.0
            )),
            OperationKind::Binary { op, lhs, rhs } => self.row(format_args!(
                "OP kind=binary op={op:?} lhs={} rhs={}",
                lhs.0, rhs.0
            )),
            OperationKind::Compare {
                predicate,
                lhs,
                rhs,
            } => self.row(format_args!(
                "OP kind=compare predicate={predicate:?} lhs={} rhs={}",
                lhs.0, rhs.0
            )),
            OperationKind::Cast { kind, value, to } => self.row(format_args!(
                "OP kind=cast cast={kind:?} value={} scalar={:?}",
                value.0,
                to.as_scalar()
            )),
            OperationKind::Select {
                condition,
                true_value,
                false_value,
            } => self.row(format_args!(
                "OP kind=select condition={} true={} false={}",
                condition.0, true_value.0, false_value.0
            )),
            OperationKind::SliceLength { slice } => {
                self.row(format_args!("OP kind=slice_length slice={}", slice.0))
            }
            OperationKind::SliceData { slice } => {
                self.row(format_args!("OP kind=slice_data slice={}", slice.0))
            }
            OperationKind::GetElementPointer { base, offset } => self.row(format_args!(
                "OP kind=gep base={} offset={}",
                base.0, offset.0
            )),
            OperationKind::Load { pointer, access } => self.row(format_args!(
                "OP kind=load pointer={} access={access:?}",
                pointer.0
            )),
            OperationKind::Store {
                pointer,
                value,
                access,
            } => self.row(format_args!(
                "OP kind=store pointer={} value={} access={access:?}",
                pointer.0, value.0
            )),
            OperationKind::GuardedLoad {
                pointer,
                predicate,
                fallback,
                access,
            } => self.row(format_args!(
                "OP kind=guarded_load pointer={} predicate={} fallback={} access={access:?}",
                pointer.0, predicate.0, fallback.0
            )),
            OperationKind::GuardedStore {
                pointer,
                predicate,
                value,
                access,
            } => self.row(format_args!(
                "OP kind=guarded_store pointer={} predicate={} value={} access={access:?}",
                pointer.0, predicate.0, value.0
            )),
            OperationKind::Alloca {
                count,
                address_space,
                alignment,
                ..
            } => self.row(format_args!(
                "OP kind=alloca count={:?} space={address_space:?} alignment={alignment}",
                count.map(|v| v.0)
            )),
            OperationKind::Call { callee, arguments } => {
                self.row(format_args!("OP kind=call"));
                self.name("callee", callee.as_str());
                self.values("call", arguments);
            }
            OperationKind::Barrier(value) => self.row(format_args!(
                "OP kind=barrier scope={:?} memory_scope={:?}",
                value.execution_scope, value.memory_scope
            )),
            OperationKind::WorkgroupBarrier(value) => self.row(format_args!(
                "OP kind=workgroup_barrier memory_scope={:?} convergence={:?}",
                value.memory_scope, value.convergence
            )),
            _ => {
                self.unknown_operations += 1;
                self.row(format_args!("OP kind=other operands_not_dumped=1"));
            }
        }
    }

    fn body(
        &mut self,
        body: &FunctionBody,
        types: Option<&[Type]>,
        report: Option<&AnalysisReport>,
    ) {
        self.row(format_args!(
            "BODY parameters={} blocks={}",
            body.parameters.len(),
            body.blocks.len()
        ));
        for (ordinal, value) in body.parameters.iter().take(MAX_ITEMS).enumerate() {
            self.row(format_args!(
                "PARAM ordinal={ordinal} id={} scalar={:?} variation={:?}",
                value.0,
                types.and_then(|t| t.get(ordinal)).and_then(Type::as_scalar),
                report.map(|r| r.value(*value))
            ));
        }
        self.truncated |= body.parameters.len() > MAX_ITEMS || body.blocks.len() > MAX_BLOCKS;
        let mut operations = 0;
        for (ordinal, block) in body.blocks.iter().take(MAX_BLOCKS).enumerate() {
            if self.stopped() {
                self.truncated = true;
                break;
            }
            self.row(format_args!(
                "BLOCK ordinal={ordinal} id={} parameters={} operations={} control={:?}",
                block.id.0,
                block.parameters.len(),
                block.operations.len(),
                report.map(|r| r.block_control(block.id))
            ));
            self.definitions("block_parameter", &block.parameters, report);
            for (index, operation) in block.operations.iter().enumerate() {
                if operations == MAX_OPERATIONS || self.stopped() {
                    self.truncated = true;
                    break;
                }
                operations += 1;
                self.row(format_args!(
                    "SITE block={} operation={index} results={}",
                    block.id.0,
                    operation.results.len()
                ));
                self.operation(&operation.kind);
                self.definitions("result", &operation.results, report);
            }
            self.terminator(block.terminator.as_ref());
        }
    }
}

pub(super) fn original(module: &Module) {
    if !enabled() {
        return;
    }
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        for (ordinal, kernel) in module.kernels.iter().take(MAX_DUMPS).enumerate() {
            let Some(dump) = reserve(&ORIGINAL_DUMPS, "original") else {
                break;
            };
            let mut out = Trace::new(io::stderr().lock());
            out.row(format_args!(
                "BEGIN phase=original dump={dump} kernel_ordinal={ordinal} kernels={} functions={}",
                module.kernels.len(),
                module.functions.len()
            ));
            out.name("kernel", kernel.id.as_str());
            out.name("function", kernel.entry.as_str());
            out.row(format_args!(
                "KERNEL rank={:?} workgroup={:?} physical_max_grid=not_carried_by_kernel",
                kernel.domain.rank(),
                kernel.workgroup_size
            ));
            out.truncated |=
                module.kernels.len() > MAX_DUMPS || module.functions.len() > MAX_FUNCTIONS;
            match module
                .functions
                .iter()
                .take(MAX_FUNCTIONS)
                .find(|f| f.id == kernel.entry)
            {
                Some(function) => match &function.body {
                    Some(body) => {
                        out.selector_slice(body, Some(&function.signature.parameters), None)
                    }
                    None => out.row(format_args!("MISSING body=1")),
                },
                None => {
                    out.truncated = true;
                    out.row(format_args!("MISSING function_in_bounded_scan=1"));
                }
            }
            out.finish("original", dump);
        }
    }));
}

pub(super) fn analyzed(analyzer: &Analyzer<'_>) {
    if !enabled() {
        return;
    }
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let diagnostics = analyzer.report.diagnostics();
        if diagnostics.len() <= MAX_ITEMS
            && !diagnostics
                .iter()
                .any(|d| matches!(d, Diagnostic::DivergentBarrier { .. }))
        {
            return;
        }
        let Some(dump) = reserve(&ANALYZED_DUMPS, "analyzed") else {
            return;
        };
        let mut out = Trace::new(io::stderr().lock());
        out.row(format_args!(
            "BEGIN phase=analyzed dump={dump} diagnostics={} controller_regions={}",
            diagnostics.len(),
            analyzer.control_regions.len()
        ));
        out.name("function", analyzer.report.function().as_str());
        out.truncated |=
            diagnostics.len() > MAX_ITEMS || analyzer.control_regions.len() > MAX_BLOCKS;
        for diagnostic in diagnostics.iter().take(MAX_ITEMS) {
            let Diagnostic::DivergentBarrier {
                block,
                operation_index,
                execution_scope,
                control,
            } = diagnostic
            else {
                continue;
            };
            out.row(format_args!("FAIL block={} operation={operation_index} scope={execution_scope:?} control={control:?} control_unknown={}", block.0, usize::from(analyzer.control_unknown.contains(block))));
            for (source, region) in analyzer.control_regions.iter().take(MAX_BLOCKS) {
                if !region.contains(block) {
                    continue;
                }
                let source_block = analyzer
                    .body
                    .blocks
                    .iter()
                    .take(MAX_BLOCKS)
                    .find(|b| b.id == *source);
                out.row(format_args!(
                    "CONTROLLER failed_block={} source={} control={:?} found={}",
                    block.0,
                    source.0,
                    analyzer.report.block_control(*source),
                    usize::from(source_block.is_some())
                ));
                if let Some(source_block) = source_block {
                    out.terminator(source_block.terminator.as_ref());
                } else {
                    out.truncated = true;
                }
            }
        }
        out.selector_slice(analyzer.body, None, Some(&analyzer.report));
        out.finish("analyzed", dump);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_trace_final_omitted_row_is_explicit() {
        let mut out = Trace::new(Vec::new());
        out.row_limit = 1;
        out.row(format_args!("FIRST"));
        out.row(format_args!("LAST"));
        out.finish("test", 0);
        let text = String::from_utf8(out.writer).unwrap();
        assert!(!text.contains("LAST"));
        assert!(text.contains("truncated=1"));
    }

    #[test]
    fn bounded_trace_byte_and_line_caps_never_emit_partial_frames() {
        let mut out = Trace::new(Vec::new());
        out.byte_limit = 8;
        out.row(format_args!("FIRST"));
        out.finish("test", 0);
        assert!(out.truncated);
        assert!(
            String::from_utf8(out.writer)
                .unwrap()
                .starts_with("UNIFORMITY_V1794 END")
        );
        let mut out = Trace::new(Vec::new());
        out.row(format_args!("{}", "x".repeat(4096)));
        assert!(out.truncated);
        assert!(out.writer.is_empty());
    }

    #[test]
    fn bounded_trace_output_failure_is_observation_only() {
        struct Broken;
        impl Write for Broken {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::Error::other("deliberate diagnostic failure"))
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut out = Trace::new(Broken);
        out.row(format_args!("FIRST"));
        out.finish("test", 0);
        assert!(out.output_error);
    }

    #[test]
    fn bounded_trace_last_extra_argument_is_explicit() {
        let mut out = Trace::new(Vec::new());
        out.values("edge", &vec![ValueId(1); MAX_ITEMS + 1]);
        out.finish("test", 0);
        assert!(out.truncated);
        let text = String::from_utf8(out.writer).unwrap();
        assert_eq!(
            text.lines().filter(|l| l.contains("VALUE role=")).count(),
            MAX_ITEMS
        );
        assert!(text.contains("truncated=1"));
    }

    #[test]
    fn bounded_trace_retains_original_body_and_existing_failure_report() {
        use crate::Variation;
        use fe2o3_kernel_ir::{BasicBlock, Constant, Operation, SynchronizationScope};
        let body = FunctionBody {
            parameters: vec![],
            blocks: vec![BasicBlock {
                id: BlockId(4),
                parameters: vec![],
                operations: vec![Operation::new(
                    vec![],
                    OperationKind::Constant(Constant::U64(9)),
                )],
                terminator: Some(Terminator::Return { values: vec![] }),
            }],
        };
        let report = AnalysisReport {
            function: "diagnostic_control".into(),
            values: Default::default(),
            block_controls: [(BlockId(4), Variation::SubgroupUniform)].into(),
            diagnostics: vec![Diagnostic::DivergentBarrier {
                block: BlockId(4),
                operation_index: 0,
                execution_scope: SynchronizationScope::Workgroup,
                control: Variation::SubgroupUniform,
            }],
        };
        let before = (body.clone(), report.clone());
        let mut out = Trace::new(Vec::new());
        out.body(&body, None, Some(&report));
        out.finish("test", 0);
        assert_eq!((body, report), before);
        assert!(!out.truncated);
        assert!(!out.output_error);
        assert!(
            String::from_utf8(out.writer)
                .unwrap()
                .contains("control=Some(SubgroupUniform)")
        );
    }

    #[test]
    fn bounded_trace_extra_final_block_and_operation_are_explicit() {
        use fe2o3_kernel_ir::{BasicBlock, Constant, Operation};
        let mut body = FunctionBody {
            parameters: vec![],
            blocks: vec![BasicBlock::new(BlockId(0)); MAX_BLOCKS + 1],
        };
        let mut out = Trace::new(io::sink());
        out.body(&body, None, None);
        assert!(out.truncated);
        body.blocks.truncate(1);
        body.blocks[0].operations =
            vec![
                Operation::new(vec![], OperationKind::Constant(Constant::Bool(true)));
                MAX_OPERATIONS + 1
            ];
        let mut out = Trace::new(io::sink());
        out.body(&body, None, None);
        assert!(out.truncated);
    }
}

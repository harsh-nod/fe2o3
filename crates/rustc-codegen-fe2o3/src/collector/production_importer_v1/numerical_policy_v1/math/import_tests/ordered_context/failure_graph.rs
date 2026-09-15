//! Failure-only retained graph observation. No classifier or admission changes.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{SemanticCallableDeclV1, SemanticFunctionIdV1};
use std::fmt::{self, Write};

const MAX_BYTES: usize = 2 * 1024 * 1024;
const MAX_ITEMS: usize = 131_072;

struct Bounded {
    text: String,
    items: usize,
}

impl Bounded {
    fn item(&mut self) -> fmt::Result {
        self.items = self.items.checked_sub(1).ok_or(fmt::Error)?;
        Ok(())
    }
}

impl Write for Bounded {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        if text.len() > MAX_BYTES.saturating_sub(self.text.len()) {
            return Err(fmt::Error);
        }
        self.text.push_str(text);
        Ok(())
    }
}

pub(in super::super) fn snapshot(owner: &ProductionSemanticSsaOwnerV1, root: SemanticFunctionIdV1) -> String {
    let mut out = Bounded {
        text: String::new(),
        items: MAX_ITEMS,
    };
    let result = (|| -> fmt::Result {
        writeln!(out, "CONTEXT_GRAPH_BEGIN root={}", root.index())?;
        let source = owner.source_semantic();
        let Some(view) = owner.execution_view_for_root(root) else {
            return writeln!(out, "no execution view");
        };
        let Some(plan) = owner.execution_plan_for_root(root) else {
            return writeln!(out, "no execution plan");
        };
        writeln!(
            out,
            "entry={} locals={} blocks={}",
            view.body().entry().index(),
            view.body().locals().len(),
            view.body().blocks().len()
        )?;
        writeln!(out, "promoted={:?}", plan.plan().promoted_variables())?;
        for (index, ty) in source.types().iter().enumerate() {
            out.item()?;
            writeln!(out, "type {index}: {:?}", ty.shape())?;
        }
        for (index, local) in view.body().locals().iter().enumerate() {
            out.item()?;
            writeln!(
                out,
                "local {index}: type={} role={:?} origin={:?}",
                local.ty().index(),
                local.role(),
                view.local_origins().get(index)
            )?;
        }
        for (index, callable) in source.callables().iter().enumerate() {
            out.item()?;
            match callable {
                SemanticCallableDeclV1::Defined { function } => {
                    writeln!(
                        out,
                        "callable {index}: defined function={}",
                        function.index()
                    )?;
                }
                SemanticCallableDeclV1::CompilerIntrinsic {
                    binding, operation, ..
                } => {
                    writeln!(
                        out,
                        "callable {index}: intrinsic={operation:?} identity={:?} inputs={:?} output={:?} ownership={:?}",
                        binding.identity(),
                        binding.abi().source_input_types(),
                        binding.abi().source_output_type(),
                        binding.abi().source_argument_ownership()
                    )?;
                }
                _ => writeln!(out, "callable {index}: {callable:?}")?,
            }
        }
        for (block, body) in view.body().blocks().iter().enumerate() {
            out.item()?;
            let origin = view.block_origins().get(block);
            writeln!(out, "bb{block}: origin={origin:?}")?;
            for (statement, item) in body.statements().iter().enumerate() {
                out.item()?;
                writeln!(out, "bb{block} stmt{statement}: {:?}", item.kind())?;
            }
            writeln!(out, "bb{block} term: {:?}", body.terminator().kind())?;
        }
        writeln!(out, "CONTEXT_GRAPH_END")
    })();
    if result.is_err() {
        out.text
            .push_str("\nCONTEXT_GRAPH_TRUNCATED: byte/item bound reached; not a complete graph\n");
    }
    out.text
}

#[test]
fn context_failure_graph_writer_is_bounded_without_splitting_utf8() {
    let mut out = Bounded {
        text: "x".repeat(MAX_BYTES - 1),
        items: MAX_ITEMS,
    };
    assert!(out.write_str("ab").is_err());
    assert_eq!(out.text.len(), MAX_BYTES - 1);
    out.write_str("x").unwrap();
    assert_eq!(out.text.len(), MAX_BYTES);
    assert!(out.write_str("x").is_err());
}

#[test]
fn context_failure_graph_item_bound_rejects_without_underflow() {
    let mut out = Bounded {
        text: String::new(),
        items: 1,
    };
    out.item().unwrap();
    assert!(out.item().is_err());
    assert_eq!(out.items, 0);
}

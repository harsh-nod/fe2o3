use crate::collector::CollectionResult;
use dialect_mir::MirOp;
use rustc_hir::def_id::LOCAL_CRATE;
use rustc_middle::mir::{BasicBlock, Body, Operand, StatementKind, TerminatorKind};
use rustc_middle::ty::{TyCtxt, TyKind};
use std::fmt::Write;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirModule {
    pub functions: Vec<MirFunction>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirFunction {
    pub export_name: String,
    pub rust_path: String,
    pub kind: MirFunctionKind,
    pub arg_count: usize,
    pub local_count: usize,
    pub blocks: Vec<MirBlock>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirFunctionKind {
    Kernel,
    Device,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirBlock {
    pub index: usize,
    pub statements: Vec<MirStatement>,
    pub terminator: Option<MirTerminator>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirStatement {
    pub kind: MirStatementKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirStatementKind {
    Assign,
    StorageLive,
    StorageDead,
    SetDiscriminant,
    Intrinsic,
    Retag,
    Coverage,
    Nop,
    Other,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirTerminator {
    pub kind: MirTerminatorKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirTerminatorKind {
    Return,
    Unreachable,
    Goto {
        target: usize,
    },
    SwitchInt {
        targets: usize,
    },
    Call {
        callee: Option<String>,
        target: Option<usize>,
    },
    Assert {
        target: usize,
    },
    Drop {
        target: usize,
    },
    Other,
}

pub fn import_collection<'tcx>(
    tcx: TyCtxt<'tcx>,
    collection: &CollectionResult<'tcx>,
) -> MirModule {
    let functions = collection
        .functions
        .iter()
        .filter_map(|function| {
            let def_id = function.instance.def_id();
            if !tcx.is_mir_available(def_id) {
                return None;
            }

            let body = tcx.instance_mir(function.instance.def);
            let rust_path = if def_id.krate == LOCAL_CRATE {
                format!(
                    "{}::{}",
                    tcx.crate_name(LOCAL_CRATE),
                    tcx.def_path_str(def_id)
                )
            } else {
                tcx.def_path_str(def_id)
            };
            Some(import_body(
                tcx,
                body,
                function.export_name.clone(),
                rust_path,
                if function.is_kernel {
                    MirFunctionKind::Kernel
                } else {
                    MirFunctionKind::Device
                },
            ))
        })
        .collect();

    MirModule { functions }
}

impl MirModule {
    pub fn summary(&self) -> String {
        let mut output = format!(
            "\n=== fe2o3 MIR import scaffold ({}) ===\n",
            MirOp::Module.name()
        );
        for function in &self.functions {
            let kind = match function.kind {
                MirFunctionKind::Kernel => "kernel",
                MirFunctionKind::Device => "device",
            };
            let _ = writeln!(
                output,
                "  [{kind}] {} ({})",
                function.export_name,
                MirOp::Func.name()
            );
            let _ = writeln!(output, "      path: {}", function.rust_path);
            let _ = writeln!(
                output,
                "      MIR:  {} bb, {} locals, {} args",
                function.blocks.len(),
                function.local_count,
                function.arg_count
            );
            for block in &function.blocks {
                let terminator = block
                    .terminator
                    .as_ref()
                    .map(|terminator| terminator.kind.summary())
                    .unwrap_or("missing terminator".to_string());
                let _ = writeln!(
                    output,
                    "      bb{} ({}): {} stmt(s), {terminator}",
                    block.index,
                    MirOp::Block.name(),
                    block.statements.len()
                );
            }
        }
        output.push_str("===================================\n");
        output
    }
}

impl MirTerminatorKind {
    fn summary(&self) -> String {
        match self {
            Self::Return => MirOp::Return.name().to_string(),
            Self::Unreachable => MirOp::Unreachable.name().to_string(),
            Self::Goto { target } => format!("{} -> bb{target}", MirOp::Branch.name()),
            Self::SwitchInt { targets } => {
                format!("{} ({targets} target(s))", MirOp::Switch.name())
            }
            Self::Call { callee, target } => {
                let callee = callee.as_deref().unwrap_or("<dynamic>");
                match target {
                    Some(target) => format!("{} {callee} -> bb{target}", MirOp::Call.name()),
                    None => format!("{} {callee} -> return", MirOp::Call.name()),
                }
            }
            Self::Assert { target } => format!("{} -> bb{target}", MirOp::Assert.name()),
            Self::Drop { target } => format!("{} -> bb{target}", MirOp::Drop.name()),
            Self::Other => "other".to_string(),
        }
    }
}

fn import_body<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    export_name: String,
    rust_path: String,
    kind: MirFunctionKind,
) -> MirFunction {
    let blocks = body
        .basic_blocks
        .iter_enumerated()
        .map(|(index, block)| MirBlock {
            index: index.as_usize(),
            statements: block
                .statements
                .iter()
                .map(|statement| MirStatement {
                    kind: statement_kind(&statement.kind),
                })
                .collect(),
            terminator: block.terminator.as_ref().map(|terminator| MirTerminator {
                kind: terminator_kind(tcx, &terminator.kind),
            }),
        })
        .collect();

    MirFunction {
        export_name,
        rust_path,
        kind,
        arg_count: body.arg_count,
        local_count: body.local_decls.len(),
        blocks,
    }
}

fn statement_kind(kind: &StatementKind<'_>) -> MirStatementKind {
    match kind {
        StatementKind::Assign(_) => MirStatementKind::Assign,
        StatementKind::StorageLive(_) => MirStatementKind::StorageLive,
        StatementKind::StorageDead(_) => MirStatementKind::StorageDead,
        StatementKind::SetDiscriminant { .. } => MirStatementKind::SetDiscriminant,
        StatementKind::Intrinsic(_) => MirStatementKind::Intrinsic,
        StatementKind::Retag(_, _) => MirStatementKind::Retag,
        StatementKind::Coverage(_) => MirStatementKind::Coverage,
        StatementKind::Nop => MirStatementKind::Nop,
        _ => MirStatementKind::Other,
    }
}

fn terminator_kind<'tcx>(tcx: TyCtxt<'tcx>, kind: &TerminatorKind<'tcx>) -> MirTerminatorKind {
    match kind {
        TerminatorKind::Return => MirTerminatorKind::Return,
        TerminatorKind::Unreachable => MirTerminatorKind::Unreachable,
        TerminatorKind::Goto { target } => MirTerminatorKind::Goto {
            target: target.as_usize(),
        },
        TerminatorKind::SwitchInt { targets, .. } => MirTerminatorKind::SwitchInt {
            targets: targets.all_targets().len(),
        },
        TerminatorKind::Call { func, target, .. } => MirTerminatorKind::Call {
            callee: call_name(tcx, func),
            target: target.map(BasicBlock::as_usize),
        },
        TerminatorKind::Assert { target, .. } => MirTerminatorKind::Assert {
            target: target.as_usize(),
        },
        TerminatorKind::Drop { target, .. } => MirTerminatorKind::Drop {
            target: target.as_usize(),
        },
        _ => MirTerminatorKind::Other,
    }
}

fn call_name<'tcx>(tcx: TyCtxt<'tcx>, func: &Operand<'tcx>) -> Option<String> {
    let Operand::Constant(constant) = func else {
        return None;
    };
    let TyKind::FnDef(def_id, _) = constant.const_.ty().kind() else {
        return None;
    };
    Some(tcx.def_path_str(*def_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_includes_function_and_block_shape() {
        let module = MirModule {
            functions: vec![MirFunction {
                export_name: "vecadd".to_string(),
                rust_path: "fe2o3_vecadd::fe2o3_kernel_vecadd".to_string(),
                kind: MirFunctionKind::Kernel,
                arg_count: 3,
                local_count: 17,
                blocks: vec![MirBlock {
                    index: 0,
                    statements: vec![
                        MirStatement {
                            kind: MirStatementKind::StorageLive,
                        },
                        MirStatement {
                            kind: MirStatementKind::Assign,
                        },
                    ],
                    terminator: Some(MirTerminator {
                        kind: MirTerminatorKind::Goto { target: 1 },
                    }),
                }],
            }],
        };

        let summary = module.summary();

        assert!(summary.contains("[kernel] vecadd (mir.func)"));
        assert!(summary.contains("fe2o3_vecadd::fe2o3_kernel_vecadd"));
        assert!(summary.contains("1 bb, 17 locals, 3 args"));
        assert!(summary.contains("bb0 (mir.block): 2 stmt(s), mir.br -> bb1"));
    }
}

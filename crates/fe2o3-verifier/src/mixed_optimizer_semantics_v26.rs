//! Finite block simulation over exact SSA occurrences. Untouched instructions
//! are shared operator parameters, not caller-authored equations or axioms.

use super::{Error, Inventory, Resource, Result, Writer};
use fe2o3_kernel_ir::{
    BinaryOp, CanonicalKirBlockCoordinateV1 as Block,
    CanonicalKirDefinitionCoordinateV1 as Definition,
    CanonicalKirDefinitionDescendantKindV1 as Descendant,
    CanonicalKirOperationCoordinateV1 as Operation, CanonicalKirOperationOriginV1 as Origin,
    CanonicalKirTransitionCandidateV1 as Rows, CheckedBinaryOperator, Constant, OperationKind,
    ScalarType, Type,
};
use std::{
    fmt::Write as _,
    mem::{align_of, size_of},
};

#[path = "mixed_optimizer_cfg_trace_v26.rs"]
mod cfg_trace;

macro_rules! emit {
    ($out:expr, $($arg:tt)*) => {
        write!($out, $($arg)*).map_err(|_| $out.error())?
    };
}

#[path = "mixed_optimizer_cfg_graph_v26.rs"]
mod cfg_graph;
#[path = "mixed_optimizer_cfg_relation_v26.rs"]
mod cfg_relation;
#[path = "mixed_optimizer_congruence_v27.rs"]
mod congruence_v27;
#[path = "mixed_optimizer_relocation_cfg_semantics_v28.rs"]
mod relocation_v28;
pub(super) use relocation_v28::generate as generate_relocation_cfg_v28;
#[path = "mixed_optimizer_relocation_composition_v28.rs"]
mod relocation_composition_v28;
#[cfg(test)]
pub(super) use relocation_composition_v28::bridge_negative_controls;
pub(super) use relocation_composition_v28::generate as generate_composed_relocation_cfg_v28;

const NONE: usize = usize::MAX;
const PARAMETERS: &str =
    "base: Seq<int>, initial: int, op: spec_fn(int, int, Seq<int>, int) -> int";
const ARGUMENTS: &str = "base, initial, op";

struct Plan {
    anchors: Vec<usize>,
    input_operations: Vec<usize>,
    output_operations: Vec<usize>,
    blocks: Vec<usize>,
    seen_definitions: Vec<usize>,
    seen_operations: Vec<usize>,
    pending_definitions: Vec<usize>,
    total_classes: Option<Vec<usize>>,
}

fn allocate(count: usize, out: &mut Writer<'_, '_>) -> Result<Vec<usize>> {
    let bytes = count
        .checked_mul(size_of::<usize>())
        .ok_or(Resource::Accounting)?;
    out.budget.reserve_storage(bytes)?;
    out.budget.charge_work(count)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    if rows.capacity() > count {
        out.budget.reserve_storage(
            (rows.capacity() - count)
                .checked_mul(size_of::<usize>())
                .ok_or(Resource::Accounting)?,
        )?;
    }
    rows.resize(count, NONE);
    Ok(rows)
}

fn block_index(inv: &Inventory<'_>, coordinate: Block) -> Result<usize> {
    let function = inv
        .functions()
        .get(coordinate.function.0 as usize)
        .ok_or(Error::Statement("block function coordinate"))?;
    let index = function
        .blocks
        .start
        .checked_add(coordinate.block as usize)
        .ok_or(Resource::Accounting)?;
    if index >= function.blocks.end || inv.blocks()[index].coordinate != coordinate {
        return Err(Error::Statement("block coordinate"));
    }
    Ok(index)
}
fn operation_index(inv: &Inventory<'_>, coordinate: Operation) -> Result<usize> {
    let block = &inv.blocks()[block_index(inv, coordinate.block)?];
    let index = block
        .operations
        .start
        .checked_add(coordinate.operation as usize)
        .ok_or(Resource::Accounting)?;
    if index >= block.operations.end || inv.operations()[index].coordinate != coordinate {
        return Err(Error::Statement("operation coordinate"));
    }
    Ok(index)
}
fn definition_index(inv: &Inventory<'_>, coordinate: Definition) -> Result<usize> {
    let (start, end, offset) = match coordinate {
        Definition::FunctionArgument { function, argument } => {
            let function = inv
                .functions()
                .get(function.0 as usize)
                .ok_or(Error::Statement("definition function coordinate"))?;
            (
                function.definitions.start,
                function.definitions.end,
                argument,
            )
        }
        Definition::BlockArgument { block, argument } => {
            let block = &inv.blocks()[block_index(inv, block)?];
            (block.parameters.start, block.parameters.end, argument)
        }
        Definition::Result { operation, result } => {
            let operation = &inv.operations()[operation_index(inv, operation)?];
            (operation.results.start, operation.results.end, result)
        }
    };
    let index = start
        .checked_add(offset as usize)
        .ok_or(Resource::Accounting)?;
    if index >= end || inv.definitions()[index].coordinate != coordinate {
        return Err(Error::Statement("definition coordinate"));
    }
    Ok(index)
}
fn descendants<'a>(
    rows: Rows<'a>,
    input: usize,
) -> Result<&'a [fe2o3_kernel_ir::CanonicalKirDefinitionDescendantV1]> {
    let range = rows.definitions[input].outputs;
    let end = (range.start as usize)
        .checked_add(range.len as usize)
        .ok_or(Resource::Accounting)?;
    rows.definition_outputs
        .get(range.start as usize..end)
        .ok_or(Error::Statement("descendant range"))
}

impl Plan {
    fn build(
        input: &Inventory<'_>,
        output: &Inventory<'_>,
        rows: Rows<'_>,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        out.budget
            .reserve_storage(size_of::<Self>() + align_of::<Self>())?;
        let mut plan = Self {
            anchors: allocate(output.definitions().len(), out)?,
            input_operations: allocate(input.operations().len(), out)?,
            output_operations: allocate(output.operations().len(), out)?,
            blocks: allocate(output.blocks().len(), out)?,
            seen_definitions: allocate(input.definitions().len(), out)?,
            seen_operations: allocate(input.operations().len(), out)?,
            pending_definitions: allocate(input.definitions().len(), out)?,
            total_classes: None,
        };
        if output.blocks().is_empty() || input.blocks().len() != output.blocks().len() {
            return Err(Error::Statement(
                "Policy9 statement does not admit CFG deletion",
            ));
        }
        for (ordinal, row) in rows.blocks.iter().enumerate() {
            out.budget.charge_work(4)?;
            if row.segments.len != 1 {
                return Err(Error::Statement("Policy9 singleton source block"));
            }
            let segment = rows
                .segments
                .get(row.segments.start as usize)
                .ok_or(Error::Statement("block segment"))?;
            if segment.connector.is_some() {
                return Err(Error::Statement("Policy9 block connector"));
            }
            let original = block_index(input, segment.input)?;
            let a = &input.blocks()[original];
            let b = &output.blocks()[ordinal];
            if a.parameters.len() != b.parameters.len()
                || a.edges.len() != b.edges.len()
                || a.terminator_uses.len() != b.terminator_uses.len()
                || std::mem::discriminant(a.terminator) != std::mem::discriminant(b.terminator)
            {
                return Err(Error::Statement(
                    "Policy9 unchanged control and block parameter roster",
                ));
            }
            plan.blocks[ordinal] = original;
        }
        for (original, row) in rows.definitions.iter().enumerate() {
            out.budget.charge_work(1)?;
            if row.input != input.definitions()[original].coordinate {
                return Err(Error::Statement("input definition order"));
            }
            for descendant in descendants(rows, original)? {
                out.budget.charge_work(4)?;
                if descendant.kind == Descendant::Retained {
                    let target = definition_index(output, descendant.output)?;
                    if plan.anchors[target] != NONE {
                        return Err(Error::Statement("duplicate retained anchor"));
                    }
                    plan.anchors[target] = original;
                }
            }
        }
        for (ordinal, row) in rows.operations.iter().enumerate() {
            out.budget.charge_work(4)?;
            match row.origin {
                Origin::Retained(origin) => {
                    let original = operation_index(input, origin)?;
                    plan.output_operations[ordinal] = original;
                    if plan.input_operations[original] != NONE {
                        return Err(Error::Statement("duplicated retained operation"));
                    }
                    plan.input_operations[original] = ordinal;
                }
                Origin::ConstantFrom(origin) => {
                    let original = definition_index(input, origin)?;
                    let target = &output.operations()[ordinal];
                    if target.results.len() != 1
                        || !matches!(target.operation.kind, OperationKind::Constant(_))
                    {
                        return Err(Error::Statement("synthesized scalar constant"));
                    }
                    if plan.anchors[target.results.start] != NONE {
                        return Err(Error::Statement("constant anchor collision"));
                    }
                    plan.anchors[target.results.start] = original;
                }
            }
        }
        out.budget.charge_work(plan.anchors.len())?;
        if plan.anchors.contains(&NONE) {
            return Err(Error::Statement("complete output definition anchors"));
        }
        Ok(plan)
    }
}

pub(super) fn generate(
    input: &Inventory<'_>,
    output: &Inventory<'_>,
    rows: Rows<'_>,
    out: &mut Writer<'_, '_>,
) -> Result<usize> {
    generate_profile(input, output, rows, false, out)
}

pub(super) fn generate_cfg_v27(
    input: &Inventory<'_>,
    output: &Inventory<'_>,
    rows: Rows<'_>,
    out: &mut Writer<'_, '_>,
) -> Result<usize> {
    generate_profile(input, output, rows, true, out)
}

fn generate_profile(
    input: &Inventory<'_>,
    output: &Inventory<'_>,
    rows: Rows<'_>,
    congruence: bool,
    out: &mut Writer<'_, '_>,
) -> Result<usize> {
    let floor = out.budget.storage();
    let generated = (|| {
        let mut plan = Plan::build(input, output, rows, out)?;
        if congruence {
            plan.total_classes = Some(congruence_v27::build(
                input,
                output,
                &plan.output_operations,
                out,
            )?);
        }
        emit!(
            out,
            "use vstd::prelude::*;\nuse vstd::seq_lib::*;\nverus! {{\n"
        );
        emit!(out, "{}", cfg_trace::PRELUDE);
        if congruence {
            emit!(
                out,
                "// V27 exact total-operator congruence; ordered occurrences remain distinct.\n"
            );
        }
        emit!(
            out,
            "// Shared operator interpretation, exact scalar bit patterns, and explicit trap state.\n"
        );
        emit!(
            out,
            "open spec fn signed(x: int, m: int) -> int {{ if x < m / 2 {{ x }} else {{ x - m }} }}\n"
        );
        select_prelude(input, output, out)?;
        for width in [8, 16, 32, 64] {
            let max = (1u128 << width) - 1;
            emit!(
                out,
                "proof fn bit_identity_{width}(x: u{width})\n ensures (x & {max}u{width}) == x, (x | 0u{width}) == x, (x ^ 0u{width}) == x,\n{{\n assert((x & {max}u{width}) == x) by(bit_vector);\n assert((x | 0u{width}) == x) by(bit_vector);\n assert((x ^ 0u{width}) == x) by(bit_vector);\n}}\n"
            );
        }
        for target in 0..output.blocks().len() {
            out.budget.charge_work(1)?;
            let original = plan.blocks[target];
            for side in [Side::Input, Side::Output] {
                let label = side.label();
                emit!(
                    out,
                    "open spec fn block_{target}_{label}({PARAMETERS}) -> Seq<int>\n recommends base.len() == {},\n{{\n",
                    input.definitions().len()
                );
                body(input, output, &plan, original, target, side, false, out)?;
                observations(input, output, rows, &plan, original, target, side, out)?;
                emit!(out, "\n}}\n");
            }
            emit!(
                out,
                "proof fn block_simulation_{target}({PARAMETERS})\n requires\n base.len() == {},\n",
                input.definitions().len()
            );
            premises(input, output, rows, &mut plan, original, target, out)?;
            emit!(
                out,
                " ensures block_{target}_n({ARGUMENTS}) == block_{target}_o({ARGUMENTS}),\n{{\n"
            );
            body(
                input,
                output,
                &plan,
                original,
                target,
                Side::Input,
                true,
                out,
            )?;
            body(
                input,
                output,
                &plan,
                original,
                target,
                Side::Output,
                true,
                out,
            )?;
            external_scalar_laws(input, &plan, target, out)?;
            emit!(out, " let n = ");
            observations(
                input,
                output,
                rows,
                &plan,
                original,
                target,
                Side::Input,
                out,
            )?;
            emit!(out, ";\n let o = ");
            observations(
                input,
                output,
                rows,
                &plan,
                original,
                target,
                Side::Output,
                out,
            )?;
            emit!(
                out,
                ";\n assert(block_{target}_n({ARGUMENTS}) == n);\n assert(block_{target}_o({ARGUMENTS}) == o);\n assert_seqs_equal!(n, o);\n}}\n"
            );
        }
        cfg_graph::generate(input, output, &plan, out)?;
        cfg_relation::generate(input, output, rows, &plan, out)?;
        emit!(out, "}}\nfn main() {{}}\n");
        Ok(output.blocks().len())
    })();
    // No caller-controlled destructors occur in this bounded producer.
    let release = out
        .budget
        .storage()
        .checked_sub(floor)
        .ok_or(Resource::Accounting)?;
    out.budget.release_storage(release)?;
    generated
}

#[derive(Clone, Copy)]
enum Side {
    Input,
    Output,
}
impl Side {
    fn label(self) -> &'static str {
        match self {
            Self::Input => "n",
            Self::Output => "o",
        }
    }
}

#[derive(Clone, Copy)]
enum Environment {
    OriginalAnchors,
    ActualDefinitions,
}

fn local(inv: &Inventory<'_>, definition: usize, block: usize) -> bool {
    matches!(inv.definitions()[definition].coordinate, Definition::Result { operation, .. }
        if operation.block == inv.blocks()[block].coordinate)
}
fn value(
    inv: &Inventory<'_>,
    plan: &Plan,
    block: usize,
    side: Side,
    definition: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    value_in(
        inv,
        plan,
        block,
        side,
        Environment::OriginalAnchors,
        definition,
        out,
    )
}

fn value_in(
    inv: &Inventory<'_>,
    plan: &Plan,
    block: usize,
    side: Side,
    environment: Environment,
    definition: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    out.budget.charge_work(1)?;
    if local(inv, definition, block) {
        emit!(out, "{}{definition}", side.label());
    } else {
        let anchor = match (side, environment) {
            (Side::Output, Environment::OriginalAnchors) => plan.anchors[definition],
            _ => definition,
        };
        emit!(out, "base[{anchor}]");
    }
    Ok(())
}
fn scalar_width(ty: &Type) -> Option<u32> {
    match ty.as_scalar()? {
        ScalarType::Bool => Some(1),
        ScalarType::I8 | ScalarType::U8 => Some(8),
        ScalarType::I16 | ScalarType::U16 | ScalarType::F16 | ScalarType::Bf16 => Some(16),
        ScalarType::I32 | ScalarType::U32 | ScalarType::F32 => Some(32),
        ScalarType::I64 | ScalarType::U64 | ScalarType::F64 | ScalarType::Index => Some(64),
        _ => None,
    }
}
fn integer(ty: &Type) -> Option<(u32, bool)> {
    match ty.as_scalar()? {
        ScalarType::I8 => Some((8, true)),
        ScalarType::U8 => Some((8, false)),
        ScalarType::I16 => Some((16, true)),
        ScalarType::U16 => Some((16, false)),
        ScalarType::I32 => Some((32, true)),
        ScalarType::U32 => Some((32, false)),
        ScalarType::I64 => Some((64, true)),
        ScalarType::U64 => Some((64, false)),
        _ => None,
    }
}
fn bits(constant: &Constant) -> u128 {
    match *constant {
        Constant::Bool(v) => u128::from(v),
        Constant::I8(v) => v as u8 as u128,
        Constant::I16(v) => v as u16 as u128,
        Constant::I32(v) => v as u32 as u128,
        Constant::I64(v) => v as u64 as u128,
        Constant::U8(v) => v as u128,
        Constant::U16(v) | Constant::F16Bits(v) | Constant::Bf16Bits(v) => v as u128,
        Constant::U32(v) | Constant::F32Bits(v) => v as u128,
        Constant::U64(v) | Constant::Index(v) | Constant::F64Bits(v) => v as u128,
    }
}

// This is the transition checker's deliberately closed total-operation set.
// Ordinary arithmetic remains ordered unless its no-overflow condition is
// established by the generated statement; floating arithmetic is never pure.
fn total(kind: &OperationKind) -> bool {
    use fe2o3_kernel_ir::{CastKind as C, UnaryOp};
    matches!(
        kind,
        OperationKind::Constant(_)
            | OperationKind::Compare { .. }
            | OperationKind::Select { .. }
            | OperationKind::SliceLength { .. }
            | OperationKind::SliceData { .. }
            | OperationKind::Unary {
                op: UnaryOp::Not,
                ..
            }
            | OperationKind::Binary {
                op: BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor | BinaryOp::Checked(_),
                ..
            }
            | OperationKind::Cast {
                kind: C::RestrictPointerAccess
                    | C::PointerToGeneric
                    | C::SliceToGeneric
                    | C::Truncate
                    | C::ZeroExtend
                    | C::SignExtend
                    | C::Bitcast,
                ..
            }
    )
}

fn opaque_total(kind: &OperationKind) -> bool {
    total(kind) && !matches!(kind, OperationKind::Select { .. })
}

const SELECT_PRELUDE: &str = "// Select uses the exact boolean branch and preserves the chosen value's representation.\nopen spec fn select_value_v28(condition: int, when_true: int, when_false: int) -> int { if condition == 1int { when_true } else { when_false } }\nproof fn select_same_value_v28(condition: int, value: int)\n ensures select_value_v28(condition, value, value) == value,\n{}\n";

fn select_prelude(
    input: &Inventory<'_>,
    output: &Inventory<'_>,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    for inv in [input, output] {
        for row in inv.operations() {
            out.budget.charge_work(1)?;
            if matches!(row.operation.kind, OperationKind::Select { .. }) {
                emit!(out, "{SELECT_PRELUDE}");
                return Ok(());
            }
        }
    }
    Ok(())
}

fn select_types(
    condition: &Type,
    when_true: &Type,
    when_false: &Type,
    result: &Type,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    out.budget.charge_work(4)?;
    if !matches!(condition, Type::Scalar(ScalarType::Bool))
        || congruence_v27::compare_type(when_true, result, out)? != std::cmp::Ordering::Equal
        || congruence_v27::compare_type(when_false, result, out)? != std::cmp::Ordering::Equal
    {
        return Err(Error::Statement(
            "concrete Select condition and branch types",
        ));
    }
    Ok(())
}

fn select_operands(
    inv: &Inventory<'_>,
    operation: usize,
    out: &mut Writer<'_, '_>,
) -> Result<[usize; 3]> {
    out.budget.charge_work(4)?;
    let row = inv
        .operations()
        .get(operation)
        .ok_or(Error::Statement("concrete Select coordinate"))?;
    if !matches!(row.operation.kind, OperationKind::Select { .. })
        || row.operands.len() != 3
        || row.results.len() != 1
    {
        return Err(Error::Statement("concrete Select arity"));
    }
    let operands = [
        inv.uses()[row.operands.start].definition,
        inv.uses()[row.operands.start + 1].definition,
        inv.uses()[row.operands.start + 2].definition,
    ];
    select_types(
        inv.definitions()[operands[0]].ty,
        inv.definitions()[operands[1]].ty,
        inv.definitions()[operands[2]].ty,
        inv.definitions()[row.results.start].ty,
        out,
    )?;
    Ok(operands)
}

fn concrete(inv: &Inventory<'_>, operation: usize) -> Option<(BinaryOp, u32, bool)> {
    let row = &inv.operations()[operation];
    let OperationKind::Binary { op, .. } = row.operation.kind else {
        return None;
    };
    if !matches!(
        op,
        BinaryOp::Add
            | BinaryOp::Subtract
            | BinaryOp::Multiply
            | BinaryOp::BitAnd
            | BinaryOp::BitOr
            | BinaryOp::BitXor
            | BinaryOp::Checked(_)
    ) {
        return None;
    }
    let (width, signed) = integer(inv.definitions()[row.results.start].ty)?;
    Some((op, width, signed))
}

fn body(
    input: &Inventory<'_>,
    output: &Inventory<'_>,
    plan: &Plan,
    original: usize,
    target: usize,
    side: Side,
    proof: bool,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    body_in(
        input,
        output,
        plan,
        original,
        target,
        side,
        proof,
        Environment::OriginalAnchors,
        out,
    )
}

fn body_in(
    input: &Inventory<'_>,
    output: &Inventory<'_>,
    plan: &Plan,
    original: usize,
    target: usize,
    side: Side,
    proof: bool,
    environment: Environment,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let (inv, block) = match side {
        Side::Input => (input, original),
        Side::Output => (output, target),
    };
    let label = side.label();
    emit!(out, " let {label}_state0: int = initial;\n");
    let mut state = 0;
    for ordinal in inv.blocks()[block].operations.clone() {
        out.budget.charge_work(2)?;
        let row = &inv.operations()[ordinal];
        let origin = match side {
            Side::Input => ordinal,
            Side::Output => plan.output_operations[ordinal],
        };
        if let OperationKind::Constant(constant) = &row.operation.kind {
            emit!(
                out,
                " let {label}{}: int = {};\n",
                row.results.start,
                bits(constant)
            );
            continue;
        }
        if matches!(row.operation.kind, OperationKind::Select { .. }) {
            let operands = select_operands(inv, ordinal, out)?;
            emit!(
                out,
                " let {label}{}: int = select_value_v28(",
                row.results.start
            );
            for definition in operands {
                value_in(inv, plan, block, side, environment, definition, out)?;
                emit!(out, ",");
            }
            emit!(out, ");\n");
            if proof {
                emit!(out, " select_same_value_v28(");
                for definition in &operands[..2] {
                    value_in(inv, plan, block, side, environment, *definition, out)?;
                    emit!(out, ",");
                }
                emit!(out, ");\n");
            }
            continue;
        }
        if let Some((operator, width, signed)) = concrete(inv, ordinal) {
            if row.operands.len() != 2
                || row.results.len()
                    != if matches!(operator, BinaryOp::Checked(_)) {
                        2
                    } else {
                        1
                    }
            {
                return Err(Error::Statement("integer semantic operation arity"));
            }
            let left = inv.uses()[row.operands.start].definition;
            let right = inv.uses()[row.operands.start + 1].definition;
            emit!(out, " let {label}_l{ordinal}: int = ");
            value_in(inv, plan, block, side, environment, left, out)?;
            emit!(out, ";\n let {label}_r{ordinal}: int = ");
            value_in(inv, plan, block, side, environment, right, out)?;
            emit!(out, ";\n");
            let modulus = 1u128 << width;
            match operator {
                BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor => {
                    let symbol = match operator {
                        BinaryOp::BitAnd => "&",
                        BinaryOp::BitOr => "|",
                        _ => "^",
                    };
                    emit!(
                        out,
                        " let {label}{}: int = (({label}_l{ordinal} as u{width}) {symbol} ({label}_r{ordinal} as u{width})) as int;\n",
                        row.results.start
                    );
                    if proof {
                        emit!(
                            out,
                            " bit_identity_{width}({label}_l{ordinal} as u{width});\n bit_identity_{width}({label}_r{ordinal} as u{width});\n"
                        );
                    }
                }
                _ => {
                    let symbol = match operator {
                        BinaryOp::Add | BinaryOp::Checked(CheckedBinaryOperator::Add) => "+",
                        BinaryOp::Subtract | BinaryOp::Checked(CheckedBinaryOperator::Subtract) => {
                            "-"
                        }
                        _ => "*",
                    };
                    emit!(out, " let {label}_math{ordinal}: int = ");
                    if signed {
                        emit!(
                            out,
                            "signed({label}_l{ordinal}, {modulus}) {symbol} signed({label}_r{ordinal}, {modulus})"
                        );
                    } else {
                        emit!(out, "{label}_l{ordinal} {symbol} {label}_r{ordinal}");
                    }
                    emit!(
                        out,
                        ";\n let {label}{}: int = {label}_math{ordinal} % {modulus};\n",
                        row.results.start
                    );
                    let (minimum, maximum) = if signed {
                        (-(1i128 << (width - 1)), (1i128 << (width - 1)) - 1)
                    } else {
                        (0, (1i128 << width) - 1)
                    };
                    emit!(
                        out,
                        " let {label}_trap{ordinal}: bool = !({minimum} <= {label}_math{ordinal} <= {maximum});\n"
                    );
                    if matches!(operator, BinaryOp::Checked(_)) {
                        emit!(
                            out,
                            " let {label}{}: int = if {label}_trap{ordinal} {{ 1 }} else {{ 0 }};\n",
                            row.results.start + 1
                        );
                    } else {
                        if origin == NONE {
                            return Err(Error::Statement("ordered integer origin"));
                        }
                        emit!(
                            out,
                            " let {label}_state{}: int = if {label}_trap{ordinal} {{ op({origin}, -1, seq![{label}_l{ordinal}, {label}_r{ordinal}], {label}_state{state}) }} else {{ {label}_state{state} }};\n",
                            state + 1
                        );
                        state += 1;
                    }
                }
            }
            continue;
        }
        if origin == NONE {
            return Err(Error::Statement("opaque operator origin"));
        }
        emit!(out, " let {label}_args{ordinal}: Seq<int> = seq![");
        for operand in row.operands.clone() {
            value_in(
                inv,
                plan,
                block,
                side,
                environment,
                inv.uses()[operand].definition,
                out,
            )?;
            emit!(out, ",");
        }
        emit!(out, "];\n");
        let effectful = !total(&row.operation.kind);
        let interpretation = total_interpretation(plan, origin, effectful)?;
        for (result, definition) in row.results.clone().enumerate() {
            out.budget.charge_work(1)?;
            emit!(
                out,
                " let {label}{definition}: int = op({interpretation}, {result}, {label}_args{ordinal}, "
            );
            if effectful {
                emit!(out, "{label}_state{state}");
            } else {
                emit!(out, "0");
            }
            emit!(out, ")");
            if let Some(width) = scalar_width(inv.definitions()[definition].ty) {
                emit!(out, " % {}", 1u128 << width);
            }
            emit!(out, ";\n");
        }
        if effectful {
            emit!(
                out,
                " let {label}_state{}: int = op({origin}, -1, {label}_args{ordinal}, {label}_state{state});\n",
                state + 1
            );
            state += 1;
        }
    }
    emit!(out, " let {label}_final: int = {label}_state{state};\n");
    Ok(())
}

fn total_interpretation(plan: &Plan, origin: usize, effectful: bool) -> Result<usize> {
    if effectful {
        return Ok(origin);
    }
    match &plan.total_classes {
        Some(classes) => classes
            .get(origin)
            .copied()
            .filter(|class| *class != NONE)
            .ok_or(Error::Statement("complete total operator congruence")),
        None => Ok(origin),
    }
}

fn has_original_equation(input: &Inventory<'_>, plan: &Plan, operation: usize) -> bool {
    concrete(input, operation).is_some()
        || matches!(
            input.operations()[operation].operation.kind,
            OperationKind::Select { .. }
        )
        || (plan.total_classes.is_some() && total(&input.operations()[operation].operation.kind))
}

fn external_total_value(
    input: &Inventory<'_>,
    plan: &Plan,
    ordinal: usize,
    result: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let row = &input.operations()[ordinal];
    if matches!(row.operation.kind, OperationKind::Select { .. }) {
        if result != 0 {
            return Err(Error::Statement("concrete Select result ordinal"));
        }
        let [condition, when_true, when_false] = select_operands(input, ordinal, out)?;
        emit!(
            out,
            "select_value_v28(base[{condition}],base[{when_true}],base[{when_false}])"
        );
        return Ok(());
    }
    if !opaque_total(&row.operation.kind)
        || plan.total_classes.is_none()
        || result >= row.results.len()
    {
        return Err(Error::Statement("original total equation scope"));
    }
    let class = total_interpretation(plan, ordinal, false)?;
    emit!(out, "op({class}, {result}, seq![");
    for usage in row.operands.clone() {
        out.budget.charge_work(2)?;
        emit!(out, "base[{}],", input.uses()[usage].definition);
    }
    emit!(out, "], 0)");
    if let Some(width) = scalar_width(input.definitions()[row.results.start + result].ty) {
        emit!(out, " % {}", 1u128 << width);
    }
    Ok(())
}

fn premises(
    input: &Inventory<'_>,
    output: &Inventory<'_>,
    rows: Rows<'_>,
    plan: &mut Plan,
    original: usize,
    target: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    external_equations(input, output, plan, original, target, out)?;
    for ordinal in input.blocks()[original].operations.clone() {
        for usage in input.operations()[ordinal].operands.clone() {
            out.budget.charge_work(1)?;
            let definition = input.uses()[usage].definition;
            if !local(input, definition, original) {
                if let Some(width) = scalar_width(input.definitions()[definition].ty) {
                    emit!(out, " 0 <= base[{definition}] < {},\n", 1u128 << width);
                }
            }
        }
    }
    for ordinal in output.blocks()[target].operations.clone() {
        for usage in output.operations()[ordinal].operands.clone() {
            incoming(input, output, rows, plan, original, target, usage, out)?;
        }
    }
    for usage in output.blocks()[target].terminator_uses.clone() {
        incoming(input, output, rows, plan, original, target, usage, out)?;
    }
    Ok(())
}

fn enqueue(plan: &mut Plan, definition: usize, epoch: usize, tail: &mut usize) -> Result<()> {
    if plan.seen_definitions[definition] != epoch {
        plan.seen_definitions[definition] = epoch;
        let slot = plan
            .pending_definitions
            .get_mut(*tail)
            .ok_or(Error::Statement("external equation census"))?;
        *slot = definition;
        *tail += 1;
    }
    Ok(())
}

// An SSA value defined in a dominating block retains its defining equation.
// Follow only exact input dependency edges needed at this block, stopping at
// block/function parameters and opaque operators. No unrelated branch equation
// becomes an assumption, and each definition/operation is visited at most once.
fn external_equations(
    input: &Inventory<'_>,
    output: &Inventory<'_>,
    plan: &mut Plan,
    block: usize,
    epoch: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let mut tail = 0;
    for ordinal in input.blocks()[block].operations.clone() {
        for usage in input.operations()[ordinal].operands.clone() {
            out.budget.charge_work(1)?;
            let definition = input.uses()[usage].definition;
            if !local(input, definition, block) {
                enqueue(plan, definition, epoch, &mut tail)?;
            }
        }
    }
    for usage in input.blocks()[block].terminator_uses.clone() {
        out.budget.charge_work(1)?;
        let definition = input.uses()[usage].definition;
        if !local(input, definition, block) {
            enqueue(plan, definition, epoch, &mut tail)?;
        }
    }
    if plan.total_classes.is_some() {
        // Dominance CSE can replace a local original result with an earlier
        // output value. Its original anchor must retain its own equation even
        // when it was not an operand of this original block.
        for ordinal in output.blocks()[epoch].operations.clone() {
            for usage in output.operations()[ordinal].operands.clone() {
                out.budget.charge_work(2)?;
                let definition = output.uses()[usage].definition;
                if !local(output, definition, epoch) {
                    let anchor = plan.anchors[definition];
                    enqueue(plan, anchor, epoch, &mut tail)?;
                }
            }
        }
        for usage in output.blocks()[epoch].terminator_uses.clone() {
            out.budget.charge_work(2)?;
            let definition = output.uses()[usage].definition;
            if !local(output, definition, epoch) {
                let anchor = plan.anchors[definition];
                enqueue(plan, anchor, epoch, &mut tail)?;
            }
        }
    }
    let mut head = 0;
    while head < tail {
        out.budget.charge_work(4)?;
        let definition = plan.pending_definitions[head];
        head += 1;
        if local(input, definition, block) {
            continue;
        }
        if let Some(width) = scalar_width(input.definitions()[definition].ty) {
            emit!(out, " 0 <= base[{definition}] < {},\n", 1u128 << width);
        }
        let Definition::Result { operation, .. } = input.definitions()[definition].coordinate
        else {
            continue;
        };
        let ordinal = operation_index(input, operation)?;
        if plan.seen_operations[ordinal] == epoch {
            continue;
        }
        plan.seen_operations[ordinal] = epoch;
        let row = &input.operations()[ordinal];
        if let OperationKind::Constant(constant) = &row.operation.kind {
            emit!(out, " base[{}] == {},\n", row.results.start, bits(constant));
            continue;
        }
        let Some((operator, width, signed)) = concrete(input, ordinal) else {
            if matches!(row.operation.kind, OperationKind::Select { .. })
                || (plan.total_classes.is_some() && opaque_total(&row.operation.kind))
            {
                for usage in row.operands.clone() {
                    out.budget.charge_work(2)?;
                    enqueue(plan, input.uses()[usage].definition, epoch, &mut tail)?;
                }
                for (result, definition) in row.results.clone().enumerate() {
                    out.budget.charge_work(1)?;
                    emit!(out, " base[{definition}] == ");
                    external_total_value(input, plan, ordinal, result, out)?;
                    emit!(out, ",\n");
                }
            }
            continue;
        };
        let left = input.uses()[row.operands.start].definition;
        let right = input.uses()[row.operands.start + 1].definition;
        enqueue(plan, left, epoch, &mut tail)?;
        enqueue(plan, right, epoch, &mut tail)?;
        let modulus = 1u128 << width;
        match operator {
            BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor => {
                let symbol = match operator {
                    BinaryOp::BitAnd => "&",
                    BinaryOp::BitOr => "|",
                    _ => "^",
                };
                emit!(
                    out,
                    " base[{}] == ((base[{left}] as u{width}) {symbol} (base[{right}] as u{width})) as int,\n",
                    row.results.start
                );
            }
            _ => {
                emit!(out, " base[{}] == (", row.results.start);
                external_arithmetic(operator, signed, modulus, left, right, out)?;
                emit!(out, ") % {modulus},\n");
                let (minimum, maximum) = if signed {
                    (-(1i128 << (width - 1)), (1i128 << (width - 1)) - 1)
                } else {
                    (0, (1i128 << width) - 1)
                };
                if matches!(operator, BinaryOp::Checked(_)) {
                    emit!(out, " base[{}] == if {minimum} <= (", row.results.start + 1);
                    external_arithmetic(operator, signed, modulus, left, right, out)?;
                    emit!(out, ") <= {maximum} {{ 0int }} else {{ 1int }},\n");
                }
            }
        }
    }
    Ok(())
}

fn external_arithmetic(
    operator: BinaryOp,
    signed: bool,
    modulus: u128,
    left: usize,
    right: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let symbol = match operator {
        BinaryOp::Add | BinaryOp::Checked(CheckedBinaryOperator::Add) => "+",
        BinaryOp::Subtract | BinaryOp::Checked(CheckedBinaryOperator::Subtract) => "-",
        _ => "*",
    };
    if signed {
        emit!(
            out,
            "signed(base[{left}], {modulus}) {symbol} signed(base[{right}], {modulus})"
        );
    } else {
        emit!(out, "base[{left}] {symbol} base[{right}]");
    }
    Ok(())
}

fn external_scalar_laws(
    input: &Inventory<'_>,
    plan: &Plan,
    epoch: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    for (ordinal, seen) in plan.seen_operations.iter().enumerate() {
        out.budget.charge_work(1)?;
        if *seen != epoch {
            continue;
        }
        if matches!(
            input.operations()[ordinal].operation.kind,
            OperationKind::Select { .. }
        ) {
            let [condition, when_true, _] = select_operands(input, ordinal, out)?;
            emit!(
                out,
                " select_same_value_v28(base[{condition}],base[{when_true}]);\n"
            );
            continue;
        }
        let Some((operator, width, _)) = concrete(input, ordinal) else {
            continue;
        };
        if !matches!(
            operator,
            BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor
        ) {
            continue;
        }
        for usage in input.operations()[ordinal].operands.clone() {
            out.budget.charge_work(1)?;
            let definition = input.uses()[usage].definition;
            emit!(
                out,
                " bit_identity_{width}(base[{definition}] as u{width});\n"
            );
        }
    }
    Ok(())
}

fn input_use(
    input: &Inventory<'_>,
    coordinate: fe2o3_kernel_ir::CanonicalKirUseCoordinateV1,
) -> Result<usize> {
    use fe2o3_kernel_ir::CanonicalKirUseCoordinateV1 as Use;
    let (start, end, offset) = match coordinate {
        Use::OperationOperand { operation, operand } => {
            let row = &input.operations()[operation_index(input, operation)?];
            (row.operands.start, row.operands.end, operand)
        }
        Use::TerminatorOperand { block, operand } => {
            let row = &input.blocks()[block_index(input, block)?];
            (row.terminator_uses.start, row.terminator_uses.end, operand)
        }
    };
    let index = start
        .checked_add(offset as usize)
        .ok_or(Resource::Accounting)?;
    if index >= end || input.uses()[index].coordinate != coordinate {
        return Err(Error::Statement("input use coordinate"));
    }
    Ok(index)
}
fn incoming(
    input: &Inventory<'_>,
    output: &Inventory<'_>,
    rows: Rows<'_>,
    plan: &Plan,
    original: usize,
    target: usize,
    usage: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    out.budget.charge_work(4)?;
    let a = input.uses()[input_use(input, rows.uses[usage].input)?].definition;
    let b = output.uses()[usage].definition;
    if !local(input, a, original) && !local(output, b, target) {
        let anchor = plan.anchors[b];
        emit!(out, " base[{a}] == base[{anchor}],\n");
    }
    Ok(())
}

fn observations(
    input: &Inventory<'_>,
    output: &Inventory<'_>,
    rows: Rows<'_>,
    plan: &Plan,
    original: usize,
    target: usize,
    side: Side,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    emit!(out, "seq![{}_final,", side.label());
    for ordinal in input.blocks()[original].operations.clone() {
        out.budget.charge_work(1)?;
        for definition in input.operations()[ordinal].results.clone() {
            for descendant in descendants(rows, definition)? {
                out.budget.charge_work(4)?;
                let output_definition = definition_index(output, descendant.output)?;
                match side {
                    Side::Input => value(input, plan, original, side, definition, out)?,
                    Side::Output => value(output, plan, target, side, output_definition, out)?,
                }
                emit!(out, ",");
            }
        }
        if plan.input_operations[ordinal] == NONE
            && concrete(input, ordinal).is_some_and(|(op, _, _)| {
                matches!(op, BinaryOp::Add | BinaryOp::Subtract | BinaryOp::Multiply)
            })
        {
            match side {
                Side::Input => emit!(out, "if n_trap{ordinal} {{ 1int }} else {{ 0int }},"),
                Side::Output => emit!(out, "0int,"),
            }
        }
    }
    for ordinal in output.blocks()[target].operations.clone() {
        for usage in output.operations()[ordinal].operands.clone() {
            observe_use(
                input, output, rows, plan, original, target, side, usage, out,
            )?;
        }
    }
    for usage in output.blocks()[target].terminator_uses.clone() {
        observe_use(
            input, output, rows, plan, original, target, side, usage, out,
        )?;
    }
    emit!(out, "]");
    Ok(())
}
fn observe_use(
    input: &Inventory<'_>,
    output: &Inventory<'_>,
    rows: Rows<'_>,
    plan: &Plan,
    original: usize,
    target: usize,
    side: Side,
    usage: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    out.budget.charge_work(4)?;
    match side {
        Side::Input => value(
            input,
            plan,
            original,
            side,
            input.uses()[input_use(input, rows.uses[usage].input)?].definition,
            out,
        )?,
        Side::Output => value(
            output,
            plan,
            target,
            side,
            output.uses()[usage].definition,
            out,
        )?,
    }
    emit!(out, ",");
    Ok(())
}

#[cfg(test)]
#[path = "mixed_optimizer_semantics_v26_tests.rs"]
mod tests;

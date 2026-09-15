//! Bounded rejection diagnostics from existing source indices, never a proof query.
use super::*;
use std::io::{self, Write};

const MAX_LINKS: usize = 4;

pub(super) fn write(
    projector: &TotalUnsignedIndexProjectorV1<'_, '_, '_>,
    record: EnumCarrierUse,
    out: &mut impl Write,
) -> io::Result<()> {
    let mut local = record.local;
    let mut seen = [None; MAX_LINKS];
    for link in 0..MAX_LINKS {
        seen[link] = Some(local);
        let index = local as usize;
        let definition = projector.definitions().get(index).copied().flatten();
        write!(
            out,
            "capability-index-enum link={link} use=bb{}:s{} carrier={} variant={}:type{} field={}:type{} local={local} declared_type={:?} definitions={:?} escaped={:?} definition={:?}",
            record.block,
            record.statement,
            record.local,
            record.variant,
            record.carrier_type,
            record.field,
            record.field_type,
            projector
                .function
                .locals()
                .get(index)
                .map(|local| local.ty().index()),
            projector.local_definitions.get(index),
            projector.address_escaped().get(index),
            definition.map(|site| (site.block, site.statement)),
        )?;
        if projector.local_definitions.get(index).copied() != Some(1) {
            return writeln!(out, " stop=nonunique-or-missing-definition");
        }
        let Some(definition) = definition else {
            return writeln!(out, " stop=no-indexed-assignment");
        };
        let Some(SemanticStatementKindV1::Assign(assignment)) = projector
            .function
            .blocks()
            .get(definition.block)
            .and_then(|block| block.statements().get(definition.statement))
            .map(|statement| statement.kind())
        else {
            return writeln!(out, " stop=indexed-site-not-assignment");
        };
        write!(out, " destination=")?;
        place(out, assignment.destination())?;
        write!(
            out,
            " result_type={} ",
            assignment.value().result_type().index()
        )?;
        rvalue(out, assignment.value().kind(), record)?;
        if assignment.destination().local().index() != local
            || !assignment.destination().projections().is_empty()
        {
            return writeln!(out, " stop=nonwhole-destination");
        }
        // Follow only the literal whole-place Use edge. No invented SSA use,
        // variant selection, expression evaluation or dominance query occurs.
        let SemanticRvalueKindV1::Use(source) = assignment.value().kind() else {
            return writeln!(out, " stop=not-direct-forwarding");
        };
        let Some(source) = raw_operand_place(source) else {
            return writeln!(out, " stop=constant-use");
        };
        if !source.projections().is_empty() {
            return writeln!(out, " stop=projected-use");
        }
        local = source.local().index();
        if seen.contains(&Some(local)) {
            return writeln!(out, " next={local} stop=cycle");
        }
        if link + 1 == MAX_LINKS {
            return writeln!(out, " next={local} stop=link-limit");
        }
        writeln!(out, " next={local}")?;
    }
    Ok(())
}

fn place(out: &mut impl Write, place: &SemanticPlaceV1) -> io::Result<()> {
    write!(
        out,
        "(local={},type={},projections={:?},projection_count={})",
        place.local().index(),
        place.ty().index(),
        &place.projections()[..place.projections().len().min(2)],
        place.projections().len(),
    )
}

fn operand(out: &mut impl Write, operand: &SemanticOperandV1) -> io::Result<()> {
    match operand {
        SemanticOperandV1::Copy(value) => {
            write!(out, "Copy")?;
            place(out, value)
        }
        SemanticOperandV1::Move(value) => {
            write!(out, "Move")?;
            place(out, value)
        }
        SemanticOperandV1::Constant(_) => write!(out, "Constant(type={})", operand.ty().index()),
    }
}

fn pair(
    out: &mut impl Write,
    left: &SemanticOperandV1,
    right: &SemanticOperandV1,
) -> io::Result<()> {
    operand(out, left)?;
    write!(out, ",")?;
    operand(out, right)
}

fn rvalue(
    out: &mut impl Write,
    value: &SemanticRvalueKindV1,
    record: EnumCarrierUse,
) -> io::Result<()> {
    match value {
        SemanticRvalueKindV1::Use(value) => {
            write!(out, "rvalue=Use operand=")?;
            operand(out, value)
        }
        SemanticRvalueKindV1::Aggregate(value) => {
            write!(
                out,
                "rvalue=Aggregate kind={:?} arity={}",
                value.kind(),
                value.operands().len()
            )?;
            for (index, value) in value.operands().iter().take(2).enumerate() {
                write!(out, " operand{index}=")?;
                operand(out, value)?;
            }
            // Report the requested field even when it lies outside the prefix;
            // this is one indexed lookup, not a scan of all aggregate operands.
            if record.field >= 2 {
                write!(out, " requested_operand{}=", record.field)?;
                if let Some(value) = value.operands().get(record.field as usize) {
                    operand(out, value)?;
                } else {
                    write!(out, "absent")?;
                }
            }
            write!(
                out,
                " operand_prefix_truncated={}",
                value.operands().len() > 2
            )
        }
        SemanticRvalueKindV1::Unary {
            operation,
            operand: value,
        } => {
            write!(out, "rvalue=Unary operation={operation:?} operand=")?;
            operand(out, value)
        }
        SemanticRvalueKindV1::Binary {
            operation,
            left,
            right,
        } => {
            write!(out, "rvalue=Binary operation={operation:?} operands=")?;
            pair(out, left, right)
        }
        SemanticRvalueKindV1::CheckedBinary(value) => {
            write!(
                out,
                "rvalue=CheckedBinary operation={:?} operands=",
                value.operation()
            )?;
            pair(out, value.left(), value.right())
        }
        SemanticRvalueKindV1::UncheckedBinary(value) => {
            write!(
                out,
                "rvalue=UncheckedBinary operation={:?} operands=",
                value.operation()
            )?;
            pair(out, value.left(), value.right())
        }
        SemanticRvalueKindV1::Cast {
            kind,
            operand: value,
        } => {
            write!(out, "rvalue=Cast kind={kind:?} operand=")?;
            operand(out, value)
        }
        SemanticRvalueKindV1::Borrow { kind, place: value } => {
            write!(out, "rvalue=Borrow kind={kind:?} place=")?;
            place(out, value)
        }
        SemanticRvalueKindV1::AddressOf {
            mutability,
            place: value,
        } => {
            write!(out, "rvalue=AddressOf mutability={mutability:?} place=")?;
            place(out, value)
        }
        SemanticRvalueKindV1::Length(value) => {
            write!(out, "rvalue=Length place=")?;
            place(out, value)
        }
        SemanticRvalueKindV1::Discriminant(value) => {
            write!(out, "rvalue=Discriminant place=")?;
            place(out, value)
        }
        SemanticRvalueKindV1::Load(value) => {
            write!(
                out,
                "rvalue=Load volatility={:?} atomic={:?} source=",
                value.volatility(),
                value.atomic()
            )?;
            place(out, value.source())
        }
    }
}

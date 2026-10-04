//! Record codecs reuse the existing descriptor Reader/Output primitives.
use crate::conditional_invocation_v1::*;
use crate::decode::Reader;
use crate::nominal_v3::Output as Writer;

pub(crate) const HEADER: usize = 28;
pub(crate) const SUBJECTS: usize = 292;
pub(crate) const THEOREM: usize = 256;
pub(crate) const OUTPUT: usize = 48;
pub(crate) const ARGUMENT: usize = 88;
pub(crate) const READ: usize = 80;
pub(crate) const PREMISE: usize = 32;
pub(crate) const FIXED: usize = HEADER + SUBJECTS + THEOREM + OUTPUT;

fn zero(r: &mut Reader<'_>, n: usize) -> FormatResult<()> {
    if r.take(n)?.iter().any(|byte| *byte != 0) {
        return Err(invalid("reserved bytes"));
    }
    Ok(())
}
fn domain(tag: u8) -> FormatResult<ConditionalAddressDomainV1> {
    match tag {
        0 => Ok(ConditionalAddressDomainV1::GuardedOutput),
        1 => Ok(ConditionalAddressDomainV1::GlobalLaunch),
        _ => Err(invalid("address domain")),
    }
}
pub(crate) fn subjects(r: &mut Reader<'_>) -> FormatResult<ConditionalSubjectsV1> {
    let kernel_id = r.fixed()?;
    let exact_graph_identity = r.fixed()?;
    let aggregate_statement_identity = r.fixed()?;
    let source_semantic_identity = r.fixed()?;
    if r.u32()? != ConditionalReferenceKindV1::Mir as u32 {
        return Err(invalid("reference kind"));
    }
    Ok(ConditionalSubjectsV1 {
        kernel_id,
        exact_graph_identity,
        aggregate_statement_identity,
        source_semantic_identity,
        reference_kind: ConditionalReferenceKindV1::Mir,
        safe_reference_identity: r.fixed()?,
        safe_reference_source_hash: r.fixed()?,
        safe_reference_mir_hash: r.fixed()?,
        kernel_subject_identity: r.fixed()?,
        kernel_mir_hash: r.fixed()?,
    })
}
pub(crate) fn put_subjects(w: &mut Writer<'_>, s: ConditionalSubjectsV1) {
    for d in [
        s.kernel_id,
        s.exact_graph_identity,
        s.aggregate_statement_identity,
        s.source_semantic_identity,
    ] {
        w.bytes(&d);
    }
    w.u32(s.reference_kind as u32);
    for d in [
        s.safe_reference_identity,
        s.safe_reference_source_hash,
        s.safe_reference_mir_hash,
        s.kernel_subject_identity,
        s.kernel_mir_hash,
    ] {
        w.bytes(&d);
    }
}
pub(crate) fn theorem(r: &mut Reader<'_>) -> FormatResult<ConditionalTheoremV1> {
    Ok(ConditionalTheoremV1 {
        statement_identity: r.fixed()?,
        generated_source_identity: r.fixed()?,
        execution_identity: r.fixed()?,
        receipt_identity: r.fixed()?,
        staging_receipt_identity: r.fixed()?,
        staging_obligation_identity: r.fixed()?,
        staging_signer_identity: r.fixed()?,
        staging_execution_identity: r.fixed()?,
    })
}
pub(crate) fn put_theorem(w: &mut Writer<'_>, t: ConditionalTheoremV1) {
    for d in [
        t.statement_identity,
        t.generated_source_identity,
        t.execution_identity,
        t.receipt_identity,
        t.staging_receipt_identity,
        t.staging_obligation_identity,
        t.staging_signer_identity,
        t.staging_execution_identity,
    ] {
        w.bytes(&d);
    }
}
fn canonical(r: &mut Reader<'_>) -> FormatResult<ConditionalCanonicalLocationV1> {
    Ok(ConditionalCanonicalLocationV1 {
        block: r.u32()?,
        operation: r.u64()?,
    })
}
fn ranked(r: &mut Reader<'_>) -> FormatResult<ConditionalRankedLocationV1> {
    Ok(ConditionalRankedLocationV1 {
        block: r.u32()?,
        operation: r.u32()?,
    })
}
fn put_canonical(w: &mut Writer<'_>, v: ConditionalCanonicalLocationV1) {
    w.u32(v.block);
    w.u64(v.operation);
}
fn put_ranked(w: &mut Writer<'_>, v: ConditionalRankedLocationV1) {
    w.u32(v.block);
    w.u32(v.operation);
}
fn value(r: &mut Reader<'_>) -> FormatResult<ConditionalRankedValueV1> {
    let (tag, a, b) = (r.u32()?, r.u32()?, r.u32()?);
    match (tag, b) {
        (0, 0) => Ok(ConditionalRankedValueV1::Argument(a)),
        (1, _) => Ok(ConditionalRankedValueV1::BlockArgument {
            block: a,
            argument: b,
        }),
        (2, 0) => Ok(ConditionalRankedValueV1::Local(a)),
        _ => Err(invalid("ranked value tag/padding")),
    }
}
fn put_value(w: &mut Writer<'_>, v: ConditionalRankedValueV1) {
    let (tag, a, b) = match v {
        ConditionalRankedValueV1::Argument(a) => (0, a, 0),
        ConditionalRankedValueV1::BlockArgument { block, argument } => (1, block, argument),
        ConditionalRankedValueV1::Local(a) => (2, a, 0),
    };
    w.u32(tag);
    w.u32(a);
    w.u32(b);
}
pub(crate) fn argument(r: &mut Reader<'_>) -> FormatResult<ConditionalArgumentBindingV1> {
    let canonical_parameter = r.u32()?;
    let source_argument = r.u32()?;
    let adjusted_argument = r.u32()?;
    let semantic_local = r.u32()?;
    let semantic_type = r.u32()?;
    let generated_field = r.u16()?;
    let role = match r.u8()? {
        0 => ConditionalArgumentRoleV1::Input,
        1 => ConditionalArgumentRoleV1::Output,
        _ => return Err(invalid("argument role")),
    };
    zero(r, 1)?;
    Ok(ConditionalArgumentBindingV1 {
        canonical_parameter,
        source_argument,
        adjusted_argument,
        semantic_local,
        semantic_type,
        generated_field,
        role,
        source_type_identity: r.fixed()?,
        device_layout_identity: r.fixed()?,
    })
}
pub(crate) fn put_argument(w: &mut Writer<'_>, a: ConditionalArgumentBindingV1) {
    for n in [
        a.canonical_parameter,
        a.source_argument,
        a.adjusted_argument,
        a.semantic_local,
        a.semantic_type,
    ] {
        w.u32(n);
    }
    w.u16(a.generated_field);
    w.u8(a.role as u8);
    w.u8(0);
    w.bytes(&a.source_type_identity);
    w.bytes(&a.device_layout_identity);
}
pub(crate) fn output(r: &mut Reader<'_>) -> FormatResult<ConditionalOutputV1> {
    let argument = r.u16()?;
    zero(r, 2)?;
    let v = ConditionalOutputV1 {
        argument,
        canonical_store: canonical(r)?,
        ranked_store: ranked(r)?,
        ranked_effect: ranked(r)?,
        element_bytes: r.u64()?,
        alignment: r.u32()?,
        address_domain: domain(r.u8()?)?,
    };
    zero(r, 3)?;
    Ok(v)
}
pub(crate) fn put_output(w: &mut Writer<'_>, o: ConditionalOutputV1) {
    w.u16(o.argument);
    w.u16(0);
    put_canonical(w, o.canonical_store);
    put_ranked(w, o.ranked_store);
    put_ranked(w, o.ranked_effect);
    w.u64(o.element_bytes);
    w.u32(o.alignment);
    w.u8(o.address_domain as u8);
    w.bytes(&[0; 3]);
}
pub(crate) fn read(r: &mut Reader<'_>) -> FormatResult<ConditionalReadOccurrenceV1> {
    let argument = r.u16()?;
    zero(r, 2)?;
    let canonical = canonical(r)?;
    let (slice, pointer, index, value) = (r.u32()?, r.u32()?, r.u32()?, r.u32()?);
    let ranked = ranked(r)?;
    let ranked_view = self::value(r)?;
    let ranked_index = self::value(r)?;
    let access_domain = domain(r.u8()?)?;
    let address_domain = domain(r.u8()?)?;
    zero(r, 2)?;
    Ok(ConditionalReadOccurrenceV1 {
        argument,
        canonical,
        slice,
        pointer,
        index,
        value,
        ranked,
        ranked_view,
        ranked_index,
        access_domain,
        address_domain,
        element_bytes: r.u64()?,
        alignment: r.u32()?,
    })
}
pub(crate) fn put_read(w: &mut Writer<'_>, r: ConditionalReadOccurrenceV1) {
    w.u16(r.argument);
    w.u16(0);
    put_canonical(w, r.canonical);
    for n in [r.slice, r.pointer, r.index, r.value] {
        w.u32(n);
    }
    put_ranked(w, r.ranked);
    put_value(w, r.ranked_view);
    put_value(w, r.ranked_index);
    w.u8(r.access_domain as u8);
    w.u8(r.address_domain as u8);
    w.u16(0);
    w.u64(r.element_bytes);
    w.u32(r.alignment);
}
pub(crate) fn premise(r: &mut Reader<'_>) -> FormatResult<ConditionalRuntimePremiseV1> {
    use ConditionalRuntimePremiseV1 as P;
    let tag = r.u8()?;
    let d = r.u8()?;
    zero(r, 2)?;
    let (a, b, width, align) = (r.u32()?, r.u32()?, r.u64()?, r.u32()?);
    zero(r, 8)?;
    match tag {
        0 if (d, a, b, width, align) == (0, 0, 0, 0, 0) => Ok(P::D1Launch),
        1 | 2 if (d, b, width, align) == (0, 0, 0, 0) => Ok(if tag == 1 {
            P::OutputWithinGlobalX { parameter: a }
        } else {
            P::WritableOutput { parameter: a }
        }),
        3 if (b, width, align) == (0, 0, 0) => Ok(P::ReadableInput {
            parameter: a,
            domain: domain(d)?,
        }),
        4 if (d, width, align) == (0, 0, 0) => Ok(P::SeparateInputOutput {
            input: a,
            output: b,
        }),
        5 if b == 0 => Ok(P::RepresentableAddress {
            parameter: a,
            domain: domain(d)?,
            element_bytes: width,
            alignment: align,
        }),
        _ => Err(invalid("premise tag/padding")),
    }
}
pub(crate) fn put_premise(w: &mut Writer<'_>, p: ConditionalRuntimePremiseV1) {
    use ConditionalRuntimePremiseV1 as P;
    let (tag, d, a, b, width, align) = match p {
        P::D1Launch => (0, 0, 0, 0, 0, 0),
        P::OutputWithinGlobalX { parameter } => (1, 0, parameter, 0, 0, 0),
        P::WritableOutput { parameter } => (2, 0, parameter, 0, 0, 0),
        P::ReadableInput { parameter, domain } => (3, domain as u8, parameter, 0, 0, 0),
        P::SeparateInputOutput { input, output } => (4, 0, input, output, 0, 0),
        P::RepresentableAddress {
            parameter,
            domain,
            element_bytes,
            alignment,
        } => (5, domain as u8, parameter, 0, element_bytes, alignment),
    };
    w.u8(tag);
    w.u8(d);
    w.u16(0);
    w.u32(a);
    w.u32(b);
    w.u64(width);
    w.u32(align);
    w.bytes(&[0; 8]);
}
pub(crate) fn root(r: &mut Reader<'_>) -> FormatResult<[u64; 4]> {
    Ok([r.u64()?, r.u64()?, r.u64()?, r.u64()?])
}

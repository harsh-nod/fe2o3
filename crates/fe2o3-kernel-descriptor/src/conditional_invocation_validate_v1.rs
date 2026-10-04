use crate::conditional_invocation_codec::{Theorem, View};
use crate::conditional_invocation_rows_v1 as row;
use crate::conditional_invocation_v1::*;
use sha2::{Digest, Sha256};

pub(crate) trait Records {
    fn argument(&self, i: usize) -> FormatResult<ConditionalArgumentBindingV1>;
    fn read(&self, i: usize) -> FormatResult<ConditionalReadOccurrenceV1>;
    fn premise(&self, i: usize) -> FormatResult<ConditionalRuntimePremiseV1>;
}
impl Records for ConditionalInvocationContractInputV1<'_> {
    fn argument(&self, i: usize) -> FormatResult<ConditionalArgumentBindingV1> {
        self.arguments
            .get(i)
            .copied()
            .ok_or(invalid("argument reference"))
    }
    fn read(&self, i: usize) -> FormatResult<ConditionalReadOccurrenceV1> {
        self.reads.get(i).copied().ok_or(invalid("read reference"))
    }
    fn premise(&self, i: usize) -> FormatResult<ConditionalRuntimePremiseV1> {
        self.premises
            .get(i)
            .copied()
            .ok_or(invalid("premise reference"))
    }
}
impl<T> Records for View<'_, T> {
    fn argument(&self, i: usize) -> FormatResult<ConditionalArgumentBindingV1> {
        self.record(1, i, row::ARGUMENT, row::argument)
    }
    fn read(&self, i: usize) -> FormatResult<ConditionalReadOccurrenceV1> {
        self.record(2, i, row::READ, row::read)
    }
    fn premise(&self, i: usize) -> FormatResult<ConditionalRuntimePremiseV1> {
        self.record(3, i, row::PREMISE, row::premise)
    }
}

pub(crate) fn length(counts: [usize; 4], fixed: usize) -> FormatResult<usize> {
    let [roots, arguments, reads, premises] = counts;
    if roots == 0
        || roots > MAX_CONDITIONAL_ROOTS_V1
        || arguments == 0
        || arguments > MAX_CONDITIONAL_ARGUMENTS_V1
        || reads > MAX_CONDITIONAL_READS_V1
        || premises > MAX_CONDITIONAL_PREMISES_V1
    {
        return Err(invalid("record count"));
    }
    let mut n = fixed;
    for (count, stride) in counts
        .into_iter()
        .zip([32, row::ARGUMENT, row::READ, row::PREMISE])
    {
        n = count
            .checked_mul(stride)
            .and_then(|v| n.checked_add(v))
            .ok_or(invalid("length overflow"))?;
    }
    if n > MAX_CONDITIONAL_INVOCATION_BYTES_V1 {
        return Err(invalid("byte limit"));
    }
    Ok(n)
}

fn layout(width: u64, alignment: u32) -> FormatResult<()> {
    if !matches!(width, 1 | 2 | 4 | 8 | 16)
        || !alignment.is_power_of_two()
        || u64::from(alignment) > width
    {
        return Err(invalid("element width/alignment"));
    }
    Ok(())
}

/// Only the exact current verifier preimage is checked. In particular we cannot
/// recompute the aggregate or graph from this projection, authenticate a signer,
/// or verify that the receipt binds these claims. The consuming owner must do so.
pub(crate) fn theorem_matches(
    domain: &[u8],
    subjects: ConditionalSubjectsV1,
    t: ConditionalTheoremV1,
    cpu_input: Option<[u8; 32]>,
) -> bool {
    let mut hash = Sha256::new();
    hash.update(domain);
    for d in [
        subjects.aggregate_statement_identity,
        t.generated_source_identity,
        t.staging_receipt_identity,
        t.staging_obligation_identity,
        t.staging_signer_identity,
        t.staging_execution_identity,
    ] {
        hash.update(d);
    }
    if let Some(cpu_input) = cpu_input {
        hash.update(cpu_input);
    }
    let expected: [u8; 32] = hash.finalize().into();
    t.statement_identity == expected
}

/// Shared by encode preflight and independent decode, after bounded precharge.
pub(crate) fn validate<T: Theorem>(
    rows: &impl Records,
    counts: [usize; 4],
    subjects: ConditionalSubjectsV1,
    theorem: T,
    output: ConditionalOutputV1,
) -> FormatResult<()> {
    use ConditionalArgumentRoleV1 as Role;
    use ConditionalRuntimePremiseV1 as P;
    length(counts, T::FIXED)?;
    let [_, arguments, reads, premises] = counts;
    // Structural parity with FunctionalRefinementSubjectsV2's MIR profile,
    // not evidence that any of the required subjects came from that owner.
    if subjects.safe_reference_source_hash != [0; 32]
        || [
            subjects.safe_reference_identity,
            subjects.safe_reference_mir_hash,
            subjects.kernel_subject_identity,
            subjects.kernel_mir_hash,
        ]
        .contains(&[0; 32])
    {
        return Err(invalid("MIR subject shape"));
    }
    if !theorem.matches(subjects) {
        return Err(invalid("theorem preimage"));
    }
    if usize::from(output.argument) >= arguments {
        return Err(invalid("output reference"));
    }
    let out = rows.argument(usize::from(output.argument))?;
    if out.role != Role::Output {
        return Err(invalid("output role"));
    }
    if output.ranked_store == output.ranked_effect {
        return Err(invalid("store/effect occurrence collision"));
    }
    layout(output.element_bytes, output.alignment)?;
    for i in 0..arguments {
        let a = rows.argument(i)?;
        if usize::from(a.generated_field) >= MAX_CONDITIONAL_ARGUMENTS_V1 {
            return Err(invalid("generated field limit"));
        }
        if (a.role == Role::Output) != (i == usize::from(output.argument)) {
            return Err(invalid("single output roster"));
        }
        for j in 0..i {
            let b = rows.argument(j)?;
            if b.canonical_parameter >= a.canonical_parameter {
                return Err(invalid("canonical parameter order"));
            }
            if a.generated_field == b.generated_field
                || a.source_argument == b.source_argument
                || a.adjusted_argument == b.adjusted_argument
                || a.semantic_local == b.semantic_local
            {
                return Err(invalid("duplicate argument mapping"));
            }
        }
        if a.role == Role::Input {
            let mut used = false;
            for j in 0..reads {
                used |= usize::from(rows.read(j)?.argument) == i;
            }
            if !used {
                return Err(invalid("unused input binding"));
            }
        }
    }
    // Checked against conditional_aggregate_v1::verify_conditional_final_graph_v1:
    // four header rows, then three per occurrence, with no parameter deduplication.
    if premises != 4 + 3 * reads {
        return Err(invalid("complete premise roster"));
    }
    let parameter = out.canonical_parameter;
    let header = [
        P::D1Launch,
        P::OutputWithinGlobalX { parameter },
        P::WritableOutput { parameter },
        P::RepresentableAddress {
            parameter,
            domain: output.address_domain,
            element_bytes: output.element_bytes,
            alignment: output.alignment,
        },
    ];
    for (i, expected) in header.into_iter().enumerate() {
        if rows.premise(i)? != expected {
            return Err(invalid("output premise order/binding"));
        }
    }
    for i in 0..reads {
        let r = rows.read(i)?;
        if usize::from(r.argument) >= arguments {
            return Err(invalid("read argument reference"));
        }
        let a = rows.argument(usize::from(r.argument))?;
        if a.role != Role::Input {
            return Err(invalid("read argument role"));
        }
        layout(r.element_bytes, r.alignment)?;
        if r.access_domain == ConditionalAddressDomainV1::GlobalLaunch
            && r.address_domain != ConditionalAddressDomainV1::GlobalLaunch
        {
            return Err(invalid("access/address domain relation"));
        }
        if r.canonical == output.canonical_store
            || r.ranked == output.ranked_store
            || r.ranked == output.ranked_effect
        {
            return Err(invalid("read/store occurrence collision"));
        }
        for j in 0..i {
            let p = rows.read(j)?;
            if r.canonical == p.canonical || r.ranked == p.ranked {
                return Err(invalid("duplicate read occurrence"));
            }
            if r.argument == p.argument && r.element_bytes != p.element_bytes {
                return Err(invalid("input element width consistency"));
            }
        }
        let parameter = a.canonical_parameter;
        let expected = [
            P::ReadableInput {
                parameter,
                domain: r.access_domain,
            },
            P::SeparateInputOutput {
                input: parameter,
                output: out.canonical_parameter,
            },
            P::RepresentableAddress {
                parameter,
                domain: r.address_domain,
                element_bytes: r.element_bytes,
                alignment: r.alignment,
            },
        ];
        for (j, p) in expected.into_iter().enumerate() {
            if rows.premise(4 + 3 * i + j)? != p {
                return Err(invalid("read premise order/binding"));
            }
        }
    }
    Ok(())
}

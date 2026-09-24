//! Conditional byte-range arithmetic, not CPU equivalence or launch authority.
//!
//! Permissions and the D1 bijection are explicit runtime assumptions. Target
//! limits remain theorem parameters: the consumer must discharge their actual
//! target values, not substitute convenient limits. No pointer provenance or
//! LLVM `inbounds` property follows from these integer-address theorems.

use std::fmt::Write as _;

use fe2o3_kernel_ir::ConditionalTotalViewAddressDomainV1 as Domain;
use fe2o3_pliron::ProductionConditionalRuntimePremiseV1 as Premise;

use super::{BoundedSource, Budget, Error, source_limit};

#[derive(Clone, Copy)]
struct Address {
    domain: Domain,
    width: u64,
    alignment: u32,
}

fn address(premise: &Premise, parameter: u32) -> Result<Address, Error> {
    let Premise::RepresentableAddress {
        parameter: actual,
        domain,
        element_bytes,
        alignment,
    } = *premise
    else {
        return Err(Error::Subject("conditional address premise"));
    };
    // Scalar byte widths only. A stronger per-access alignment can require a
    // sparse index domain, which this complete-prefix theorem does not model.
    if actual != parameter
        || !matches!(element_bytes, 1 | 2 | 4 | 8 | 16)
        || !alignment.is_power_of_two()
        || u64::from(alignment) > element_bytes
    {
        return Err(Error::Subject("conditional address width/alignment"));
    }
    Ok(Address {
        domain,
        width: element_bytes,
        alignment,
    })
}

fn read_row(row: &[Premise], output: u32) -> Result<(u32, Domain, Address), Error> {
    let [
        Premise::ReadableInput { parameter, domain },
        Premise::SeparateInputOutput {
            input,
            output: actual_output,
        },
        formation,
    ] = row
    else {
        return Err(Error::Subject("conditional read premise roster"));
    };
    if parameter == &output || input != parameter || actual_output != &output {
        return Err(Error::Subject("conditional read/output parameter"));
    }
    let formation = address(formation, *parameter)?;
    if *domain == Domain::GlobalLaunch && formation.domain != Domain::GlobalLaunch {
        return Err(Error::Subject("global read with guarded address"));
    }
    Ok((*parameter, *domain, formation))
}

pub(super) fn append_premise_theorem(
    out: &mut BoundedSource,
    premises: &[Premise],
    output: u32,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    budget.charge_work(4)?;
    let Some(header) = premises.get(..4) else {
        return Err(Error::Subject("conditional output premise roster"));
    };
    let [
        Premise::D1Launch,
        Premise::OutputWithinGlobalX { parameter: extent },
        Premise::WritableOutput {
            parameter: writable,
        },
        formation,
    ] = header
    else {
        return Err(Error::Subject("conditional output premise roster"));
    };
    if *extent != output || *writable != output || (premises.len() - 4) % 3 != 0 {
        return Err(Error::Subject("conditional output/read premise roster"));
    }
    let output_address = address(formation, output)?;
    let reads = &premises[4..];
    // Validate before emitting. Repeated read occurrences are legal, but one
    // canonical input cannot change element width. Every comparison is charged;
    // this borrowed scan avoids allocating an unmetered parameter-index map.
    for (ordinal, row) in reads.chunks_exact(3).enumerate() {
        budget.charge_work(3)?;
        let (parameter, _, current) = read_row(row, output)?;
        for prior in reads.chunks_exact(3).take(ordinal) {
            budget.charge_work(3)?;
            let (previous, _, previous_address) = read_row(prior, output)?;
            if previous == parameter && previous_address.width != current.width {
                return Err(Error::Subject("conditional input element width changed"));
            }
        }
    }
    out.write_str(PRELUDE).map_err(source_limit)?;
    append_output(out, output, output_address)?;
    for (ordinal, row) in reads.chunks_exact(3).enumerate() {
        budget.charge_work(3)?;
        let (parameter, access, formation) = read_row(row, output)?;
        append_read(
            out,
            ordinal,
            parameter,
            output,
            access,
            formation,
            output_address,
        )?;
    }
    Ok(())
}

fn prefix(domain: Domain) -> &'static str {
    match domain {
        Domain::GlobalLaunch => "g",
        Domain::GuardedOutput => "(if g == 0 { 0 } else if n == 0 { 1 } else { n })",
    }
}

fn selected_index(domain: Domain) -> &'static str {
    match domain {
        Domain::GlobalLaunch => "i",
        Domain::GuardedOutput => "(if i < n { i } else { 0 })",
    }
}

fn append_formation_body(out: &mut BoundedSource, a: Address) -> Result<(), Error> {
    let k = prefix(a.domain);
    let q = selected_index(a.domain);
    let w = a.width;
    let align = a.alignment;
    write!(
        out,
        r#"
        if 0 <= i < g {{
            let k: int = {k};
            let q: int = {q};
            assert(0 < k);
            assert(0 <= q < k);
            assert(q <= k - 1);
            assert(0 <= {w} * q <= offset_max);
            assert(0 <= base + {w} * q <= pointer_max);
            assert((base + {w} * q) % {align} == 0);
        }}
"#
    )
    .map_err(source_limit)
}

fn append_permission_body(out: &mut BoundedSource, width: u64, bound: &str) -> Result<(), Error> {
    write!(
        out,
        r#"
        if 0 <= i < {bound} {{
            assert forall|byte: int|
                base + {width} * i <= byte < base + {width} * (i + 1)
                implies #[trigger] permission(byte) by {{
                assert(base <= byte < base + {width} * {bound});
            }};
        }}
"#
    )
    .map_err(source_limit)
}

fn append_output(out: &mut BoundedSource, parameter: u32, a: Address) -> Result<(), Error> {
    let w = a.width;
    let align = a.alignment;
    let k = prefix(a.domain);
    let q = selected_index(a.domain);
    write!(
        out,
        r#"
    proof fn fe2o3_conditional_output_p{parameter}_v1(
        base: int, n: int, g: int, i: int, j: int,
        index_max: int, offset_max: int, pointer_max: int,
        permission: spec_fn(int) -> bool,
        global_x: spec_fn(int) -> int, owner: spec_fn(int) -> int,
    )
        requires
            fe2o3_conditional_d1_v1(g, global_x, owner),
            0 <= n <= g,
            fe2o3_conditional_span_v1(base, n, {w}, pointer_max),
            n > 0 ==> base % {align} == 0,
            fe2o3_conditional_permissions_v1(permission, base, {w} * n),
            fe2o3_conditional_prefix_v1(
                base, {k}, {w}, {align}, index_max, offset_max, pointer_max),
        ensures
            0 <= i < n ==> 0 <= owner(i) < g && global_x(owner(i)) == i,
            0 <= i < g ==> fe2o3_conditional_formed_v1(
                base, {q}, {w}, {align}, index_max, offset_max, pointer_max),
            0 <= i < n ==> fe2o3_conditional_permissions_v1(
                permission, base + {w} * i, {w}),
            0 <= i < n && 0 <= j < n && i != j ==>
                base + {w} * i != base + {w} * j
                && fe2o3_conditional_separate_v1(
                    base + {w} * i, {w}, base + {w} * j, {w}),
    {{
"#
    )
    .map_err(source_limit)?;
    append_formation_body(out, a)?;
    append_permission_body(out, w, "n")?;
    out.write_str(
        r#"
        if 0 <= i < n {
            assert(0 <= i < g);
            assert(0 <= owner(i) < g && global_x(owner(i)) == i);
        }
        if 0 <= i < n && 0 <= j < n && i != j {
            if i < j { assert(i + 1 <= j); }
            else { assert(j + 1 <= i); }
        }
    }
"#,
    )
    .map_err(source_limit)
}

fn append_read(
    out: &mut BoundedSource,
    ordinal: usize,
    parameter: u32,
    output: u32,
    access: Domain,
    a: Address,
    output_address: Address,
) -> Result<(), Error> {
    let bound = match access {
        Domain::GuardedOutput => "n",
        Domain::GlobalLaunch => "g",
    };
    let w = a.width;
    let ow = output_address.width;
    let align = a.alignment;
    let k = prefix(a.domain);
    let q = selected_index(a.domain);
    write!(
        out,
        r#"
    proof fn fe2o3_conditional_read_{ordinal}_p{parameter}_out{output}_v1(
        base: int, len: int, output_base: int, n: int, g: int, i: int, j: int,
        index_max: int, offset_max: int, pointer_max: int,
        permission: spec_fn(int) -> bool,
    )
        requires
            0 <= n <= g,
            {bound} <= len,
            fe2o3_conditional_span_v1(base, len, {w}, pointer_max),
            fe2o3_conditional_span_v1(output_base, n, {ow}, pointer_max),
            {bound} > 0 ==> base % {align} == 0,
            fe2o3_conditional_permissions_v1(permission, base, {w} * {bound}),
            fe2o3_conditional_separate_v1(base, {w} * len, output_base, {ow} * n),
            fe2o3_conditional_prefix_v1(
                base, {k}, {w}, {align}, index_max, offset_max, pointer_max),
        ensures
            0 <= i < {bound} ==> 0 <= i < len && i < g,
            0 <= i < g ==> fe2o3_conditional_formed_v1(
                base, {q}, {w}, {align}, index_max, offset_max, pointer_max),
            0 <= i < {bound} ==> fe2o3_conditional_permissions_v1(
                permission, base + {w} * i, {w}),
            0 <= i < {bound} && 0 <= j < n ==> fe2o3_conditional_separate_v1(
                base + {w} * i, {w}, output_base + {ow} * j, {ow}),
    {{
"#
    )
    .map_err(source_limit)?;
    append_formation_body(out, a)?;
    append_permission_body(out, w, bound)?;
    write!(
        out,
        r#"
        if 0 <= i < {bound} && 0 <= j < n {{
            assert(0 < len && 0 < n);
            assert(base + {w} * (i + 1) <= base + {w} * len);
            assert(output_base + {ow} * (j + 1) <= output_base + {ow} * n);
        }}
    }}
"#
    )
    .map_err(source_limit)
}

const PRELUDE: &str = r#"
    // These predicates are runtime assumptions over an abstract byte address
    // space. Permission includes liveness; no allocation is inferred from an
    // address-only launch tail, and no CPU/ISA semantics are asserted here.
    pub open spec fn fe2o3_conditional_d1_v1(
        g: int, global_x: spec_fn(int) -> int, owner: spec_fn(int) -> int,
    ) -> bool {
        &&& 0 <= g
        &&& (forall|t: int| 0 <= t < g ==>
            0 <= #[trigger] global_x(t) < g && owner(global_x(t)) == t)
        &&& (forall|i: int| 0 <= i < g ==>
            0 <= #[trigger] owner(i) < g && global_x(owner(i)) == i)
    }

    proof fn fe2o3_conditional_unique_invocation_v1(
        g: int, t: int, u: int,
        global_x: spec_fn(int) -> int, owner: spec_fn(int) -> int,
    )
        requires
            fe2o3_conditional_d1_v1(g, global_x, owner),
            0 <= t < g, 0 <= u < g,
        ensures global_x(t) == global_x(u) ==> t == u,
    {
        assert(owner(global_x(t)) == t);
        assert(owner(global_x(u)) == u);
    }

    pub open spec fn fe2o3_conditional_span_v1(
        base: int, len: int, width: int, pointer_max: int,
    ) -> bool {
        0 <= pointer_max && 0 <= base <= pointer_max && 0 <= len && 0 < width
        && width * len <= pointer_max && base + width * len <= pointer_max + 1
    }

    pub open spec fn fe2o3_conditional_permissions_v1(
        permission: spec_fn(int) -> bool, base: int, bytes: int,
    ) -> bool {
        forall|byte: int| base <= byte < base + bytes ==> #[trigger] permission(byte)
    }

    pub open spec fn fe2o3_conditional_separate_v1(
        a: int, a_bytes: int, b: int, b_bytes: int,
    ) -> bool {
        a_bytes == 0 || b_bytes == 0 || a + a_bytes <= b || b + b_bytes <= a
    }

    pub open spec fn fe2o3_conditional_prefix_v1(
        base: int, count: int, width: int, align: int,
        index_max: int, offset_max: int, pointer_max: int,
    ) -> bool {
        0 <= count && 0 < width && 0 < align && width % align == 0
        && 0 <= index_max && 0 <= offset_max && 0 <= pointer_max
        && (count == 0 || (
            0 <= base <= pointer_max && base % align == 0
            && count - 1 <= index_max && width * (count - 1) <= offset_max
            && base + width * (count - 1) <= pointer_max))
    }

    pub open spec fn fe2o3_conditional_formed_v1(
        base: int, index: int, width: int, align: int,
        index_max: int, offset_max: int, pointer_max: int,
    ) -> bool {
        0 < width && 0 < align && 0 <= index <= index_max
        && 0 <= width * index <= offset_max
        && 0 <= base + width * index <= pointer_max
        && (base + width * index) % align == 0
    }
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

    fn output(domain: Domain) -> [Premise; 4] {
        [
            Premise::D1Launch,
            Premise::OutputWithinGlobalX { parameter: 0 },
            Premise::WritableOutput { parameter: 0 },
            Premise::RepresentableAddress {
                parameter: 0,
                domain,
                element_bytes: 4,
                alignment: 4,
            },
        ]
    }

    fn read(parameter: u32, access: Domain, address: Domain) -> [Premise; 3] {
        [
            Premise::ReadableInput {
                parameter,
                domain: access,
            },
            Premise::SeparateInputOutput {
                input: parameter,
                output: 0,
            },
            Premise::RepresentableAddress {
                parameter,
                domain: address,
                element_bytes: 4,
                alignment: 4,
            },
        ]
    }

    fn generate(premises: &[Premise]) -> Result<String, Error> {
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        let mut out = BoundedSource::new()?;
        append_premise_theorem(&mut out, premises, 0, &mut budget)?;
        Ok(out.0)
    }

    #[test]
    fn guarded_output_keeps_empty_launch_and_selected_zero_tail() {
        let source = generate(&output(Domain::GuardedOutput)).unwrap();
        assert!(source.contains("if g == 0 { 0 } else if n == 0 { 1 } else { n }"));
        assert!(source.contains("if i < n { i } else { 0 }"));
        assert!(source.contains("count == 0 ||"));
        assert!(source.contains("0 <= n <= g"));
        assert!(!source.contains("requires\n            0 < n"));
        assert!(source.contains("owner(global_x(t)) == t"));
        assert!(source.contains("base + 4 * i != base + 4 * j"));
    }

    #[test]
    fn guarded_access_and_global_address_are_distinct() {
        let mut premises = output(Domain::GlobalLaunch).to_vec();
        premises.extend(read(1, Domain::GuardedOutput, Domain::GlobalLaunch));
        premises.extend(read(2, Domain::GlobalLaunch, Domain::GlobalLaunch));
        let source = generate(&premises).unwrap();
        let guarded = source
            .split("proof fn fe2o3_conditional_read_0_p1_out0_v1")
            .nth(1)
            .unwrap()
            .split("proof fn fe2o3_conditional_read_1_p2_out0_v1")
            .next()
            .unwrap();
        assert!(guarded.contains("n <= len"));
        assert!(guarded.contains("permission, base, 4 * n"));
        assert!(guarded.contains("base, g, 4, 4, index_max, offset_max, pointer_max"));
        assert!(guarded.contains("base, 4 * len, output_base, 4 * n"));
        let global = source
            .split("proof fn fe2o3_conditional_read_1_p2_out0_v1")
            .nth(1)
            .unwrap();
        assert!(global.contains("g <= len"));
        assert!(global.contains("permission, base, 4 * g"));
        assert!(!source.contains("assume("));
        assert!(!source.contains("external_body"));
    }

    #[test]
    fn every_address_domain_uses_separate_target_limits_and_permissions() {
        let mut premises = output(Domain::GuardedOutput).to_vec();
        premises.extend(read(1, Domain::GuardedOutput, Domain::GuardedOutput));
        let source = generate(&premises).unwrap();
        assert!(source.contains("count - 1 <= index_max"));
        assert!(source.contains("width * (count - 1) <= offset_max"));
        assert!(source.contains("base + width * (count - 1) <= pointer_max"));
        assert!(source.contains("implies #[trigger] permission(byte) by"));
        assert!(source.contains("a_bytes == 0 || b_bytes == 0"));
        assert!(source.contains("0 <= i < n && 0 <= j < n ==> fe2o3_conditional_separate_v1"));
    }

    #[test]
    fn malformed_headers_and_trailing_or_reordered_rows_are_rejected() {
        let valid = output(Domain::GlobalLaunch);
        for length in 0..4 {
            assert!(generate(&valid[..length]).is_err());
        }
        for index in 0..4 {
            let mut bad = valid;
            bad[index] = Premise::WritableOutput { parameter: 99 };
            assert!(generate(&bad).is_err());
        }
        let mut bad = valid.to_vec();
        bad.push(Premise::D1Launch);
        assert!(generate(&bad).is_err());
        bad = valid.to_vec();
        bad.extend(read(1, Domain::GuardedOutput, Domain::GlobalLaunch));
        bad.swap(4, 5);
        assert!(generate(&bad).is_err());
    }

    #[test]
    fn malformed_read_parameters_domains_widths_and_alignments_are_rejected() {
        let mut valid = output(Domain::GlobalLaunch).to_vec();
        valid.extend(read(1, Domain::GuardedOutput, Domain::GlobalLaunch));
        for replacement in [
            Premise::SeparateInputOutput {
                input: 2,
                output: 0,
            },
            Premise::SeparateInputOutput {
                input: 1,
                output: 2,
            },
        ] {
            let mut bad = valid.clone();
            bad[5] = replacement;
            assert!(generate(&bad).is_err());
        }
        for (width, alignment) in [(0, 4), (3, 1), (4, 0), (4, 3), (4, 8), (u64::MAX, 1)] {
            let mut bad = valid.clone();
            bad[6] = Premise::RepresentableAddress {
                parameter: 1,
                domain: Domain::GlobalLaunch,
                element_bytes: width,
                alignment,
            };
            assert!(generate(&bad).is_err());
        }
        let mut bad = output(Domain::GlobalLaunch).to_vec();
        bad.extend(read(0, Domain::GuardedOutput, Domain::GlobalLaunch));
        assert!(generate(&bad).is_err());
        bad = output(Domain::GlobalLaunch).to_vec();
        bad.extend(read(1, Domain::GlobalLaunch, Domain::GuardedOutput));
        assert!(generate(&bad).is_err());
    }

    #[test]
    fn repeated_read_occurrences_keep_unique_names_but_not_conflicting_widths() {
        let mut premises = output(Domain::GlobalLaunch).to_vec();
        premises.extend(read(1, Domain::GuardedOutput, Domain::GlobalLaunch));
        premises.extend(read(1, Domain::GlobalLaunch, Domain::GlobalLaunch));
        let source = generate(&premises).unwrap();
        assert!(source.contains("fe2o3_conditional_read_0_p1_out0_v1"));
        assert!(source.contains("fe2o3_conditional_read_1_p1_out0_v1"));
        premises[9] = Premise::RepresentableAddress {
            parameter: 1,
            domain: Domain::GlobalLaunch,
            element_bytes: 8,
            alignment: 4,
        };
        assert!(generate(&premises).is_err());
    }

    #[test]
    fn roster_scans_use_original_work_and_do_not_add_storage() {
        let mut premises = output(Domain::GlobalLaunch).to_vec();
        premises.extend(read(1, Domain::GuardedOutput, Domain::GlobalLaunch));
        premises.extend(read(2, Domain::GlobalLaunch, Domain::GlobalLaunch));
        // Header 4, validation 6, one previous-row comparison 3, emission 6.
        for (limit, accepted) in [(3, false), (12, false), (18, false), (19, true)] {
            let mut work = Work::new(limit);
            let mut budget = Budget::new(&mut work, usize::MAX);
            budget.reserve_storage(17).unwrap();
            let mut out = BoundedSource::new().unwrap();
            let result = append_premise_theorem(&mut out, &premises, 0, &mut budget);
            assert_eq!(result.is_ok(), accepted);
            if !accepted {
                assert!(matches!(result, Err(Error::Resource(_))));
            }
            assert_eq!(budget.storage(), 17);
            if accepted {
                assert_eq!(budget.work(), 19);
            }
        }
    }

    #[test]
    fn bounded_writer_refuses_source_growth() {
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        let mut out = BoundedSource::new().unwrap();
        while out.0.len() < super::super::SOURCE_LIMIT {
            out.write_char(' ').unwrap();
        }
        let capacity = out.0.capacity();
        assert!(
            append_premise_theorem(&mut out, &output(Domain::GlobalLaunch), 0, &mut budget)
                .is_err()
        );
        assert_eq!(out.0.capacity(), capacity);
        assert_eq!(out.0.len(), super::super::SOURCE_LIMIT);
    }
}

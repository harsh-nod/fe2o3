//! Bounded failure telemetry for the existing genuine consumer only.
//! No verifier decisions, source facts, budgets or returned errors are changed.
use super::*;
use fe2o3_kernel_ir::TensorLayoutFindingV1 as ContractFinding;
use fe2o3_pliron::{
    PlironTensorLayoutDataflowIssueV1 as DataflowFinding, PlironTensorLayoutFindingV1 as Finding,
};

const MAX_FINDINGS: usize = 8;
const MAX_DETAIL_BYTES: usize = 160;

fn selected(findings: &[Finding]) -> &[Finding] {
    &findings[..findings.len().min(MAX_FINDINGS)]
}

fn contract_tag(finding: &ContractFinding) -> &'static str {
    match finding {
        ContractFinding::UnsupportedProfile => "unsupported_profile",
        ContractFinding::ProfileMismatch { .. } => "profile_mismatch",
        ContractFinding::RoleMismatch { .. } => "role_mismatch",
        ContractFinding::ShapeOrElementMismatch { .. } => "shape_or_element",
        ContractFinding::FragmentWidthMismatch { .. } => "fragment_width",
        ContractFinding::UnsupportedSymbolicMap { .. } => "unsupported_map",
        ContractFinding::MalformedSymbolicMap { .. } => "malformed_map",
        ContractFinding::SymbolicMapMismatch { .. } => "map_mismatch",
        ContractFinding::CoordinateOutOfBounds { .. } => "coordinate_bounds",
        ContractFinding::DuplicateCoordinate { .. } => "duplicate_coordinate",
        ContractFinding::IncompleteCoverage { .. } => "incomplete_coverage",
        ContractFinding::BroadcastContractMismatch { .. } => "broadcast",
        ContractFinding::PackingMismatch { .. } => "packing",
        ContractFinding::SwizzleMismatch { .. } => "swizzle",
        ContractFinding::TailMaskMismatch => "tail_mask",
    }
}

fn tag(finding: &Finding) -> (&'static str, &'static str) {
    match finding {
        Finding::Contract { finding, .. } => ("contract", contract_tag(finding)),
        Finding::ActiveLaneMismatch { .. } => ("active_lanes", "none"),
        Finding::ExecutionLayoutMismatch { .. } => ("execution_layout", "none"),
        Finding::ConvergenceMismatch { .. } => ("convergence", "none"),
        Finding::MalformedContract { .. } => ("malformed_contract", "none"),
        Finding::DivergentInstructionTrace { .. } => ("divergent_trace", "none"),
        Finding::PartialSubgroupParticipation { .. } => ("partial_subgroup", "none"),
        Finding::DivergentSubgroupControl { .. } => ("divergent_control", "none"),
        Finding::ConvergenceAnalysisIncomplete { .. } => ("convergence_incomplete", "none"),
        Finding::Dataflow(issue) => (
            "dataflow",
            match &**issue {
                DataflowFinding::MergeConflict { .. } => "merge_conflict",
                DataflowFinding::ConsumerMismatch { .. } => "consumer_mismatch",
            },
        ),
        Finding::ResourceLimitExceeded => ("resource_limit", "none"),
    }
}

fn coordinates(finding: &Finding) -> [Option<usize>; 3] {
    match finding {
        Finding::Contract {
            block, operation, ..
        }
        | Finding::ActiveLaneMismatch {
            block, operation, ..
        }
        | Finding::ExecutionLayoutMismatch {
            block, operation, ..
        }
        | Finding::ConvergenceMismatch {
            block, operation, ..
        }
        | Finding::MalformedContract { block, operation } => [Some(*block), Some(*operation), None],
        Finding::DivergentSubgroupControl {
            block,
            operation,
            controller,
        } => [Some(*block), Some(*operation), Some(*controller)],
        // Other findings have no single site triple. Their potentially large
        // collections are never walked or formatted by this diagnostic.
        Finding::DivergentInstructionTrace { .. }
        | Finding::PartialSubgroupParticipation { .. }
        | Finding::ConvergenceAnalysisIncomplete { .. }
        | Finding::Dataflow(_)
        | Finding::ResourceLimitExceeded => [None; 3],
    }
}

fn detail(finding: &Finding) -> &[u8] {
    match finding {
        Finding::ConvergenceAnalysisIncomplete { detail } => detail.as_bytes(),
        Finding::Contract {
            finding: ContractFinding::ProfileMismatch { field },
            ..
        } => field.as_bytes(),
        _ => &[],
    }
}

// Hex encodes at most 160 ORIGINAL bytes, including a potentially split UTF-8
// code point. No source/error string can inject another log line or require a
// whole-string clone, Unicode scan, Debug, Display or dynamic output buffer.
fn hex_prefix<'a>(input: &[u8], out: &'a mut [u8; 2 * MAX_DETAIL_BYTES]) -> &'a str {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let prefix = &input[..input.len().min(MAX_DETAIL_BYTES)];
    for (index, byte) in prefix.iter().copied().enumerate() {
        out[2 * index] = DIGITS[(byte >> 4) as usize];
        out[2 * index + 1] = DIGITS[(byte & 15) as usize];
    }
    std::str::from_utf8(&out[..2 * prefix.len()]).expect("fixed ASCII hex")
}

fn tensor_findings(error: &Error) -> Option<&[Finding]> {
    let Error::Compile { error, .. } = error else {
        return None;
    };
    match &**error {
        ProductionRankedCompileErrorV1::Session(ProductionSessionErrorV1::RankedTensorLayout(
            error,
        )) => Some(error.report().findings()),
        _ => None,
    }
}

pub(super) fn report(error: &Error) {
    let Some(findings) = tensor_findings(error) else {
        return;
    };
    let shown = selected(findings);
    eprintln!(
        "fe2o3-nominal-ranked-tensor-findings-v1 total={} shown={} omitted={} accepted=false",
        findings.len(),
        shown.len(),
        findings.len() - shown.len()
    );
    for (index, finding) in shown.iter().enumerate() {
        let (kind, subkind) = tag(finding);
        let [block, operation, controller] = coordinates(finding);
        let raw = detail(finding);
        let mut encoded = [0u8; 2 * MAX_DETAIL_BYTES];
        let hex = hex_prefix(raw, &mut encoded);
        eprintln!(
            "fe2o3-nominal-ranked-tensor-finding-v1 index={} kind={} subkind={} block={:?} operation={:?} controller={:?} detail_bytes={} detail_shown={} detail_truncated={} detail_hex={} accepted=false",
            index,
            kind,
            subkind,
            block,
            operation,
            controller,
            raw.len(),
            raw.len().min(MAX_DETAIL_BYTES),
            raw.len() > MAX_DETAIL_BYTES,
            if hex.is_empty() { "-" } else { hex }
        );
    }
}

#[test]
fn tensor_diagnostic_fixed_categories_do_not_format_owned_payloads() {
    let rows = [
        (
            Finding::ActiveLaneMismatch {
                block: 2,
                operation: 3,
                expected: 64,
                actual: 32,
            },
            "active_lanes",
        ),
        (
            Finding::ExecutionLayoutMismatch {
                block: 2,
                operation: 3,
                declared: 32,
                required: 64,
            },
            "execution_layout",
        ),
        (
            Finding::ConvergenceMismatch {
                block: 2,
                operation: 3,
                actual: TensorConvergenceAttr::Opaque,
            },
            "convergence",
        ),
        (
            Finding::MalformedContract {
                block: 2,
                operation: 3,
            },
            "malformed_contract",
        ),
        (
            Finding::DivergentInstructionTrace {
                first_invocation: vec![1],
                first_trace: vec![(2, 3)],
                second_invocation: vec![4],
                second_trace: vec![(5, 6)],
            },
            "divergent_trace",
        ),
        (
            Finding::PartialSubgroupParticipation {
                grid: 1,
                workgroup: 2,
                subgroup: 3,
                expected: 64,
                actual: 32,
            },
            "partial_subgroup",
        ),
        (
            Finding::DivergentSubgroupControl {
                block: 2,
                operation: 3,
                controller: 4,
            },
            "divergent_control",
        ),
        (
            Finding::ConvergenceAnalysisIncomplete {
                detail: "unresolved branch".into(),
            },
            "convergence_incomplete",
        ),
        (Finding::ResourceLimitExceeded, "resource_limit"),
    ];
    for (finding, expected) in rows {
        assert_eq!(tag(&finding), (expected, "none"));
    }
}

#[test]
fn tensor_diagnostic_contract_tags_are_closed() {
    use fe2o3_kernel_ir::TensorOperandRoleV1 as Role;
    let rows = [
        (ContractFinding::UnsupportedProfile, "unsupported_profile"),
        (
            ContractFinding::ProfileMismatch {
                field: "never formatted\n",
            },
            "profile_mismatch",
        ),
        (
            ContractFinding::RoleMismatch {
                position: Role::A,
                actual: Role::B,
            },
            "role_mismatch",
        ),
        (
            ContractFinding::ShapeOrElementMismatch { role: Role::A },
            "shape_or_element",
        ),
        (
            ContractFinding::FragmentWidthMismatch {
                role: Role::A,
                actual: 7,
            },
            "fragment_width",
        ),
        (
            ContractFinding::UnsupportedSymbolicMap { role: Role::A },
            "unsupported_map",
        ),
        (
            ContractFinding::MalformedSymbolicMap { role: Role::A },
            "malformed_map",
        ),
        (
            ContractFinding::SymbolicMapMismatch { role: Role::A },
            "map_mismatch",
        ),
        (
            ContractFinding::CoordinateOutOfBounds { role: Role::A },
            "coordinate_bounds",
        ),
        (
            ContractFinding::DuplicateCoordinate { role: Role::A },
            "duplicate_coordinate",
        ),
        (
            ContractFinding::IncompleteCoverage { role: Role::A },
            "incomplete_coverage",
        ),
        (
            ContractFinding::BroadcastContractMismatch { role: Role::A },
            "broadcast",
        ),
        (
            ContractFinding::PackingMismatch { role: Role::A },
            "packing",
        ),
        (
            ContractFinding::SwizzleMismatch { role: Role::A },
            "swizzle",
        ),
        (ContractFinding::TailMaskMismatch, "tail_mask"),
    ];
    for (finding, expected) in rows {
        let finding = Finding::Contract {
            block: 2,
            operation: 3,
            finding,
        };
        assert_eq!(tag(&finding), ("contract", expected));
        assert_eq!(coordinates(&finding), [Some(2), Some(3), None]);
    }
}

#[test]
fn tensor_diagnostic_coordinates_preserve_exact_machine_integers() {
    let finding = Finding::DivergentSubgroupControl {
        block: usize::MAX,
        operation: usize::MAX - 1,
        controller: usize::MAX - 2,
    };
    assert_eq!(
        coordinates(&finding),
        [Some(usize::MAX), Some(usize::MAX - 1), Some(usize::MAX - 2)]
    );
    assert_eq!(coordinates(&Finding::ResourceLimitExceeded), [None; 3]);
}

#[test]
fn tensor_diagnostic_empty_detail_has_no_payload() {
    let mut out = [0u8; 2 * MAX_DETAIL_BYTES];
    assert_eq!(hex_prefix(&[], &mut out), "");
    assert!(detail(&Finding::ResourceLimitExceeded).is_empty());
}

#[test]
fn tensor_diagnostic_raw_bytes_cannot_inject_lines_or_split_utf8_output() {
    let mut out = [0u8; 2 * MAX_DETAIL_BYTES];
    assert_eq!(hex_prefix(b"A\n\0\xc3\xa9", &mut out), "410a00c3a9");
    let mut input = [b'a'; MAX_DETAIL_BYTES + 1];
    input[MAX_DETAIL_BYTES - 1] = 0xc3;
    input[MAX_DETAIL_BYTES] = 0xa9;
    let text = hex_prefix(&input, &mut out);
    assert_eq!(text.len(), 2 * MAX_DETAIL_BYTES);
    assert!(text.ends_with("c3"));
    assert!(text.bytes().all(|b| b.is_ascii_hexdigit()));
}

#[test]
fn tensor_diagnostic_exact_and_over_limit_details_are_distinct() {
    let mut out = [0u8; 2 * MAX_DETAIL_BYTES];
    let exact = [b'x'; MAX_DETAIL_BYTES];
    assert_eq!(hex_prefix(&exact, &mut out).len(), 2 * MAX_DETAIL_BYTES);
    let over = [b'x'; MAX_DETAIL_BYTES + 1];
    assert_eq!(
        hex_prefix(&exact, &mut out),
        hex_prefix(&over, &mut [0; 2 * MAX_DETAIL_BYTES])
    );
    assert!(exact.len() <= MAX_DETAIL_BYTES);
    assert!(over.len() > MAX_DETAIL_BYTES);
}

#[test]
fn tensor_diagnostic_selection_keeps_original_first_eight_without_roster_clone() {
    let findings = vec![Finding::ResourceLimitExceeded; 257];
    assert_eq!(selected(&findings).len(), MAX_FINDINGS);
    assert!(std::ptr::eq(
        selected(&findings).as_ptr(),
        findings.as_ptr()
    ));
    assert!(selected(&[]).is_empty());
    assert_eq!(selected(&findings[..MAX_FINDINGS]).len(), MAX_FINDINGS);
}

#[test]
fn tensor_diagnostic_unrelated_failure_is_not_tensor_or_expected_acceptance() {
    let error = Error::Incomplete("unchanged original error");
    assert!(tensor_findings(&error).is_none());
    assert!(classify(Case::Positive, Err(error)).is_err());
    let error = Error::Incomplete("not an analysis denial");
    assert!(tensor_findings(&error).is_none());
    assert!(classify(Case::ZeroAnalysis, Err(error)).is_err());
}

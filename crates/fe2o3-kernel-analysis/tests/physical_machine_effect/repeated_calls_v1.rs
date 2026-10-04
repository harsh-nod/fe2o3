//! Inert wire/graph controls, not LLVM decoding or authenticated native evidence.
use super::*;
use sha2::{Digest, Sha256};

#[derive(Clone)]
struct Row {
    function: usize,
    offset: u64,
    ordinal: u32,
    target: u64,
    opcode: &'static str,
    branch: u8,
}

struct Fixture {
    request: PhysicalMachineEffectRequestV1,
    names: Vec<String>,
    ranges: Vec<(u64, u64)>,
    adjacency: Vec<Vec<usize>>,
    rows: Vec<Row>,
}

impl Fixture {
    fn new(calls: Vec<Vec<usize>>, call_budget: u32) -> Self {
        let names = (0..calls.len())
            .map(|i| format!("f{i:02}"))
            .collect::<Vec<_>>();
        let mut end = 4u64;
        let ranges = calls
            .iter()
            .map(|targets| {
                let size = (targets.len() as u64 + 1) * 4;
                let range = (end, size);
                end += size;
                range
            })
            .collect::<Vec<_>>();
        let payload = vec![0x81; end as usize];
        let request = request_with(
            &payload,
            vec![entry(
                "f00",
                PhysicalMachineEffectBudgetV1::new(0, 0, 0, 64, call_budget),
            )],
        );
        let mut rows = Vec::new();
        let mut adjacency = Vec::new();
        for (function, targets) in calls.iter().enumerate() {
            let mut edges = targets.clone();
            edges.sort_unstable();
            edges.dedup();
            adjacency.push(edges);
            for (ordinal, target) in targets.iter().enumerate() {
                rows.push(Row {
                    function,
                    offset: ranges[function].0 + ordinal as u64 * 4,
                    ordinal: ordinal as u32,
                    target: ranges[*target].0,
                    opcode: "S_SWAPPC_B64_vi",
                    branch: 3,
                });
            }
            rows.push(Row {
                function,
                offset: ranges[function].0 + targets.len() as u64 * 4,
                ordinal: targets.len() as u32,
                target: 0,
                opcode: "S_SETPC_B64_vi",
                branch: 4,
            });
        }
        Self {
            request,
            names,
            ranges,
            adjacency,
            rows,
        }
    }

    fn effect_bytes(&self) -> Vec<u8> {
        let functions = self
            .names
            .iter()
            .enumerate()
            .map(|(index, name)| Function {
                symbol: name,
                offset: self.ranges[index].0,
                size: self.ranges[index].1,
                callees: self.adjacency[index]
                    .iter()
                    .map(|target| self.names[*target].as_str())
                    .collect(),
            })
            .collect::<Vec<_>>();
        let effects = self
            .names
            .iter()
            .enumerate()
            .map(|(i, name)| Effect {
                entry: "f00",
                function: name,
                offset: self.ranges[i].0 + self.ranges[i].1 - 4,
                kind: 4,
                width: 0,
            })
            .collect::<Vec<_>>();
        evidence_with_entry_range(
            &self.request,
            self.ranges[0].0,
            self.ranges[0].1,
            &functions,
            &effects,
        )
    }

    fn trace_bytes(&self, effect_bytes: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(PHYSICAL_MACHINE_TRACE_EVIDENCE_DOMAIN_V1);
        push_u32(&mut out, 0);
        push_u16(&mut out, PHYSICAL_MACHINE_TRACE_SCHEMA_VERSION_V1);
        out.extend_from_slice(&self.request.execution_challenge().as_bytes());
        out.extend_from_slice(&self.request.identity().sha256());
        push_u64(&mut out, self.request.identity().byte_len());
        let mut hash = Sha256::new();
        hash.update(b"FE2O3/GFX942-PHYSICAL-MACHINE-EFFECT-EVIDENCE-IDENTITY/V1\0");
        hash.update(effect_bytes);
        out.extend_from_slice(&hash.finalize());
        push_u64(&mut out, effect_bytes.len() as u64);
        out.extend_from_slice(&self.request.payload_identity().sha256());
        push_u64(&mut out, self.request.payload_identity().byte_len());
        out.extend_from_slice(&self.request.analyzer_identity().as_bytes());
        out.extend_from_slice(&self.request.toolchain_identity().as_bytes());
        push_u16(&mut out, 1);
        push_u32(&mut out, self.rows.len() as u32);
        for row in &self.rows {
            push_text(&mut out, &self.names[row.function]);
            push_u32(&mut out, row.ordinal);
            push_u64(&mut out, row.offset);
            push_u32(&mut out, 1);
            if row.branch == 4 {
                push_u16(&mut out, 0);
            } else {
                push_u16(&mut out, 1);
                push_u32(&mut out, row.ordinal + 1);
            }
        }
        push_u32(&mut out, self.rows.len() as u32);
        for row in &self.rows {
            push_text(&mut out, &self.names[row.function]);
            push_u64(&mut out, row.offset);
            push_u32(&mut out, row.ordinal);
            push_text(&mut out, row.opcode);
            push_u16(&mut out, 4);
            out.extend_from_slice(&[0x81; 4]);
            push_u16(&mut out, 0); // no fabricated operand facts
            push_u16(&mut out, 0);
            push_u16(&mut out, 0);
            push_u16(&mut out, 0);
            out.push(row.branch);
            push_u64(&mut out, row.target);
            push_u16(&mut out, if row.branch == 4 { 4 } else { 0 });
            out.push(0);
            push_u16(&mut out, 0);
        }
        let length = out.len() as u32;
        let position = PHYSICAL_MACHINE_TRACE_EVIDENCE_DOMAIN_V1.len();
        out[position..position + 4].copy_from_slice(&length.to_le_bytes());
        out
    }

    fn decode(
        &self,
    ) -> Result<PhysicalMachineAnalysisEvidenceV1, PhysicalMachineAnalysisEvidenceErrorV1> {
        let effects = self.effect_bytes();
        PhysicalMachineAnalysisEvidenceV1::decode_canonical_for(
            &self.request,
            &encode_analysis_bundle(&effects, &self.trace_bytes(&effects)),
        )
    }
}

#[test]
fn repeated_sites_preserve_multiplicity_identity_and_unique_adjacency() {
    let fixture = Fixture::new(vec![vec![1, 1], vec![]], 2);
    let analysis = fixture.decode().unwrap();
    assert_eq!(analysis.effects().functions()[0].direct_callees(), &["f01"]);
    let calls = analysis
        .trace()
        .instructions()
        .iter()
        .filter(|row| {
            row.branch_kind() == fe2o3_kernel_analysis::PhysicalMachineBranchKindV1::DirectCall
        })
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 2);
    assert_ne!(calls[0].instruction_offset(), calls[1].instruction_offset());
    assert_eq!(calls[0].branch_target(), calls[1].branch_target());
    assert_eq!(analysis.effects().effects().len(), 2); // one static return per function
    assert!(!analysis.grants_launch_authority());
}

#[test]
fn actual_two_sites_do_not_fit_a_single_edge_budget() {
    assert!(Fixture::new(vec![vec![1, 1], vec![]], 2).decode().is_ok());
    assert_eq!(
        Fixture::new(vec![vec![1, 1], vec![]], 1).decode(),
        Err(PhysicalMachineAnalysisEvidenceErrorV1::Trace(
            PhysicalMachineTraceEvidenceErrorV1::DirectCallBudget
        ))
    );
}

#[test]
fn standalone_calls_require_the_complete_trace_but_zero_calls_stay_unchanged() {
    for count in [1, 2] {
        let fixture = Fixture::new(vec![vec![1; count], vec![]], 2);
        assert!(fixture.decode().is_ok());
        assert_eq!(
            PhysicalMachineEffectEvidenceV1::decode_canonical_for(
                &fixture.request,
                &fixture.effect_bytes()
            ),
            Err(PhysicalMachineEffectEvidenceErrorV1::TraceRequiredForDirectCalls)
        );
    }
    let fixture = Fixture::new(vec![vec![]], 0);
    let bytes = fixture.effect_bytes();
    let standalone =
        PhysicalMachineEffectEvidenceV1::decode_canonical_for(&fixture.request, &bytes).unwrap();
    assert_eq!(standalone.canonical_bytes(), bytes);
    assert_eq!(fixture.decode().unwrap().effects(), &standalone);
}

#[test]
fn single_direct_call_and_immediate_call_spelling_still_admit() {
    let mut fixture = Fixture::new(vec![vec![1], vec![]], 1);
    assert!(fixture.decode().is_ok());
    fixture.rows[0].opcode = "S_CALL_B64_vi";
    assert!(fixture.decode().is_ok());
}

#[test]
fn global_site_cap_is_256_before_graph_deduplication() {
    assert!(
        Fixture::new(vec![vec![1; 128], vec![2; 128], vec![]], u32::MAX)
            .decode()
            .is_ok()
    );
    assert_eq!(
        Fixture::new(vec![vec![1; 128], vec![2; 129], vec![]], u32::MAX).decode(),
        Err(PhysicalMachineAnalysisEvidenceErrorV1::Trace(
            PhysicalMachineTraceEvidenceErrorV1::DirectCallSiteCount
        ))
    );
    assert!(
        Fixture::new(vec![vec![1; 256], vec![]], u32::MAX)
            .decode()
            .is_ok()
    );
    assert_eq!(
        Fixture::new(vec![vec![1; 257], vec![]], u32::MAX).decode(),
        Err(PhysicalMachineAnalysisEvidenceErrorV1::Trace(
            PhysicalMachineTraceEvidenceErrorV1::DirectCallSiteCount
        ))
    );
}

#[test]
fn nested_shared_helper_sites_are_static_not_dynamic_expansion() {
    let calls = vec![vec![1, 1], vec![2, 2], vec![]];
    let analysis = Fixture::new(calls.clone(), 4).decode().unwrap();
    assert_eq!(analysis.effects().effects().len(), 3);
    assert_eq!(
        Fixture::new(calls, 3).decode(),
        Err(PhysicalMachineAnalysisEvidenceErrorV1::Trace(
            PhysicalMachineTraceEvidenceErrorV1::DirectCallBudget
        ))
    );
}

#[test]
fn self_and_mutual_cycles_are_rejected_even_with_complete_trace() {
    for calls in [vec![vec![0]], vec![vec![1], vec![0]]] {
        let fixture = Fixture::new(calls, 64);
        assert_eq!(
            fixture.decode(),
            Err(PhysicalMachineAnalysisEvidenceErrorV1::Effects(
                PhysicalMachineEffectEvidenceErrorV1::CyclicCallGraph
            ))
        );
    }
}

#[test]
fn longest_bounded_acyclic_chain_admits_and_next_function_refuses() {
    for count in [64, 65] {
        let calls = (0..count)
            .map(|i| if i + 1 < count { vec![i + 1] } else { vec![] })
            .collect();
        let result = Fixture::new(calls, 64).decode();
        if count == 64 {
            assert!(result.is_ok());
        } else {
            assert_eq!(
                result,
                Err(PhysicalMachineAnalysisEvidenceErrorV1::Effects(
                    PhysicalMachineEffectEvidenceErrorV1::FunctionCount
                ))
            );
        }
    }
}

#[test]
fn hidden_call_opcode_and_foreign_target_refuse_without_erasing_another_site() {
    let fixture = Fixture::new(vec![vec![1, 1], vec![]], 2);
    assert!(fixture.decode().is_ok());
    for mutation in 0..4 {
        let mut changed = Fixture::new(vec![vec![1, 1], vec![]], 2);
        match mutation {
            0 => {
                changed.rows[1].branch = 0;
                changed.rows[1].target = 0;
            }
            1 => {
                changed.rows[1].opcode = "S_NOP_vi";
            }
            2 => {
                changed.rows[1].target += 4;
            }
            _ => {
                changed.rows[1].target = u64::MAX;
            }
        }
        assert_eq!(
            changed.decode(),
            Err(PhysicalMachineAnalysisEvidenceErrorV1::Trace(
                PhysicalMachineTraceEvidenceErrorV1::InvalidDirectCall
            )),
            "mutation {mutation}"
        );
    }
}

#[test]
fn missing_duplicate_and_overflowed_instruction_ranges_refuse() {
    let baseline = Fixture::new(vec![vec![1, 1], vec![]], 2);
    assert!(baseline.decode().is_ok());
    for mutation in 0..3 {
        let mut changed = Fixture::new(vec![vec![1, 1], vec![]], 2);
        match mutation {
            0 => {
                changed.rows.remove(1);
            }
            1 => {
                changed.rows.insert(1, changed.rows[0].clone());
            }
            _ => {
                changed.rows[1].offset = u64::MAX;
            }
        }
        assert!(matches!(
            changed.decode(),
            Err(PhysicalMachineAnalysisEvidenceErrorV1::Trace(_))
        ));
    }
}

#[test]
fn missing_extra_and_duplicate_adjacency_cannot_replace_actual_sites() {
    let calls = vec![vec![1, 2], vec![2], vec![]];
    assert!(Fixture::new(calls.clone(), 3).decode().is_ok());
    let mut missing = Fixture::new(calls.clone(), 3);
    missing.adjacency[0] = vec![1]; // f02 still genuinely reachable via f01
    assert_eq!(
        missing.decode(),
        Err(PhysicalMachineAnalysisEvidenceErrorV1::Trace(
            PhysicalMachineTraceEvidenceErrorV1::InvalidDirectCall
        ))
    );
    let mut duplicate = Fixture::new(calls, 3);
    duplicate.adjacency[0] = vec![1, 1, 2];
    assert_eq!(
        duplicate.decode(),
        Err(PhysicalMachineAnalysisEvidenceErrorV1::Effects(
            PhysicalMachineEffectEvidenceErrorV1::NonCanonicalOrder
        ))
    );
    let mut extra = Fixture::new(vec![vec![1], vec![2], vec![]], 3);
    extra.adjacency[0].push(2);
    assert_eq!(
        extra.decode(),
        Err(PhysicalMachineAnalysisEvidenceErrorV1::Trace(
            PhysicalMachineTraceEvidenceErrorV1::InvalidDirectCall
        ))
    );
}

#[test]
fn unchanged_request_identity_binding_precedes_call_admission() {
    let first = Fixture::new(vec![vec![1, 1], vec![]], 2);
    let second = Fixture::new(vec![vec![1, 1], vec![]], 1);
    let effects = first.effect_bytes();
    let trace = first.trace_bytes(&effects);
    assert_eq!(
        PhysicalMachineAnalysisEvidenceV1::decode_canonical_for(
            &second.request,
            &encode_analysis_bundle(&effects, &trace)
        ),
        Err(PhysicalMachineAnalysisEvidenceErrorV1::Effects(
            PhysicalMachineEffectEvidenceErrorV1::RequestIdentityMismatch
        ))
    );
}

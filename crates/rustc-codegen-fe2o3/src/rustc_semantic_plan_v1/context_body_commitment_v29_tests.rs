//! Regression coverage for the unreleased optional V5 context child extension.
//! Ordinary source carriers exercise commitments only, never capability issuance.

use super::*;
use crate::rustc_semantic_adapter_v1::canonical_target_layout_v1;
use crate::test_temp_dir::TestTempDir;
use fe2o3_mir_model::semantic_mir_v1::HARD_MAX_CANONICAL_BYTES_V1;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::Compiler;
use sha2::{Digest, Sha256};

fn transcript_fields_v5(mut bytes: &[u8]) -> Vec<&[u8]> {
    let mut fields = Vec::new();
    while !bytes.is_empty() {
        let length = u64::from_le_bytes(bytes[..8].try_into().unwrap()) as usize;
        fields.push(&bytes[8..8 + length]);
        bytes = &bytes[8 + length..];
    }
    fields
}

fn context_fields(entry: &BoundContextEntryV29<'_>) -> Vec<Vec<u8>> {
    let mut fields = Vec::new();
    entry
        .commitment(|field| {
            fields.push(field.to_vec());
            Ok::<_, ()>(())
        })
        .unwrap();
    fields
}

fn expected_context_fields(function: SemanticFunctionIdV1) -> Vec<Vec<u8>> {
    let mut fields = vec![
        b"fe2o3/semantic-mir/context-entry/v29".to_vec(),
        vec![9; 32],
        function.index().to_le_bytes().to_vec(),
        7_u32.to_le_bytes().to_vec(),
    ];
    for (coordinates, unwind) in [
        ([11_u64, 12, 13, 14], 0),
        ([21, 22, 23, 24], 1),
        ([31, 32, 33, 34], 0),
        ([41, 42, 43, 44], 1),
    ] {
        fields.extend(coordinates.map(|value| value.to_le_bytes().to_vec()));
        fields.push(vec![unwind]);
    }
    fields.extend([51_u32, 52, 53, 61, 62, 63].map(|value| value.to_le_bytes().to_vec()));
    fields.push(vec![0]);
    fields.push(33_u64.to_le_bytes().to_vec());
    fields.push(3_u64.to_le_bytes().to_vec());
    fields
}

fn framed_sha256(fields: impl IntoIterator<Item = impl AsRef<[u8]>>) -> [u8; 32] {
    let mut hash = Sha256::new();
    for field in fields {
        let field = field.as_ref();
        hash.update((field.len() as u64).to_le_bytes());
        hash.update(field);
    }
    hash.finalize().into()
}

fn ordinary_body_fields(body: &RetainedSemanticBodyProducerV1) -> Vec<Vec<u8>> {
    // Freeze the ordinary-body payload separately from the production assembly
    // so adding an absent-child marker or changing append order is observable.
    let mut fields = vec![body.function.index().to_le_bytes().to_vec()];
    let source = |fields: &mut Vec<Vec<u8>>, value: RetainedSemanticSourceProducerV1| {
        fields.push(value.expansion_chain_sha256.to_vec());
        for origin in [value.provenance.expansion(), value.provenance.call_site()] {
            let Some(origin) = origin else {
                fields.push(vec![0]);
                continue;
            };
            fields.push(vec![1]);
            fields.push(origin.file().as_bytes().to_vec());
            let (start, end) = origin.byte_range();
            fields.push(start.to_le_bytes().to_vec());
            fields.push(end.to_le_bytes().to_vec());
            let (line, column) = origin.start_coordinate();
            fields.push(line.to_le_bytes().to_vec());
            fields.push(column.to_le_bytes().to_vec());
            let (line, column) = origin.end_coordinate();
            fields.push(line.to_le_bytes().to_vec());
            fields.push(column.to_le_bytes().to_vec());
        }
    };
    source(&mut fields, body.source);
    for local in &body.locals {
        fields.push(local.identity.as_bytes().to_vec());
        fields.push(local.rustc_local.to_le_bytes().to_vec());
        fields.push(local.ty.index().to_le_bytes().to_vec());
        source(&mut fields, local.source);
    }
    fields.extend(
        body.raw_to_semantic_locals
            .iter()
            .map(|local| local.index().to_le_bytes().to_vec()),
    );
    fields.push(body.entry.index().to_le_bytes().to_vec());
    for block in &body.blocks {
        fields.push(block.identity.as_bytes().to_vec());
        fields.push(block.rustc_block.to_le_bytes().to_vec());
        source(&mut fields, block.source);
        for statement in &block.statements {
            source(&mut fields, *statement);
        }
        source(&mut fields, block.terminator);
    }
    fields.extend(
        body.raw_to_semantic_blocks
            .iter()
            .map(|block| block.index().to_le_bytes().to_vec()),
    );
    fields
}

fn expected_child(
    ordinal: u64,
    function: SemanticFunctionIdV1,
    fields: &[Vec<u8>],
) -> PreflightChildCommitmentV5 {
    let header = [
        b"fe2o3/semantic-mir/rustc-preflight-plan/v5/body".to_vec(),
        vec![5],
        ordinal.to_le_bytes().to_vec(),
        function.index().to_le_bytes().to_vec(),
    ];
    PreflightChildCommitmentV5 {
        field_count: fields.len() as u64,
        payload_framed_bytes: fields.iter().map(|field| 8 + field.len() as u64).sum(),
        sha256: framed_sha256(header.iter().chain(fields)),
    }
}

fn body_rows(transcript: &[u8], bodies: usize) -> Vec<Vec<Vec<u8>>> {
    let fields = transcript_fields_v5(transcript);
    // Domains, target, inventory, 12 cardinalities, 11 resource counts, then
    // five non-body child records, each with five fields.
    let start = 4 + 12 + 11 + 5 * 5;
    assert_eq!(fields[start], [5]);
    assert_eq!(fields[start + 1], (bodies as u64).to_le_bytes());
    fields[start + 2..start + 2 + bodies * 12]
        .chunks_exact(12)
        .map(|row| row.iter().map(|field| field.to_vec()).collect())
        .collect()
}

#[derive(Default)]
struct ContextCommitmentCallbacks {
    completed: bool,
}

impl Callbacks for ContextCommitmentCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        let mut functions = tcx
            .hir_body_owners()
            .filter(|id| tcx.def_kind(id.to_def_id()) == rustc_hir::def::DefKind::Fn)
            .map(|id| {
                let instance = Instance::mono(tcx, id.to_def_id());
                RetainedSemanticFunctionProducerV1 {
                    identities: canonical_function_identities_v1(tcx, instance),
                    instance,
                    role: CollectedFunctionRole::InternalHelper,
                    export_name: None,
                    kernel_binding: None,
                    generated_host_contract_identity: None,
                    frontend_contract: None,
                }
            })
            .collect::<Vec<_>>();
        functions.sort_by_key(|row| row.identities.function());
        let target = canonical_target_layout_v1(
            &crate::semantic_layout_bridge::rustc_semantic_layout_target_v1(tcx).unwrap(),
        );
        let root_index = functions
            .iter()
            .position(|row| tcx.item_name(row.instance.def_id()).as_str() == "root")
            .unwrap();
        let function = SemanticFunctionIdV1::from_index(root_index as u32);
        let root = functions[root_index].instance;
        let plan = build_production_semantic_preflight_plan_v1(
            tcx,
            target,
            functions.into_boxed_slice(),
            vec![function].into_boxed_slice(),
            [7; 32],
            DebugSourceCaptureRequestV2::Disabled,
            None,
        )
        .unwrap();
        assert_eq!(plan.bodies.len(), 2);
        let root_ordinal = plan
            .bodies
            .iter()
            .position(|body| body.function == function)
            .unwrap();
        let ordinary_ordinal = 1 - root_ordinal;
        let ordinary_function = plan.bodies[ordinary_ordinal].function;
        assert_eq!(
            tcx.item_name(
                plan.functions[ordinary_function.index() as usize]
                    .instance
                    .def_id()
            )
            .as_str(),
            "ordinary"
        );
        // Nine ordinary fields precede the three commitment fields in each row.
        let child_start = 4 + 12 + 11 + 5 * 5 + 2 + root_ordinal * 12 + 9;
        let fixture = || BoundContextEntryV29::commitment_test_fixture_v29(tcx, root, function);
        let expected = expected_context_fields(function);
        assert_eq!(expected.len(), 33);
        assert_eq!(context_fields(&fixture()), expected);

        let source_files = plan
            .bodies
            .iter()
            .flat_map(|body| {
                std::iter::once(body.source)
                    .chain(body.locals.iter().map(|local| local.source))
                    .chain(body.blocks.iter().flat_map(|block| {
                        [block.source, block.terminator]
                            .into_iter()
                            .chain(block.statements.iter().copied())
                    }))
            })
            .flat_map(|source| [source.provenance.expansion(), source.provenance.call_site()])
            .flatten()
            .map(|origin| origin.file())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let original_fields = transcript_fields_v5(&plan.canonical_transcript);
        let raw_counts: [u64; 11] = std::array::from_fn(|index| {
            u64::from_le_bytes(original_fields[16 + index].try_into().unwrap())
        });
        let counts = RawMirPreflightCountsV1 {
            types: raw_counts[0],
            functions: raw_counts[1],
            roots: raw_counts[2],
            locals: raw_counts[3],
            blocks: raw_counts[4],
            statements: raw_counts[5],
            projections: raw_counts[6],
            operands: raw_counts[7],
            call_arguments: raw_counts[8],
            switch_targets: raw_counts[9],
            validation_work: raw_counts[10],
        };
        let [call] = plan.direct_calls.as_ref() else {
            panic!("fixture must retain exactly one root-to-ordinary call")
        };
        assert_eq!(call.caller, function);
        assert_eq!(call.callee, ordinary_function);
        let edges = BTreeSet::from([CallEdgeV1 {
            caller: function,
            callee: ordinary_function,
        }]);
        assert!(plan.terminals.is_empty());
        let assemble = |entries: &BTreeMap<_, _>| {
            preflight_plan_identity_and_transcript_v1(
                target,
                [7; 32],
                &plan.types,
                &source_files,
                &plan.functions,
                &plan.function_abis,
                &plan.terminals,
                &plan.bodies,
                &plan.roots,
                &edges,
                &plan.direct_calls,
                &plan.terminal_expansions,
                &plan.normalized_intrinsics,
                entries,
                counts,
                tcx,
            )
            .unwrap()
        };
        let ordinary = assemble(&BTreeMap::new());
        assert_eq!(ordinary.0, plan.sha256);
        assert_eq!(ordinary.1, plan.canonical_transcript);
        let bound = assemble(&BTreeMap::from([(function, fixture())]));
        assert_ne!(ordinary.0, bound.0);
        assert_eq!(ordinary.0, <[u8; 32]>::from(Sha256::digest(&ordinary.1)));
        assert_eq!(bound.0, <[u8; 32]>::from(Sha256::digest(&bound.1)));
        let ordinary_rows = body_rows(&ordinary.1, 2);
        let bound_rows = body_rows(&bound.1, 2);
        assert_eq!(
            ordinary_rows[ordinary_ordinal], bound_rows[ordinary_ordinal],
            "ordinary body gained context fields"
        );
        assert_eq!(
            &ordinary_rows[root_ordinal][..9],
            &bound_rows[root_ordinal][..9]
        );
        for (ordinal, body) in plan.bodies.iter().enumerate() {
            let mut fields = ordinary_body_fields(body);
            let ordinary_child = expected_child(ordinal as u64, body.function, &fields);
            assert_eq!(
                ordinary_rows[ordinal][9],
                ordinary_child.field_count.to_le_bytes()
            );
            assert_eq!(
                ordinary_rows[ordinal][10],
                ordinary_child.payload_framed_bytes.to_le_bytes()
            );
            assert_eq!(ordinary_rows[ordinal][11], ordinary_child.sha256);
            if body.function == function {
                fields.extend(expected.clone());
            }
            let bound_child = expected_child(ordinal as u64, body.function, &fields);
            assert_eq!(
                bound_rows[ordinal][9],
                bound_child.field_count.to_le_bytes()
            );
            assert_eq!(
                bound_rows[ordinal][10],
                bound_child.payload_framed_bytes.to_le_bytes()
            );
            assert_eq!(bound_rows[ordinal][11], bound_child.sha256);
        }
        let ordinary_fields = transcript_fields_v5(&ordinary.1);
        let bound_fields = transcript_fields_v5(&bound.1);
        let differences = ordinary_fields
            .iter()
            .zip(&bound_fields)
            .enumerate()
            .filter_map(|(index, (left, right))| (left != right).then_some(index))
            .collect::<Vec<_>>();
        assert_eq!(ordinary_fields.len(), bound_fields.len());
        assert_eq!(
            differences,
            [child_start, child_start + 1, child_start + 2],
            "only the issued body's child commitment changes"
        );

        for field in 1..33 {
            let mut changed = fixture();
            changed.mutate_commitment_test_field_v29(field);
            let fields = context_fields(&changed);
            let differences = fields
                .iter()
                .zip(&expected)
                .enumerate()
                .filter_map(|(index, (left, right))| (left != right).then_some(index))
                .collect::<Vec<_>>();
            assert_eq!(differences, [field], "mutation field {field}");
            let mutated = assemble(&BTreeMap::from([(function, changed)]));
            let rows = body_rows(&mutated.1, 2);
            assert_ne!(
                rows[root_ordinal][11], bound_rows[root_ordinal][11],
                "unbound context field {field}"
            );
            assert_eq!(rows[ordinary_ordinal], bound_rows[ordinary_ordinal]);
            assert_ne!(mutated.0, bound.0, "parent missed context field {field}");
            let mutated_fields = transcript_fields_v5(&mutated.1);
            let differences = mutated_fields
                .iter()
                .zip(&bound_fields)
                .enumerate()
                .filter_map(|(index, (left, right))| (left != right).then_some(index))
                .collect::<Vec<_>>();
            assert_eq!(mutated_fields.len(), bound_fields.len());
            assert_eq!(
                differences,
                [child_start + 2],
                "context field {field} changed another child"
            );
        }
        let mut erased = fixture();
        erased.mutate_commitment_test_field_v29(30);
        erased.mutate_commitment_test_field_v29(16);
        let erased_fields = context_fields(&erased);
        assert_eq!(erased_fields[30], [1]);
        assert_eq!(
            erased_fields[31],
            34_u64.to_le_bytes(),
            "restoration follows issuer destination"
        );

        // The field sink must stop immediately and propagate its exact error.
        for stop in 0..33 {
            let mut visited = 0;
            let result = fixture().commitment(|_| {
                let index = visited;
                visited += 1;
                if index == stop { Err(index) } else { Ok(()) }
            });
            assert_eq!(result, Err(stop));
            assert_eq!(visited, stop + 1);
        }
        let extension_bytes: u64 = expected.iter().map(|field| 8 + field.len() as u64).sum();
        let maximum = HARD_MAX_CANONICAL_BYTES_V1;
        for excess in [0, 1] {
            let mut child = PreflightChildCommitmentBuilderV5::body(0, function);
            // Set only the existing accounting counter; avoid allocating the
            // hard maximum merely to exercise the exact/one-short boundary.
            child.payload_framed_bytes = maximum - extension_bytes + excess;
            let result = fixture().commitment(|field| child.field(field));
            if excess == 0 {
                result.unwrap();
                assert_eq!(child.payload_framed_bytes, maximum);
                assert_eq!(child.field_count, 33);
            } else {
                assert!(
                    matches!(result, Err(ProductionSemanticPreflightErrorV1::CommitmentBoundExceeded {
                    scope: "body child", actual, maximum: observed,
                }) if actual == maximum + 1 && observed == maximum)
                );
                assert_eq!(child.field_count, 32);
            }
        }
        assert_eq!(
            assemble(&BTreeMap::new()),
            ordinary,
            "no retained child state leaks"
        );
        self.completed = true;
        Compilation::Stop
    }
}

#[test]
fn issued_context_v5_child_binds_exact_fields_mutations_parent_and_budget() {
    let directory = TestTempDir::create("fe2o3-context-child-commitment");
    let source = directory.path().join("fixture.rs");
    std::fs::write(
        &source,
        "pub fn root(a: u32) -> u32 { ordinary(a) }\n#[inline(never)]\nfn ordinary(b: u32) -> u32 { b }\n",
    )
    .unwrap();
    let sysroot = std::process::Command::new("rustc")
        .args(["--print", "sysroot"])
        .output()
        .unwrap();
    assert!(sysroot.status.success());
    let args = vec![
        "rustc".into(),
        "--crate-name=fe2o3_context_child_fixture".into(),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--emit=metadata".into(),
        "-Zmir-opt-level=0".into(),
        "-Cpanic=abort".into(),
        "--sysroot".into(),
        String::from_utf8(sysroot.stdout).unwrap().trim().into(),
        "-o".into(),
        directory.path().join("fixture.rmeta").display().to_string(),
        source.display().to_string(),
    ];
    let mut callbacks = ContextCommitmentCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    assert!(callbacks.completed);
}

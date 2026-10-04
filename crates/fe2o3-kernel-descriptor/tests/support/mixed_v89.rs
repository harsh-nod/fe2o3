#![allow(dead_code)]
use fe2o3_kernel_descriptor::mixed_conditional_v26::*;
use fe2o3_kernel_descriptor::mixed_conditional_v86::*;
use fe2o3_kernel_descriptor::*;
#[path = "conditional_v5.rs"]
mod fixture;
pub use fixture::free;

// Inert codec data. Source, physical ABI and proof consumers must authenticate
// these associations independently; no compiler or execution evidence is implied.
pub fn inputs(target: &str, count: usize) -> (Vec<u8>, Vec<Vec<u8>>) {
    fixture::with_input(target, count, 2, |input| {
        let mut nominal =
            vec![0; encoded_device_descriptor_table_v3_len(&input.nominal, &mut free).unwrap()];
        encode_device_descriptor_table_v3(&input.nominal, &mut nominal, &mut free).unwrap();
        let table = decode_device_descriptor_table_v3(&nominal, &mut free).unwrap();
        let descriptor = mixed_descriptor_subject_v26(&table, &mut free).unwrap();
        let rows = (0..count)
            .map(|ordinal| {
                let kernel = table.kernel(ordinal, &mut free).unwrap();
                contract(MixedContractSubjectsV26 {
                    kernel_id: *kernel.kernel_id().as_bytes(),
                    source_semantic_identity: [2; 32],
                    original_graph_identity: [3; 32],
                    output_graph_identity: [4; 32],
                    descriptor_identity: descriptor,
                    original_root: (count - 1 - ordinal) as u32,
                    output_function: (ordinal * 2 + 1) as u32,
                    source_rank: kernel.launch().rank(),
                    index_width: 64,
                    exact_grid: [32, 1, 1],
                    source_argument_count: kernel.argument_count() as u32,
                    generated_field_count: kernel.component_count() as u32,
                    explicit_argument_bytes: kernel.abi_layout().explicit_argument_size(),
                    kernarg_alignment: kernel.abi_layout().kernarg_segment_alignment(),
                })
            })
            .collect();
        (nominal, rows)
    })
}

pub fn operation(function: u32, block: u32, operation: u32) -> MixedOperationV26 {
    MixedOperationV26 {
        function,
        block,
        operation,
    }
}
pub fn definition(function: u32, ordinal: u32) -> MixedDefinitionV26 {
    MixedDefinitionV26::Result {
        operation: operation(function, 0, ordinal),
        result: 0,
    }
}
pub fn argument(source: u32, reads: u32, writes: u32) -> MixedArgumentV26 {
    MixedArgumentV26 {
        source_argument: source,
        generated_field: source as u16,
        physical_parameter: source * 2,
        semantic_type: source + 20,
        semantic_type_identity: [6; 32],
        descriptor_type_identity: [7; 32],
        device_layout_identity: [8; 32],
        scalar: MixedScalarV26::F32,
        pointer_offset: source * 16,
        length_offset: source * 16 + 8,
        reads,
        writes,
        source_exclusive: writes != 0,
    }
}
pub fn occurrence(function: u32, ordinal: u32, writing: bool) -> MixedOccurrenceV86 {
    let argument = if writing { 2 } else { 0 };
    MixedOccurrenceV86 {
        argument,
        original_instance: 0,
        original_operation: operation(10, 2, ordinal),
        output_operation: operation(function, 2, ordinal),
        original_formation: operation(10, 1, ordinal),
        output_formation: operation(function, 1, ordinal),
        output_address_index: definition(function, 2),
        output_guard: if writing {
            MixedAccessGuardV86::ExplicitPredicate {
                condition: definition(function, 3),
                bound_comparison: definition(function, 4),
            }
        } else {
            MixedAccessGuardV86::CfgEdge {
                edge: MixedEdgeV26 {
                    function,
                    block: 0,
                    successor: 0,
                },
                condition: definition(function, 4),
            }
        },
        slice_value: 19,
        pointer_value: 29,
        index_value: 39,
        guard_index_value: 49,
        length_value: 59,
        predicate_value: 69,
        path: if writing {
            MixedGuardPathV26::ExplicitPredicate
        } else {
            MixedGuardPathV26::TrueEdge {
                source: 0,
                successor: 0,
                target: 2,
            }
        },
        element_bytes: 4,
        alignment: 4,
        address_space: MixedMemorySpaceV26::Global,
        writing,
        volatile: false,
        invocation_axis: 0,
        invocation_value: 39,
        access_envelope: MixedIndexEnvelopeV26::LogicalExtent { argument },
        formation_envelope: MixedIndexEnvelopeV26::InvocationAxis { axis: 0 },
    }
}
pub fn contract(subjects: MixedContractSubjectsV26) -> Vec<u8> {
    let arguments = [argument(0, 1, 0), argument(1, 0, 0), argument(2, 0, 2)];
    let f = subjects.output_function;
    encode_contract(
        subjects,
        &arguments,
        &[
            occurrence(f, 1, false),
            occurrence(f, 2, true),
            occurrence(f, 3, true),
        ],
    )
}
pub fn encode_contract(
    subjects: MixedContractSubjectsV26,
    arguments: &[MixedArgumentV26],
    occurrences: &[MixedOccurrenceV86],
) -> Vec<u8> {
    let input = MixedContractInputV86 {
        subjects,
        arguments,
        occurrences,
    };
    let mut bytes = vec![0; encoded_mixed_contract_v86_len(&input, &mut free).unwrap()];
    encode_mixed_contract_v86(&input, &mut bytes, &mut free).unwrap();
    bytes
}
pub fn views(rows: &[Vec<u8>]) -> Vec<MixedContractV86<'_>> {
    rows.iter()
        .map(|row| decode_mixed_contract_v86(row, &mut free).unwrap())
        .collect()
}
pub fn wire(nominal: &[u8], rows: &[Vec<u8>]) -> Vec<u8> {
    let rows = views(rows);
    let mut output = vec![0; encoded_mixed_descriptor_v89_len(nominal, &rows, &mut free).unwrap()];
    encode_mixed_descriptor_v89(nominal, &rows, &mut output, &mut free).unwrap();
    output
}

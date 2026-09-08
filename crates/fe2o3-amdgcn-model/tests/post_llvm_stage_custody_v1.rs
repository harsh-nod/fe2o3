use fe2o3_amdgcn_model::{
    Gfx942ObjectToHsacoPreservationErrorV1, Gfx942PostLlvmStageEnvelopeErrorV1,
    Gfx950PostLlvmStageEnvelopeErrorV1, check_gfx942_object_to_hsaco_preservation_v1,
    check_gfx942_post_llvm_stage_envelopes_v1, check_gfx950_object_to_hsaco_preservation_v1,
    check_gfx950_post_llvm_stage_envelopes_v1,
};
use fe2o3_compiler_lineage::{
    ExactLlvmPassOccurrenceContentsV1, ExactProductionLlvmPhaseContentsV1,
    FIXED_PRODUCTION_LLVM_PHASES_V1, LlvmPassInvocationV1, LlvmPassIrUnitV1,
    PostLlvmPipelineOccurrenceTranscriptV1, PostLlvmStageCustodyV1,
    check_exact_fixed_production_pipeline_contents_v1,
    check_exact_post_llvm_pipeline_occurrence_v1, check_exact_post_llvm_stage_contents_v1,
};

const LLVM: &[u8] = b"target triple = \"amdgcn-amd-amdhsa\"";
const WORKER: &[u8] = b"exact production worker";
const ASSEMBLER: &[u8] = b"exact LLVM text assembler";
const ASSEMBLED: &[u8] = b"BC\xc0\xde-assembled-only";

fn elf(kind: u16, osabi: u8) -> Vec<u8> {
    elf_with_metadata(kind, osabi, b"\x81\xaeversion-marker\x06")
}

fn gfx950_elf(kind: u16, osabi: u8) -> Vec<u8> {
    let mut bytes = elf(kind, osabi);
    bytes[48..52].copy_from_slice(&0x0a4f_u32.to_le_bytes());
    bytes
}

fn elf_with_metadata(kind: u16, osabi: u8, metadata: &[u8]) -> Vec<u8> {
    let mut note = Vec::new();
    note.extend_from_slice(&7_u32.to_le_bytes());
    note.extend_from_slice(&(metadata.len() as u32).to_le_bytes());
    note.extend_from_slice(&32_u32.to_le_bytes());
    note.extend_from_slice(b"AMDGPU\0");
    while note.len() % 4 != 0 {
        note.push(0);
    }
    note.extend_from_slice(metadata);
    while note.len() % 4 != 0 {
        note.push(0);
    }
    let names = b"\0.note\0.shstrtab\0";
    let note_offset = 64;
    let names_offset = note_offset + note.len();
    let section_offset = (names_offset + names.len() + 7) & !7;
    let mut bytes = vec![0_u8; section_offset + 3 * 64];
    bytes[..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    bytes[7] = osabi;
    bytes[8] = 4;
    bytes[16..18].copy_from_slice(&kind.to_le_bytes());
    bytes[18..20].copy_from_slice(&224_u16.to_le_bytes());
    bytes[20..24].copy_from_slice(&1_u32.to_le_bytes());
    bytes[48..52].copy_from_slice(&0x64c_u32.to_le_bytes());
    bytes[52..54].copy_from_slice(&64_u16.to_le_bytes());
    bytes[40..48].copy_from_slice(&(section_offset as u64).to_le_bytes());
    bytes[58..60].copy_from_slice(&64_u16.to_le_bytes());
    bytes[60..62].copy_from_slice(&3_u16.to_le_bytes());
    bytes[62..64].copy_from_slice(&2_u16.to_le_bytes());
    bytes[note_offset..note_offset + note.len()].copy_from_slice(&note);
    bytes[names_offset..names_offset + names.len()].copy_from_slice(names);

    let note_section = section_offset + 64;
    bytes[note_section..note_section + 4].copy_from_slice(&1_u32.to_le_bytes());
    bytes[note_section + 4..note_section + 8].copy_from_slice(&7_u32.to_le_bytes());
    bytes[note_section + 8..note_section + 16].copy_from_slice(&2_u64.to_le_bytes());
    bytes[note_section + 24..note_section + 32]
        .copy_from_slice(&(note_offset as u64).to_le_bytes());
    bytes[note_section + 32..note_section + 40].copy_from_slice(&(note.len() as u64).to_le_bytes());
    bytes[note_section + 48..note_section + 56].copy_from_slice(&4_u64.to_le_bytes());

    let names_section = section_offset + 128;
    bytes[names_section..names_section + 4].copy_from_slice(&7_u32.to_le_bytes());
    bytes[names_section + 4..names_section + 8].copy_from_slice(&3_u32.to_le_bytes());
    bytes[names_section + 24..names_section + 32]
        .copy_from_slice(&(names_offset as u64).to_le_bytes());
    bytes[names_section + 32..names_section + 40]
        .copy_from_slice(&(names.len() as u64).to_le_bytes());
    bytes[names_section + 48..names_section + 56].copy_from_slice(&1_u64.to_le_bytes());
    bytes
}

fn append_aligned(bytes: &mut Vec<u8>, alignment: usize, contents: &[u8]) -> usize {
    while bytes.len() % alignment != 0 {
        bytes.push(0);
    }
    let offset = bytes.len();
    bytes.extend_from_slice(contents);
    offset
}

#[allow(clippy::too_many_arguments)]
fn write_section(
    bytes: &mut [u8],
    table: usize,
    index: usize,
    name: u32,
    section_type: u32,
    flags: u64,
    offset: usize,
    size: usize,
    link: u32,
    info: u32,
    alignment: u64,
    entry_size: u64,
) {
    let section = table + index * 64;
    bytes[section..section + 4].copy_from_slice(&name.to_le_bytes());
    bytes[section + 4..section + 8].copy_from_slice(&section_type.to_le_bytes());
    bytes[section + 8..section + 16].copy_from_slice(&flags.to_le_bytes());
    bytes[section + 24..section + 32].copy_from_slice(&(offset as u64).to_le_bytes());
    bytes[section + 32..section + 40].copy_from_slice(&(size as u64).to_le_bytes());
    bytes[section + 40..section + 44].copy_from_slice(&link.to_le_bytes());
    bytes[section + 44..section + 48].copy_from_slice(&info.to_le_bytes());
    bytes[section + 48..section + 56].copy_from_slice(&alignment.to_le_bytes());
    bytes[section + 56..section + 64].copy_from_slice(&entry_size.to_le_bytes());
}

fn rich_elf(
    kind: u16,
    osabi: u8,
    text_section_name: &str,
    symbol_name: &str,
    relocation_type: u32,
) -> Vec<u8> {
    let metadata = b"\x81\xaeversion-marker\x06";
    let mut note = Vec::new();
    note.extend_from_slice(&7_u32.to_le_bytes());
    note.extend_from_slice(&(metadata.len() as u32).to_le_bytes());
    note.extend_from_slice(&32_u32.to_le_bytes());
    note.extend_from_slice(b"AMDGPU\0");
    while note.len() % 4 != 0 {
        note.push(0);
    }
    note.extend_from_slice(metadata);
    while note.len() % 4 != 0 {
        note.push(0);
    }

    let mut section_names = vec![0];
    let mut add_section_name = |name: &str| {
        let offset = section_names.len() as u32;
        section_names.extend_from_slice(name.as_bytes());
        section_names.push(0);
        offset
    };
    let text_name = add_section_name(text_section_name);
    let note_name = add_section_name(".note");
    let symbol_table_name = add_section_name(".symtab");
    let string_table_name = add_section_name(".strtab");
    let relocation_name = add_section_name(".rela.text");
    let section_names_name = add_section_name(".shstrtab");

    let mut strings = vec![0];
    let symbol_name_offset = strings.len() as u32;
    strings.extend_from_slice(symbol_name.as_bytes());
    strings.push(0);

    let text = b"\x01\x02\x03\x04\x05\x06\x07\x08";
    let mut symbols = vec![0; 48];
    symbols[24..28].copy_from_slice(&symbol_name_offset.to_le_bytes());
    symbols[28] = 0x12;
    symbols[30..32].copy_from_slice(&1_u16.to_le_bytes());
    symbols[40..48].copy_from_slice(&(text.len() as u64).to_le_bytes());
    let mut relocation = vec![0; 24];
    relocation[..8].copy_from_slice(&0_u64.to_le_bytes());
    relocation[8..16].copy_from_slice(&((1_u64 << 32) | u64::from(relocation_type)).to_le_bytes());

    let mut bytes = vec![0; 64];
    let text_offset = append_aligned(&mut bytes, 4, text);
    let note_offset = append_aligned(&mut bytes, 4, &note);
    let symbols_offset = append_aligned(&mut bytes, 8, &symbols);
    let strings_offset = append_aligned(&mut bytes, 1, &strings);
    let relocation_offset = append_aligned(&mut bytes, 8, &relocation);
    let section_names_offset = append_aligned(&mut bytes, 1, &section_names);
    while bytes.len() % 8 != 0 {
        bytes.push(0);
    }
    let section_table = bytes.len();
    bytes.resize(section_table + 7 * 64, 0);

    bytes[..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    bytes[7] = osabi;
    bytes[8] = 4;
    bytes[16..18].copy_from_slice(&kind.to_le_bytes());
    bytes[18..20].copy_from_slice(&224_u16.to_le_bytes());
    bytes[20..24].copy_from_slice(&1_u32.to_le_bytes());
    bytes[40..48].copy_from_slice(&(section_table as u64).to_le_bytes());
    bytes[48..52].copy_from_slice(&0x64c_u32.to_le_bytes());
    bytes[52..54].copy_from_slice(&64_u16.to_le_bytes());
    bytes[58..60].copy_from_slice(&64_u16.to_le_bytes());
    bytes[60..62].copy_from_slice(&7_u16.to_le_bytes());
    bytes[62..64].copy_from_slice(&6_u16.to_le_bytes());

    write_section(
        &mut bytes,
        section_table,
        1,
        text_name,
        1,
        0x6,
        text_offset,
        text.len(),
        0,
        0,
        4,
        0,
    );
    write_section(
        &mut bytes,
        section_table,
        2,
        note_name,
        7,
        0x2,
        note_offset,
        note.len(),
        0,
        0,
        4,
        0,
    );
    write_section(
        &mut bytes,
        section_table,
        3,
        symbol_table_name,
        2,
        0,
        symbols_offset,
        symbols.len(),
        4,
        1,
        8,
        24,
    );
    write_section(
        &mut bytes,
        section_table,
        4,
        string_table_name,
        3,
        0,
        strings_offset,
        strings.len(),
        0,
        0,
        1,
        0,
    );
    write_section(
        &mut bytes,
        section_table,
        5,
        relocation_name,
        4,
        0,
        relocation_offset,
        relocation.len(),
        3,
        1,
        8,
        24,
    );
    write_section(
        &mut bytes,
        section_table,
        6,
        section_names_name,
        3,
        0,
        section_names_offset,
        section_names.len(),
        0,
        0,
        1,
        0,
    );
    bytes
}

fn checked(
    pre: Vec<u8>,
    post: Vec<u8>,
    object: Vec<u8>,
    hsaco: Vec<u8>,
) -> fe2o3_compiler_lineage::CheckedPostLlvmStageContentsV1 {
    let after_strip = b"bitcode-after-strip";
    let after_exports = b"bitcode-after-exports";
    let after_o2 = b"bitcode-after-expanded-O2";
    let phases = FIXED_PRODUCTION_LLVM_PHASES_V1
        .into_iter()
        .zip([
            (LLVM, pre.as_slice()),
            (pre.as_slice(), after_strip.as_slice()),
            (after_strip.as_slice(), after_exports.as_slice()),
            (after_exports.as_slice(), after_o2.as_slice()),
            (after_o2.as_slice(), post.as_slice()),
            (post.as_slice(), post.as_slice()),
            (post.as_slice(), object.as_slice()),
            (object.as_slice(), hsaco.as_slice()),
        ])
        .map(|(phase, (input, output))| {
            ExactProductionLlvmPhaseContentsV1::new(phase, input, output).unwrap()
        })
        .collect::<Vec<_>>();
    let pass_occurrences = vec![
        ExactLlvmPassOccurrenceContentsV1::new(
            0,
            1,
            LlvmPassIrUnitV1::Module,
            "default-O2",
            b"verify-each".as_slice(),
            after_exports.as_slice(),
            after_o2.as_slice(),
        )
        .unwrap(),
    ];
    let record = PostLlvmStageCustodyV1::from_exact_stage_bytes(
        LLVM,
        &pre,
        &post,
        &object,
        &hsaco,
        "llvmorg-22.0.0-fe2o3",
        vec![LlvmPassInvocationV1::new("default-O2", b"verify-each".to_vec()).unwrap()],
    )
    .unwrap()
    .with_fixed_production_pipeline(WORKER, "worker-build-v1", &phases)
    .unwrap();
    let checked =
        check_exact_post_llvm_stage_contents_v1(record, LLVM.to_vec(), pre, post, object, hsaco)
            .unwrap();
    let checked = check_exact_fixed_production_pipeline_contents_v1(
        checked,
        WORKER,
        "worker-build-v1",
        "llvmorg-22.0.0-fe2o3",
        phases.clone(),
    )
    .unwrap();
    let transcript = PostLlvmPipelineOccurrenceTranscriptV1::capture_unavailable_production_v4(
        [1; 32],
        [2; 32],
        [3; 32],
        WORKER,
        ASSEMBLER,
        "worker-build-v1",
        "llvmorg-22.0.0-fe2o3",
        LLVM,
        ASSEMBLED,
        &phases,
        &pass_occurrences,
    )
    .unwrap();
    check_exact_post_llvm_pipeline_occurrence_v1(
        checked,
        transcript,
        WORKER,
        ASSEMBLER,
        LLVM,
        ASSEMBLED,
        LLVM,
        ASSEMBLED,
        phases,
        pass_occurrences,
    )
    .unwrap()
}

#[test]
fn raw_bitcode_relocatable_and_final_hsaco_envelopes_are_checked_without_overclaim() {
    let evidence = check_gfx942_post_llvm_stage_envelopes_v1(checked(
        b"BC\xc0\xde-pre".to_vec(),
        b"BC\xc0\xde-post".to_vec(),
        elf(1, 0),
        elf(3, 64),
    ))
    .unwrap();
    assert!(evidence.retains_exact_llvm_stage_contents_and_recorded_passes());
    assert!(evidence.retains_exact_gfx942_relocatable_object());
    assert!(evidence.retains_exact_gfx942_amdhsa_elf_envelope());
    assert!(evidence.proves_code_object_version());
    assert!(evidence.retains_exact_cov6_metadata_descriptor_across_link());
    assert!(!evidence.proves_hsaco_metadata());
    assert!(!evidence.proves_llvm_optimization_semantics());
    assert!(!evidence.proves_instruction_selection());
    assert!(!evidence.proves_object_to_hsaco_preservation());
    assert!(!evidence.grants_authority());
}

#[test]
fn gfx950_has_a_distinct_target_checked_stage_and_preservation_owner() {
    let hsaco = gfx950_elf(3, 64);
    let evidence = check_gfx950_post_llvm_stage_envelopes_v1(checked(
        b"BC\xc0\xde-pre".to_vec(),
        b"BC\xc0\xde-post".to_vec(),
        gfx950_elf(1, 64),
        hsaco.clone(),
    ))
    .unwrap();
    assert!(evidence.retains_exact_gfx950_relocatable_object());
    assert!(evidence.retains_exact_gfx950_amdhsa_elf_envelope());
    assert!(evidence.retains_exact_cov6_metadata_descriptor_across_link());
    let preserved = check_gfx950_object_to_hsaco_preservation_v1(evidence, &hsaco).unwrap();
    assert!(preserved.retains_object_to_hsaco_structural_and_replay_premises());
    assert!(preserved.proves_relocation_semantics());
    assert!(!preserved.grants_authority());
}

#[test]
fn gfx942_and_gfx950_target_header_substitution_is_rejected_both_ways() {
    assert_eq!(
        check_gfx950_post_llvm_stage_envelopes_v1(checked(
            b"BC\xc0\xde-pre".to_vec(),
            b"BC\xc0\xde-post".to_vec(),
            elf(1, 64),
            gfx950_elf(3, 64),
        ))
        .unwrap_err(),
        Gfx950PostLlvmStageEnvelopeErrorV1::InvalidRelocatableObject
    );
    assert_eq!(
        check_gfx942_post_llvm_stage_envelopes_v1(checked(
            b"BC\xc0\xde-pre".to_vec(),
            b"BC\xc0\xde-post".to_vec(),
            gfx950_elf(1, 64),
            elf(3, 64),
        ))
        .unwrap_err(),
        Gfx942PostLlvmStageEnvelopeErrorV1::InvalidRelocatableObject
    );
}

#[test]
fn wrapped_bitcode_is_range_checked() {
    let mut wrapped = vec![0_u8; 24];
    wrapped[..4].copy_from_slice(b"\xde\xc0\x17\x0b");
    wrapped[8..12].copy_from_slice(&20_u32.to_le_bytes());
    wrapped[12..16].copy_from_slice(&4_u32.to_le_bytes());
    wrapped[20..24].copy_from_slice(b"BC\xc0\xde");
    assert!(
        check_gfx942_post_llvm_stage_envelopes_v1(checked(
            wrapped.clone(),
            wrapped,
            elf(1, 64),
            elf(3, 64),
        ))
        .is_ok()
    );
}

#[test]
fn malformed_bitcode_object_hsaco_and_target_flags_fail_named_axes() {
    let cases = [
        (
            b"not-bitcode".to_vec(),
            b"BC\xc0\xde-post".to_vec(),
            elf(1, 0),
            elf(3, 64),
            Gfx942PostLlvmStageEnvelopeErrorV1::InvalidPreOptimizationBitcode,
        ),
        (
            b"BC\xc0\xde-pre".to_vec(),
            b"not-bitcode".to_vec(),
            elf(1, 0),
            elf(3, 64),
            Gfx942PostLlvmStageEnvelopeErrorV1::InvalidPostOptimizationBitcode,
        ),
        (
            b"BC\xc0\xde-pre".to_vec(),
            b"BC\xc0\xde-post".to_vec(),
            elf(3, 0),
            elf(3, 64),
            Gfx942PostLlvmStageEnvelopeErrorV1::InvalidRelocatableObject,
        ),
        (
            b"BC\xc0\xde-pre".to_vec(),
            b"BC\xc0\xde-post".to_vec(),
            elf(1, 0),
            elf(1, 64),
            Gfx942PostLlvmStageEnvelopeErrorV1::InvalidFinalAmdhsaElfEnvelope,
        ),
    ];
    for (pre, post, object, hsaco, expected) in cases {
        assert_eq!(
            check_gfx942_post_llvm_stage_envelopes_v1(checked(pre, post, object, hsaco))
                .unwrap_err(),
            expected
        );
    }

    let mut wrong_target = elf(3, 64);
    wrong_target[48..52].copy_from_slice(&0x64f_u32.to_le_bytes());
    assert_eq!(
        check_gfx942_post_llvm_stage_envelopes_v1(checked(
            b"BC\xc0\xde-pre".to_vec(),
            b"BC\xc0\xde-post".to_vec(),
            elf(1, 0),
            wrong_target,
        ))
        .unwrap_err(),
        Gfx942PostLlvmStageEnvelopeErrorV1::InvalidFinalAmdhsaElfEnvelope
    );
}

#[test]
fn cov6_metadata_descriptor_substitution_is_rejected_as_a_link_preservation_axis() {
    assert_eq!(
        check_gfx942_post_llvm_stage_envelopes_v1(checked(
            b"BC\xc0\xde-pre".to_vec(),
            b"BC\xc0\xde-post".to_vec(),
            elf_with_metadata(1, 0, b"object metadata"),
            elf_with_metadata(3, 64, b"substituted final metadata"),
        ))
        .unwrap_err(),
        Gfx942PostLlvmStageEnvelopeErrorV1::Cov6MetadataNotPreserved
    );
}

#[test]
fn named_sections_symbols_relocations_and_exact_link_replay_are_retained_as_premises() {
    let object = rich_elf(1, 0, ".text", "scalar_gemm_v1", 5);
    let mut hsaco = rich_elf(3, 64, ".text", "scalar_gemm_v1", 5);
    hsaco[64..72].fill(0);
    let envelopes = check_gfx942_post_llvm_stage_envelopes_v1(checked(
        b"BC\xc0\xde-pre".to_vec(),
        b"BC\xc0\xde-post".to_vec(),
        object,
        hsaco.clone(),
    ))
    .unwrap();
    let evidence = check_gfx942_object_to_hsaco_preservation_v1(envelopes, &hsaco).unwrap();
    assert!(evidence.retains_object_to_hsaco_structural_and_replay_premises());
    assert_eq!(evidence.sections().len(), 1);
    assert_eq!(evidence.sections()[0].name(), ".text");
    assert!(evidence.sections()[0].has_relocations());
    assert_eq!(evidence.symbols()[0].name(), "scalar_gemm_v1");
    assert_eq!(evidence.relocations()[0].relocation_type(), 5);
    assert!(evidence.proves_relocation_semantics());
    assert!(!evidence.proves_linker_semantic_refinement());
    assert!(!evidence.grants_authority());
}

#[test]
fn replay_section_symbol_and_retained_relocation_substitutions_fail_closed() {
    fn reject(
        object: Vec<u8>,
        hsaco: Vec<u8>,
        replay: Vec<u8>,
    ) -> Gfx942ObjectToHsacoPreservationErrorV1 {
        let envelopes = check_gfx942_post_llvm_stage_envelopes_v1(checked(
            b"BC\xc0\xde-pre".to_vec(),
            b"BC\xc0\xde-post".to_vec(),
            object,
            hsaco,
        ))
        .unwrap();
        check_gfx942_object_to_hsaco_preservation_v1(envelopes, &replay).unwrap_err()
    }

    let object = rich_elf(1, 0, ".text", "scalar_gemm_v1", 5);
    let mut hsaco = rich_elf(3, 64, ".text", "scalar_gemm_v1", 5);
    hsaco[64..72].fill(0);
    assert_eq!(
        reject(object.clone(), hsaco, b"substituted replay".to_vec()),
        Gfx942ObjectToHsacoPreservationErrorV1::HsacoReplayMismatch
    );

    let renamed_section = rich_elf(3, 64, ".code", "scalar_gemm_v1", 5);
    assert_eq!(
        reject(object.clone(), renamed_section.clone(), renamed_section),
        Gfx942ObjectToHsacoPreservationErrorV1::SectionNotPreserved
    );

    let renamed_symbol = rich_elf(3, 64, ".text", "other_kernel", 5);
    assert_eq!(
        reject(object.clone(), renamed_symbol.clone(), renamed_symbol),
        Gfx942ObjectToHsacoPreservationErrorV1::SymbolNotPreserved
    );

    let changed_relocation = rich_elf(3, 64, ".text", "scalar_gemm_v1", 4);
    assert_eq!(
        reject(object, changed_relocation.clone(), changed_relocation),
        Gfx942ObjectToHsacoPreservationErrorV1::RelocationRosterNotPreserved
    );

    let unsupported = rich_elf(1, 0, ".text", "scalar_gemm_v1", 7);
    let final_bytes = rich_elf(3, 64, ".text", "scalar_gemm_v1", 7);
    assert_eq!(
        reject(unsupported, final_bytes.clone(), final_bytes),
        Gfx942ObjectToHsacoPreservationErrorV1::UnsupportedRelocationType
    );

    let object = rich_elf(1, 0, ".text", "scalar_gemm_v1", 5);
    let wrong_application = rich_elf(3, 64, ".text", "scalar_gemm_v1", 5);
    assert_eq!(
        reject(object, wrong_application.clone(), wrong_application),
        Gfx942ObjectToHsacoPreservationErrorV1::RelocationApplicationMismatch
    );
}

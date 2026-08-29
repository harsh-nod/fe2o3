use fe2o3_kernel_analysis::{
    Gfx942InstructionRegisterFactsV1, Gfx942MachineDataflowV1, Gfx942ReachingDefinitionV1,
    Gfx942RegisterAliasV1, Gfx942RegisterFactsErrorV1, Gfx942RegisterUnitV1,
    PHYSICAL_MACHINE_ANALYSIS_BUNDLE_DOMAIN_V1, PHYSICAL_MACHINE_ANALYSIS_BUNDLE_SCHEMA_VERSION_V1,
    PHYSICAL_MACHINE_EFFECT_EVIDENCE_DOMAIN_V1, PHYSICAL_MACHINE_EFFECT_REQUEST_DOMAIN_V1,
    PHYSICAL_MACHINE_EFFECT_SCHEMA_VERSION_V1, PHYSICAL_MACHINE_TRACE_EVIDENCE_DOMAIN_V1,
    PHYSICAL_MACHINE_TRACE_SCHEMA_VERSION_V1, PhysicalMachineAnalysisEvidenceErrorV1,
    PhysicalMachineAnalysisEvidenceV1, PhysicalMachineAnalyzerIdentityV1,
    PhysicalMachineEffectAnalysisBasisV1, PhysicalMachineEffectBudgetV1,
    PhysicalMachineEffectEntryRequestV1, PhysicalMachineEffectEvidenceErrorV1,
    PhysicalMachineEffectEvidenceV1, PhysicalMachineEffectKindV1,
    PhysicalMachineEffectRequestErrorV1, PhysicalMachineEffectRequestV1,
    PhysicalMachineExecutionChallengeV1, PhysicalMachineOperandValueV1, PhysicalMachineTargetV1,
    PhysicalMachineToolchainIdentityV1, PhysicalMachineTraceEvidenceErrorV1,
    PhysicalMachineTraceEvidenceV1,
};
use std::{collections::BTreeSet, fs};

const CODE_OFFSET: u64 = 4;
const CODE_SIZE: u64 = 16;

#[derive(Clone)]
struct Function<'a> {
    symbol: &'a str,
    offset: u64,
    size: u64,
    callees: Vec<&'a str>,
}

#[derive(Clone)]
struct Effect<'a> {
    entry: &'a str,
    function: &'a str,
    offset: u64,
    kind: u8,
    width: u16,
}

fn budget() -> PhysicalMachineEffectBudgetV1 {
    PhysicalMachineEffectBudgetV1::new(8, 4, 4, 2, 2)
}

fn entry(
    symbol: &str,
    budget: PhysicalMachineEffectBudgetV1,
) -> PhysicalMachineEffectEntryRequestV1 {
    PhysicalMachineEffectEntryRequestV1::new(symbol, budget).unwrap()
}

fn request_with(
    payload: &[u8],
    entries: Vec<PhysicalMachineEffectEntryRequestV1>,
) -> PhysicalMachineEffectRequestV1 {
    PhysicalMachineEffectRequestV1::new(
        PhysicalMachineExecutionChallengeV1::from_sha256_bytes([0x10; 32]),
        PhysicalMachineAnalyzerIdentityV1::from_sha256_bytes([0x11; 32]),
        PhysicalMachineToolchainIdentityV1::from_sha256_bytes([0x22; 32]),
        payload.to_vec(),
        entries,
    )
    .unwrap()
}

fn request() -> PhysicalMachineEffectRequestV1 {
    request_with(
        b"exact finalized gfx942 hsaco",
        vec![entry("arbitrary_entry", budget())],
    )
}

fn effects() -> Vec<Effect<'static>> {
    vec![
        Effect {
            entry: "arbitrary_entry",
            function: "arbitrary_entry",
            offset: CODE_OFFSET,
            kind: 1,
            width: 8,
        },
        Effect {
            entry: "arbitrary_entry",
            function: "arbitrary_entry",
            offset: CODE_OFFSET,
            kind: 2,
            width: 4,
        },
        Effect {
            entry: "arbitrary_entry",
            function: "arbitrary_entry",
            offset: CODE_OFFSET + 4,
            kind: 1,
            width: 8,
        },
        Effect {
            entry: "arbitrary_entry",
            function: "arbitrary_entry",
            offset: CODE_OFFSET + 4,
            kind: 3,
            width: 4,
        },
        Effect {
            entry: "arbitrary_entry",
            function: "arbitrary_entry",
            offset: CODE_OFFSET + 8,
            kind: 4,
            width: 0,
        },
    ]
}

fn evidence(
    request: &PhysicalMachineEffectRequestV1,
    functions: &[Function<'_>],
    effects: &[Effect<'_>],
) -> Vec<u8> {
    evidence_with_entry_range(request, CODE_OFFSET, CODE_SIZE, functions, effects)
}

fn evidence_with_entry_range(
    request: &PhysicalMachineEffectRequestV1,
    entry_offset: u64,
    entry_size: u64,
    functions: &[Function<'_>],
    effects: &[Effect<'_>],
) -> Vec<u8> {
    let mut output = Vec::new();
    output.extend_from_slice(PHYSICAL_MACHINE_EFFECT_EVIDENCE_DOMAIN_V1);
    push_u32(&mut output, 0);
    push_u16(&mut output, PHYSICAL_MACHINE_EFFECT_SCHEMA_VERSION_V1);
    output.extend_from_slice(&request.execution_challenge().as_bytes());
    output.extend_from_slice(&request.identity().sha256());
    push_u64(&mut output, request.identity().byte_len());
    output.extend_from_slice(&request.payload_identity().sha256());
    push_u64(&mut output, request.payload_identity().byte_len());
    output.extend_from_slice(&request.analyzer_identity().as_bytes());
    output.extend_from_slice(&request.toolchain_identity().as_bytes());
    push_u16(&mut output, 1);

    push_u16(&mut output, request.entries().len() as u16);
    for entry in request.entries() {
        push_text(&mut output, entry.symbol());
        output.extend_from_slice(&[0x33; 32]);
        push_u64(&mut output, entry_offset);
        push_u64(&mut output, entry_size);
    }

    push_u32(&mut output, functions.len() as u32);
    for function in functions {
        push_text(&mut output, function.symbol);
        push_u64(&mut output, function.offset);
        push_u64(&mut output, function.size);
        push_u16(&mut output, function.callees.len() as u16);
        for callee in &function.callees {
            push_text(&mut output, callee);
        }
    }

    push_u32(&mut output, effects.len() as u32);
    for effect in effects {
        push_text(&mut output, effect.entry);
        push_text(&mut output, effect.function);
        push_u64(&mut output, effect.offset);
        output.push(effect.kind);
        push_u16(&mut output, effect.width);
    }

    let length = output.len() as u32;
    let offset = PHYSICAL_MACHINE_EFFECT_EVIDENCE_DOMAIN_V1.len();
    output[offset..offset + 4].copy_from_slice(&length.to_le_bytes());
    output
}

fn entry_function() -> Function<'static> {
    Function {
        symbol: "arbitrary_entry",
        offset: CODE_OFFSET,
        size: CODE_SIZE,
        callees: Vec::new(),
    }
}

fn push_u16(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(output: &mut Vec<u8>, value: u64) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_text(output: &mut Vec<u8>, value: &str) {
    push_u16(output, value.len() as u16);
    output.extend_from_slice(value.as_bytes());
}

#[derive(Clone)]
enum TraceOperand<'a> {
    Register(&'a str, Option<u16>),
    Signed(i64),
}

#[derive(Clone)]
struct TraceInstruction<'a> {
    offset: u64,
    block: u32,
    opcode: &'a str,
    encoding: [u8; 4],
    definitions: u16,
    operands: Vec<TraceOperand<'a>>,
    implicit_definitions: Vec<&'a str>,
    implicit_uses: Vec<&'a str>,
    branch: u8,
    target: u64,
    flags: u16,
    memory: u8,
    width: u16,
}

#[derive(Clone)]
struct TraceBlock {
    ordinal: u32,
    first_offset: u64,
    instruction_count: u32,
    successors: Vec<u32>,
}

#[derive(Clone, Copy)]
struct TraceMutationOffsets {
    first_encoding: usize,
    conditional_target: usize,
    conditional_successor: usize,
    trap_flags: usize,
}

fn loop_trace_fixture() -> (
    PhysicalMachineEffectRequestV1,
    PhysicalMachineEffectEvidenceV1,
    Vec<u8>,
    TraceMutationOffsets,
) {
    let mut payload = vec![0_u8; 64];
    let encodings = [
        [0x10, 0x11, 0x12, 0x13],
        [0x20, 0x21, 0x22, 0x23],
        [0x30, 0x31, 0x32, 0x33],
        [0x40, 0x41, 0x42, 0x43],
        [0x50, 0x51, 0x52, 0x53],
        [0x60, 0x61, 0x62, 0x63],
        [0x70, 0x71, 0x72, 0x73],
    ];
    for (index, encoding) in encodings.iter().enumerate() {
        let offset = 8 + index * 4;
        payload[offset..offset + 4].copy_from_slice(encoding);
    }
    let request = request_with(&payload, vec![entry("loop_entry", budget())]);
    let function = Function {
        symbol: "loop_entry",
        offset: 8,
        size: 32,
        callees: Vec::new(),
    };
    let effect_records = vec![
        Effect {
            entry: "loop_entry",
            function: "loop_entry",
            offset: 12,
            kind: 1,
            width: 8,
        },
        Effect {
            entry: "loop_entry",
            function: "loop_entry",
            offset: 12,
            kind: 2,
            width: 4,
        },
        Effect {
            entry: "loop_entry",
            function: "loop_entry",
            offset: 28,
            kind: 1,
            width: 8,
        },
        Effect {
            entry: "loop_entry",
            function: "loop_entry",
            offset: 28,
            kind: 3,
            width: 4,
        },
        Effect {
            entry: "loop_entry",
            function: "loop_entry",
            offset: 32,
            kind: 4,
            width: 0,
        },
    ];
    let effect_bytes = evidence_with_entry_range(
        &request,
        8,
        32,
        std::slice::from_ref(&function),
        &effect_records,
    );
    let effects =
        PhysicalMachineEffectEvidenceV1::decode_canonical_for(&request, &effect_bytes).unwrap();
    let blocks = [
        TraceBlock {
            ordinal: 0,
            first_offset: 8,
            instruction_count: 1,
            successors: vec![1],
        },
        TraceBlock {
            ordinal: 1,
            first_offset: 12,
            instruction_count: 2,
            successors: vec![2, 3],
        },
        TraceBlock {
            ordinal: 2,
            first_offset: 20,
            instruction_count: 2,
            successors: vec![1],
        },
        TraceBlock {
            ordinal: 3,
            first_offset: 28,
            instruction_count: 2,
            successors: Vec::new(),
        },
    ];
    let instructions = [
        TraceInstruction {
            offset: 8,
            block: 0,
            opcode: "S_MOV_B32_vi",
            encoding: encodings[0],
            definitions: 1,
            operands: vec![
                TraceOperand::Register("SGPR4", None),
                TraceOperand::Signed(0),
            ],
            implicit_definitions: Vec::new(),
            implicit_uses: Vec::new(),
            branch: 0,
            target: 0,
            flags: 0,
            memory: 0,
            width: 0,
        },
        TraceInstruction {
            offset: 12,
            block: 1,
            opcode: "GLOBAL_LOAD_DWORD_vi",
            encoding: encodings[1],
            definitions: 1,
            operands: vec![
                TraceOperand::Register("VGPR0", None),
                TraceOperand::Register("VGPR2_VGPR3", None),
            ],
            implicit_definitions: Vec::new(),
            implicit_uses: vec!["EXEC"],
            branch: 0,
            target: 0,
            flags: 1,
            memory: 1,
            width: 4,
        },
        TraceInstruction {
            offset: 16,
            block: 1,
            opcode: "S_CBRANCH_SCC1_vi",
            encoding: encodings[2],
            definitions: 0,
            operands: Vec::new(),
            implicit_definitions: Vec::new(),
            implicit_uses: vec!["SCC"],
            branch: 1,
            target: 28,
            flags: 4,
            memory: 0,
            width: 0,
        },
        TraceInstruction {
            offset: 20,
            block: 2,
            opcode: "S_TRAP_vi",
            encoding: encodings[3],
            definitions: 0,
            operands: vec![TraceOperand::Signed(2)],
            implicit_definitions: Vec::new(),
            implicit_uses: Vec::new(),
            branch: 0,
            target: 0,
            flags: 32,
            memory: 0,
            width: 0,
        },
        TraceInstruction {
            offset: 24,
            block: 2,
            opcode: "S_BRANCH_vi",
            encoding: encodings[4],
            definitions: 0,
            operands: Vec::new(),
            implicit_definitions: Vec::new(),
            implicit_uses: Vec::new(),
            branch: 2,
            target: 12,
            flags: 4,
            memory: 0,
            width: 0,
        },
        TraceInstruction {
            offset: 28,
            block: 3,
            opcode: "GLOBAL_STORE_DWORD_vi",
            encoding: encodings[5],
            definitions: 0,
            operands: vec![
                TraceOperand::Register("VGPR6_VGPR7", None),
                TraceOperand::Register("VGPR4", None),
            ],
            implicit_definitions: Vec::new(),
            implicit_uses: vec!["EXEC"],
            branch: 0,
            target: 0,
            flags: 2,
            memory: 2,
            width: 4,
        },
        TraceInstruction {
            offset: 32,
            block: 3,
            opcode: "S_ENDPGM_vi",
            encoding: encodings[6],
            definitions: 0,
            operands: Vec::new(),
            implicit_definitions: Vec::new(),
            implicit_uses: Vec::new(),
            branch: 4,
            target: 0,
            flags: 4,
            memory: 0,
            width: 0,
        },
    ];

    let (trace, offsets) = encode_trace(&request, &effects, &blocks, &instructions);
    (request, effects, trace, offsets)
}

fn encode_trace(
    request: &PhysicalMachineEffectRequestV1,
    effects: &PhysicalMachineEffectEvidenceV1,
    blocks: &[TraceBlock],
    instructions: &[TraceInstruction<'_>],
) -> (Vec<u8>, TraceMutationOffsets) {
    let mut output = Vec::new();
    output.extend_from_slice(PHYSICAL_MACHINE_TRACE_EVIDENCE_DOMAIN_V1);
    push_u32(&mut output, 0);
    push_u16(&mut output, PHYSICAL_MACHINE_TRACE_SCHEMA_VERSION_V1);
    output.extend_from_slice(&request.execution_challenge().as_bytes());
    output.extend_from_slice(&request.identity().sha256());
    push_u64(&mut output, request.identity().byte_len());
    output.extend_from_slice(&effects.identity().sha256());
    push_u64(&mut output, effects.identity().byte_len());
    output.extend_from_slice(&request.payload_identity().sha256());
    push_u64(&mut output, request.payload_identity().byte_len());
    output.extend_from_slice(&request.analyzer_identity().as_bytes());
    output.extend_from_slice(&request.toolchain_identity().as_bytes());
    push_u16(&mut output, 1);

    push_u32(&mut output, blocks.len() as u32);
    let mut conditional_successor = 0;
    for block in blocks {
        push_text(&mut output, "loop_entry");
        push_u32(&mut output, block.ordinal);
        push_u64(&mut output, block.first_offset);
        push_u32(&mut output, block.instruction_count);
        push_u16(&mut output, block.successors.len() as u16);
        for (index, successor) in block.successors.iter().enumerate() {
            if block.ordinal == 1 && index == 0 {
                conditional_successor = output.len();
            }
            push_u32(&mut output, *successor);
        }
    }

    push_u32(&mut output, instructions.len() as u32);
    let mut first_encoding = 0;
    let mut conditional_target = 0;
    let mut trap_flags = 0;
    for (index, instruction) in instructions.iter().enumerate() {
        push_text(&mut output, "loop_entry");
        push_u64(&mut output, instruction.offset);
        push_u32(&mut output, instruction.block);
        push_text(&mut output, instruction.opcode);
        push_u16(&mut output, instruction.encoding.len() as u16);
        if index == 0 {
            first_encoding = output.len();
        }
        output.extend_from_slice(&instruction.encoding);
        push_u16(&mut output, instruction.definitions);
        push_u16(&mut output, instruction.operands.len() as u16);
        for operand in &instruction.operands {
            match operand {
                TraceOperand::Register(register, tied) => {
                    output.push(1);
                    push_u16(&mut output, tied.unwrap_or(u16::MAX));
                    push_text(&mut output, register);
                }
                TraceOperand::Signed(value) => {
                    output.push(2);
                    push_u16(&mut output, u16::MAX);
                    push_u64(&mut output, *value as u64);
                }
            }
        }
        push_u16(&mut output, instruction.implicit_definitions.len() as u16);
        for register in &instruction.implicit_definitions {
            push_text(&mut output, register);
        }
        push_u16(&mut output, instruction.implicit_uses.len() as u16);
        for register in &instruction.implicit_uses {
            push_text(&mut output, register);
        }
        output.push(instruction.branch);
        if instruction.branch == 1 {
            conditional_target = output.len();
        }
        push_u64(&mut output, instruction.target);
        if instruction.opcode == "S_TRAP_vi" {
            trap_flags = output.len();
        }
        push_u16(&mut output, instruction.flags);
        output.push(instruction.memory);
        push_u16(&mut output, instruction.width);
    }
    let length = output.len() as u32;
    let offset = PHYSICAL_MACHINE_TRACE_EVIDENCE_DOMAIN_V1.len();
    output[offset..offset + 4].copy_from_slice(&length.to_le_bytes());
    (
        output,
        TraceMutationOffsets {
            first_encoding,
            conditional_target,
            conditional_successor,
            trap_flags,
        },
    )
}

fn encode_analysis_bundle(effects: &[u8], trace: &[u8]) -> Vec<u8> {
    let mut output = Vec::new();
    output.extend_from_slice(PHYSICAL_MACHINE_ANALYSIS_BUNDLE_DOMAIN_V1);
    push_u32(&mut output, 0);
    push_u16(
        &mut output,
        PHYSICAL_MACHINE_ANALYSIS_BUNDLE_SCHEMA_VERSION_V1,
    );
    push_u32(&mut output, effects.len() as u32);
    output.extend_from_slice(effects);
    push_u32(&mut output, trace.len() as u32);
    output.extend_from_slice(trace);
    let length = output.len() as u32;
    let offset = PHYSICAL_MACHINE_ANALYSIS_BUNDLE_DOMAIN_V1.len();
    output[offset..offset + 4].copy_from_slice(&length.to_le_bytes());
    output
}

#[test]
fn configured_retained_machine_analysis_reopens_offline() {
    let Some(request_path) = std::env::var_os("FE2O3_MACHINE_ANALYSIS_RETAINED_REQUEST") else {
        return;
    };
    let Some(bundle_path) = std::env::var_os("FE2O3_MACHINE_ANALYSIS_RETAINED_BUNDLE") else {
        return;
    };
    let request = PhysicalMachineEffectRequestV1::decode_canonical(
        &fs::read(request_path).expect("read retained machine request"),
    )
    .expect("decode retained machine request");
    let analysis = PhysicalMachineAnalysisEvidenceV1::decode_canonical_for(
        &request,
        &fs::read(bundle_path).expect("read retained machine bundle"),
    )
    .expect("decode retained machine bundle");
    if let Ok(expected) = std::env::var("FE2O3_MACHINE_ANALYSIS_RETAINED_ENTRY") {
        assert_eq!(request.entries().len(), 1);
        assert_eq!(request.entries()[0].symbol(), expected);
    }
    assert!(analysis.binds_exact_payload_instruction_bytes());
    assert!(!analysis.establishes_machine_semantics());
    assert!(!analysis.establishes_compiler_refinement());
    assert!(!analysis.grants_load_authority());
    assert!(!analysis.grants_launch_authority());

    let dataflow = Gfx942MachineDataflowV1::derive(analysis.trace())
        .expect("derive bounded gfx942 machine dataflow");
    assert_eq!(dataflow.trace_identity(), analysis.trace().identity());
    assert!(!dataflow.establishes_machine_semantics());
    assert!(!dataflow.establishes_compiler_refinement());
    assert!(!dataflow.grants_launch_authority());

    let functions = dataflow.function_symbols().collect::<Vec<_>>();
    assert_eq!(functions.len(), 1, "retained scalar slice has one function");
    let function = functions[0];
    let trap = analysis
        .trace()
        .instructions()
        .iter()
        .filter(|instruction| instruction.opcode() == "S_TRAP_vi")
        .collect::<Vec<_>>();
    assert_eq!(trap.len(), 1, "retained scalar slice has one trap site");
    let trap = trap[0];
    let predecessor = analysis
        .trace()
        .blocks()
        .iter()
        .filter(|block| block.successors().contains(&trap.block_ordinal()))
        .collect::<Vec<_>>();
    assert_eq!(predecessor.len(), 1, "trap block has one predecessor");
    let predecessor = predecessor[0];
    let branch = analysis
        .trace()
        .instructions()
        .iter()
        .find(|instruction| {
            instruction.block_ordinal() == predecessor.ordinal()
                && instruction.opcode() == "S_CBRANCH_EXECNZ_vi"
        })
        .expect("trap predecessor branches on EXEC");
    assert!(
        dataflow
            .block_dominates(function, predecessor.ordinal(), trap.block_ordinal(),)
            .unwrap()
    );

    let expected = dataflow
        .reaching_definitions_before(
            function,
            branch.instruction_offset(),
            Gfx942RegisterUnitV1::ExecLow,
        )
        .unwrap();
    assert_eq!(
        expected.len(),
        1,
        "trap EXEC mask has one reaching definition"
    );
    assert_eq!(
        dataflow
            .reaching_definitions_before(
                function,
                branch.instruction_offset(),
                Gfx942RegisterUnitV1::ExecHigh,
            )
            .unwrap(),
        expected,
        "both EXEC halves share the exact reaching definition",
    );
    let Gfx942ReachingDefinitionV1::Instruction { offset: definition } = expected[0] else {
        panic!("trap EXEC mask must not be live-in");
    };
    let definition_instruction = analysis
        .trace()
        .instructions()
        .iter()
        .find(|instruction| instruction.instruction_offset() == definition)
        .expect("reaching definition instruction exists");
    assert_eq!(definition_instruction.opcode(), "S_AND_SAVEEXEC_B64_vi");
    assert!(
        dataflow
            .instruction_dominates(function, definition, branch.instruction_offset())
            .unwrap()
    );

    let mut registers = BTreeSet::new();
    for instruction in analysis.trace().instructions() {
        Gfx942InstructionRegisterFactsV1::derive(instruction)
            .expect("derive closed gfx942 register facts");
        for operand in instruction.operands() {
            if let PhysicalMachineOperandValueV1::Register(register) = operand.value() {
                registers.insert(register.as_str());
            }
        }
        registers.extend(
            instruction
                .implicit_definitions()
                .iter()
                .map(String::as_str),
        );
        registers.extend(instruction.implicit_uses().iter().map(String::as_str));
    }
    eprintln!(
        "offline gfx942 analysis: request={:?} bundle={:?} blocks={} instructions={} registers={registers:?}",
        request.identity(),
        analysis.identity(),
        analysis.trace().blocks().len(),
        analysis.trace().instructions().len(),
    );
    if std::env::var_os("FE2O3_MACHINE_ANALYSIS_DUMP_TRACE").is_some() {
        for block in analysis.trace().blocks() {
            eprintln!("block {block:?}");
        }
        for instruction in analysis.trace().instructions() {
            eprintln!("instruction {instruction:?}");
        }
    }
}

#[test]
fn gfx942_machine_dataflow_is_loop_aware_bounded_and_inert() {
    let (request, effects, trace_bytes, _) = loop_trace_fixture();
    let trace =
        PhysicalMachineTraceEvidenceV1::decode_canonical_for(&request, &effects, &trace_bytes)
            .unwrap();
    let dataflow = Gfx942MachineDataflowV1::derive(&trace).unwrap();

    assert_eq!(dataflow.trace_identity(), trace.identity());
    assert_eq!(
        dataflow.function_symbols().collect::<Vec<_>>(),
        ["loop_entry"]
    );
    assert!(dataflow.block_dominates("loop_entry", 0, 3).unwrap());
    assert!(dataflow.block_dominates("loop_entry", 1, 2).unwrap());
    assert!(!dataflow.block_dominates("loop_entry", 2, 3).unwrap());
    assert!(dataflow.block_post_dominates("loop_entry", 3, 1).unwrap());
    assert!(!dataflow.block_post_dominates("loop_entry", 2, 1).unwrap());
    assert!(dataflow.block_post_dominates("loop_entry", 1, 0).unwrap());
    assert!(dataflow.instruction_dominates("loop_entry", 8, 28).unwrap());
    let loops = dataflow.natural_loops("loop_entry").unwrap();
    assert_eq!(loops.len(), 1);
    assert_eq!(loops[0].header(), 1);
    assert_eq!(loops[0].latch(), 2);
    assert_eq!(loops[0].blocks(), [1, 2]);
    assert_eq!(loops[0].exits(), [(1, 3)]);
    assert_eq!(
        dataflow.block_dominates("missing", 0, 0),
        Err(
            fe2o3_kernel_analysis::Gfx942MachineDataflowErrorV1::UnknownFunction(
                "missing".to_owned(),
            )
        ),
    );
    assert_eq!(
        dataflow.block_dominates("loop_entry", 0, 4),
        Err(fe2o3_kernel_analysis::Gfx942MachineDataflowErrorV1::UnknownBlock(4)),
    );
    assert_eq!(
        dataflow.block_post_dominates("loop_entry", 4, 0),
        Err(fe2o3_kernel_analysis::Gfx942MachineDataflowErrorV1::UnknownBlock(4)),
    );
    assert_eq!(
        dataflow.instruction_dominates("loop_entry", 8, 36),
        Err(fe2o3_kernel_analysis::Gfx942MachineDataflowErrorV1::UnknownInstruction(36)),
    );

    assert_eq!(
        dataflow
            .reaching_definitions_before("loop_entry", 12, Gfx942RegisterUnitV1::Sgpr(4))
            .unwrap(),
        [Gfx942ReachingDefinitionV1::Instruction { offset: 8 }],
    );
    assert_eq!(
        dataflow
            .reaching_definitions_before("loop_entry", 12, Gfx942RegisterUnitV1::Vgpr(2))
            .unwrap(),
        [Gfx942ReachingDefinitionV1::LiveIn],
    );
    assert_eq!(
        dataflow
            .reaching_definitions_before("loop_entry", 12, Gfx942RegisterUnitV1::Vgpr(0))
            .unwrap(),
        [
            Gfx942ReachingDefinitionV1::LiveIn,
            Gfx942ReachingDefinitionV1::Instruction { offset: 12 },
        ],
        "the loop-header use sees both the live-in and prior-iteration definition",
    );
    assert!(!dataflow.establishes_machine_semantics());
    assert!(!dataflow.establishes_compiler_refinement());
    assert!(!dataflow.grants_launch_authority());
}

#[test]
fn gfx942_register_aliases_are_atomic_contiguous_and_bounded() {
    let wide = Gfx942RegisterAliasV1::decode("SGPR4_SGPR5_SGPR6_SGPR7").unwrap();
    assert_eq!(
        wide.units(),
        &[
            Gfx942RegisterUnitV1::Sgpr(4),
            Gfx942RegisterUnitV1::Sgpr(5),
            Gfx942RegisterUnitV1::Sgpr(6),
            Gfx942RegisterUnitV1::Sgpr(7),
        ]
    );
    assert!(wide.overlaps(&Gfx942RegisterAliasV1::decode("SGPR7").unwrap()));
    assert!(!wide.overlaps(&Gfx942RegisterAliasV1::decode("VGPR7").unwrap()));
    assert_eq!(
        Gfx942RegisterAliasV1::decode("EXEC").unwrap().units(),
        &[
            Gfx942RegisterUnitV1::ExecLow,
            Gfx942RegisterUnitV1::ExecHigh
        ]
    );
    assert_eq!(
        Gfx942RegisterAliasV1::decode("VCC_LO").unwrap().units(),
        &[Gfx942RegisterUnitV1::VccLow]
    );

    for (name, expected) in [
        (
            "AGPR0",
            Gfx942RegisterFactsErrorV1::UnsupportedRegister("AGPR0".to_owned()),
        ),
        (
            "SGPR01",
            Gfx942RegisterFactsErrorV1::NonCanonicalRegister("SGPR01".to_owned()),
        ),
        (
            "SGPR3_SGPR5",
            Gfx942RegisterFactsErrorV1::NonContiguousRegisterAlias("SGPR3_SGPR5".to_owned()),
        ),
        (
            "VGPR0_SGPR1",
            Gfx942RegisterFactsErrorV1::MixedRegisterAlias("VGPR0_SGPR1".to_owned()),
        ),
        (
            "VGPR256",
            Gfx942RegisterFactsErrorV1::RegisterIndexOutOfRange {
                name: "VGPR256".to_owned(),
                index: 256,
                maximum: 255,
            },
        ),
    ] {
        assert_eq!(Gfx942RegisterAliasV1::decode(name), Err(expected));
    }
}

#[test]
fn canonical_record_binds_exact_payload_worker_target_graph_and_effects() {
    let request = request();
    let bytes = evidence(&request, &[entry_function()], &effects());
    let decoded = PhysicalMachineEffectEvidenceV1::decode_canonical_for(&request, &bytes).unwrap();

    assert_eq!(decoded.schema_version(), 1);
    assert_eq!(decoded.execution_challenge(), request.execution_challenge());
    assert_eq!(
        decoded.target(),
        PhysicalMachineTargetV1::Gfx942XnackMinusCov6
    );
    assert_eq!(
        decoded.analysis_basis(),
        PhysicalMachineEffectAnalysisBasisV1::FinalizedHsacoViaMeasuredLlvmObjectMc
    );
    assert_eq!(decoded.request_identity(), request.identity());
    assert_eq!(decoded.payload_identity(), request.payload_identity());
    assert_eq!(decoded.entry_points()[0].symbol(), "arbitrary_entry");
    assert_eq!(
        decoded.entry_points()[0].descriptor_identity().as_bytes(),
        [0x33; 32]
    );
    assert_eq!(decoded.entry_points()[0].code_offset(), CODE_OFFSET);
    assert_eq!(decoded.entry_points()[0].code_size(), CODE_SIZE);
    assert_eq!(decoded.functions()[0].symbol(), "arbitrary_entry");
    assert!(decoded.functions()[0].direct_callees().is_empty());
    assert_eq!(
        decoded.effects()[1].kind(),
        PhysicalMachineEffectKindV1::GlobalRead
    );
    assert_eq!(decoded.effects()[1].byte_width(), 4);
    assert!(decoded.is_derived_from_exact_payload());
    assert!(!decoded.authenticates_analyzer());
    assert!(!decoded.establishes_compiler_refinement());
    assert!(!decoded.grants_load_authority());
    assert!(!decoded.grants_launch_authority());
    assert_eq!(decoded.canonical_bytes(), bytes);
    assert_eq!(decoded.identity().byte_len(), bytes.len() as u64);
}

#[test]
fn request_and_evidence_are_deterministic_golden_records() {
    let first = request_with(
        b"golden-payload",
        vec![
            entry("_secondary.entry$1", budget()),
            entry("arbitrary_entry", budget()),
        ],
    );
    let second = request_with(
        b"golden-payload",
        vec![
            entry("arbitrary_entry", budget()),
            entry("_secondary.entry$1", budget()),
        ],
    );
    assert_eq!(first.canonical_bytes(), second.canonical_bytes());
    assert_eq!(first.identity(), second.identity());

    let bytes = evidence(&request(), &[entry_function()], &effects());
    let first = PhysicalMachineEffectEvidenceV1::decode_canonical_for(&request(), &bytes).unwrap();
    let second = PhysicalMachineEffectEvidenceV1::decode_canonical_for(&request(), &bytes).unwrap();
    assert_eq!(first.identity(), second.identity());
    assert_eq!(
        first.identity().sha256(),
        [
            0x0d, 0x67, 0x6d, 0x09, 0x8f, 0xb7, 0x97, 0xf3, 0xf3, 0xd2, 0x8e, 0x6c, 0x73, 0xd9,
            0x15, 0x8d, 0xcf, 0x89, 0x3d, 0xe5, 0x6a, 0x12, 0xae, 0xf8, 0x1f, 0x58, 0xb2, 0x68,
            0xc6, 0xe8, 0xd1, 0xc4,
        ]
    );
}

#[test]
fn canonical_request_reopens_offline_and_rejects_substitution() {
    let request = request_with(
        b"offline-retained-payload",
        vec![entry("alpha", budget()), entry("omega", budget())],
    );
    let decoded =
        PhysicalMachineEffectRequestV1::decode_canonical(request.canonical_bytes()).unwrap();
    assert_eq!(decoded, request);
    assert_eq!(decoded.canonical_bytes(), request.canonical_bytes());

    let mut wrong_domain = request.canonical_bytes().to_vec();
    wrong_domain[0] ^= 1;
    assert_eq!(
        PhysicalMachineEffectRequestV1::decode_canonical(&wrong_domain),
        Err(PhysicalMachineEffectRequestErrorV1::InvalidDomain)
    );

    let mut wrong_version = request.canonical_bytes().to_vec();
    let version_offset = PHYSICAL_MACHINE_EFFECT_REQUEST_DOMAIN_V1.len() + 4;
    wrong_version[version_offset..version_offset + 2].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        PhysicalMachineEffectRequestV1::decode_canonical(&wrong_version),
        Err(PhysicalMachineEffectRequestErrorV1::UnsupportedVersion)
    );

    let mut wrong_payload_identity = request.canonical_bytes().to_vec();
    let payload_identity_offset = PHYSICAL_MACHINE_EFFECT_REQUEST_DOMAIN_V1.len() + 4 + 2 + 32 * 3;
    wrong_payload_identity[payload_identity_offset] ^= 1;
    assert_eq!(
        PhysicalMachineEffectRequestV1::decode_canonical(&wrong_payload_identity),
        Err(PhysicalMachineEffectRequestErrorV1::PayloadIdentityMismatch)
    );

    let mut reordered = request.canonical_bytes().to_vec();
    let first_entry = PHYSICAL_MACHINE_EFFECT_REQUEST_DOMAIN_V1.len() + 4 + 2 + 32 * 4 + 8 + 2;
    let entry_bytes = 2 + 5 + 5 * 4;
    let entries = reordered[first_entry..first_entry + entry_bytes * 2].to_vec();
    reordered[first_entry..first_entry + entry_bytes].copy_from_slice(&entries[entry_bytes..]);
    reordered[first_entry + entry_bytes..first_entry + entry_bytes * 2]
        .copy_from_slice(&entries[..entry_bytes]);
    assert_eq!(
        PhysicalMachineEffectRequestV1::decode_canonical(&reordered),
        Err(PhysicalMachineEffectRequestErrorV1::NonCanonicalEncoding)
    );

    let mut truncated = request.canonical_bytes().to_vec();
    truncated.pop();
    assert_eq!(
        PhysicalMachineEffectRequestV1::decode_canonical(&truncated),
        Err(PhysicalMachineEffectRequestErrorV1::LengthMismatch)
    );
}

#[test]
fn payload_mutation_cannot_reuse_evidence() {
    let original = request();
    let bytes = evidence(&original, &[entry_function()], &effects());
    let mutated = request_with(
        b"exact finalized gfx942 hsacp",
        vec![entry("arbitrary_entry", budget())],
    );
    assert_eq!(
        PhysicalMachineEffectEvidenceV1::decode_canonical_for(&mutated, &bytes),
        Err(PhysicalMachineEffectEvidenceErrorV1::RequestIdentityMismatch)
    );
}

#[test]
fn symbol_and_identity_substitution_fail_closed() {
    let request = request();
    let mut bytes = evidence(&request, &[entry_function()], &effects());
    let entry_offset = PHYSICAL_MACHINE_EFFECT_EVIDENCE_DOMAIN_V1.len()
        + 4
        + 2
        + 32
        + 32
        + 8
        + 32
        + 8
        + 32
        + 32
        + 2
        + 2
        + 2;
    bytes[entry_offset] = b'z';
    assert_eq!(
        PhysicalMachineEffectEvidenceV1::decode_canonical_for(&request, &bytes),
        Err(PhysicalMachineEffectEvidenceErrorV1::EntrySetMismatch)
    );

    let mut bytes = evidence(&request, &[entry_function()], &effects());
    let analyzer_offset =
        PHYSICAL_MACHINE_EFFECT_EVIDENCE_DOMAIN_V1.len() + 4 + 2 + 32 + 32 + 8 + 32 + 8;
    bytes[analyzer_offset] ^= 1;
    assert_eq!(
        PhysicalMachineEffectEvidenceV1::decode_canonical_for(&request, &bytes),
        Err(PhysicalMachineEffectEvidenceErrorV1::AnalyzerIdentityMismatch)
    );
}

#[test]
fn open_call_edge_and_effect_expansion_fail_closed() {
    let request = request();
    let open = Function {
        symbol: "arbitrary_entry",
        offset: CODE_OFFSET,
        size: CODE_SIZE,
        callees: vec!["missing_helper"],
    };
    assert_eq!(
        PhysicalMachineEffectEvidenceV1::decode_canonical_for(
            &request,
            &evidence(&request, &[open], &effects())
        ),
        Err(PhysicalMachineEffectEvidenceErrorV1::OpenCallGraph)
    );

    let tight = request_with(
        b"exact finalized gfx942 hsaco",
        vec![entry(
            "arbitrary_entry",
            PhysicalMachineEffectBudgetV1::new(8, 0, 4, 2, 2),
        )],
    );
    assert_eq!(
        PhysicalMachineEffectEvidenceV1::decode_canonical_for(
            &tight,
            &evidence(&tight, &[entry_function()], &effects())
        ),
        Err(PhysicalMachineEffectEvidenceErrorV1::EffectExpansion)
    );
}

#[test]
fn code_ranges_may_end_exactly_at_the_payload_boundary() {
    let payload = [0u8; 32];
    let request = request_with(&payload, vec![entry("arbitrary_entry", budget())]);
    let function = Function {
        symbol: "arbitrary_entry",
        offset: 16,
        size: 16,
        callees: Vec::new(),
    };
    let effects = [
        Effect {
            entry: "arbitrary_entry",
            function: "arbitrary_entry",
            offset: 16,
            kind: 4,
            width: 0,
        },
        Effect {
            entry: "arbitrary_entry",
            function: "arbitrary_entry",
            offset: 31,
            kind: 4,
            width: 0,
        },
    ];
    let bytes = evidence_with_entry_range(&request, 16, 16, &[function], &effects);

    let decoded = PhysicalMachineEffectEvidenceV1::decode_canonical_for(&request, &bytes).unwrap();
    assert_eq!(decoded.entry_points()[0].code_offset(), 16);
    assert_eq!(decoded.entry_points()[0].code_size(), 16);
    assert_eq!(decoded.effects()[0].instruction_offset(), 16);
    assert_eq!(decoded.effects()[1].instruction_offset(), 31);
}

#[test]
fn invalid_entry_ranges_fail_closed_without_panicking() {
    let payload = [0u8; 32];
    let request = request_with(&payload, vec![entry("arbitrary_entry", budget())]);

    for (offset, size) in [(32, 1), (31, 2), (u64::MAX, 1), (0, 0)] {
        let bytes =
            evidence_with_entry_range(&request, offset, size, &[entry_function()], &effects());
        let result = std::panic::catch_unwind(|| {
            PhysicalMachineEffectEvidenceV1::decode_canonical_for(&request, &bytes)
        });
        assert!(result.is_ok(), "entry range ({offset}, {size}) panicked");
        assert_eq!(
            result.unwrap(),
            Err(PhysicalMachineEffectEvidenceErrorV1::InvalidFunctionRange),
            "entry range ({offset}, {size}) was not rejected"
        );
    }
}

#[test]
fn invalid_function_ranges_fail_closed_without_panicking() {
    let payload = [0u8; 32];
    let request = request_with(&payload, vec![entry("arbitrary_entry", budget())]);

    for (offset, size) in [(32, 1), (31, 2), (u64::MAX, 1), (0, 0)] {
        let function = Function {
            symbol: "arbitrary_entry",
            offset,
            size,
            callees: Vec::new(),
        };
        let bytes = evidence_with_entry_range(&request, 16, 16, &[function], &[]);
        let result = std::panic::catch_unwind(|| {
            PhysicalMachineEffectEvidenceV1::decode_canonical_for(&request, &bytes)
        });
        assert!(result.is_ok(), "function range ({offset}, {size}) panicked");
        assert_eq!(
            result.unwrap(),
            Err(PhysicalMachineEffectEvidenceErrorV1::InvalidFunctionRange),
            "function range ({offset}, {size}) was not rejected"
        );
    }
}

#[test]
fn instruction_offsets_outside_the_function_fail_closed() {
    let payload = [0u8; 32];
    let request = request_with(&payload, vec![entry("arbitrary_entry", budget())]);
    let function = Function {
        symbol: "arbitrary_entry",
        offset: 16,
        size: 16,
        callees: Vec::new(),
    };

    for offset in [15, 32, u64::MAX] {
        let effects = [Effect {
            entry: "arbitrary_entry",
            function: "arbitrary_entry",
            offset,
            kind: 4,
            width: 0,
        }];
        let bytes =
            evidence_with_entry_range(&request, 16, 16, std::slice::from_ref(&function), &effects);
        let result = std::panic::catch_unwind(|| {
            PhysicalMachineEffectEvidenceV1::decode_canonical_for(&request, &bytes)
        });
        assert!(result.is_ok(), "instruction offset {offset} panicked");
        assert_eq!(
            result.unwrap(),
            Err(PhysicalMachineEffectEvidenceErrorV1::EffectOutsideClosure),
            "instruction offset {offset} was not rejected"
        );
    }
}

#[test]
fn entry_symbols_are_workload_neutral_and_bounded() {
    for valid in [
        "other",
        "_private_entry7",
        ".hidden.entry",
        "$generated$entry",
    ] {
        assert_eq!(
            PhysicalMachineEffectEntryRequestV1::new(valid, budget())
                .unwrap()
                .symbol(),
            valid
        );
    }

    let too_long = "k".repeat(257);
    for invalid in [
        "",
        "7leading_digit",
        "contains/slash",
        "non_ascii_\u{e9}",
        &too_long,
    ] {
        assert_eq!(
            PhysicalMachineEffectEntryRequestV1::new(invalid, budget()),
            Err(PhysicalMachineEffectRequestErrorV1::InvalidEntrySymbol {
                byte_len: invalid.len(),
            })
        );
    }
}

#[test]
fn request_rejects_reserved_identities() {
    assert_eq!(
        PhysicalMachineEffectRequestV1::new(
            PhysicalMachineExecutionChallengeV1::from_sha256_bytes([1; 32]),
            PhysicalMachineAnalyzerIdentityV1::from_sha256_bytes([0; 32]),
            PhysicalMachineToolchainIdentityV1::from_sha256_bytes([2; 32]),
            vec![1],
            vec![entry("arbitrary_entry", budget())],
        ),
        Err(PhysicalMachineEffectRequestErrorV1::ZeroIdentity(
            "analyzer"
        ))
    );
    assert_eq!(
        PhysicalMachineEffectRequestV1::new(
            PhysicalMachineExecutionChallengeV1::from_sha256_bytes([0; 32]),
            PhysicalMachineAnalyzerIdentityV1::from_sha256_bytes([1; 32]),
            PhysicalMachineToolchainIdentityV1::from_sha256_bytes([2; 32]),
            vec![1],
            vec![entry("arbitrary_entry", budget())],
        ),
        Err(PhysicalMachineEffectRequestErrorV1::ZeroIdentity(
            "execution challenge"
        ))
    );
}

#[test]
fn machine_trace_binds_exact_bytes_def_use_effects_and_loop_cfg() {
    let (request, effects, bytes, _) = loop_trace_fixture();
    let trace =
        PhysicalMachineTraceEvidenceV1::decode_canonical_for(&request, &effects, &bytes).unwrap();

    assert_eq!(trace.blocks().len(), 4);
    assert_eq!(trace.instructions().len(), 7);
    assert_eq!(trace.blocks()[2].successors(), &[1]);
    assert_eq!(trace.instructions()[4].branch_target(), Some(12));
    assert_eq!(trace.instructions()[1].opcode(), "GLOBAL_LOAD_DWORD_vi");
    assert_eq!(
        trace.instructions()[1].encoding(),
        &[0x20, 0x21, 0x22, 0x23]
    );
    assert_eq!(trace.instructions()[1].explicit_definition_count(), 1);
    assert_eq!(trace.instructions()[1].implicit_uses(), &["EXEC"]);
    assert!(trace.instructions()[1].flags().may_load());
    assert!(trace.instructions()[3].flags().may_trap());
    let load = Gfx942InstructionRegisterFactsV1::derive(&trace.instructions()[1]).unwrap();
    assert_eq!(load.definitions(), &[Gfx942RegisterUnitV1::Vgpr(0)]);
    assert_eq!(
        load.uses(),
        &[
            Gfx942RegisterUnitV1::Vgpr(2),
            Gfx942RegisterUnitV1::Vgpr(3),
            Gfx942RegisterUnitV1::ExecLow,
            Gfx942RegisterUnitV1::ExecHigh,
        ]
    );
    assert!(!load.establishes_machine_semantics());
    assert!(!load.establishes_compiler_refinement());
    assert!(!load.grants_launch_authority());
    assert_eq!(trace.identity().byte_len(), bytes.len() as u64);
    assert!(trace.binds_exact_payload_instruction_bytes());
    assert!(!trace.establishes_machine_semantics());
    assert!(!trace.establishes_compiler_refinement());
    assert!(!trace.grants_load_authority());
    assert!(!trace.grants_launch_authority());
}

#[test]
fn machine_trace_rejects_instruction_byte_substitution() {
    let (request, effects, mut bytes, offsets) = loop_trace_fixture();
    bytes[offsets.first_encoding] ^= 1;
    assert_eq!(
        PhysicalMachineTraceEvidenceV1::decode_canonical_for(&request, &effects, &bytes),
        Err(PhysicalMachineTraceEvidenceErrorV1::InstructionBytesMismatch)
    );
}

#[test]
fn machine_trace_rejects_trap_with_memory_flags() {
    let (request, effects, mut bytes, offsets) = loop_trace_fixture();
    bytes[offsets.trap_flags..offsets.trap_flags + 2].copy_from_slice(&(32_u16 | 1).to_le_bytes());
    assert_eq!(
        PhysicalMachineTraceEvidenceV1::decode_canonical_for(&request, &effects, &bytes),
        Err(PhysicalMachineTraceEvidenceErrorV1::InvalidMemoryAccess)
    );
}

#[test]
fn machine_trace_rejects_branch_target_and_edge_substitution() {
    let (request, effects, bytes, offsets) = loop_trace_fixture();

    let mut changed_target = bytes.clone();
    changed_target[offsets.conditional_target..offsets.conditional_target + 8]
        .copy_from_slice(&14_u64.to_le_bytes());
    assert_eq!(
        PhysicalMachineTraceEvidenceV1::decode_canonical_for(&request, &effects, &changed_target,),
        Err(PhysicalMachineTraceEvidenceErrorV1::InvalidControlFlow)
    );

    let mut changed_edge = bytes;
    changed_edge[offsets.conditional_successor..offsets.conditional_successor + 4]
        .copy_from_slice(&1_u32.to_le_bytes());
    assert_eq!(
        PhysicalMachineTraceEvidenceV1::decode_canonical_for(&request, &effects, &changed_edge),
        Err(PhysicalMachineTraceEvidenceErrorV1::InvalidControlFlow)
    );
}

#[test]
fn machine_trace_rejects_substituted_effect_evidence_identity() {
    let (request, effects, mut bytes, _) = loop_trace_fixture();
    let effect_identity_offset =
        PHYSICAL_MACHINE_TRACE_EVIDENCE_DOMAIN_V1.len() + 4 + 2 + 32 + 32 + 8;
    bytes[effect_identity_offset] ^= 1;
    assert_eq!(
        PhysicalMachineTraceEvidenceV1::decode_canonical_for(&request, &effects, &bytes),
        Err(PhysicalMachineTraceEvidenceErrorV1::IdentityMismatch(
            "machine-effect evidence"
        ))
    );
}

#[test]
fn machine_trace_rejects_missing_effect_site() {
    let (request, _, mut trace_bytes, _) = loop_trace_fixture();
    let function = Function {
        symbol: "loop_entry",
        offset: 8,
        size: 32,
        callees: Vec::new(),
    };
    let incomplete_effects = [
        Effect {
            entry: "loop_entry",
            function: "loop_entry",
            offset: 12,
            kind: 1,
            width: 8,
        },
        Effect {
            entry: "loop_entry",
            function: "loop_entry",
            offset: 28,
            kind: 1,
            width: 8,
        },
        Effect {
            entry: "loop_entry",
            function: "loop_entry",
            offset: 28,
            kind: 3,
            width: 4,
        },
        Effect {
            entry: "loop_entry",
            function: "loop_entry",
            offset: 32,
            kind: 4,
            width: 0,
        },
    ];
    let effect_bytes = evidence_with_entry_range(
        &request,
        8,
        32,
        std::slice::from_ref(&function),
        &incomplete_effects,
    );
    let incomplete =
        PhysicalMachineEffectEvidenceV1::decode_canonical_for(&request, &effect_bytes).unwrap();
    let effect_identity_offset =
        PHYSICAL_MACHINE_TRACE_EVIDENCE_DOMAIN_V1.len() + 4 + 2 + 32 + 32 + 8;
    trace_bytes[effect_identity_offset..effect_identity_offset + 32]
        .copy_from_slice(&incomplete.identity().sha256());
    trace_bytes[effect_identity_offset + 32..effect_identity_offset + 40]
        .copy_from_slice(&incomplete.identity().byte_len().to_le_bytes());

    assert_eq!(
        PhysicalMachineTraceEvidenceV1::decode_canonical_for(&request, &incomplete, &trace_bytes,),
        Err(PhysicalMachineTraceEvidenceErrorV1::EffectTraceMismatch)
    );
}

#[test]
fn machine_analysis_bundle_keeps_effects_and_trace_indivisible() {
    let (request, effects, trace_bytes, _) = loop_trace_fixture();
    let bytes = encode_analysis_bundle(effects.canonical_bytes(), &trace_bytes);
    let analysis = PhysicalMachineAnalysisEvidenceV1::decode_canonical_for(&request, &bytes)
        .expect("valid machine analysis bundle");

    assert_eq!(analysis.effects().identity(), effects.identity());
    assert_eq!(analysis.trace().canonical_bytes(), trace_bytes);
    assert_eq!(analysis.identity().byte_len(), bytes.len() as u64);
    assert!(analysis.binds_exact_payload_instruction_bytes());
    assert!(!analysis.establishes_machine_semantics());
    assert!(!analysis.establishes_compiler_refinement());
    assert!(!analysis.grants_load_authority());
    assert!(!analysis.grants_launch_authority());
}

#[test]
fn machine_analysis_bundle_rejects_component_omission_and_trace_mutation() {
    let (request, effects, trace_bytes, offsets) = loop_trace_fixture();
    let bytes = encode_analysis_bundle(effects.canonical_bytes(), &trace_bytes);
    let component_length_offset = PHYSICAL_MACHINE_ANALYSIS_BUNDLE_DOMAIN_V1.len() + 4 + 2;

    let mut omitted_effects = bytes.clone();
    omitted_effects[component_length_offset..component_length_offset + 4]
        .copy_from_slice(&0_u32.to_le_bytes());
    assert_eq!(
        PhysicalMachineAnalysisEvidenceV1::decode_canonical_for(&request, &omitted_effects),
        Err(PhysicalMachineAnalysisEvidenceErrorV1::ComponentLength)
    );

    let trace_length_offset = component_length_offset + 4 + effects.canonical_bytes().len();
    let mut omitted_trace = bytes.clone();
    omitted_trace[trace_length_offset..trace_length_offset + 4]
        .copy_from_slice(&0_u32.to_le_bytes());
    assert_eq!(
        PhysicalMachineAnalysisEvidenceV1::decode_canonical_for(&request, &omitted_trace),
        Err(PhysicalMachineAnalysisEvidenceErrorV1::ComponentLength)
    );

    let trace_start = trace_length_offset + 4;
    let mut mutated_trace = bytes;
    mutated_trace[trace_start + offsets.first_encoding] ^= 1;
    assert_eq!(
        PhysicalMachineAnalysisEvidenceV1::decode_canonical_for(&request, &mutated_trace),
        Err(PhysicalMachineAnalysisEvidenceErrorV1::Trace(
            PhysicalMachineTraceEvidenceErrorV1::InstructionBytesMismatch
        ))
    );
}

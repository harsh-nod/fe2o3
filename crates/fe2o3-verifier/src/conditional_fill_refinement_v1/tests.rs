use super::*;

fn inputs() -> (
    fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3,
    crate::ValidatedConditionalCompilerProofInputsV1,
) {
    let handoff = fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3::decode(include_bytes!(
        "../conditional_fill_program_v1/fill.handoff"
    ))
    .unwrap();
    let receipts = handoff.capsule().receipts();
    let inputs = crate::validate_conditional_compiler_proof_inputs_v1(
        receipts.proof_binding(),
        receipts.semantic_mir(),
        receipts.middle_end(),
        receipts.kernel_ir(),
        receipts.mir_to_kir_correspondence(),
        receipts.formal_memory(),
    )
    .unwrap();
    (handoff, inputs)
}

#[test]
fn genuine_program_generates_closed_distinct_operand_graphs() {
    let (handoff, inputs) = inputs();
    let lineage =
        crate::validate_conditional_compiler_target_lineage_v1(handoff.capsule(), &inputs).unwrap();
    let program = crate::check_conditional_fill_program_v1(&inputs, &lineage).unwrap();
    let source = generate::source(&program.recipes, 272).unwrap();
    let text = std::str::from_utf8(source.source()).unwrap();
    for name in [
        "semantic_store",
        "neutral_store",
        "target_store",
        "execute_refining_group",
        "execute_refining_byte",
    ] {
        assert!(text.contains(&format!("fn {name}(")));
    }
    assert!(!text.contains("include!"));
    assert_ne!(
        program.recipes[0].canonical_bytes(),
        program.recipes[1].canonical_bytes()
    );
    assert!(!program.recipes[0].spans.is_empty());
    assert!(program.recipes.iter().all(|recipe| recipe.effect.is_some()));
    let small = generate::source(&program.recipes, 16).unwrap();
    assert_ne!(small.identity(), source.identity());
    assert!(generate::source(&program.recipes, 24).is_err());
    for recipe in 0..3 {
        let mut changed = program.recipes.clone();
        crate::conditional_fill_program_v1::recipe::tests::replace_value_with_zero(
            &mut changed[recipe],
        );
        assert_ne!(
            changed[recipe].canonical_bytes(),
            program.recipes[recipe].canonical_bytes()
        );
        assert_ne!(
            generate::source(&changed, 272).unwrap().identity(),
            source.identity()
        );
    }
    if let Some(path) = std::env::var_os("FE2O3_FILL_REFINEMENT_CAPTURE") {
        std::fs::write(path, source.source()).unwrap();
    }
}

#[test]
#[ignore = "requires native LLVM analyzer and protected pinned Verus runtime; no GPU required"]
fn protected_native_conditional_fill_refinement() {
    use fe2o3_kernel_analysis::check_gfx942_fill_analysis_v1;
    let (handoff, inputs) = inputs();
    let lineage =
        crate::validate_conditional_compiler_target_lineage_v1(handoff.capsule(), &inputs).unwrap();
    let program = crate::check_conditional_fill_program_v1(&inputs, &lineage).unwrap();
    let execution = native_analysis(program.function_symbol());
    let machine = check_gfx942_fill_analysis_v1(&execution, program.function_symbol()).unwrap();
    let runtime = runtime();
    let proof = execute_conditional_fill_refinement_v1(&runtime, &program, &machine, 180).unwrap();
    assert!(std::ptr::eq(proof.program(), &program));
    assert!(std::ptr::eq(proof.machine(), &machine));
    assert_eq!(proof.boundary(), BOUNDARY);
    assert!(proof.retains_strictly_imported_signed_receipt());
    assert!(!proof.grants_launch_authority());
    assert_eq!(proof.signed_receipt_wire().len(), 524);
    if let Some(directory) = std::env::var_os("FE2O3_FILL_REFINEMENT_EVIDENCE") {
        let directory = std::path::Path::new(&directory);
        for (name, bytes) in [
            ("generated.rs", proof.generated_source()),
            ("obligation.bin", proof.obligation_preimage()),
            ("refinement.receipt", proof.signed_receipt_wire()),
            ("refinement.key", proof.receipt_verifying_key().as_slice()),
            ("fill.request", execution.request().canonical_bytes()),
            ("fill.bundle", execution.analysis().canonical_bytes()),
            ("fill.receipt", execution.canonical_receipt_bytes()),
        ] {
            std::fs::write(directory.join(name), bytes).unwrap();
        }
    }
    for (name, recipes) in operand_mutants(&program.recipes) {
        let source = generate::source(&recipes, machine.kernel().kernarg_storage_bytes()).unwrap();
        assert_ne!(source.source(), proof.generated_source());
        let output = runtime.execute_generated_rust_verify(
            &source,
            std::time::Instant::now() + std::time::Duration::from_secs(180),
            1024 * 1024,
            crate::functional_refinement_runtime_v1::GeneratedVerusExecutionProfileV1::ClosedFill,
        ).unwrap();
        if let Some(directory) = std::env::var_os("FE2O3_FILL_REFINEMENT_EVIDENCE") {
            let directory = std::path::Path::new(&directory);
            for (suffix, bytes) in [
                ("rs", source.source()),
                ("stdout", output.stdout.as_slice()),
                ("stderr", output.stderr.as_slice()),
            ] {
                std::fs::write(directory.join(format!("{name}.{suffix}")), bytes).unwrap();
            }
        }
        assert_logical_store_rejection(name, source.source(), &output);
        eprintln!("conditional fill mutant {name}: expected logical rejection");
    }
    let mut changed = crate::check_conditional_fill_program_v1(&inputs, &lineage).unwrap();
    for recipe in &mut changed.recipes {
        crate::conditional_fill_program_v1::recipe::tests::alias_store_value(recipe);
    }
    let alias = execute_conditional_fill_refinement_v1(&runtime, &changed, &machine, 180).unwrap();
    assert_ne!(alias.generated_source(), proof.generated_source());
    assert_ne!(alias.binding(), proof.binding());
    assert!(alias.retains_strictly_imported_signed_receipt());
    eprintln!("conditional fill actual dependency aliases: proved with distinct binding");
    drop(alias);
    crate::conditional_fill_program_v1::recipe::tests::replace_value_with_zero(
        &mut changed.recipes[0],
    );
    let error =
        execute_conditional_fill_refinement_v1(&runtime, &changed, &machine, 180).unwrap_err();
    let ConditionalFillRefinementErrorV1::Execution(error) = error else {
        panic!("wrong rejection: {error}")
    };
    assert_eq!(
        error.kind(),
        crate::FunctionalRefinementVerusExecutionErrorKindV2::UnexpectedProofResult
    );
    eprintln!("conditional fill public producer: wrong value returned no proof owner");
}

fn native_analysis(
    symbol: &str,
) -> fe2o3_kernel_analysis::AuthenticatedPhysicalMachineAnalysisExecutionV1 {
    native_analysis_payload(symbol, native_payload())
}

fn native_payload() -> Vec<u8> {
    std::fs::read(
        std::env::var_os("FE2O3_FILL_ANALYSIS_HSACO").expect("genuine finalized fill payload"),
    )
    .unwrap()
}

fn native_analysis_payload(
    symbol: &str,
    payload: Vec<u8>,
) -> fe2o3_kernel_analysis::AuthenticatedPhysicalMachineAnalysisExecutionV1 {
    use fe2o3_kernel_analysis::{
        AuthenticatedPhysicalMachineEffectLimitsV1, AuthenticatedPhysicalMachineEffectWorkerV1,
        PhysicalMachineEffectBudgetV1, PhysicalMachineEffectEntryRequestV1,
        inspect_physical_machine_effect_worker_candidate_v1,
    };
    let path =
        std::env::var_os("FE2O3_MACHINE_EFFECT_NATIVE_WORKER").expect("native analyzer path");
    let limits = AuthenticatedPhysicalMachineEffectLimitsV1::new(
        std::time::Duration::from_secs(60),
        1024 * 1024,
        16384,
    )
    .unwrap();
    let candidate = inspect_physical_machine_effect_worker_candidate_v1(&path, limits).unwrap();
    let worker =
        AuthenticatedPhysicalMachineEffectWorkerV1::open(&path, candidate.policy(), limits)
            .unwrap();
    worker
        .verify_deployed_no_fork_profile_for_test(limits)
        .unwrap();
    worker
        .analyze(
            payload,
            vec![
                PhysicalMachineEffectEntryRequestV1::new(
                    symbol,
                    PhysicalMachineEffectBudgetV1::new(2, 1, 1, 1, 0),
                )
                .unwrap(),
            ],
            limits,
        )
        .unwrap()
}

fn runtime() -> FunctionalRefinementVerusRuntimeLeaseV1 {
    FunctionalRefinementVerusRuntimeLeaseV1::open(
        std::env::var_os("FE2O3_FUNCTIONAL_REFINEMENT_TEST_RUNTIME_ROOT")
            .expect("protected pinned runtime root"),
    )
    .unwrap()
}

#[test]
#[ignore = "requires native LLVM analyzer and protected pinned Verus runtime; no GPU required"]
fn protected_native_owned_conditional_fill_refinement() {
    let (proof, original_buffers, challenge, analysis_identity, source, obligation) = {
        let (handoff, inputs) = inputs();
        let lineage =
            crate::validate_conditional_compiler_target_lineage_v1(handoff.capsule(), &inputs)
                .unwrap();
        let program = crate::check_conditional_fill_program_v1(&inputs, &lineage).unwrap();
        let analysis = native_analysis(program.function_symbol());
        let machine = fe2o3_kernel_analysis::check_gfx942_fill_analysis_v1(
            &analysis,
            program.function_symbol(),
        )
        .unwrap();
        let source =
            generate::source(&program.recipes, machine.kernel().kernarg_storage_bytes()).unwrap();
        let obligation = obligation(&program, &machine, &source);
        let source = source.source().to_vec();
        let buffers = original_owner_buffers(&inputs, &lineage, &analysis);
        let challenge = analysis.execution_challenge();
        let identity = analysis.identity();
        let runtime = runtime();
        let proof =
            execute_owned_conditional_fill_refinement_v1(&runtime, inputs, lineage, analysis, 180)
                .unwrap();
        (proof, buffers, challenge, identity, source, obligation)
    };
    assert_eq!(
        original_buffers,
        original_owner_buffers(proof.inputs(), proof.lineage(), proof.analysis_execution(),)
    );
    assert_eq!(proof.analysis_execution().execution_challenge(), challenge);
    assert_eq!(proof.analysis_execution().identity(), analysis_identity);
    assert_eq!(proof.generated_source(), source);
    assert_eq!(proof.obligation_preimage(), obligation);
    assert_eq!(proof.boundary(), BOUNDARY);
    assert!(proof.retains_strictly_imported_signed_receipt());
    assert!(!proof.grants_launch_authority());
    assert_eq!(proof.signed_receipt_wire().len(), 524);
    let program =
        crate::check_conditional_fill_program_v1(proof.inputs(), proof.lineage()).unwrap();
    let machine = fe2o3_kernel_analysis::check_gfx942_fill_analysis_v1(
        proof.analysis_execution(),
        program.function_symbol(),
    )
    .unwrap();
    assert_eq!(
        proof.obligation_preimage(),
        super::obligation(
            &program,
            &machine,
            &generate::source(&program.recipes, machine.kernel().kernarg_storage_bytes()).unwrap(),
        )
    );
    if let Some(directory) = std::env::var_os("FE2O3_FILL_REFINEMENT_EVIDENCE") {
        let directory = std::path::Path::new(&directory);
        for (name, bytes) in [
            ("owned-generated.rs", proof.generated_source()),
            ("owned-obligation.bin", proof.obligation_preimage()),
            ("owned-refinement.receipt", proof.signed_receipt_wire()),
            (
                "owned-refinement.key",
                proof.receipt_verifying_key().as_slice(),
            ),
            (
                "owned-fill.request",
                proof.analysis_execution().request().canonical_bytes(),
            ),
            (
                "owned-fill.bundle",
                proof.analysis_execution().analysis().canonical_bytes(),
            ),
            (
                "owned-fill.receipt",
                proof.analysis_execution().canonical_receipt_bytes(),
            ),
        ] {
            std::fs::write(directory.join(name), bytes).unwrap();
        }
    }
    eprintln!(
        "owned conditional fill: original buffers, analyzer execution and proof retained after runtime scope"
    );
}

#[test]
#[ignore = "requires native LLVM analyzer and protected pinned Verus runtime; no GPU required"]
fn protected_native_owned_conditional_fill_rejects_descriptor_before_execution() {
    use fe2o3_kernel_analysis::{Gfx942FillAnalysisErrorV1, Gfx942FillErrorV1, Gfx942FillKernelV1};
    let (handoff, inputs) = inputs();
    let lineage =
        crate::validate_conditional_compiler_target_lineage_v1(handoff.capsule(), &inputs).unwrap();
    let program = crate::check_conditional_fill_program_v1(&inputs, &lineage).unwrap();
    let mut payload = native_payload();
    let offset = Gfx942FillKernelV1::inspect(&payload, 0)
        .unwrap()
        .binding()
        .descriptor_file_offset() as usize;
    payload[offset + 56..offset + 58].copy_from_slice(&0xau16.to_le_bytes());
    payload[offset + 52..offset + 56].copy_from_slice(&0x88u32.to_le_bytes());
    let analysis = native_analysis_payload(program.function_symbol(), payload);
    let error =
        execute_owned_conditional_fill_refinement_v1(&runtime(), inputs, lineage, analysis, 0)
            .unwrap_err();
    assert!(
        matches!(
            error,
            ConditionalFillRefinementErrorV1::Machine(Gfx942FillAnalysisErrorV1::Kernel(
                Gfx942FillErrorV1::EntryLayout
            ))
        ),
        "unexpected failure: {error}"
    );
    eprintln!(
        "owned conditional fill: genuine analyzed descriptor mutation rejected before proof execution"
    );
}

fn original_owner_buffers(
    inputs: &crate::ValidatedConditionalCompilerProofInputsV1,
    lineage: &crate::ValidatedCompilerTargetLineageV1,
    analysis: &fe2o3_kernel_analysis::AuthenticatedPhysicalMachineAnalysisExecutionV1,
) -> [usize; 6] {
    [
        inputs.semantic_mir().canonical_encoding().as_ptr() as usize,
        lineage
            .replay()
            .replay()
            .target_bound_kernel_ir_bytes()
            .as_ptr() as usize,
        analysis.request().exact_payload_bytes().as_ptr() as usize,
        analysis.request().canonical_bytes().as_ptr() as usize,
        analysis.analysis().canonical_bytes().as_ptr() as usize,
        analysis.canonical_receipt_bytes().as_ptr() as usize,
    ]
}

fn operand_mutants(
    recipes: &[crate::conditional_fill_program_v1::recipe::Recipe; 3],
) -> Vec<(
    &'static str,
    [crate::conditional_fill_program_v1::recipe::Recipe; 3],
)> {
    use crate::conditional_fill_program_v1::recipe::{Expr, tests::replace_value_with_zero};
    let mut mutants = Vec::new();
    for (index, name) in [
        "semantic-zero-value",
        "neutral-zero-value",
        "target-zero-value",
    ]
    .into_iter()
    .enumerate()
    {
        let mut changed = recipes.clone();
        replace_value_with_zero(&mut changed[index]);
        mutants.push((name, changed));
    }
    let mut changed = recipes.clone();
    let node = changed[1]
        .nodes
        .iter_mut()
        .find(|node| matches!(node.expression, Expr::Less(..)))
        .unwrap();
    let Expr::Less(a, b) = node.expression else {
        unreachable!()
    };
    node.expression = Expr::Less(b, a);
    mutants.push(("neutral-reversed-predicate", changed));
    let mut changed = recipes.clone();
    let zero = changed[2]
        .nodes
        .iter()
        .find_map(|node| match node.expression {
            Expr::Select(_, _, zero) => Some(zero),
            _ => None,
        })
        .unwrap();
    let node = changed[2]
        .nodes
        .iter_mut()
        .find(|node| matches!(node.expression, Expr::Offset(..)))
        .unwrap();
    let Expr::Offset(base, _) = node.expression else {
        unreachable!()
    };
    node.expression = Expr::Offset(base, zero);
    mutants.push(("target-zero-offset", changed));
    let mut changed = recipes.clone();
    for recipe in &mut changed {
        replace_value_with_zero(recipe);
    }
    mutants.push(("all-equal-wrong-value", changed));
    mutants
}

fn assert_logical_store_rejection(
    name: &str,
    source: &[u8],
    output: &crate::functional_refinement_runtime_v1::FunctionalRefinementRuntimeProcessOutputV1,
) {
    assert_eq!((output.exit_code, output.signal), (Some(1), None), "{name}");
    assert_eq!(
        output.stdout,
        b"verification results:: 48 verified, 1 errors\n",
        "{name}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let source = std::str::from_utf8(source).unwrap();
    let start = source
        .lines()
        .position(|line| line.starts_with("proof fn generated_store_relation("))
        .unwrap()
        + 1;
    let end = source
        .lines()
        .position(|line| line.starts_with("fn execute_refining_group("))
        .unwrap()
        + 1;
    let stderr = std::str::from_utf8(&output.stderr).unwrap();
    let mut errors = 0;
    let mut spans = 0;
    let mut footer = None;
    for line in stderr.lines() {
        if let Some(count) = line.strip_prefix("error: aborting due to ") {
            let count = count
                .strip_suffix(" previous errors")
                .or_else(|| count.strip_suffix(" previous error"))
                .unwrap()
                .parse::<usize>()
                .unwrap();
            assert!(footer.replace(count).is_none(), "{name}: duplicate footer");
            continue;
        }
        if line.starts_with("error") {
            assert_eq!(
                line, "error: postcondition not satisfied",
                "{name}: {stderr}"
            );
            errors += 1;
        }
        if let Some(location) = line
            .trim_start()
            .strip_prefix("--> ")
            .or_else(|| line.trim_start().strip_prefix("::: "))
        {
            let line = location
                .strip_prefix("/proc/self/fd/187:")
                .unwrap()
                .split(':')
                .next()
                .unwrap()
                .parse::<usize>()
                .unwrap();
            assert!(
                (start..end).contains(&line),
                "{name}: unrelated failure: {stderr}"
            );
            spans += 1;
        }
    }
    assert!(errors > 0 && spans >= errors, "{name}: {stderr}");
    assert_eq!(footer, Some(errors), "{name}: {stderr}");
    for rejected in [
        "warning:",
        "resource limit",
        "timed out",
        "internal error",
        "error[",
        "unsupported",
        "panic",
        "not yet supported",
    ] {
        assert!(!stderr.contains(rejected), "{name}: {stderr}");
    }
}

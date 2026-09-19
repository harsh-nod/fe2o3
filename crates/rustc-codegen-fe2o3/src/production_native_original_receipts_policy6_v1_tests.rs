//! Genuine unsigned source-owner components, never fabricated signed custody.
use super::super::final_receipts::input_association::{
    exercise_original_kernel_identity_substitution_v1, exercise_unsigned_original_identities_v1,
};
use super::*;
use fe2o3_compiler_lineage::{
    MultiRootProofRosterInputsV3, MultiRootProofRosterRootInputV3 as RootInput,
    encode_native_neutral_module_v1,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

#[derive(Clone, Copy)]
enum Fault {
    None,
    Root,
    Workgroup,
    Payload,
    Roster,
    Semantic,
    Kind,
    Order,
    Name,
}

fn wires(
    inputs: OutputInputsV1<'_>,
    ranked: &Ranked,
    reports: &OriginalNativeFormalMemoryV1<'_>,
    fault: Fault,
) -> (Vec<u8>, Vec<u8>) {
    let source = inputs.owner.source(inputs.catalog).unwrap();
    assert!(std::ptr::eq(reports.original(), source.original));
    let subject = subject(source.original, inputs.catalog).unwrap();
    let kernel = encode_native_neutral_module_v1(
        &subject,
        source.original.canonical().canonical_bytes(),
        inputs.catalog.canonical_bytes(),
    )
    .unwrap();
    let mut payloads = reports
        .kernels()
        .iter()
        .map(|report| {
            Formal::from_current_obligations(report)
                .unwrap()
                .canonical_bytes()
                .to_vec()
        })
        .collect::<Vec<_>>();
    if matches!(fault, Fault::Payload) {
        let last = payloads[0].len() - 1;
        payloads[0][last] ^= 1;
    }
    let mut rows = ranked
        .roots()
        .iter()
        .enumerate()
        .map(|(ordinal, root)| {
            let export = std::str::from_utf8(root.export_symbol()).unwrap();
            let index = source
                .original
                .module()
                .kernels
                .iter()
                .position(|k| k.id.as_str() == export)
                .unwrap();
            RootInput {
                semantic_root: root.semantic_root().index(),
                semantic_root_identity: *root.semantic_root_identity().as_bytes(),
                kernel_binding: *root.kernel_binding(),
                source_rank: root.source_rank(),
                workgroup: source.launch.roots()[ordinal]
                    .source_launch()
                    .exact_workgroup()
                    .unwrap(),
                logical_name: root.logical_name(),
                export_symbol: export,
                kernel_id: export,
                payload: &payloads[index],
            }
        })
        .collect::<Vec<_>>();
    if matches!(fault, Fault::Root) {
        rows[0].semantic_root_identity[0] ^= 1;
    }
    if matches!(fault, Fault::Workgroup) {
        rows[0].workgroup[0] = if rows[0].workgroup[0] == 1 { 2 } else { 1 };
    }
    let mut order = ranked
        .canonical_kernel_order()
        .iter()
        .map(|n| *n as u32)
        .collect::<Vec<_>>();
    if matches!(fault, Fault::Order | Fault::Name) {
        assert_eq!(rows.len(), 2);
        let [first, second] = [rows[0], rows[1]];
        if matches!(fault, Fault::Order) {
            rows[0].kernel_binding = second.kernel_binding;
            rows[1].kernel_binding = first.kernel_binding;
            order.reverse();
        } else {
            rows[0].logical_name = second.logical_name;
            rows[0].export_symbol = second.export_symbol;
            rows[0].kernel_id = second.kernel_id;
            rows[1].logical_name = first.logical_name;
            rows[1].export_symbol = first.export_symbol;
            rows[1].kernel_id = first.kernel_id;
        }
    }
    let mut roster = *ranked.canonical_roster_identity().as_bytes();
    if matches!(fault, Fault::Roster) {
        roster[0] ^= 1;
    }
    let mut semantic = *source.semantic.semantic_sha256().as_bytes();
    if matches!(fault, Fault::Semantic) {
        semantic[0] ^= 1;
    }
    let formal = Roster::new(MultiRootProofRosterInputsV3 {
        kind: if matches!(fault, Fault::Kind) {
            Kind::MiddleEnd
        } else {
            Kind::FormalMemory
        },
        semantic_mir_sha256: semantic,
        native_neutral_subject: subject,
        roster_identity: roster,
        canonical_kernel_order: &order,
        roots: &rows,
    })
    .unwrap()
    .into_canonical_bytes();
    (kernel, formal)
}

fn check_case(
    inputs: OutputInputsV1<'_>,
    ranked: &Ranked,
    typed: &[TypedDescriptorRootV1],
    kernel: &[u8],
    formal: &[u8],
    expected: Option<&'static str>,
    budget: &mut Budget<'_>,
) {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let result = check_component(
        inputs.owner,
        inputs.catalog,
        ranked,
        typed,
        kernel,
        formal,
        budget,
    );
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    match expected {
        None => result.unwrap(),
        Some(expected) => assert!(
            matches!(result, Err(Error::Mismatch(actual)) if actual == expected),
            "{result:?}"
        ),
    }
}

/// Called from every actual source configuration. It checks the same private
/// component as the signed endpoint, while leaving missing-proof refusal intact.
pub(crate) fn exercise_unsigned_component_v1(
    inputs: OutputInputsV1<'_>,
    ranked: &Ranked,
    profile: Profile,
    typed: &[TypedDescriptorRootV1],
    budget: &mut Budget<'_>,
) -> Result<()> {
    scope(budget, |budget| {
        check_output_inputs_v1(inputs, profile, typed, budget)?;
        if let OutputOwnerV1::Erased6(_) = inputs.owner {
            exercise_raw_erased_refusal_v1(inputs)?;
        }
        let reports = analyze(inputs.owner, budget)?;
        let source = inputs.owner.source(inputs.catalog)?;
        assert!(std::ptr::eq(reports.original(), source.original));
        assert_eq!(
            reports.original().canonical().identity(),
            source.original.canonical().identity()
        );
        assert!(!reports.grants_artifact_or_launch_authority());
        let (kernel, formal) = wires(inputs, ranked, &reports, Fault::None);
        let (again_kernel, again_formal) = wires(inputs, ranked, &reports, Fault::None);
        assert_eq!(kernel, again_kernel);
        assert_eq!(formal, again_formal);
        check_case(inputs, ranked, typed, &kernel, &formal, None, budget);
        // These exact bytes already passed the strict original-N component.
        // This tests identity joins only, never a signed V4 association.
        exercise_unsigned_original_identities_v1(
            source.semantic.canonical_encoding(),
            &kernel,
            &formal,
            budget,
        )
        .expect("actual unsigned original-N V4 identity components");
        for other in [source.erased, Some(inputs.owner.output())]
            .into_iter()
            .flatten()
        {
            if other.canonical().canonical_bytes() != source.original.canonical().canonical_bytes()
            {
                let changed = encode_native_neutral_module_v1(
                    &subject(other, inputs.catalog)?,
                    other.canonical().canonical_bytes(),
                    inputs.catalog.canonical_bytes(),
                )
                .unwrap();
                check_case(
                    inputs,
                    ranked,
                    typed,
                    &changed,
                    &formal,
                    Some("original KernelIr subject"),
                    budget,
                );
                exercise_original_kernel_identity_substitution_v1(&kernel, &changed, budget)
                    .expect("actual E/I cannot substitute for original-N identity");
            }
        }
        Ok(())
    })
}

/// Constructed genuine Direct two-root source adds malformed/axis/resource controls.
pub(crate) fn exercise_hostile_component_v1(
    inputs: OutputInputsV1<'_>,
    ranked: &Ranked,
    profile: Profile,
    typed: &[TypedDescriptorRootV1],
    wrong_names: &[TypedDescriptorRootV1],
    budget: &mut Budget<'_>,
) -> Result<()> {
    exercise_unsigned_component_v1(inputs, ranked, profile, typed, budget)?;
    scope(budget, |budget| {
        let reports = analyze(inputs.owner, budget)?;
        let source = inputs.owner.source(inputs.catalog)?;
        assert_eq!(ranked.root_count(), 2);
        assert!(source.erased.is_none());
        let (kernel, formal) = wires(inputs, ranked, &reports, Fault::None);
        let mut reordered = typed.to_vec();
        reordered.reverse();
        check_case(inputs, ranked, &reordered, &kernel, &formal, None, budget);
        let first_export = ranked.roots()[0].export_symbol();
        for (is_first, expected) in [
            (true, "unique original receipt root join"),
            (false, "complete original receipt root join"),
        ] {
            let repeated = typed
                .iter()
                .find(|row| (row.entry_symbol().as_bytes() == first_export) == is_first)
                .unwrap();
            let duplicate = vec![repeated.clone(); typed.len()];
            check_case(
                inputs,
                ranked,
                &duplicate,
                &kernel,
                &formal,
                Some(expected),
                budget,
            );
        }
        assert_eq!(typed.len(), wrong_names.len());
        for (expected, wrong) in typed.iter().zip(wrong_names) {
            assert_eq!(expected.entry_symbol(), wrong.entry_symbol());
            assert_eq!(
                expected.kernel_binding_bytes(),
                wrong.kernel_binding_bytes()
            );
            assert_ne!(expected.logical_name(), wrong.logical_name());
        }
        check_case(
            inputs,
            ranked,
            wrong_names,
            &kernel,
            &formal,
            Some("exact original receipt root axes"),
            budget,
        );
        let OutputOwnerV1::Direct6(direct) = inputs.owner else {
            panic!("genuine Direct6 hostile fixture");
        };
        for wrong_digest in [
            *direct
                .source_semantic_kir()
                .canonical_kernel_ir_identity()
                .digest(),
            <[u8; 32]>::from(Sha256::digest(
                source.original.canonical().canonical_bytes(),
            )),
        ] {
            assert_ne!(
                &wrong_digest,
                source.original.canonical().identity().digest()
            );
            let wrong = Subject::new(
                wrong_digest,
                source.original.canonical().identity().canonical_length(),
                *inputs.catalog.digest(),
                inputs.catalog.canonical_bytes().len() as u64,
            )
            .unwrap();
            let wrong = encode_native_neutral_module_v1(
                &wrong,
                source.original.canonical().canonical_bytes(),
                inputs.catalog.canonical_bytes(),
            )
            .unwrap();
            check_case(
                inputs,
                ranked,
                typed,
                &wrong,
                &formal,
                Some("original KernelIr subject"),
                budget,
            );
        }
        for (index, expected) in [
            (112, "original KernelIr graph bytes"),
            (
                112 + source.original.canonical().canonical_bytes().len(),
                "original KernelIr catalog bytes",
            ),
        ] {
            let mut changed = kernel.clone();
            changed[index] ^= 1;
            check_case(
                inputs,
                ranked,
                typed,
                &changed,
                &formal,
                Some(expected),
                budget,
            );
        }
        for (fault, expected) in [
            (Fault::Root, "exact original receipt root axes"),
            (Fault::Workgroup, "exact original receipt root axes"),
            (Fault::Payload, "fresh original-N formal payload"),
            (Fault::Roster, "complete original FormalMemory roster"),
            (Fault::Semantic, "complete original FormalMemory roster"),
            (Fault::Kind, "complete original FormalMemory roster"),
            (Fault::Order, "original receipt descriptor-canonical order"),
            (Fault::Name, "exact original receipt root axes"),
        ] {
            let (kernel, formal) = wires(inputs, ranked, &reports, fault);
            assert_eq!(Roster::decode(&formal).unwrap().root_count(), 2);
            check_case(
                inputs,
                ranked,
                typed,
                &kernel,
                &formal,
                Some(expected),
                budget,
            );
        }
        check_case(
            inputs,
            ranked,
            typed,
            b"",
            &formal,
            Some("original receipt preimage extent"),
            budget,
        );
        check_case(
            inputs,
            ranked,
            typed,
            &kernel,
            b"",
            Some("original receipt preimage extent"),
            budget,
        );
        let oversized_bytes = MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3 + 1;
        budget.reserve_storage(oversized_bytes)?;
        let oversized = vec![0; oversized_bytes];
        check_case(
            inputs,
            ranked,
            typed,
            &oversized,
            &formal,
            Some("original receipt preimage extent"),
            budget,
        );
        drop(oversized);
        budget.release_storage(oversized_bytes)?;
        let floor = budget.storage();
        // Independent component invocations, never replacement of a live ledger.
        let run = |work_limit, storage_limit| {
            let mut work = Work::new(work_limit);
            let mut attempt = Budget::new(&mut work, storage_limit);
            attempt.reserve_storage(floor).unwrap();
            let result = check_component(
                inputs.owner,
                inputs.catalog,
                ranked,
                typed,
                &kernel,
                &formal,
                &mut attempt,
            );
            assert_eq!(attempt.storage(), floor);
            (result, attempt.work(), attempt.peak_storage())
        };
        let (result, work, peak) = run(100_000_000, budget.storage_limit());
        result.unwrap();
        run(work, peak).0.unwrap();
        assert!(run(work - 1, peak).0.is_err());
        assert!(run(work, peak - 1).0.is_err());
        Ok(())
    })
}

fn exact_call_refusal(
    reasons: &[fe2o3_kernel_ir::FormalMemoryIncompleteReason],
    original: &Graph,
    erased: &Graph,
    kernel: &fe2o3_kernel_ir::Kernel,
) {
    assert!(!reasons.is_empty());
    let body = original
        .module()
        .function(&kernel.entry)
        .unwrap()
        .body
        .as_ref()
        .unwrap();
    for reason in reasons.iter() {
        let fe2o3_kernel_ir::FormalMemoryIncompleteReason::CallEffectsUnavailable {
            location,
            callee,
        } = reason
        else {
            panic!("unexpected original-N formal gap: {reason:?}");
        };
        let block = body
            .blocks
            .iter()
            .find(|block| block.id == location.block)
            .unwrap();
        assert!(matches!(&block.operations[location.operation_index].kind,
            fe2o3_kernel_ir::OperationKind::Call { callee: actual, .. } if actual == callee));
        assert!(original.module().function(callee).unwrap().body.is_some());
        assert!(erased.module().function(callee).is_none());
    }
}

/// The raw analyzer still refuses real private calls. Only the typed source
/// adapter may discharge them; successful E reports remain no substitute for N.
fn exercise_raw_erased_refusal_v1(inputs: OutputInputsV1<'_>) -> Result<()> {
    let source = inputs.owner.source(inputs.catalog)?;
    let erased = source.erased.unwrap();
    assert_ne!(
        source.original.canonical().canonical_bytes(),
        erased.canonical().canonical_bytes()
    );
    for kernel in &source.original.module().kernels {
        let mut extents = [1; 3];
        for (axis, extent) in kernel.domain.extents().enumerate() {
            extents[axis] = match extent {
                fe2o3_kernel_ir::LaunchExtent::Static(value) => u64::from(value),
                fe2o3_kernel_ir::LaunchExtent::Dynamic => 2,
            };
        }
        let fe2o3_kernel_ir::FormalMemoryObligationAnalysis::Incomplete { reasons, .. } =
            fe2o3_kernel_ir::derive_kernel_memory_obligations_for_launch(
                source.original.module(),
                &kernel.id,
                fe2o3_kernel_ir::ExplicitLaunchExtent::Exact {
                    rank: kernel.domain.rank(),
                    extents,
                },
                fe2o3_kernel_ir::FormalIndexWidth::Bits64,
            )
            .unwrap()
        else {
            panic!("raw UnitLocal N must retain its real call-effect gap")
        };
        exact_call_refusal(&reasons, source.original, erased, kernel);
    }
    Ok(())
}

/// Direct constructed source verifies the actual connected N pointer and keeps
/// the report owner live only while its exact source borrow is live.
pub(crate) fn exercise_direct_analysis_v1(
    owner: OutputOwnerV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<()> {
    scope(budget, |budget| {
        let OutputOwnerV1::Direct6(direct) = owner else {
            panic!("actual Direct6 fixture");
        };
        let reports = analyze(owner, budget)?;
        let original = direct
            .source_semantic_kir()
            .pre_ranked_executable()
            .unwrap();
        assert!(std::ptr::eq(reports.original(), original));
        assert_eq!(reports.kernels().len(), original.module().kernels.len());
        assert!(!reports.grants_artifact_or_launch_authority());
        let floor = direct
            .source_semantic_kir()
            .pre_ranked_retained_analysis_storage_v1()
            .unwrap();
        assert!(floor > 0);
        let mut work = Work::new(100_000_000);
        let mut short = Budget::new(&mut work, budget.storage_limit());
        short.reserve_storage(floor - 1)?;
        assert!(matches!(
            analyze_original_native_formal_memory_v1(direct.source_semantic_kir(), &mut short),
            Err(OriginalNativeFormalMemoryErrorV1::Resource(
                Resource::Accounting
            ))
        ));
        assert_eq!(short.storage(), floor - 1);
        Ok(())
    })
}

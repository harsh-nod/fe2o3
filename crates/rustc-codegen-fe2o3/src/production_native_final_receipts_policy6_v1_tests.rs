//! Component fixtures are unsigned. They cannot construct the signed native
//! owner required by the consuming endpoint or grant publication authority.
use super::*;
use fe2o3_compiler_lineage::{
    MultiRootProofRosterInputsV3, MultiRootProofRosterKindV3 as Kind,
    MultiRootProofRosterRootInputV3 as RootInput, MultiRootProofRosterTranscriptV3 as Roster,
    encode_native_neutral_module_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrWorkBudgetV1 as Work, InertFormalMemoryReceiptFormatV4 as Formal,
};

#[derive(Clone, Copy)]
enum Fault {
    None,
    RootIdentity,
    Workgroup,
    Payload,
    RosterIdentity,
    Kind,
    DescriptorPermutation,
    RootIdentityPermutation,
    RootNamePermutation,
    FormalPayloadPermutation,
}

fn wires(inputs: OutputInputsV1<'_>, ranked: &Ranked, fault: Fault) -> (Vec<u8>, Vec<u8>) {
    let owner = inputs.owner.output();
    let subject = joins::subject(owner, inputs.catalog).unwrap();
    let kernel = encode_native_neutral_module_v1(
        &subject,
        owner.canonical().canonical_bytes(),
        inputs.catalog.canonical_bytes(),
    )
    .unwrap();
    let reports = joins::reports(inputs.owner).unwrap();
    let mut payloads = reports
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
    let source = inputs.owner.source(inputs.catalog).unwrap();
    let mut rows = ranked
        .roots()
        .iter()
        .enumerate()
        .map(|(ordinal, root)| {
            let export = std::str::from_utf8(root.export_symbol()).unwrap();
            let output_index = owner
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
                payload: &payloads[output_index],
            }
        })
        .collect::<Vec<_>>();
    if matches!(fault, Fault::RootIdentity) {
        rows[0].semantic_root_identity[0] ^= 1;
    }
    if matches!(fault, Fault::Workgroup) {
        rows[0].workgroup[0] = if rows[0].workgroup[0] == 1 { 2 } else { 1 };
    }
    if matches!(
        fault,
        Fault::DescriptorPermutation
            | Fault::RootIdentityPermutation
            | Fault::RootNamePermutation
            | Fault::FormalPayloadPermutation
    ) {
        assert_eq!(rows.len(), 2);
        let [first, second] = [rows[0], rows[1]];
        match fault {
            Fault::DescriptorPermutation => {
                rows[0].kernel_binding = second.kernel_binding;
                rows[1].kernel_binding = first.kernel_binding;
            }
            Fault::RootIdentityPermutation => {
                rows[0].semantic_root_identity = second.semantic_root_identity;
                rows[1].semantic_root_identity = first.semantic_root_identity;
            }
            Fault::RootNamePermutation => {
                rows[0].logical_name = second.logical_name;
                rows[0].export_symbol = second.export_symbol;
                rows[0].kernel_id = second.kernel_id;
                rows[1].logical_name = first.logical_name;
                rows[1].export_symbol = first.export_symbol;
                rows[1].kernel_id = first.kernel_id;
            }
            Fault::FormalPayloadPermutation => {
                assert_ne!(first.payload, second.payload);
                rows[0].payload = second.payload;
                rows[1].payload = first.payload;
            }
            _ => unreachable!(),
        }
    }
    let mut identity = *ranked.canonical_roster_identity().as_bytes();
    if matches!(fault, Fault::RosterIdentity) {
        identity[0] ^= 1;
    }
    let mut order = ranked
        .canonical_kernel_order()
        .iter()
        .map(|i| u32::try_from(*i).unwrap())
        .collect::<Vec<_>>();
    if matches!(fault, Fault::DescriptorPermutation) {
        // Keep the inert roster well formed for its swapped bindings. The
        // authenticated owner, not a malformed codec input, must refuse it.
        order.reverse();
    }
    let formal = Roster::new(MultiRootProofRosterInputsV3 {
        kind: if matches!(fault, Fault::Kind) {
            Kind::MiddleEnd
        } else {
            Kind::FormalMemory
        },
        semantic_mir_sha256: *source.semantic.semantic_sha256().as_bytes(),
        native_neutral_subject: subject,
        roster_identity: identity,
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
    let (receipts, storage) =
        NativeFinalOutputReceiptsPolicy6V1::try_from_preimages_v1(kernel, formal, budget).unwrap();
    assert_eq!(budget.storage(), floor);
    budget.reserve_storage(storage.retained_storage()).unwrap();
    assert_eq!(receipts.retained_storage(), storage.retained_storage());
    assert_eq!(receipts.kernel_ir().canonical_preimage(), kernel);
    assert_eq!(receipts.formal_memory().canonical_preimage(), formal);
    let result = joins::check(inputs, ranked, &receipts, typed, budget);
    assert_eq!(budget.storage(), floor + storage.retained_storage());
    assert!(budget.work_ledger_identity_v1() == ledger);
    match expected {
        None => result.unwrap(),
        Some(expected) => assert!(
            matches!(result, Err(E::Mismatch(actual)) if actual == expected),
            "{result:?}"
        ),
    }
    drop(receipts);
    budget.release_storage(storage.retained_storage()).unwrap();
    assert_eq!(budget.storage(), floor);
}

/// The existing two-root erased fixture deliberately uses decreasing binding
/// bytes. Its descriptor permutation must differ from physical N/I order.
pub(crate) fn exercise_permuted_root_component_v1(
    inputs: OutputInputsV1<'_>,
    ranked: &Ranked,
    profile: Profile,
    typed: &[TypedDescriptorRootV1],
    budget: &mut Budget<'_>,
) -> R<()> {
    let floor = budget.storage();
    check_output_inputs_v1(inputs, profile, typed, budget)?;
    let names = ["canonical_assertion_root", "erased_second_root"];
    let source = inputs.owner.source(inputs.catalog)?;
    assert_eq!(
        source
            .original
            .module()
            .kernels
            .iter()
            .map(|root| root.id.as_str())
            .collect::<Vec<_>>(),
        names
    );
    assert_eq!(
        inputs
            .owner
            .output()
            .module()
            .kernels
            .iter()
            .map(|root| root.id.as_str())
            .collect::<Vec<_>>(),
        names
    );
    assert_eq!(ranked.root_count(), 2);
    for (index, root) in ranked.roots().iter().enumerate() {
        assert_eq!(root.semantic_root().index(), index as u32);
        assert_eq!(root.export_symbol(), names[index].as_bytes());
        assert_eq!(root.kernel_binding(), &[247 - index as u8; 32]);
    }
    assert_eq!(ranked.canonical_kernel_order(), &[1, 0]);
    let (_, descriptor, _) = inputs.prepared.native_output_parts_v1();
    assert_eq!(
        descriptor
            .table()
            .kernels()
            .iter()
            .map(|root| root.entry_name().as_str())
            .collect::<Vec<_>>(),
        [names[1], names[0]]
    );
    let (kernel, formal) = wires(inputs, ranked, Fault::None);
    let decoded = Roster::decode(&formal).unwrap();
    assert_eq!(decoded.canonical_kernel_order(), &[1, 0]);
    assert_ne!(
        decoded.root(0).unwrap().payload(),
        decoded.root(1).unwrap().payload()
    );
    drop(decoded);
    check_case(inputs, ranked, typed, &kernel, &formal, None, budget);
    for (fault, expected) in [
        (
            Fault::DescriptorPermutation,
            "final receipt descriptor-canonical order",
        ),
        (
            Fault::RootIdentityPermutation,
            "exact final receipt root axes",
        ),
        (Fault::RootNamePermutation, "exact final receipt root axes"),
        (
            Fault::FormalPayloadPermutation,
            "fresh final-I formal payload",
        ),
    ] {
        let (kernel, formal) = wires(inputs, ranked, fault);
        // Every hostile candidate remains a well-formed two-root codec object.
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
    assert_eq!(budget.storage(), floor);
    Ok(())
}

/// Called from the genuine ordinary-source observation and the constructed
/// unsigned no-op fixture. It invokes the exact private production joins after
/// complete actual output replay; no signed native owner or proof is forged.
pub(crate) fn exercise_unsigned_component_v1(
    inputs: OutputInputsV1<'_>,
    ranked: &Ranked,
    profile: Profile,
    typed: &[TypedDescriptorRootV1],
    budget: &mut Budget<'_>,
) -> R<()> {
    let floor = budget.storage();
    check_output_inputs_v1(inputs, profile, typed, budget)?;
    let (kernel, formal) = wires(inputs, ranked, Fault::None);
    let (again_kernel, again_formal) = wires(inputs, ranked, Fault::None);
    assert_eq!(kernel, again_kernel);
    assert_eq!(formal, again_formal);
    check_case(inputs, ranked, typed, &kernel, &formal, None, budget);
    let mut changed = kernel.clone();
    changed[112] ^= 1;
    check_case(
        inputs,
        ranked,
        typed,
        &changed,
        &formal,
        Some("final KernelIr graph bytes"),
        budget,
    );
    let mut changed = kernel.clone();
    changed[112 + inputs.owner.output().canonical().canonical_bytes().len()] ^= 1;
    check_case(
        inputs,
        ranked,
        typed,
        &changed,
        &formal,
        Some("final KernelIr catalog bytes"),
        budget,
    );
    for (fault, expected) in [
        (Fault::RootIdentity, "exact final receipt root axes"),
        (Fault::Workgroup, "exact final receipt root axes"),
        (Fault::Payload, "fresh final-I formal payload"),
        (Fault::RosterIdentity, "complete final FormalMemory roster"),
        (Fault::Kind, "complete final FormalMemory roster"),
    ] {
        let (kernel, formal) = wires(inputs, ranked, fault);
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
    let old = match inputs.owner {
        OutputOwnerV1::Direct6(owner) => owner.checked_output().intermediate_policy5().owner(),
        OutputOwnerV1::Erased6(owner) => owner.checked_output().intermediate_policy5().owner(),
        _ => panic!("not Policy6"),
    };
    if old.canonical().canonical_bytes() != inputs.owner.output().canonical().canonical_bytes() {
        let old_kernel = encode_native_neutral_module_v1(
            &joins::subject(old, inputs.catalog)?,
            old.canonical().canonical_bytes(),
            inputs.catalog.canonical_bytes(),
        )
        .unwrap();
        check_case(
            inputs,
            ranked,
            typed,
            &old_kernel,
            &formal,
            Some("final KernelIr subject"),
            budget,
        );
    }
    let foreign = match profile {
        Profile::Gfx942 => Profile::Gfx950,
        Profile::Gfx950 => Profile::Gfx942,
    };
    assert!(matches!(
        check_output_inputs_v1(inputs, foreign, typed, budget),
        Err(E::Descriptor(_)) | Err(E::Mismatch("exact prepared target/LLVM handoff"))
    ));
    assert_eq!(budget.storage(), floor);
    Ok(())
}

/// Independent invocations of the private component, not replacement/reset of
/// a production work ledger. The actual graph/receipt owners remain borrowed.
pub(crate) fn exercise_join_budget_controls_v1(
    inputs: OutputInputsV1<'_>,
    ranked: &Ranked,
    typed: &[TypedDescriptorRootV1],
    budget: &mut Budget<'_>,
) {
    let initial = budget.storage();
    let (kernel, formal) = wires(inputs, ranked, Fault::None);
    let (receipts, storage) =
        NativeFinalOutputReceiptsPolicy6V1::try_from_preimages_v1(&kernel, &formal, budget)
            .unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let floor = budget.storage();
    let run = |work_limit, storage_limit| {
        let mut work = Work::new(work_limit);
        let mut attempt = Budget::new(&mut work, storage_limit);
        attempt.reserve_storage(floor).unwrap();
        let ledger = attempt.work_ledger_identity_v1();
        let result = joins::check(inputs, ranked, &receipts, typed, &mut attempt);
        assert_eq!(attempt.storage(), floor);
        assert!(attempt.work_ledger_identity_v1() == ledger);
        (result, attempt.work(), attempt.peak_storage())
    };
    let (result, work, peak) = run(100_000_000, budget.storage_limit());
    result.unwrap();
    run(work, peak).0.unwrap();
    assert!(matches!(run(work - 1, peak).0, Err(E::Resource(_))));
    assert!(matches!(run(work, peak - 1).0, Err(E::Resource(_))));
    drop(receipts);
    budget.release_storage(storage.retained_storage()).unwrap();
    assert_eq!(budget.storage(), initial);
}

#[test]
fn owned_receipt_copy_has_exact_work_storage_and_nonzero_floor() {
    fn run(work_limit: usize, storage_limit: usize) -> (bool, usize, usize) {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(29).unwrap();
        let before = budget.work_ledger_identity_v1();
        let result = NativeFinalOutputReceiptsPolicy6V1::try_from_preimages_v1(
            b"kernel",
            b"formal",
            &mut budget,
        );
        let ok = result.is_ok();
        if let Ok((owner, receipt)) = result {
            assert_eq!(
                receipt.retained_storage(),
                size_of::<NativeFinalOutputReceiptsPolicy6V1>() + 12
            );
            drop(owner);
        }
        assert_eq!(budget.storage(), 29);
        assert!(budget.work_ledger_identity_v1() == before);
        (ok, budget.work(), budget.peak_storage())
    }
    let (ok, work, peak) = run(10000, 10000);
    assert!(ok);
    assert!(run(work, peak).0);
    assert!(!run(work - 1, peak).0);
    assert!(!run(work, peak - 1).0);
}

#[test]
fn opaque_receipt_copy_does_not_admit_empty_or_oversized_preimages() {
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, 1024);
    budget.reserve_storage(17).unwrap();
    for (kernel, formal) in [(&b""[..], &b"x"[..]), (&b"x"[..], &b""[..])] {
        assert!(matches!(
            NativeFinalOutputReceiptsPolicy6V1::try_from_preimages_v1(kernel, formal, &mut budget),
            Err(E::Mismatch("final receipt preimage extent"))
        ));
        assert_eq!(budget.storage(), 17);
    }
    let oversized = vec![1; MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3 + 1];
    assert!(matches!(
        NativeFinalOutputReceiptsPolicy6V1::try_from_preimages_v1(&oversized, b"x", &mut budget),
        Err(E::Mismatch("final receipt preimage extent"))
    ));
    assert_eq!(budget.storage(), 17);
}

#[test]
fn refused_or_panicking_receipt_scope_drops_inputs_before_restoring_the_floor() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    struct Mark(Arc<AtomicBool>);
    impl Drop for Mark {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    for panic in [false, true] {
        let mut work = Work::new(10000);
        let mut budget = Budget::new(&mut work, 10000);
        budget.reserve_storage(31).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let dropped = Arc::new(AtomicBool::new(false));
        let result: R<()> = scoped(&mut budget, |budget| {
            let (owner, storage) = NativeFinalOutputReceiptsPolicy6V1::try_from_preimages_v1(
                b"kernel", b"formal", budget,
            )?;
            budget.reserve_storage(storage.retained_storage())?;
            // Tuple fields drop in declaration order: the marker observes that
            // the actual owned canonical payloads have already been discarded.
            let _retained = (owner, Mark(Arc::clone(&dropped)));
            if panic {
                panic!("receipt component panic control");
            }
            Err(E::Mismatch("receipt component refusal control"))
        });
        if panic {
            assert!(matches!(result, Err(E::Panicked)));
        } else {
            assert!(matches!(
                result,
                Err(E::Mismatch("receipt component refusal control"))
            ));
        }
        assert!(dropped.load(Ordering::SeqCst));
        assert_eq!(budget.storage(), 31);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert!(budget.work() > 0);
    }
}

use super::*;
use crate::{
    CompilerExecutionClientProcessIdentityV1 as Client,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerPolicyV3 as Policy,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

#[allow(dead_code)]
#[path = "../tests/support/native_attestation_fixture.rs"]
mod fixture;

fn retain<T, E: fmt::Debug>(result: std::result::Result<(T, Storage), E>, b: &mut Budget<'_>) -> T {
    let (value, charge) = result.unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    value
}

fn inputs(mask: u8, nonce: u8, b: &mut Budget<'_>) -> (Intake, Terminal, Subject, Manifest, Ready) {
    let wire = fixture::policy_wire(3);
    b.reserve_storage(wire.len()).unwrap();
    let policy = retain(Policy::decode(&wire, b), b);
    let subject_wire = fixture::subject_wire(3);
    b.reserve_storage(subject_wire.len()).unwrap();
    let (subject, charge) = Subject::decode(&subject_wire, b).unwrap();
    b.reserve_storage(charge.retained_storage()).unwrap();
    let hello = retain(
        Intake::hello(
            &policy,
            *subject.rustc_invocation_sha256(),
            [nonce; 32],
            mask,
            4096,
            (71, 72),
            b,
        ),
        b,
    );
    let challenge = retain(Intake::challenge(&hello, [0x73; 32], b), b);
    let last = retain(
        Intake::input(&challenge, challenge.roles().last().unwrap(), b),
        b,
    );
    let terminal = retain(Terminal::new(&last, Termination::Exited(0), b), b);
    let manifest = retain(
        Manifest::new(
            Client::new(1234, 1000, 1001).unwrap(),
            Service::new(6000, 7000).unwrap(),
            &policy,
            b,
        ),
        b,
    );
    let ready = retain(Ready::new(5678, &manifest, &policy, b), b);
    (last, terminal, subject, manifest, ready)
}

fn record(mask: u8, nonce: u8, b: &mut Budget<'_>) -> (Intake, Record) {
    let (last, terminal, subject, manifest, ready) = inputs(mask, nonce, b);
    let value = retain(Record::new(terminal, subject, manifest, ready, b), b);
    (last, value)
}

#[test]
fn original_account_completion_decode_keeps_aggregate_and_codec_budgets_distinct() {
    use fe2o3_kernel_ir::CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned;
    let mut fixture_work = Work::new(1_000_000);
    let mut fixture_budget = Budget::new(&mut fixture_work, 1_000_000);
    let (_, record) = record(7, 0x72, &mut fixture_budget);
    let bytes = *record.canonical_bytes();
    let floor = fe2o3_artifact_transaction::MAX_COMPILER_MODULE_HANDOFF_STORAGE_V5 + N;
    let mut peak = Record::COMPOSED_DECODE_STORAGE;
    for case in 0..3 {
        let work = Record::COMPOSED_DECODE_WORK - usize::from(case == 1);
        let limit = floor + peak - usize::from(case == 2);
        let mut account = Owned::new(Work::new(work), limit);
        account.with_budget(|b| {
            b.reserve_storage(floor).unwrap();
            let ledger = b.work_ledger_identity_v1();
            let identity = b.storage_account_identity_v1();
            let result = Record::decode_in_original_account_v1(&bytes, b);
            if case == 0 {
                let (decoded, charge) = result.unwrap();
                assert_eq!(decoded, record);
                assert_eq!(charge.additional_storage(), decoded.retained_storage());
                assert_eq!(b.work(), work);
                peak = b.peak_storage() - floor;
            } else {
                assert!(result.is_err());
                if case == 1 {
                    assert!(b.failed_work().is_some());
                }
                if case == 2 {
                    assert!(b.failed_storage().is_some());
                }
            }
            assert_eq!(b.storage(), floor);
            assert_eq!(b.storage_limit(), limit);
            assert!(b.work_ledger_identity_v1() == ledger);
            assert_eq!(b.storage_account_identity_v1(), identity);
        });
    }
}

#[test]
fn structured_roundtrip_preserves_exact_publication_and_intake_for_every_mask() {
    for mask in 0..8 {
        let mut work = Work::new(usize::MAX);
        let mut b = Budget::new(&mut work, 1_000_000);
        let (last, value) = record(mask, 0x72, &mut b);
        let (other, _) = record(mask, 0x74, &mut b);
        assert!(value.matches_intake(&last, &mut b).unwrap());
        assert!(!value.matches_intake(&other, &mut b).unwrap());
        assert_eq!(value.terminal().termination(), Termination::Exited(0));
        assert_eq!(value.subject().canonical_bytes(), &fixture::subject_wire(3));
        assert_eq!(
            value.readiness().launch_manifest_identity(),
            value.manifest().identity()
        );
        b.reserve_storage(N).unwrap();
        let decoded = retain(Record::decode(value.canonical_bytes(), &mut b), &mut b);
        assert_eq!(decoded, value);
        assert!(Terminal::decode(value.canonical_bytes(), &mut b).is_err());
        assert!(Intake::decode(value.canonical_bytes(), &mut b).is_err());
        assert_eq!(N, 1210);
        assert!(!value.subject().grants_compiler_authority());
        assert!(!value.subject().grants_publication_authority());
    }
}

#[test]
fn failed_terminal_cannot_become_a_publication_completion() {
    for end in [
        Termination::Exited(1),
        Termination::Exited(255),
        Termination::Signaled(9),
    ] {
        let mut work = Work::new(usize::MAX);
        let mut b = Budget::new(&mut work, 1_000_000);
        let (last, _, subject, manifest, ready) = inputs(0, 0x72, &mut b);
        let terminal = retain(Terminal::new(&last, end, &mut b), &mut b);
        assert!(matches!(
            Record::new(terminal, subject, manifest, ready, &mut b),
            Err(Error::Framing(_))
        ));
    }
}

#[test]
fn malformed_partial_legacy_and_corrupt_nested_records_refuse() {
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, 1_000_000);
    let (last, value) = record(0, 0x72, &mut b);
    b.reserve_storage(N).unwrap();
    for index in 0..N {
        let mut bytes = *value.canonical_bytes();
        bytes[index] ^= 1;
        assert!(Record::decode(&bytes, &mut b).is_err(), "byte {index}");
    }
    for len in [0, HEADER, TERMINAL_BYTES, N - 1] {
        assert!(Record::decode(&value.canonical_bytes()[..len], &mut b).is_err());
    }
    assert!(Record::decode(&[0; N + 1], &mut b).is_err());
    let ack = retain(Intake::enforcement_unavailable(&last, &mut b), &mut b);
    let mut bytes = *value.canonical_bytes();
    bytes[HEADER..SUBJECT_START].copy_from_slice(ack.canonical_bytes());
    let identity = digest(&bytes);
    bytes[DIGEST_START..].copy_from_slice(&identity);
    assert!(Record::decode(&bytes, &mut b).is_err());
}

#[test]
fn canonically_resealed_subject_or_policy_substitution_refuses() {
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, 1_000_000);
    let (_, value) = record(0, 0x72, &mut b);
    b.reserve_storage(N).unwrap();
    let mut bytes = *value.canonical_bytes();
    let subject = &mut bytes[SUBJECT_START..MANIFEST_START];
    subject[120] ^= 1;
    fixture::seal(subject, "INERT-COMPILER-EXECUTION-SUBJECT", 3);
    let identity = digest(&bytes);
    bytes[DIGEST_START..].copy_from_slice(&identity);
    assert!(matches!(
        Record::decode(&bytes, &mut b),
        Err(Error::Framing("root publication completion associations"))
    ));
    let mut bytes = *value.canonical_bytes();
    let terminal = &mut bytes[HEADER..SUBJECT_START];
    terminal[88] ^= 1;
    let mut hash = Sha256::new();
    hash.update(b"FE2O3/COMPILER-ROOT-COMPLETION/V1\0");
    hash.update(&terminal[..TERMINAL_BYTES - 32]);
    terminal[TERMINAL_BYTES - 32..].copy_from_slice(&hash.finalize());
    let identity = digest(&bytes);
    bytes[DIGEST_START..].copy_from_slice(&identity);
    assert!(matches!(
        Record::decode(&bytes, &mut b),
        Err(Error::Framing("root publication completion associations"))
    ));
}

#[test]
fn decode_uses_one_original_account_exact_work_and_preserves_refusal_history() {
    let mut initial_work = Work::new(usize::MAX);
    let mut initial = Budget::new(&mut initial_work, 1_000_000);
    let (_, source) = record(0, 0x72, &mut initial);
    let work_quote = COMPILER_EXECUTION_ROOT_PUBLICATION_COMPLETION_DECODE_WORK_V1;
    let storage_quote = COMPILER_EXECUTION_ROOT_PUBLICATION_COMPLETION_STORAGE_V1;
    let mut peak = 0;
    for mode in 0..4 {
        let floor = N - usize::from(mode == 1);
        let limit = if mode == 3 {
            peak - 1
        } else {
            N + storage_quote
        };
        let mut work = Work::new(work_quote - usize::from(mode == 2));
        let mut b = Budget::new(&mut work, limit);
        b.reserve_storage(floor).unwrap();
        let ledger = b.work_ledger_identity_v1();
        let result = Record::decode(source.canonical_bytes(), &mut b);
        assert_eq!(b.storage(), floor);
        assert!(ledger == b.work_ledger_identity_v1());
        if mode == 0 {
            let (decoded, charge) = result.unwrap();
            assert_eq!(decoded, source);
            assert_eq!(charge.additional_storage(), RETAINED);
            assert_eq!(b.work(), work_quote);
            peak = b.peak_storage();
            assert!(peak <= N + storage_quote);
        } else {
            assert!(
                matches!(result, Err(Error::Resource(_))),
                "mode {mode}: {result:?}"
            );
        }
    }
}

#[test]
fn consuming_constructor_returns_only_growth_and_exact_local_work() {
    for mode in 0..4 {
        let mut source_work = Work::new(usize::MAX);
        let mut source_budget = Budget::new(&mut source_work, 1_000_000);
        let (_, terminal, subject, manifest, ready) = inputs(0, 0x72, &mut source_budget);
        let mut work = Work::new(LOCAL_WORK - usize::from(mode == 1));
        let mut b = Budget::new(&mut work, INHERITED + FRAME - usize::from(mode == 3));
        let floor = INHERITED - usize::from(mode == 2);
        b.reserve_storage(floor).unwrap();
        let result = Record::new(terminal, subject, manifest, ready, &mut b);
        assert_eq!(b.storage(), floor);
        if mode != 0 {
            assert!(matches!(result, Err(Error::Resource(_))));
        } else {
            let (value, charge) = result.unwrap();
            assert_eq!(
                INHERITED + charge.additional_storage(),
                value.retained_storage()
            );
            assert_eq!(b.work(), LOCAL_WORK);
        }
    }
}

#[test]
fn exact_transcript_match_quote_keeps_the_complete_record_floor() {
    let mut source_work = Work::new(usize::MAX);
    let mut source_budget = Budget::new(&mut source_work, 1_000_000);
    let (last, value) = record(0, 0x72, &mut source_budget);
    let full = value.retained_storage() + last.retained_storage();
    for mode in 0..3 {
        let floor = full - usize::from(mode == 2);
        let mut work = Work::new(
            COMPILER_EXECUTION_ROOT_PUBLICATION_COMPLETION_MATCH_WORK_V1 - usize::from(mode == 1),
        );
        let mut b = Budget::new(&mut work, full + TERMINAL_SCRATCH);
        b.reserve_storage(floor).unwrap();
        let result = value.matches_intake(&last, &mut b);
        assert_eq!(b.storage(), floor);
        if mode == 0 {
            assert!(result.unwrap());
            assert_eq!(
                b.work(),
                COMPILER_EXECUTION_ROOT_PUBLICATION_COMPLETION_MATCH_WORK_V1
            );
        } else {
            assert!(matches!(result, Err(Error::Resource(_))));
        }
    }
}

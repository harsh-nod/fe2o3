use super::*;

fn caller(index: u32) -> SemanticFunctionIdV1 {
    SemanticFunctionIdV1::from_index(index)
}

fn callee(index: u32) -> SemanticCallableIdV1 {
    SemanticCallableIdV1::from_index(index)
}

fn axes() -> OccurrenceAxesV30 {
    OccurrenceAxesV30 {
        caller: SemanticFunctionIdentityV1::from_sha256([3; 32]),
        raw_block: 5,
        block: SemanticBlockIdentityV1::from_sha256([4; 32]),
        opcode: 128,
        callee: SemanticFunctionIdentityV1::from_sha256([5; 32]),
        arguments: 2,
    }
}

fn source() -> SemanticInlineAssemblySourceV30 {
    axes().source([1; 32], [2; 32]).unwrap()
}

#[test]
fn empty_owner_admits_no_occurrence() {
    let mut owner = InlineSourceOccurrencesV30::default();
    assert!(owner.is_empty());
    assert_eq!(owner.require_drained(), Ok(()));
    assert_eq!(owner.take(caller(0), 0, callee(0)), Ok(None));
}

#[test]
fn exact_site_is_consumed_once_and_cannot_be_reinserted() {
    let mut owner = InlineSourceOccurrencesV30::default();
    owner.insert((caller(1), 5), callee(3), source()).unwrap();
    assert!(!owner.is_empty());
    assert!(owner.require_drained().is_err());
    assert_eq!(owner.take(caller(1), 5, callee(3)), Ok(Some(source())));
    assert!(owner.is_empty());
    assert_eq!(owner.require_drained(), Ok(()));
    assert!(owner.take(caller(1), 5, callee(3)).is_err());
    assert!(owner.take(caller(1), 5, callee(4)).is_err());
    assert!(owner.insert((caller(1), 5), callee(3), source()).is_err());
}

#[test]
fn caller_block_and_callee_substitutions_do_not_consume_the_original() {
    let mut owner = InlineSourceOccurrencesV30::default();
    owner.insert((caller(1), 5), callee(3), source()).unwrap();
    assert_eq!(owner.take(caller(2), 5, callee(3)), Ok(None));
    assert_eq!(owner.take(caller(1), 6, callee(3)), Ok(None));
    assert!(owner.take(caller(1), 5, callee(4)).is_err());
    assert!(owner.require_drained().is_err());
    assert_eq!(owner.take(caller(1), 5, callee(3)), Ok(Some(source())));
}

#[test]
fn duplicate_site_rejects_without_overwriting_its_callee_or_source() {
    let mut owner = InlineSourceOccurrencesV30::default();
    owner.insert((caller(1), 5), callee(3), source()).unwrap();
    let different_source = axes().source([9; 32], [2; 32]).unwrap();
    assert!(
        owner
            .insert((caller(1), 5), callee(4), different_source)
            .is_err()
    );
    assert_eq!(owner.take(caller(1), 5, callee(3)), Ok(Some(source())));
}

#[test]
fn occurrence_bound_counts_consumed_entries_and_is_exact() {
    let mut owner = InlineSourceOccurrencesV30::default();
    for block in 0..MAX_OCCURRENCES_V30 as u32 {
        owner
            .insert((caller(1), block), callee(3), source())
            .unwrap();
    }
    assert!(
        owner
            .insert((caller(1), MAX_OCCURRENCES_V30 as u32), callee(3), source())
            .is_err()
    );
    assert_eq!(owner.take(caller(1), 0, callee(3)), Ok(Some(source())));
    assert!(owner.insert((caller(2), 0), callee(3), source()).is_err());
    assert!(owner.require_drained().is_err());
    assert!(require_bound(HARD_MAX_BLOCKS_V1 as usize, HARD_MAX_BLOCKS_V1).is_ok());
    assert!(require_bound(HARD_MAX_BLOCKS_V1 as usize + 1, HARD_MAX_BLOCKS_V1).is_err());
}

#[test]
fn contract_identity_binds_resource_presence_and_field_boundaries() {
    let base = contract_identity(b"frontend", None);
    assert_ne!(base, contract_identity(b"frontend", Some(b"")));
    assert_ne!(base, contract_identity(b"frontene", None));
    assert_ne!(
        contract_identity(b"ab", Some(b"c")),
        contract_identity(b"a", Some(b"bc"))
    );
    assert_ne!(
        contract_identity(b"frontend", Some(b"resource")),
        contract_identity(b"frontend", Some(b"resourcd"))
    );
    assert_eq!(base, contract_identity(b"frontend", None));
}

#[test]
fn unit_identity_commits_exact_compiler_transcript_and_domain() {
    assert_eq!(unit_identity(b"plan"), unit_identity(b"plan"));
    assert_ne!(unit_identity(b"plan"), unit_identity(b"plao"));
    assert_ne!(unit_identity(b"plan"), unit_identity(b"plan\0"));
    assert_ne!(
        unit_identity(b"plan"),
        domain_digest(CONTRACT_DOMAIN, &[b"plan"])
    );
}

#[test]
fn every_static_occurrence_axis_changes_statement_identity() {
    let original = axes();
    let base = original.source([1; 32], [2; 32]).unwrap();
    let variants = [
        OccurrenceAxesV30 {
            caller: SemanticFunctionIdentityV1::from_sha256([9; 32]),
            ..original
        },
        OccurrenceAxesV30 {
            raw_block: 6,
            ..original
        },
        OccurrenceAxesV30 {
            block: SemanticBlockIdentityV1::from_sha256([9; 32]),
            ..original
        },
        OccurrenceAxesV30 {
            opcode: 129,
            ..original
        },
        OccurrenceAxesV30 {
            callee: SemanticFunctionIdentityV1::from_sha256([9; 32]),
            ..original
        },
        OccurrenceAxesV30 {
            arguments: 1,
            ..original
        },
    ];
    for variant in variants {
        assert_ne!(
            base.statement(),
            variant.source([1; 32], [2; 32]).unwrap().statement()
        );
    }
    assert_ne!(
        base.statement(),
        original.source([9; 32], [2; 32]).unwrap().statement()
    );
    assert_ne!(
        base.statement(),
        original.source([1; 32], [9; 32]).unwrap().statement()
    );
    assert_eq!(base.function(), original.caller);
    assert_eq!(base.frontend_unit(), [1; 32]);
    assert_eq!(base.contract(), [2; 32]);
    assert!(original.source([0; 32], [2; 32]).is_err());
}

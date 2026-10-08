//! Descriptive source controls only; no execution/completion authority is minted.
use super::*;

type Sources = RuntimeGfx942GeneratedResidentRegistryV1<Carrier, 16>;

#[test]
fn registry16_exact_original_order_count_and_contract_are_not_n4_or_repeat2() {
    let mut source = Sources::new(core::array::from_fn(carrier));
    let roster = source.validate_sources(42).unwrap();
    assert_eq!(
        roster.source_identity.profile(),
        GeneratedProfileV1::NativeFillRegistry16
    );
    assert_eq!(
        (roster.count, roster.readback_bytes, roster.fixup_count),
        (16, 30_784, 16)
    );
    let GeneratedContractsV1::Registry16(contracts) = roster.dispatch_contract_sha256 else {
        panic!()
    };
    for (i, member) in source.members.iter().enumerate() {
        assert_eq!(contracts[i], member.authority.contract);
        assert_eq!(roster.buffers[i].unwrap().bytes, (1 + i as u64 * 64) * 4);
        assert_eq!(roster.buffers[i].unwrap().ordinal, i);
    }
    let first_four = RuntimeGfx942GeneratedRegistry4V1::new(core::array::from_fn(|i| {
        Borrowed(&source.members[i])
    }));
    assert!(!roster.matches(&first_four.validate_sources(42).unwrap()));
    let reordered =
        RuntimeGfx942GeneratedResidentRegistryV1::<_, 16>::new(core::array::from_fn(|i| {
            Borrowed(&source.members[15 - i])
        }));
    assert!(!roster.matches(&reordered.validate_sources(42).unwrap()));
    let duplicated =
        RuntimeGfx942GeneratedResidentRegistryV1::<_, 16>::new(core::array::from_fn(|i| {
            Borrowed(&source.members[if i == 15 { 0 } else { i }])
        }));
    assert!(duplicated.validate_sources(42).is_err());
    source.repeat2 = true;
    assert!(source.validate_sources(42).is_err());
}

#[test]
fn registry16_nested_loans_keep_all_original_buffers_and_close_every_source() {
    let source = Sources::new(core::array::from_fn(carrier));
    let roster = source.validate_sources(42).unwrap();
    let pointers = source
        .members
        .each_ref()
        .map(|member| member.storage.buffers()[0].bytes().as_ptr());
    let called = Cell::new(0);
    source
        .with_native_inputs_v1(42, &roster, |programs, buffers| {
            assert_eq!(programs.len(), 16);
            for (i, buffer) in buffers.iter().enumerate() {
                assert_eq!(buffer.len(), 1);
                assert_eq!(buffer[0].bytes().as_ptr(), pointers[i]);
            }
            called.set(called.get() + 1);
            Ok(())
        })
        .unwrap()
        .unwrap();
    assert_eq!(called.get(), 1);
    for index in [0, 3, 4, 7, 8, 11, 12, 15] {
        let stale = &source.members[index].authority.stale;
        assert!(
            source
                .with_native_inputs_v1(42, &roster, |_, _| {
                    stale.set(true);
                    Ok(())
                })
                .is_err()
        );
        assert!(
            source
                .with_native_inputs_v1(42, &roster, |_, _| panic!("stale entered"))
                .is_err()
        );
        stale.set(false);
    }
}

#[test]
fn registry16_control_transfer_refuses_wrong_capacity_before_consuming_originals() {
    let mut source = Sources::new(core::array::from_fn(carrier));
    let roster = source.validate_sources(42).unwrap();
    let mut mutable = source.source_mut_v1().unwrap();
    let mut short = core::array::from_fn::<_, 15, _>(|_| None);
    assert!(!mutable.transfer_controls_into(&mut short));
    assert!(short.iter().all(Option::is_none));
    assert!(mutable.matches_roster(&roster));
    let mut exact = core::array::from_fn::<_, 16, _>(|_| None);
    assert!(mutable.transfer_controls_into(&mut exact));
    assert!(exact.iter().all(Option::is_some));
    assert!(!mutable.matches_roster(&roster));
    assert!(!mutable.transfer_controls_into(&mut exact));
}
#[test]
fn registry16_contract_roster_has_bounded_copy_storage() {
    fn requires_copy<T: Copy>() {}
    requires_copy::<super::super::GeneratedContractsV1>();
    assert!(core::mem::size_of::<super::super::GeneratedContractsV1>() <= 528);
}

#[test]
fn empty_templates_require_exact_element_alignment_not_null_or_pointer_alignment() {
    for alignment in [1, 2, 4, 8, 16] {
        let mut s = slice(0, 0, None);
        s.alignment = alignment;
        s.element_bytes = u64::from(alignment);
        let p = premises(&[s], &[], 64).unwrap();
        assert_eq!(live_span(&s, &[]).unwrap(), (u64::from(alignment), 0));
        assert!(p.check_live(&[]).is_ok());
        for pointer in [0, u64::from(alignment) * 2, u64::MAX] {
            let mut bytes = kernarg(&[s]);
            bytes[..8].copy_from_slice(&pointer.to_le_bytes());
            assert!(matches!(
                ConditionalDispatchPremisesV1::new(
                    [1; 32],
                    [2; 32],
                    [3; 32],
                    &bytes,
                    geometry(64),
                    &[s],
                    0,
                    ConditionalDispatchDomainV1::GuardedOutput,
                    &[],
                ),
                Err(ConditionalDispatchErrorV1::Binding)
            ));
        }
    }
}

#[test]
fn empty_sentinel_never_substitutes_for_nonempty_backing_or_invalid_layout() {
    for (length, offset) in [(1, 0), (0, 1)] {
        let mut s = slice(0, length, None);
        s.buffer_byte_offset = offset;
        assert!(matches!(
            premises(&[s], &[], 64),
            Err(ConditionalDispatchErrorV1::Binding)
        ));
    }
    for alignment in [0, 3, 8] {
        let mut s = slice(0, 0, None);
        s.alignment = alignment;
        assert!(matches!(
            premises(&[s], &[], 64),
            Err(ConditionalDispatchErrorV1::Layout)
        ));
    }
    let s = slice(0, 0, Some(0));
    assert_eq!(template_pointer(&s), 0);
    assert_eq!(read_word(&kernarg(&[s]), 0).unwrap(), 0);
    let p = premises(&[s], &[], 64).unwrap();
    assert!(p.check_live(&[facts(0x1000, 4, 1, 1)]).is_ok());
    assert!(request(&[s], &[4]).with_conditional_premises_v1(p).is_ok());
}

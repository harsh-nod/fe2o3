//! Host decoder/gate correspondence only; runtime graph transitions are tested there.

use super::*;

#[test]
fn graph_encoder_preserves_original_typed_storage_and_credit_on_every_attempt() {
    let mut output = output(Box::new([0x1234_5678u32, 0xaabb_ccdd]));
    let original = RuntimeGeneratedResultDomainV1::from_owner(Arc::clone(&output.gate));
    let foreign = RuntimeGeneratedResultDomainV1::from_owner(Arc::new(()));
    let usage = output.budget.usage();
    let mut scratch = [0xa5; 8];
    assert!(matches!(
        output
            .observer
            .encode_graph_completed_v1(&original, &mut scratch),
        Err(Error::BindingMismatch)
    ));
    assert_eq!(output.budget.usage(), usage);
    let expected = [0x78, 0x56, 0x34, 0x12, 0xdd, 0xcc, 0xbb, 0xaa];
    output.custody.decode(&expected, &output.gate).unwrap();
    output.gate.commit();
    assert!(matches!(
        output
            .observer
            .encode_graph_completed_v1(&foreign, &mut scratch),
        Err(Error::BindingMismatch)
    ));
    assert_eq!(scratch, [0xa5; 8]);
    let slot = output.observer.slot.clone();
    let guard = slot.state.lock().unwrap();
    assert_eq!(
        output
            .observer
            .encode_graph_completed_v1(&original, &mut scratch)
            .unwrap(),
        None
    );
    drop(guard);
    assert!(matches!(
        output
            .observer
            .encode_graph_completed_v1(&original, &mut scratch[..7]),
        Err(Error::ByteLength)
    ));
    assert_eq!(output.budget.usage(), usage);
    assert_eq!(
        output
            .observer
            .encode_graph_completed_v1(&original, &mut scratch)
            .unwrap(),
        Some(())
    );
    assert_eq!(scratch, expected);
    assert_eq!(output.budget.usage(), usage);
    let result = output.observer.try_take().unwrap().unwrap();
    assert_eq!(result.as_slice(), &[0x1234_5678, 0xaabb_ccdd]);
    assert_eq!(output.budget.usage(), usage);
    assert!(matches!(
        output
            .observer
            .encode_graph_completed_v1(&original, &mut scratch),
        Err(Error::OutputUnavailable)
    ));
    drop(result);
    assert_eq!(output.budget.usage().reserved_peak_bytes, 0);
}

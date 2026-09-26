//! V4 framing and debit goldens relative to the unchanged nominal/V1 codecs.
use fe2o3_kernel_descriptor::*;
#[path = "support/conditional_v4.rs"]
mod fixture;
fn free(_: usize) -> Result<(), ()> {
    Ok(())
}

#[test]
fn v4_exact_framing_and_preflight_charge_sequence_stay_unchanged() {
    for target in ["gfx942:xnack-", "gfx950:xnack-"] {
        fixture::with_input(target, 1, 0, |input| {
            let mut expected_trace = Vec::new();
            let prefix = encoded_device_descriptor_table_v3_len(&input.nominal, &mut |w| {
                expected_trace.push(w);
                Ok::<_, ()>(())
            })
            .unwrap();
            let contract = &input.contracts[0];
            let n = contract.canonical_bytes().len();
            // One output argument, one type, no reads. These are the original
            // V4 decode/join debits, not a second validation implementation.
            expected_trace.extend([1, 256 * n + 4096, 33, 1, 352, 1, 33, 80, 1, 353, 1, 33, 1]);
            let total = prefix + 4 + 36 + n;
            let mut expected = vec![0; prefix];
            encode_device_descriptor_table_v3(&input.nominal, &mut expected, &mut free).unwrap();
            expected[8..10].copy_from_slice(&4u16.to_le_bytes());
            expected[12..16].copy_from_slice(&(total as u32).to_le_bytes());
            expected.extend_from_slice(&[1, 0, 0, 0]);
            expected.extend_from_slice(&(n as u32).to_le_bytes());
            expected.extend_from_slice(contract.identity().as_bytes());
            expected.extend_from_slice(contract.canonical_bytes());
            let mut actual_trace = Vec::new();
            assert_eq!(
                encoded_device_descriptor_table_v4_len(&input, &mut |w| {
                    actual_trace.push(w);
                    Ok::<_, ()>(())
                })
                .unwrap(),
                total
            );
            assert_eq!(actual_trace, expected_trace);
            expected_trace.push(total * 2 + 1);
            let mut output = vec![0xa5; total];
            actual_trace.clear();
            encode_device_descriptor_table_v4(&input, &mut output, &mut |w| {
                actual_trace.push(w);
                Ok::<_, ()>(())
            })
            .unwrap();
            assert_eq!(actual_trace, expected_trace);
            assert_eq!(output, expected);
            assert!(decode_device_descriptor_table_v4(&output, &mut free).is_ok());
            // Error adapters must not reorder callbacks or replace their payload.
            for deny in 0..expected_trace.len() {
                let mut seen = Vec::new();
                output.fill(0xa5);
                let error = encode_device_descriptor_table_v4(&input, &mut output, &mut |w| {
                    seen.push(w);
                    if seen.len() == deny + 1 {
                        Err(deny)
                    } else {
                        Ok(())
                    }
                })
                .unwrap_err();
                match error {
                    DescriptorWireErrorV4::Nominal(DescriptorWireErrorV3::Work(index))
                    | DescriptorWireErrorV4::Contract(ConditionalInvocationWireErrorV1::Work(
                        index,
                    )) => assert_eq!(index, deny),
                    other => panic!("wrong denial: {other:?}"),
                }
                assert_eq!(seen, expected_trace[..=deny]);
                assert!(output.iter().all(|b| *b == 0xa5));
            }
        });
    }
}

#[test]
fn v4_version_error_and_early_denial_precede_contract_work() {
    let mut bytes = fixture::wire("gfx942:xnack-", 1, 0);
    bytes[8..10].copy_from_slice(&5u16.to_le_bytes());
    let mut trace = Vec::new();
    assert!(matches!(
        decode_device_descriptor_table_v4(&bytes, &mut |w| {
            trace.push(w);
            Ok::<_, ()>(())
        }),
        Err(DescriptorWireErrorV4::Nominal(
            DescriptorWireErrorV3::Decode(DecodeError::UnknownVersion(5))
        ))
    ));
    assert_eq!(trace, [1, 25, 7]);
    assert!(matches!(
        decode_device_descriptor_table_v4(&bytes, &mut |_| Err(17)),
        Err(DescriptorWireErrorV4::Nominal(DescriptorWireErrorV3::Work(
            17
        )))
    ));
}

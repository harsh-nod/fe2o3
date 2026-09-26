use super::*;

#[derive(Debug, Eq, PartialEq)]
struct Denied(usize);

struct Meter {
    limit: usize,
    accepted: usize,
    calls: Vec<usize>,
    first_denial: Option<usize>,
}
impl Meter {
    fn new(limit: usize) -> Self {
        Self {
            limit,
            accepted: 0,
            calls: Vec::new(),
            first_denial: None,
        }
    }
    fn pay(&mut self, n: usize) -> Result<(), Denied> {
        self.calls.push(n);
        if n > self.limit - self.accepted {
            self.first_denial.get_or_insert(n);
            return Err(Denied(n));
        }
        self.accepted += n;
        Ok(())
    }
}

#[test]
fn v2_exact_and_one_short_work_preserve_precharge_and_output() {
    let f = Fixture::new(1, 1);
    let input = input(&f, [17; 32]);
    let bytes = wire(&f);
    let n = bytes.len();
    let prepay = n * 256 + 4096;
    let write = 2 * n + 1;
    for short in [false, true] {
        let mut meter = Meter::new(1 + prepay - usize::from(short));
        let length = encoded_conditional_invocation_contract_v2_len(&input, &mut |n| meter.pay(n));
        assert_eq!(length.is_ok(), !short);
        assert_eq!(meter.calls, [1, prepay]);
        assert_eq!(meter.accepted, if short { 1 } else { 1 + prepay });
        assert_eq!(meter.first_denial, short.then_some(prepay));
        let mut meter = Meter::new(1 + prepay - usize::from(short));
        let decoded = decode_conditional_invocation_contract_v2(&bytes, &mut |n| meter.pay(n));
        assert_eq!(decoded.is_ok(), !short);
        assert_eq!(meter.calls, [1, prepay]);
        assert_eq!(meter.accepted, if short { 1 } else { 1 + prepay });
        assert_eq!(meter.first_denial, short.then_some(prepay));
        let mut output = vec![0xa5; n];
        let mut meter = Meter::new(1 + prepay + write - usize::from(short));
        let encoded =
            encode_conditional_invocation_contract_v2(&input, &mut output, &mut |n| meter.pay(n));
        assert_eq!(encoded.is_ok(), !short);
        assert_eq!(meter.calls, [1, prepay, write]);
        assert_eq!(
            meter.accepted,
            if short {
                1 + prepay
            } else {
                1 + prepay + write
            }
        );
        assert_eq!(meter.first_denial, short.then_some(write));
        if short {
            assert!(output.iter().all(|b| *b == 0xa5));
        } else {
            assert_eq!(output, bytes);
        }
    }
    for length in [n - 1, n + 1] {
        let mut output = vec![0xa5; length];
        let mut meter = Meter::new(usize::MAX);
        assert!(
            matches!(encode_conditional_invocation_contract_v2(&input, &mut output, &mut |n| meter.pay(n)),
            Err(ConditionalInvocationWireErrorV1::OutputLength { expected, actual })
            if expected == n && actual == length)
        );
        assert_eq!(meter.calls, [1, prepay]);
        assert!(output.iter().all(|b| *b == 0xa5));
    }
}

#[test]
fn v1_and_v2_every_charge_refusal_precedes_mutation() {
    let f = Fixture::new(1, 1);
    for version in [1, 2] {
        let bytes = if version == 1 { f.wire() } else { wire(&f) };
        let n = bytes.len();
        let charges = [1, n * 256 + 4096, 2 * n + 1];
        for stop in 0..3 {
            let mut output = vec![0xa5; n];
            let mut seen = Vec::new();
            let mut charge = |n| {
                seen.push(n);
                if seen.len() == stop + 1 {
                    Err(Denied(n))
                } else {
                    Ok(())
                }
            };
            let result = if version == 1 {
                encode_conditional_invocation_contract_v1(&f.input(), &mut output, &mut charge)
            } else {
                encode_conditional_invocation_contract_v2(
                    &input(&f, [17; 32]),
                    &mut output,
                    &mut charge,
                )
            };
            assert!(
                matches!(result, Err(ConditionalInvocationWireErrorV1::Work(Denied(n))) if n == charges[stop])
            );
            assert_eq!(seen, charges[..=stop]);
            assert!(output.iter().all(|b| *b == 0xa5));
        }
        for stop in 0..2 {
            let mut seen = Vec::new();
            let mut charge = |n| {
                seen.push(n);
                if seen.len() == stop + 1 {
                    Err(Denied(n))
                } else {
                    Ok(())
                }
            };
            let result = if version == 1 {
                decode_conditional_invocation_contract_v1(&bytes, &mut charge).map(|_| ())
            } else {
                decode_conditional_invocation_contract_v2(&bytes, &mut charge).map(|_| ())
            };
            assert!(
                matches!(result, Err(ConditionalInvocationWireErrorV1::Work(Denied(n))) if n == charges[stop])
            );
            assert_eq!(seen, charges[..=stop]);
        }
    }
}

#[test]
fn v2_cursor_retry_queries_and_prior_denials_keep_exact_debits() {
    let f = Fixture::new(1, 1);
    let bytes = wire(&f);
    let v = decode_conditional_invocation_contract_v2(&bytes, &mut free).unwrap();
    for short in [false, true] {
        let mut meter = Meter::new(353 - usize::from(short));
        let result = v.argument(0, &mut |n| meter.pay(n));
        assert_eq!(result.is_ok(), !short);
        assert_eq!(meter.calls, [353]);
        assert_eq!(meter.accepted, if short { 0 } else { 353 });
        let mut meter = Meter::new(32 - usize::from(short));
        let result = v.require_identity(v.identity(), &mut |n| meter.pay(n));
        assert_eq!(result.is_ok(), !short);
        assert_eq!(meter.calls, [32]);
    }
    for limit in [0, 352, 353] {
        let mut cursor = v.arguments();
        let mut meter = Meter::new(limit);
        let result = cursor.next(&mut |n| meter.pay(n));
        assert_eq!(result.is_ok(), limit == 353);
        assert_eq!(
            meter.accepted,
            match limit {
                0 => 0,
                352 => 1,
                _ => 353,
            }
        );
        if limit != 353 {
            assert_eq!(cursor.next(&mut free).unwrap(), Some(f.arguments[0]));
        }
    }
    let mut meter = Meter::new(353);
    assert_eq!(meter.pay(1000), Err(Denied(1000)));
    assert_eq!(
        v.argument(0, &mut |n| meter.pay(n)).unwrap(),
        f.arguments[0]
    );
    assert_eq!(meter.first_denial, Some(1000));
    assert_eq!(meter.accepted, 353);
    let mut meter = Meter::new(353);
    assert!(v.argument(usize::MAX, &mut |n| meter.pay(n)).is_err());
    assert_eq!(meter.accepted, 353);
    let mut cursor = v.typed_roots();
    assert_eq!(cursor.next(&mut free).unwrap(), Some(f.roots[0]));
    let mut meter = Meter::new(0);
    assert!(cursor.next(&mut |n| meter.pay(n)).is_err());
    assert_eq!(cursor.next(&mut free).unwrap(), None);
}

#[test]
fn v2_limits_refuse_after_only_the_original_initial_debit() {
    let oversized = vec![0; MAX_CONDITIONAL_INVOCATION_BYTES_V2 + 1];
    let mut meter = Meter::new(1);
    assert!(decode_conditional_invocation_contract_v2(&oversized, &mut |n| meter.pay(n)).is_err());
    assert_eq!(meter.calls, [1]);
    assert_eq!(meter.first_denial, None);
    let f = Fixture::new(64, 64);
    let mut meter = Meter::new(1);
    assert!(
        encoded_conditional_invocation_contract_v2_len(&input(&f, [17; 32]), &mut |n| meter.pay(n))
            .is_err()
    );
    assert_eq!(meter.calls, [1]);
    assert_eq!(meter.first_denial, None);
}

#[test]
fn v2_charge_unwind_leaves_output_and_cursor_position_untouched() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let f = Fixture::new(1, 1);
    let bytes = wire(&f);
    for stop in 0..3 {
        let mut output = vec![0xa5; bytes.len()];
        let mut calls = 0;
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _ = encode_conditional_invocation_contract_v2(
                &input(&f, [17; 32]),
                &mut output,
                &mut |_| {
                    calls += 1;
                    assert_ne!(calls, stop + 1, "caller charge unwind");
                    Ok::<(), ()>(())
                },
            );
        }));
        assert!(result.is_err());
        assert!(output.iter().all(|b| *b == 0xa5));
    }
    let v = decode_conditional_invocation_contract_v2(&bytes, &mut free).unwrap();
    let mut cursor = v.arguments();
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _ = cursor.next(&mut |n| {
            assert_ne!(n, 352, "row precharge unwind");
            Ok::<(), ()>(())
        });
    }));
    assert!(result.is_err());
    assert_eq!(cursor.next(&mut free).unwrap(), Some(f.arguments[0]));
}

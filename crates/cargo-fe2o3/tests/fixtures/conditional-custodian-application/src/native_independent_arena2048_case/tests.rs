use super::*;
use std::{
    cell::Cell,
    future::{Future, poll_fn},
    task::{Context, Poll, Waker},
};

fn report() -> Report {
    // An inert parser fixture, not fabricated native observation or authority.
    Report {
        events: 4097,
        publications: MEMBERS,
        completions: MEMBERS,
        pending: 1,
        maximum_unobserved: MEMBERS,
        pre_metadata_bytes: SOURCE_BYTES + 1_000_000,
        pre_metadata_records: 20,
        residual_metadata_bytes: 1_000_000,
        residual_metadata_records: 10,
        pre_result_bytes: 3 * SOURCE_BYTES,
        pre_result_records: 2 * MEMBERS,
        residual_source_bytes: SOURCE_BYTES,
        residual_source_records: MEMBERS,
        witness: Some(Witness {
            earlier: 0,
            later: 1,
            earlier_published: 1,
            later_published: 2,
            later_completed: 1025,
            earlier_pending: 1026,
        }),
    }
}

#[test]
fn independent_marker_refuses_every_existing_family_and_extra_argument() {
    let mut args = [
        "--native-v5-independent-arena2048",
        "--producer-source",
        "src/lib.rs",
        "--device",
        "0x000000000000abcd",
    ];
    assert_eq!(parse(args.map(OsString::from)).unwrap().device, 0xabcd);
    for marker in [
        "--native-v5",
        "--native-v5-registry4",
        "--native-v5-registry4-repeat2",
        "--native-v5-registry16",
        "--native-v5-arena1024",
        "--native-v5-independent-arena1024",
    ] {
        args[0] = marker;
        assert!(parse(args.map(OsString::from)).is_err());
    }
    args[0] = "--native-v5-independent-arena2048";
    let mut extra = args.map(OsString::from).to_vec();
    extra.push("--unbounded".into());
    assert!(parse(extra).is_err());
}

#[test]
fn exact_predeclared_mixed_counts_and_output_bytes_are_closed() {
    let mut sum = 0;
    for member in 0..MEMBERS {
        let length = count(member).unwrap();
        assert_eq!(
            length,
            if member.is_multiple_of(2) {
                LONG
            } else {
                SHORT
            }
        );
        assert!(length.is_multiple_of(64));
        sum += (length * 4) as u64;
    }
    assert_eq!(sum, SOURCE_BYTES);
    assert_eq!(SOURCE_BYTES, 268_697_600);
    assert!(count(MEMBERS).is_err());
    for member in [0, 1, 1024, 2047] {
        let mut output: Vec<_> = (0..count(member).unwrap() as u32).collect();
        check_output(member, &output).unwrap();
        output[0] = u32::MAX;
        assert!(check_output(member, &output).is_err());
        output[0] = 0;
        output.pop();
        assert!(check_output(member, &output).is_err());
    }
}

#[test]
fn strict_report_refuses_capacity_count_refund_and_decoder_only_substitution() {
    report().validate().unwrap();
    for change in [
        |r: &mut Report| r.publications -= 1,
        |r: &mut Report| r.completions -= 1,
        |r: &mut Report| r.events += 1,
        |r: &mut Report| r.maximum_unobserved = MEMBERS + 1,
        |r: &mut Report| r.pre_metadata_bytes = METADATA_BYTES + 1,
        |r: &mut Report| r.pre_metadata_records = METADATA_RECORDS + 1,
        |r: &mut Report| r.residual_metadata_bytes = 0,
        |r: &mut Report| r.pre_result_bytes -= 1,
        |r: &mut Report| r.pre_result_records -= 1,
        |r: &mut Report| r.residual_source_bytes = 0,
        |r: &mut Report| r.residual_source_records = 0,
    ] {
        let mut bad = report();
        change(&mut bad);
        assert!(bad.validate().is_err());
    }
}

#[test]
fn witness_requires_both_original_publications_and_ready_before_pending() {
    for change in [
        |w: &mut Witness| w.earlier = w.later,
        |w: &mut Witness| w.later = MEMBERS,
        |w: &mut Witness| w.earlier_published = 0,
        |w: &mut Witness| w.earlier_published = w.later_published,
        |w: &mut Witness| w.later_completed = w.later_published,
        |w: &mut Witness| w.earlier_pending = w.later_completed,
        |w: &mut Witness| w.earlier_pending = 4098,
    ] {
        let mut bad = report();
        change(bad.witness.as_mut().unwrap());
        assert!(bad.validate().is_err());
    }
    let mut no_witness = report();
    no_witness.witness = None;
    let fields = no_witness.fields(0xabcd).unwrap();
    assert!(fields.contains("\"native_out_of_order_measured\":false"));
    assert!(fields.contains("\"out_of_order_witness\":null"));
    assert!(fields.contains("\"native_durations_measured\":false"));
    assert!(fields.contains("\"thousands_inflight_qualified\":false"));
    assert!(
        report()
            .fields(0xabcd)
            .unwrap()
            .contains("\"native_out_of_order_measured\":true")
    );
}

#[test]
fn all_2048_observers_are_required_without_repoll_or_early_common_close() {
    let scans = Cell::new(0);
    let polls: Vec<_> = (0..MEMBERS).map(|_| Cell::new(0)).collect();
    let mut observers: Vec<_> = (0..MEMBERS)
        .map(|index| {
            let scans = &scans;
            let polls = &polls[index];
            Some(poll_fn(move |_| {
                polls.set(polls.get() + 1);
                if scans.get() >= if index + 1 == MEMBERS { 2 } else { 1 } {
                    Poll::Ready(Ok::<_, ()>(()))
                } else {
                    Poll::Pending
                }
            }))
        })
        .collect();
    let driver = poll_fn(|_| {
        scans.set(scans.get() + 1);
        Poll::Pending::<std::result::Result<(), ()>>
    });
    {
        let mut joined = std::pin::pin!(observation::all_copied(driver, &mut observers));
        let mut cx = Context::from_waker(Waker::noop());
        assert!(joined.as_mut().poll(&mut cx).is_pending());
        assert_eq!(joined.as_mut().poll(&mut cx), Poll::Ready(Ok(())));
    }
    assert!(observers.iter().all(Option::is_none));
    assert!(polls[..MEMBERS - 1].iter().all(|poll| poll.get() == 1));
    assert_eq!(polls[MEMBERS - 1].get(), 2);
}

#[test]
fn closed_driver_and_incomplete_observer_roster_refuse() {
    let mut observers: Vec<_> = (0..MEMBERS)
        .map(|_| Some(std::future::ready(Ok::<_, ()>(()))))
        .collect();
    let mut cx = Context::from_waker(Waker::noop());
    {
        let mut joined = std::pin::pin!(observation::all_copied(
            std::future::ready(Ok(())),
            &mut observers
        ));
        assert!(matches!(joined.as_mut().poll(&mut cx), Poll::Ready(Err(_))));
    }
    observers[MEMBERS - 1] = None;
    let mut joined = std::pin::pin!(observation::all_copied(
        std::future::pending(),
        &mut observers
    ));
    assert!(matches!(joined.as_mut().poll(&mut cx), Poll::Ready(Err(_))));
}

//! Assertions over events emitted by the actual source transaction and consumer.
use super::{Event, Observation, Outcome};

pub(crate) fn check(observation: &Observation, formula: &serde_json::Value) {
    let [
        Event::Retained {
            root,
            receipt,
            reserved_receipt_bytes,
            work,
            storage,
        },
        Event::PhaseRetained(initial),
        Event::ReplayCallback {
            root: callback_root,
            receipt: callback_receipt,
            wire,
            verifying_key,
            aggregate,
            source,
            graph,
            epoch: _,
            work: callback_work,
            storage: callback_storage,
        },
        Event::ReplayAccepted {
            root: accepted_root,
            receipt: accepted_receipt,
            work: accepted_work,
            storage: accepted_storage,
        },
        Event::PhaseFinished {
            before,
            after,
            outcome: Outcome::Accepted,
        },
        Event::PhaseDropped {
            before: dropped,
            after: released,
            poisoned: false,
        },
    ] = observation.events.as_slice()
    else {
        panic!(
            "actual conditional retention/replay sequence: {:?}",
            observation.events
        )
    };
    assert_eq!(root, callback_root);
    assert_eq!(root, accepted_root);
    assert_eq!(receipt, callback_receipt);
    assert_eq!(receipt, accepted_receipt);
    for (name, identity) in [
        ("statement", receipt.statement),
        ("generated_source", receipt.generated_source),
        ("execution", receipt.execution),
        ("receipt", receipt.receipt),
    ] {
        assert_ne!(identity, [0; 32]);
        assert_eq!(formula[name], serde_json::json!(identity));
    }
    for identity in [
        &receipt.reference_identity,
        &receipt.reference_mir,
        &receipt.kernel_identity,
        &receipt.kernel_mir,
        aggregate,
        source,
        graph,
        verifying_key,
    ] {
        assert_ne!(identity, &[0; 32]);
    }
    assert!(!wire.is_empty());
    assert!(*reserved_receipt_bytes > 0 && storage >= reserved_receipt_bytes);
    assert!(*work <= initial.work && *storage <= initial.storage);
    assert_eq!(initial, before);
    assert!(initial.work < *callback_work && callback_work <= accepted_work);
    assert!(*accepted_work <= after.work);
    assert!(*callback_storage >= initial.storage && *accepted_storage >= initial.storage);
    assert_eq!(initial.storage, after.storage);
    assert!(initial.failed_work.is_none() && initial.failed_storage.is_none());
    assert!(after.failed_work.is_none() && after.failed_storage.is_none());
    assert_eq!(after, dropped);
    assert_eq!(released.storage, 0);
    assert_eq!(released.work, after.work);
    assert_eq!(released.peak_storage, after.peak_storage);
    assert_eq!(released.failed_work, after.failed_work);
    assert_eq!(released.failed_storage, after.failed_storage);
}

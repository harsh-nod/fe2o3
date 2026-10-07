// Shared executable source for later proof refinement; no proof is claimed yet.
macro_rules! distributed_receipt_classifier_body_v1 {
    ($record:ident, $receipt:ident) => {{
        use DistributedPublicationContractErrorV1 as E;
        use ModelDistributedReceiptDispositionV1 as D;
        use ReportedDistributedPublicationV1 as P;
        if $record.binding != $receipt.binding {
            return Err(E::BindingMismatch);
        }
        if let Some(previous) = $record.last {
            if $receipt.description.sequence == previous.sequence {
                return if $receipt.description == previous {
                    Ok(D::ExactDuplicate)
                } else {
                    Err(E::ConflictingDuplicate)
                };
            }
            if $receipt.description.sequence < previous.sequence {
                return Err(E::StaleReceipt);
            }
        }
        if $record.interrupted {
            return Err(E::Interrupted);
        }
        match $record.last {
            None => {
                if $receipt.description.sequence != 1 {
                    return Err(E::SequenceGap);
                }
                if $receipt.description.outcome == P::Completed {
                    return Err(E::InvalidTransition);
                }
            }
            Some(previous) => {
                let Some(next) = previous.sequence.checked_add(1) else {
                    return Err(E::SequenceExhausted);
                };
                if $receipt.description.sequence != next {
                    return Err(E::SequenceGap);
                }
                if previous.outcome != P::Published
                    || !matches!(
                        $receipt.description.outcome,
                        P::Completed | P::FailedMayStillExecute
                    )
                {
                    return Err(E::InvalidTransition);
                }
            }
        }
        Ok(D::Recorded)
    }};
}

macro_rules! distributed_receipt_record_body_v1 {
    ($record:ident, $receipt:ident) => {{
        let disposition = classify_receipt($record, &$receipt)?;
        if disposition == ModelDistributedReceiptDispositionV1::Recorded {
            $record.last = Some($receipt.description);
        }
        Ok(disposition)
    }};
}

macro_rules! distributed_observation_body_v1 {
    ($record:ident, $event:ident) => {{
        if $event == DistributedObservationEventV1::ConnectionLost {
            $record.interrupted = true;
        }
    }};
}

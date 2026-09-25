use super::*;
use crate::context_version_journal::ContextWriterKindV1 as WriterKindV1;

include!("query_bodies.rs");

#[allow(unused_macros)]
#[macro_use]
mod equality_templates {
    include!("../context_version_journal/retained_bodies.rs");
}

#[inline]
fn producer_query_consumer_same_exec_v1(
    left: ContextWriterKeyV1,
    right: ContextWriterKeyV1,
) -> bool {
    retained_writer_key_body!(left, right)
}

#[inline]
pub(super) fn producer_writer_same_exec_v1(
    left: ContextWriterReferenceV1,
    right: ContextWriterReferenceV1,
) -> bool {
    producer_writer_same_body!(left, right)
}

#[inline]
pub(super) fn producer_reference_same_exec_v1(
    left: ContextProducerReadReferenceV1,
    right: ContextProducerReadReferenceV1,
) -> bool {
    producer_reference_same_body!(left, right)
}

//! Read-only retained heap of the non-authoritative cleanup identity.
use super::ProducerIdentity;
use std::mem::size_of;

impl ProducerIdentity {
    /// Visit the original two String capacities, excluding the enclosing header.
    ///
    /// The callback receives (element count, element width). It must check
    /// multiplication and accumulated bytes, debit one item per callback and
    /// enforce its remaining limits before accepting. The zero-byte first
    /// callback is a bounded owner visit, also required for empty payloads.
    /// Exactly three callbacks occur on success; the first error stops visits.
    /// Partial callback state is not a complete report. No clone, serialization,
    /// path normalization, authority check or constructor behavior is changed.
    pub fn visit_retained_heap_storage_v1<E>(
        &self,
        mut visit: impl FnMut(usize, usize) -> Result<(), E>,
    ) -> Result<(), E> {
        visit(0, 1)?;
        let Self {
            stable_source,
            crate_name,
        } = self;
        let _: &String = stable_source;
        let _: &String = crate_name;
        visit(stable_source.capacity(), size_of::<u8>())?;
        visit(crate_name.capacity(), size_of::<u8>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn original_constructor_and_spare_capacities_are_observed_without_mutation() {
        let mut producer =
            ProducerIdentity::from_codegen("kernel", Some(Path::new("src/kernel.rs"))).unwrap();
        producer.stable_source.reserve(103);
        producer.crate_name.reserve(71);
        let before = producer.clone();
        let expected = [
            0,
            producer.stable_source.capacity(),
            producer.crate_name.capacity(),
        ];
        let mut index = 0;
        producer
            .visit_retained_heap_storage_v1(|count, width| {
                assert_eq!(width, 1);
                assert_eq!(count, expected[index]);
                index += 1;
                Ok::<_, ()>(())
            })
            .unwrap();
        assert_eq!(index, 3);
        assert_eq!(producer, before);
    }

    #[test]
    fn every_callback_refusal_short_circuits() {
        let producer = ProducerIdentity::from_codegen("kernel", None).unwrap();
        for refuse_at in 1..=3 {
            let mut calls = 0;
            let result = producer.visit_retained_heap_storage_v1(|_, _| {
                calls += 1;
                if calls == refuse_at {
                    Err(refuse_at)
                } else {
                    Ok(())
                }
            });
            assert_eq!(result, Err(refuse_at));
            assert_eq!(calls, refuse_at);
        }
    }

    #[test]
    fn root_header_is_not_in_heap_visitor() {
        let producer = ProducerIdentity::from_codegen("x", None).unwrap();
        let mut bytes = 0usize;
        producer
            .visit_retained_heap_storage_v1(|n, w| {
                bytes = bytes.checked_add(n.checked_mul(w).unwrap()).unwrap();
                Ok::<_, ()>(())
            })
            .unwrap();
        assert_eq!(
            bytes,
            producer.stable_source.capacity() + producer.crate_name.capacity()
        );
        assert_eq!(size_of::<ProducerIdentity>(), 2 * size_of::<String>());
    }
}

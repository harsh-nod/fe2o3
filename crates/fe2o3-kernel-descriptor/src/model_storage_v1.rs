//! Data-only capacity projection of the exact current V1 descriptor owner.
use super::*;
use std::mem::size_of;

fn inline<T: Copy>(_: &T) {}
fn vector<T, E>(
    value: &Vec<T>,
    visit: &mut impl FnMut(usize, usize) -> Result<(), E>,
) -> Result<(), E> {
    visit(value.capacity(), size_of::<T>())
}
fn text<E>(value: &Text, visit: &mut impl FnMut(usize, usize) -> Result<(), E>) -> Result<(), E> {
    let Text(value) = value;
    visit(value.capacity(), size_of::<u8>())
}
fn name<E>(
    value: &ValidName,
    visit: &mut impl FnMut(usize, usize) -> Result<(), E>,
) -> Result<(), E> {
    let ValidName(value) = value;
    visit(value.capacity(), size_of::<u8>())
}

impl DeviceDescriptorTableV1 {
    /// Visits all retained heap extents, excluding this table's inline header.
    ///
    /// Each pair is (element capacity/count, element size). A (0, 1) pair marks
    /// an owner visit. Strings and Vecs report actual capacity, including unused
    /// capacity; fixed leaf payload is included in its enclosing header/Vec.
    /// The caller MUST use checked multiplication and checked accumulation and
    /// impose its item/byte limits. Returning Err stops before any later visit.
    ///
    /// This read-only traversal does not serialize, clone, revalidate, mutate,
    /// allocate a ledger or grant authority. It contains no Arc aliases. The
    /// caller counts size_of::<Self>() once only if not already covered by an
    /// enclosing header. This is not a peak-allocation or RSS observation.
    pub fn visit_logical_retained_heap_v1<E>(
        &self,
        visit: &mut impl FnMut(usize, usize) -> Result<(), E>,
    ) -> Result<(), E> {
        visit(0, 1)?;
        let Self {
            canonical_code_object_digest,
            code_object_version,
            compiler,
            producer,
            device_target,
            type_records,
            layout_records,
            kernels,
        } = self;
        inline(canonical_code_object_digest);
        inline(code_object_version);
        inline(device_target);
        let CompilerIdentityV1 {
            name: compiler_name,
            release,
            commit,
        } = compiler;
        inline(commit);
        text(compiler_name, visit)?;
        text(release, visit)?;
        let ProducerIdentityV1 {
            name: producer_name,
            version,
        } = producer;
        text(producer_name, visit)?;
        text(version, visit)?;
        vector(type_records, visit)?;
        for value in type_records {
            visit(0, 1)?;
            let SourceTypeRecordV1 {
                identity,
                descriptor,
            } = value;
            inline(identity);
            let SourceTypeDescriptorV1 { kind, element } = descriptor;
            inline(kind);
            inline(element);
        }
        vector(layout_records, visit)?;
        for value in layout_records {
            visit(0, 1)?;
            let DeviceLayoutRecordV1 {
                identity,
                descriptor,
            } = value;
            inline(identity);
            let DeviceLayoutDescriptorV1 {
                kind,
                element,
                size,
                alignment,
                pointer_width,
                length_width,
            } = descriptor;
            inline(kind);
            inline(element);
            inline(size);
            inline(alignment);
            inline(pointer_width);
            inline(length_width);
        }
        vector(kernels, visit)?;
        for kernel in kernels {
            visit(0, 1)?;
            let KernelDescriptorV1 {
                kernel_id,
                logical_name,
                entry_name,
                descriptor_symbol,
                source_evidence,
                executable_ir_evidence,
                capabilities,
                abi_layout,
                launch,
                arguments,
            } = kernel;
            inline(kernel_id);
            inline(source_evidence);
            inline(executable_ir_evidence);
            name(logical_name, visit)?;
            name(entry_name, visit)?;
            name(descriptor_symbol, visit)?;
            vector(capabilities, visit)?;
            // Compile-time witness: capability elements retain no owned heap.
            let _: fn(&CapabilityV1) = inline::<CapabilityV1>;
            inline(abi_layout);
            let LaunchConstraintsV1 {
                rank,
                block_size,
                max_grid,
                max_flat_workgroup_size,
                static_shared_memory_bytes,
                max_dynamic_shared_memory_bytes,
            } = launch;
            inline(rank);
            inline(block_size);
            inline(max_grid);
            inline(max_flat_workgroup_size);
            inline(static_shared_memory_bytes);
            inline(max_dynamic_shared_memory_bytes);
            vector(arguments, visit)?;
            for argument in arguments {
                visit(0, 1)?;
                let LogicalArgumentV1 {
                    source_index,
                    name: argument_name,
                    source_type,
                    device_layout,
                    ownership,
                    access,
                    alias,
                    components,
                } = argument;
                inline(source_index);
                inline(source_type);
                inline(device_layout);
                inline(ownership);
                inline(access);
                inline(alias);
                name(argument_name, visit)?;
                vector(components, visit)?;
                for component in components {
                    visit(0, 1)?;
                    let PhysicalAbiComponentV1 {
                        kind,
                        offset,
                        size,
                        alignment,
                        access,
                        alias,
                    } = component;
                    inline(kind);
                    inline(offset);
                    inline(size);
                    inline(alignment);
                    inline(access);
                    inline(alias);
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CanonicalCodeObjectDigest;

    fn spare_text(s: &str, capacity: usize) -> Text {
        let mut value = String::with_capacity(capacity);
        value.push_str(s);
        Text::new(value).unwrap()
    }
    fn spare_name(s: &str, capacity: usize) -> ValidName {
        let mut value = String::with_capacity(capacity);
        value.push_str(s);
        ValidName::new(value).unwrap()
    }
    fn fixture() -> DeviceDescriptorTableV1 {
        let ty = SourceTypeRecordV1::new(SourceTypeDescriptorV1::scalar(ScalarTypeV1::U32));
        let layout = DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::scalar(ScalarTypeV1::U32));
        let mut argument =
            LogicalArgumentV1::scalar(0, spare_name("value", 53), &ty, &layout, 0).unwrap();
        argument.components.reserve(11);
        let mut types = Vec::with_capacity(3);
        types.push(ty);
        let mut layouts = Vec::with_capacity(5);
        layouts.push(layout);
        let mut arguments = Vec::with_capacity(7);
        arguments.push(argument);
        let mut capabilities = Vec::with_capacity(13);
        capabilities.push(CapabilityV1::Subgroup);
        let evidence = BuildEvidenceV1::new(
            EvidenceIdentity::from_opaque_bytes([1; 32]),
            EvidenceDigest::from_sha256_bytes([2; 32]),
        );
        let kernel = KernelDescriptorV1::new(
            KernelId::from_bytes([3; 32]),
            spare_name("logical", 59),
            spare_name("entry", 61),
            spare_name("entry.kd", 67),
            evidence,
            evidence,
            capabilities,
            KernelAbiLayoutV1::new(4, 4, 4).unwrap(),
            LaunchConstraintsV1::new(
                1,
                BlockSizeV1::Any,
                DimensionsV1::new(1, 1, 1).unwrap(),
                64,
                0,
                0,
            )
            .unwrap(),
            arguments,
        )
        .unwrap();
        let mut kernels = Vec::with_capacity(17);
        kernels.push(kernel);
        DeviceDescriptorTableV1::new(
            CanonicalCodeObjectDigest::from_bytes([0; 32]),
            CodeObjectVersion::V5,
            CompilerIdentityV1::new(
                spare_text("compiler", 31),
                spare_text("release", 37),
                [4; 20],
            ),
            ProducerIdentityV1::new(spare_text("producer", 41), spare_text("version", 43)),
            DeviceTargetV1::parse("gfx942:xnack-").unwrap(),
            types,
            layouts,
            kernels,
        )
        .unwrap()
    }
    fn total(value: &DeviceDescriptorTableV1) -> (usize, usize) {
        let mut bytes = 0usize;
        let mut visits = 0usize;
        value
            .visit_logical_retained_heap_v1(&mut |count, width| {
                let next = count
                    .checked_mul(width)
                    .and_then(|b| bytes.checked_add(b))
                    .ok_or(())?;
                let next_visits = visits.checked_add(1).ok_or(())?;
                bytes = next;
                visits = next_visits;
                Ok::<_, ()>(())
            })
            .unwrap();
        (bytes, visits)
    }

    #[test]
    fn descriptor_all_owned_fields_use_actual_capacity_without_root_header() {
        let value = fixture();
        let kernel = &value.kernels[0];
        let argument = &kernel.arguments[0];
        let expected = value.compiler.name.0.capacity()
            + value.compiler.release.0.capacity()
            + value.producer.name.0.capacity()
            + value.producer.version.0.capacity()
            + value.type_records.capacity() * size_of::<SourceTypeRecordV1>()
            + value.layout_records.capacity() * size_of::<DeviceLayoutRecordV1>()
            + value.kernels.capacity() * size_of::<KernelDescriptorV1>()
            + kernel.logical_name.0.capacity()
            + kernel.entry_name.0.capacity()
            + kernel.descriptor_symbol.0.capacity()
            + kernel.capabilities.capacity() * size_of::<CapabilityV1>()
            + kernel.arguments.capacity() * size_of::<LogicalArgumentV1>()
            + argument.name.0.capacity()
            + argument.components.capacity() * size_of::<PhysicalAbiComponentV1>();
        let before = crate::encode_device_descriptor_table_v1(&value).unwrap();
        assert_eq!(total(&value).0, expected);
        assert_eq!(
            crate::encode_device_descriptor_table_v1(&value).unwrap(),
            before
        );
        assert_eq!(value.kernels[0].arguments[0].components.len(), 1);
        assert!(value.kernels[0].arguments[0].components.capacity() > 1);
    }

    #[test]
    fn descriptor_refusal_short_circuits_every_visit_and_preserves_owner() {
        let value = fixture();
        let before = crate::encode_device_descriptor_table_v1(&value).unwrap();
        for stop in 1..=total(&value).1 {
            let mut visits = 0;
            assert_eq!(
                value.visit_logical_retained_heap_v1(&mut |_, _| {
                    visits += 1;
                    if visits == stop { Err(stop) } else { Ok(()) }
                }),
                Err(stop)
            );
            assert_eq!(visits, stop);
        }
        assert_eq!(
            crate::encode_device_descriptor_table_v1(&value).unwrap(),
            before
        );
    }

    #[test]
    fn descriptor_visitor_propagates_checked_caller_overflow() {
        let value = fixture();
        let mut bytes = usize::MAX;
        let mut visits = 0;
        assert_eq!(
            value.visit_logical_retained_heap_v1(&mut |count, width| {
                visits += 1;
                bytes = count
                    .checked_mul(width)
                    .and_then(|b| bytes.checked_add(b))
                    .ok_or("overflow")?;
                Ok(())
            }),
            Err("overflow")
        );
        assert_eq!(visits, 2);
        assert_eq!(bytes, usize::MAX);
    }

    #[test]
    fn descriptor_empty_vectors_keep_their_actual_spare_capacity() {
        let mut value = fixture();
        value.type_records.clear();
        value.layout_records.clear();
        value.kernels.clear();
        // A storage visitor is not revalidation. Observe this private test
        // state rather than reconstructing a different decoded owner.
        let expected = value.compiler.name.0.capacity()
            + value.compiler.release.0.capacity()
            + value.producer.name.0.capacity()
            + value.producer.version.0.capacity()
            + value.type_records.capacity() * size_of::<SourceTypeRecordV1>()
            + value.layout_records.capacity() * size_of::<DeviceLayoutRecordV1>()
            + value.kernels.capacity() * size_of::<KernelDescriptorV1>();
        assert_eq!(total(&value).0, expected);
    }
}

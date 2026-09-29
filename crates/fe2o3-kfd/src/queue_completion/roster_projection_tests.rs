use super::*;
use fe2o3_runtime_model::{
    AllocationGenerationV1, AllocationIdV1, DeviceGenerationV1, DeviceKeyV1, MappingIdV1,
    MemoryAllocationKeyV1, PhysicalDeviceIdV1, QueueGenerationV1, QueueInstanceIdV1, VmIdV1,
    VmKeyV1,
};

#[derive(Debug, Eq, PartialEq)]
enum HashCall {
    Usize(usize),
    U64(u64),
    Write(Vec<u8>),
}

#[derive(Default)]
struct RecordingHasher {
    calls: Vec<HashCall>,
}

impl Hasher for RecordingHasher {
    fn finish(&self) -> u64 {
        panic!("roster commitments must consume SHA-256, not Hasher::finish")
    }

    fn write(&mut self, bytes: &[u8]) {
        self.calls.push(HashCall::Write(bytes.to_vec()));
    }

    fn write_usize(&mut self, value: usize) {
        self.calls.push(HashCall::Usize(value));
        self.write(&value.to_ne_bytes());
    }

    fn write_u64(&mut self, value: u64) {
        self.calls.push(HashCall::U64(value));
        self.write(&value.to_ne_bytes());
    }
}

fn vm(base: u64) -> VmKeyV1 {
    VmKeyV1 {
        device: DeviceKeyV1 {
            physical: PhysicalDeviceIdV1(base + 1),
            generation: DeviceGenerationV1(base + 2),
        },
        id: VmIdV1(base + 3),
    }
}

fn mapping(base: u64) -> MemoryMappingKeyV1 {
    MemoryMappingKeyV1 {
        allocation: MemoryAllocationKeyV1 {
            vm: vm(base),
            id: AllocationIdV1(base + 4),
            generation: AllocationGenerationV1(base + 5),
        },
        id: MappingIdV1(base + 6),
    }
}

fn template(index: usize) -> CompletionPacketTemplateV1 {
    // Deliberately distinct nested identity fields exercise the hash projection;
    // these metadata-only fixtures grant no mapping or submission authority.
    let index = index as u64;
    CompletionPacketTemplateV1::new(
        AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
        AqlDispatchOrderingV1::WaitForPrior,
        0,
        0,
        ObservedGpuAddressV1::new(0x40_0000 + index * 256).unwrap(),
        ObservedGpuAddressV1::new(0x80_0000 + index * 16).unwrap(),
        16,
        CompletionDispatchGenerationBindingV1::new(
            QueueKeyV1 {
                vm: vm(10),
                id: QueueInstanceIdV1(14),
                generation: QueueGenerationV1(15),
            },
            mapping(100 + index * 32),
            mapping(116 + index * 32),
            16,
        ),
    )
}

// Independent copy of the pre-projection algorithm, including its refusal and
// partial-feed order. Do not route this oracle through the production helpers.
fn legacy_feed<H: Hasher>(
    dispatches: &[CompletionDispatchGenerationBindingV1],
    hasher: &mut H,
) -> Result<(QueueKeyV1, u64), Gfx942CompletionErrorV1> {
    let first = dispatches
        .first()
        .ok_or(Gfx942CompletionErrorV1::ZeroPacketCount)?;
    if first.dispatch_generation == 0 {
        return Err(Gfx942CompletionErrorV1::StaleBatchGeneration);
    }
    dispatches.len().hash(hasher);
    for dispatch in dispatches {
        if dispatch.queue != first.queue
            || dispatch.dispatch_generation != first.dispatch_generation
        {
            return Err(Gfx942CompletionErrorV1::StaleBatchGeneration);
        }
        dispatch.hash(hasher);
    }
    Ok((first.queue, first.dispatch_generation))
}

fn legacy_roster(
    dispatches: &[CompletionDispatchGenerationBindingV1],
) -> Result<CompletionDispatchRosterV1, Gfx942CompletionErrorV1> {
    let mut hasher = CompletionOccurrenceHasherV1(Sha256::new());
    let (queue, dispatch_generation) = legacy_feed(dispatches, &mut hasher)?;
    Ok(CompletionDispatchRosterV1 {
        queue,
        packet_count: dispatches.len(),
        dispatch_generation,
        roster_sha256: hasher.0.finalize().into(),
    })
}

fn assert_feed_shape(hasher: &RecordingHasher, len: usize, hashed_bindings: usize) {
    assert_eq!(hasher.calls.first(), Some(&HashCall::Usize(len)));
    assert_eq!(
        hasher.calls.get(1),
        Some(&HashCall::Write(len.to_ne_bytes().to_vec()))
    );
    // Queue (5), code mapping (6), kernarg mapping (6), dispatch generation (1).
    let fields = hashed_bindings * 18;
    assert_eq!(hasher.calls.len(), 2 + fields * 2);
    for pair in hasher.calls[2..].chunks_exact(2) {
        let HashCall::U64(value) = pair[0] else {
            panic!("binding field was not hashed as its original u64 type")
        };
        assert_eq!(pair[1], HashCall::Write(value.to_ne_bytes().to_vec()));
    }
}

#[test]
fn roster_hash_adapters_use_trait_dispatch_and_preserve_literal_field_order() {
    #[derive(Hash)]
    struct ShadowedHash {
        first: u64,
        second: u64,
    }

    impl ShadowedHash {
        fn hash<H: Hasher>(&self, hasher: &mut H) {
            hasher.write_u64(u64::MAX);
        }
    }

    let value = ShadowedHash {
        first: 17,
        second: 29,
    };
    let mut inherent = RecordingHasher::default();
    value.hash(&mut inherent);
    assert_eq!(
        inherent.calls,
        vec![
            HashCall::U64(u64::MAX),
            HashCall::Write(u64::MAX.to_ne_bytes().to_vec()),
        ]
    );

    let mut actual = RecordingHasher::default();
    completion_roster_hash_length!(2_usize, &mut actual);
    completion_roster_hash_binding!(value, &mut actual);
    assert_eq!(
        actual.calls,
        vec![
            HashCall::Usize(2),
            HashCall::Write(2_usize.to_ne_bytes().to_vec()),
            HashCall::U64(17),
            HashCall::Write(17_u64.to_ne_bytes().to_vec()),
            HashCall::U64(29),
            HashCall::Write(29_u64.to_ne_bytes().to_vec()),
        ]
    );

    let mut actual = RecordingHasher::default();
    completion_roster_hash_length!(1_usize, &mut actual);
    completion_roster_hash_binding!(template(0).generations(), &mut actual);
    let mut expected = vec![
        HashCall::Usize(1),
        HashCall::Write(1_usize.to_ne_bytes().to_vec()),
    ];
    // Literal queue, code, kernarg, and generation fields; no Hash-based oracle.
    for field in [
        11_u64, 12, 13, 14, 15, 101, 102, 103, 104, 105, 106, 117, 118, 119, 120, 121, 122, 16,
    ] {
        expected.push(HashCall::U64(field));
        expected.push(HashCall::Write(field.to_ne_bytes().to_vec()));
    }
    assert_eq!(actual.calls, expected);
}

#[test]
fn roster_projection_matches_legacy_digest_typed_feed_and_exact_visits_at_scale() {
    for len in [1, 32, 1024, 8192] {
        let templates: Vec<_> = (0..len).map(template).collect();
        let bindings: Vec<_> = templates.iter().map(|value| value.generations()).collect();
        let expected = legacy_roster(&bindings).unwrap();
        assert_eq!(completion_dispatch_roster_v1(&bindings), Ok(expected));
        assert_eq!(
            completion_template_dispatch_roster_v1(&templates),
            Ok(expected)
        );
        assert_eq!(
            completion_dispatch_roster_with_visits_v1(&bindings),
            Ok((expected, len))
        );

        let mut legacy = RecordingHasher::default();
        let legacy_result = legacy_feed(&bindings, &mut legacy);
        let mut projected = RecordingHasher::default();
        let mut visited = Vec::new();
        let projected_result = hash_completion_dispatch_roster_projected_v1(
            &templates,
            |value| {
                let binding = value.generations();
                visited.push(binding);
                binding
            },
            &mut projected,
        );
        assert_eq!(projected_result, legacy_result);
        assert_eq!(visited, bindings);
        assert_eq!(projected.calls, legacy.calls);
        assert_feed_shape(&projected, len, len);
    }
}

#[test]
fn roster_projection_empty_and_first_zero_generation_do_not_hash() {
    for len in [0, 1, 32, 1024, 8192] {
        let mut templates: Vec<_> = (0..len).map(template).collect();
        if let Some(first) = templates.first_mut() {
            first.generations.dispatch_generation = 0;
        }
        let bindings: Vec<_> = templates.iter().map(|value| value.generations()).collect();
        let expected = || {
            if len == 0 {
                Gfx942CompletionErrorV1::ZeroPacketCount
            } else {
                Gfx942CompletionErrorV1::StaleBatchGeneration
            }
        };
        assert_eq!(legacy_roster(&bindings), Err(expected()));
        assert_eq!(completion_dispatch_roster_v1(&bindings), Err(expected()));
        assert_eq!(
            completion_template_dispatch_roster_v1(&templates),
            Err(expected())
        );
        let mut hasher = RecordingHasher::default();
        let mut visits = 0;
        let result = hash_completion_dispatch_roster_projected_v1(
            &templates,
            |value| {
                visits += 1;
                value.generations()
            },
            &mut hasher,
        );
        assert_eq!(result, Err(expected()));
        assert_eq!(visits, usize::from(len != 0));
        assert!(hasher.calls.is_empty());
    }
}

fn change_binding_field(binding: &mut CompletionDispatchGenerationBindingV1, field: usize) {
    match field {
        0 => binding.queue.vm.device.physical.0 += 1,
        1 => binding.queue.vm.device.generation.0 += 1,
        2 => binding.queue.vm.id.0 += 1,
        3 => binding.queue.id.0 += 1,
        4 => binding.queue.generation.0 += 1,
        5 => binding.code.allocation.vm.device.physical.0 += 1,
        6 => binding.code.allocation.vm.device.generation.0 += 1,
        7 => binding.code.allocation.vm.id.0 += 1,
        8 => binding.code.allocation.id.0 += 1,
        9 => binding.code.allocation.generation.0 += 1,
        10 => binding.code.id.0 += 1,
        11 => binding.kernarg.allocation.vm.device.physical.0 += 1,
        12 => binding.kernarg.allocation.vm.device.generation.0 += 1,
        13 => binding.kernarg.allocation.vm.id.0 += 1,
        14 => binding.kernarg.allocation.id.0 += 1,
        15 => binding.kernarg.allocation.generation.0 += 1,
        16 => binding.kernarg.id.0 += 1,
        17 => binding.dispatch_generation += 1,
        _ => panic!("unknown generation-binding field"),
    }
}

#[test]
fn roster_projection_mismatches_preserve_first_error_and_hashed_prefix() {
    for len in [32, 1024, 8192] {
        for mismatch in [0, len / 2, len - 1] {
            for field in [0, 1, 2, 3, 4, 17] {
                let mut templates: Vec<_> = (0..len).map(template).collect();
                change_binding_field(&mut templates[mismatch].generations, field);
                let bindings: Vec<_> = templates.iter().map(|value| value.generations()).collect();
                let expected = Err(Gfx942CompletionErrorV1::StaleBatchGeneration);
                assert_eq!(completion_dispatch_roster_v1(&bindings), expected);
                assert_eq!(completion_template_dispatch_roster_v1(&templates), expected);
                let mut legacy = RecordingHasher::default();
                let legacy_result = legacy_feed(&bindings, &mut legacy);
                let mut projected = RecordingHasher::default();
                let mut visited = Vec::new();
                let projected_result = hash_completion_dispatch_roster_projected_v1(
                    &templates,
                    |value| {
                        let binding = value.generations();
                        visited.push(binding);
                        binding
                    },
                    &mut projected,
                );
                assert_eq!(projected_result, legacy_result);
                assert_eq!(
                    projected_result,
                    Err(Gfx942CompletionErrorV1::StaleBatchGeneration)
                );
                // A changed first entry becomes the baseline; the second then
                // fails, exactly as in the original binding-slice traversal.
                let rejected = mismatch.max(1);
                assert_eq!(visited, bindings[..=rejected]);
                assert_eq!(projected.calls, legacy.calls);
                assert_feed_shape(&projected, len, rejected);
            }
        }
    }
}

#[test]
fn roster_projection_commits_every_nested_binding_field_and_order() {
    let original = template(0);
    let baseline = completion_template_dispatch_roster_v1(&[original]).unwrap();
    for field in 0..18 {
        let mut changed = original;
        change_binding_field(&mut changed.generations, field);
        let actual = completion_template_dispatch_roster_v1(&[changed]).unwrap();
        assert_eq!(actual, legacy_roster(&[changed.generations()]).unwrap());
        assert_ne!(
            actual.roster_sha256, baseline.roster_sha256,
            "field {field}"
        );
    }
    let mut templates: Vec<_> = (0..32).map(template).collect();
    let ordered = completion_template_dispatch_roster_v1(&templates).unwrap();
    templates.reverse();
    let reversed = completion_template_dispatch_roster_v1(&templates).unwrap();
    let bindings: Vec<_> = templates.iter().map(|value| value.generations()).collect();
    assert_eq!(reversed, legacy_roster(&bindings).unwrap());
    assert_ne!(reversed.roster_sha256, ordered.roster_sha256);
}

#[test]
fn roster_projection_excludes_non_generation_template_fields() {
    let original = template(0);
    let mut changed = original;
    changed.geometry = AqlDispatchGeometryV1::new([64, 4, 1], [32, 2, 1]).unwrap();
    changed.ordering = AqlDispatchOrderingV1::Independent;
    changed.private_segment_size = 128;
    changed.group_segment_size = 256;
    changed.kernel_object = ObservedGpuAddressV1::new(0xc0_0000).unwrap();
    changed.kernarg_address = ObservedGpuAddressV1::new(0xd0_0000).unwrap();
    changed.kernarg_alignment = 64;
    assert_ne!(changed, original);
    assert_eq!(
        completion_template_dispatch_roster_v1(&[changed]),
        legacy_roster(&[original.generations()])
    );
}

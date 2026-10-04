fn check_empty_scalar_slices<T: GeneratedDeviceScalarV1 + PartialEq + std::fmt::Debug>(
    initial: [T; 2],
) {
    let fields = vec![
        slice_field::<T>("input", 0, false),
        slice_field::<T>("output", 16, true),
    ];
    let manifest = AbiLayout::new(32, 8, PointerWidth::Bits64, fields.clone()).unwrap();
    let generated =
        CompilerGeneratedArgumentLayoutV1::new(32, 8, PointerWidth::Bits64, fields).unwrap();
    let plan =
        validate_argument_packing(KernelId::from_bytes([0x45; 32]), &manifest, &generated).unwrap();
    let mut output = initial;
    let input = GeneratedKfdReadSlice::new(&initial[1..1])
        .bind_argument(&plan, 0)
        .unwrap();
    let destination = GeneratedKfdReadWriteSlice::new(&mut output[1..1])
        .bind_argument(&plan, 1)
        .unwrap();
    let packed = GeneratedKfdArgumentBinding::from_compiler_generated_parts(
        Vec::new(),
        vec![input, destination],
    )
    .pack(&plan)
    .unwrap();
    let address = T::RUST_SCALAR_TYPE.size_bytes();
    assert_ne!(address, 0);
    assert_eq!(&packed.explicit_kernarg()[..8], &address.to_le_bytes());
    assert_eq!(&packed.explicit_kernarg()[16..24], &address.to_le_bytes());
    assert_eq!(&packed.explicit_kernarg()[8..16], &[0; 8]);
    assert_eq!(&packed.explicit_kernarg()[24..32], &[0; 8]);
    assert!(packed.buffers().is_empty());
    assert!(packed.pointer_fixups().is_empty());
    let observation = packed.packing_observation();
    assert!(observation.matches_explicit_kernarg(packed.explicit_kernarg()));
    assert!(!observation.matches_explicit_kernarg(&[0; 32]));
    assert_eq!(observation.buffers().len(), 2);
    for (index, binding) in observation.buffers().iter().enumerate() {
        assert_eq!(binding.argument_index(), index);
        assert_eq!(binding.buffer_index(), None);
        assert_eq!(binding.access(), None);
        assert_eq!(binding.initial_bytes(), 0);
        assert_eq!(binding.initial_sha256(), [0; 32]);
    }
    packed.completion.apply_completed_buffers(&[]).unwrap();
    assert_eq!(output, initial);
}

#[test]
fn every_generated_scalar_uses_its_own_empty_slice_alignment() {
    check_empty_scalar_slices([1_i8, 2]);
    check_empty_scalar_slices([1_u8, 2]);
    check_empty_scalar_slices([1_i16, 2]);
    check_empty_scalar_slices([1_u16, 2]);
    check_empty_scalar_slices([1_i32, 2]);
    check_empty_scalar_slices([1_u32, 2]);
    check_empty_scalar_slices([1_i64, 2]);
    check_empty_scalar_slices([1_u64, 2]);
    check_empty_scalar_slices([1.0_f32, 2.0]);
    check_empty_scalar_slices([1.0_f64, 2.0]);
}

#[test]
fn mixed_empty_and_nonempty_slices_preserve_real_buffer_fixups() {
    let plan = plan();
    let input: [i32; 0] = [];
    let mut output = [17_i32, 23];
    let source = GeneratedKfdReadSlice::new(&input)
        .bind_argument(&plan, 0)
        .unwrap();
    let destination = GeneratedKfdReadWriteSlice::new(&mut output)
        .bind_argument(&plan, 1)
        .unwrap();
    let packed = GeneratedKfdArgumentBinding::from_compiler_generated_parts(
        vec![plan.scalar(2, 0_u32).unwrap()],
        vec![source, destination],
    )
    .pack(&plan)
    .unwrap();
    assert_eq!(&packed.explicit_kernarg()[..8], &4_u64.to_le_bytes());
    assert_eq!(&packed.explicit_kernarg()[8..16], &[0; 8]);
    assert_eq!(&packed.explicit_kernarg()[16..24], &[0; 8]);
    assert_eq!(&packed.explicit_kernarg()[24..32], &2_u64.to_le_bytes());
    assert_eq!(packed.buffers().len(), 1);
    assert_eq!(
        packed.pointer_fixups(),
        [Gfx942KfdDispatchPointerFixupV1::new(16, 0, 0, 4)]
    );
    assert_eq!(
        packed.packing_observation().buffers()[0].buffer_index(),
        None
    );
    assert_eq!(
        packed.packing_observation().buffers()[1].buffer_index(),
        Some(0)
    );
    drop(packed);
    assert_eq!(output, [17, 23]);
}

#[test]
fn empty_write_only_slices_preserve_mapped_and_unmapped_borrows() {
    for space in [
        None,
        Some(RustDisjointIndexSpaceV1::blocked_index_1d(1, 8).unwrap()),
    ] {
        let plan = write_only_plan(space);
        let mut output = [17_i32, 23];
        let slice = GeneratedKfdWriteSlice::new(&mut output[1..1]);
        let binding = match space {
            None => slice.bind_argument(&plan, 0),
            Some(space) => slice.bind_mapped_argument(&plan, 0, space),
        }
        .unwrap();
        let packed =
            GeneratedKfdArgumentBinding::from_compiler_generated_parts(Vec::new(), vec![binding])
                .pack(&plan)
                .unwrap();
        assert_eq!(&packed.explicit_kernarg()[..8], &4_u64.to_le_bytes());
        assert!(packed.buffers().is_empty());
        assert!(packed.pointer_fixups().is_empty());
        packed.completion.apply_completed_buffers(&[]).unwrap();
        assert_eq!(output, [17, 23]);
    }
}

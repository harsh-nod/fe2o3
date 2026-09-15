fn transpose_codec_cases() -> Vec<SemanticCompilerIntrinsicOperationV1> {
    use SemanticGfx950TransposeOperationV1 as T;
    let id = SemanticTypeIdV1;
    let mut result = Vec::new();
    for format in [
        SemanticGfx950LdsTransposeFormatV1::Fp4E2M1,
        SemanticGfx950LdsTransposeFormatV1::Fp8E4M3,
    ] {
        for operation in [
            T::Issue {
                partition_reference: id(0),
                partition: id(1),
                tile: id(2),
            },
            T::Stage {
                input_tile: id(0),
                output_tile: id(1),
                view_reference: id(2),
                view: id(3),
                index: id(4),
                global_reference: id(5),
                global: id(6),
            },
            T::Publish {
                input_tile: id(0),
                input_workgroup: id(1),
                transition: id(2),
                output_workgroup: id(3),
                output_tile: id(4),
            },
            T::Read {
                tile: id(0),
                lane_reference: id(1),
                lane: id(2),
                fragment: id(3),
                registers: id(4),
                word: id(5),
            },
        ] {
            let transition = matches!(operation, T::Publish { .. });
            let transpose = SemanticGfx950TransposeContractV1::new(
                operation,
                format,
                SemanticTypeIdentityV1(identity(120)),
                transition.then_some(SemanticTypeIdentityV1(identity(121))),
            )
            .unwrap();
            result.push(SemanticCompilerIntrinsicOperationV1::ExecutionCapability {
                contract: execution_contract(
                    SemanticExecutionCapabilityOperationV1::Gfx950Transpose(transpose),
                )
                .unwrap(),
            });
        }
    }
    result
}

#[test]
fn transpose_v24_all_eight_records_round_trip_and_require_new_version() {
    let cases = transpose_codec_cases();
    assert_eq!(
        minimum_wire_version(&version_selection_request(cases.clone())),
        SemanticMirWireVersionV1::V24
    );
    for operation in cases {
        let bytes = compiler_intrinsic_round_trip(operation, SemanticMirWireVersionV1::V24);
        let mut old = CanonicalDecoderV1::new(&bytes, SemanticMirLimitsV1::default());
        old.wire_version = SemanticMirWireVersionV1::V23;
        assert!(matches!(
            old.compiler_intrinsic(),
            Err(SemanticMirDecodeErrorV1::InvalidTag {
                context: "execution capability operation",
                value: 29,
                ..
            })
        ));
        let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
        assert!(matches!(
            encode_compiler_intrinsic_operation(
                &mut writer,
                operation,
                SemanticMirWireVersionV1::V23
            ),
            Err(SemanticMirErrorV1::WireVersionCannotRepresent {
                required: SemanticMirWireVersionV1::V24,
                ..
            })
        ));
        assert!(writer.finish().is_empty());
    }
}

#[test]
fn transpose_decoder_rejects_unknown_family_format_and_reserved_slot() {
    for operation in transpose_codec_cases() {
        let SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract } = operation
        else {
            unreachable!()
        };
        let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
        encode_execution_capability_operation(&mut writer, contract.operation()).unwrap();
        let bytes = writer.finish();
        assert_eq!(bytes[0], 29);
        for (offset, byte) in [(0, 28), (0, 30), (1, 4), (2, 2)] {
            let mut changed = bytes.clone();
            changed[offset] = byte;
            let mut decoder = CanonicalDecoderV1::new(&changed, SemanticMirLimitsV1::default());
            decoder.wire_version = SemanticMirWireVersionV1::V24;
            assert!(matches!(
                decoder.execution_capability_operation(),
                Err(SemanticMirDecodeErrorV1::InvalidTag { .. })
            ));
        }
        for len in 0..bytes.len() {
            let mut decoder =
                CanonicalDecoderV1::new(&bytes[..len], SemanticMirLimitsV1::default());
            decoder.wire_version = SemanticMirWireVersionV1::V24;
            assert!(
                decoder.execution_capability_operation().is_err(),
                "truncated transpose prefix {len}"
            );
        }
    }
}

#[test]
fn transpose_v24_preserves_legacy_transpose_record_bytes() {
    let id = SemanticTypeIdV1;
    for operation in [
        SemanticCompilerIntrinsicOperationV1::Gfx950LdsTransposeCurrent {
            tile: id(0),
            lane: id(1),
            format: SemanticGfx950LdsTransposeFormatV1::Fp4E2M1,
        },
        SemanticCompilerIntrinsicOperationV1::Gfx950LdsTransposeStage {
            input_tile: id(0),
            output_tile: id(1),
            view: id(2),
            format: SemanticGfx950LdsTransposeFormatV1::Fp8E4M3,
        },
        SemanticCompilerIntrinsicOperationV1::Gfx950LdsTransposePublish {
            input_tile: id(0),
            output_tile: id(1),
            format: SemanticGfx950LdsTransposeFormatV1::Fp4E2M1,
        },
    ] {
        assert_eq!(
            compiler_intrinsic_round_trip(operation, SemanticMirWireVersionV1::V23),
            compiler_intrinsic_round_trip(operation, SemanticMirWireVersionV1::V24)
        );
    }
    // Existing full V19/V20/V21 decoder fixtures remain mounted unchanged.
}

#[test]
fn transpose_decoder_rejects_duplicate_types_zero_brand_and_missing_obligations() {
    for operation in transpose_codec_cases() {
        let SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract } = operation
        else {
            unreachable!()
        };
        let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
        encode_execution_capability_operation(&mut writer, contract.operation()).unwrap();
        let bytes = writer.finish();
        for zero_brand in [false, true] {
            let mut changed = bytes.clone();
            if zero_brand {
                let len = changed.len();
                changed[len - 32..].fill(0);
            } else {
                let first: [u8; 4] = changed[3..7].try_into().unwrap();
                changed[7..11].copy_from_slice(&first);
            }
            let mut decoder = CanonicalDecoderV1::new(&changed, SemanticMirLimitsV1::default());
            decoder.wire_version = SemanticMirWireVersionV1::V24;
            assert!(matches!(
                decoder.execution_capability_operation(),
                Err(SemanticMirDecodeErrorV1::Validation(
                    SemanticMirErrorV1::InvalidFunctionAbi
                ))
            ));
        }
        let mut changed = compiler_intrinsic_round_trip(operation, SemanticMirWireVersionV1::V24);
        let len = changed.len();
        changed[len - 34..len - 32].fill(0);
        let mut decoder = CanonicalDecoderV1::new(&changed, SemanticMirLimitsV1::default());
        decoder.wire_version = SemanticMirWireVersionV1::V24;
        assert!(matches!(
            decoder.compiler_intrinsic(),
            Err(SemanticMirDecodeErrorV1::Validation(
                SemanticMirErrorV1::InvalidFunctionAbi
            ))
        ));
        let mut limited = CanonicalWriterV1::new((bytes.len() - 1) as u64);
        assert!(matches!(
            encode_execution_capability_operation(&mut limited, contract.operation()),
            Err(SemanticMirErrorV1::LimitExceeded {
                resource: SemanticMirResourceV1::CanonicalBytes,
                ..
            })
        ));
    }
}

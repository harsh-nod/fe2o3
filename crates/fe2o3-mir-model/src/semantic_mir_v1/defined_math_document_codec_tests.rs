#[test]
fn defined_math_v21_codec_preserves_record_boundaries_before_admission() {
    let limits = SemanticMirLimitsV1::default();
    for (version, request) in [
        (
            SemanticMirWireVersionV1::V19,
            policy_math_request(SemanticF32MathFunctionV1::Sqrt),
        ),
        (
            SemanticMirWireVersionV1::V20,
            borrowed_workgroup_request(true),
        ),
        (
            SemanticMirWireVersionV1::V21,
            defined_math_document_request(false),
        ),
        (
            SemanticMirWireVersionV1::V21,
            defined_math_document_request(true),
        ),
    ] {
        let mut bytes = Vec::new();
        let mut boundaries = Vec::new();
        for ty in &request.types {
            let mut writer = CanonicalWriterV1::new(16384);
            encode_type(&mut writer, ty).unwrap();
            bytes.extend(writer.finish());
            boundaries.push(bytes.len());
        }
        let mut decoder = CanonicalDecoderV1::new(&bytes, limits);
        decoder.wire_version = version;
        for (index, ty) in request.types.iter().enumerate() {
            assert_eq!(decoder.ty().unwrap(), *ty, "{version:?} type {index}");
            assert_eq!(
                decoder.offset, boundaries[index],
                "{version:?} type {index} boundary"
            );
        }
        decoder.finish().unwrap();

        bytes.clear();
        boundaries.clear();
        for function in &request.functions {
            let mut writer = CanonicalWriterV1::new(16384);
            encode_function(&mut writer, function, version).unwrap();
            bytes.extend(writer.finish());
            boundaries.push(bytes.len());
        }
        let mut decoder = CanonicalDecoderV1::new(&bytes, limits);
        decoder.wire_version = version;
        for (index, function) in request.functions.iter().enumerate() {
            assert_eq!(
                decoder.function().unwrap(),
                *function,
                "{version:?} function {index}"
            );
            assert_eq!(
                decoder.offset, boundaries[index],
                "{version:?} function {index} tail"
            );
        }
        decoder.finish().unwrap();

        let encoded = encode_request(&request, version, limits).unwrap();
        let mut decoder =
            CanonicalDecoderV1::with_expected_wire_version(&encoded, limits, Some(version));
        assert_eq!(
            decoder.request().unwrap(),
            request,
            "{version:?} raw request"
        );
        decoder.finish().unwrap();
    }
}

#[test]
fn defined_math_v21_unsorted_type_error_is_admission_not_cursor_drift() {
    let limits = SemanticMirLimitsV1::default();
    let mut request = defined_math_document_request(true);
    request.types[12].identity = SemanticTypeIdentityV1(identity(24));
    let error = SemanticMirErrorV1::NonDeterministicOrder {
        entity: SemanticMirEntityV1::Type,
    };
    assert_eq!(request.clone().admit_exact_v21(limits).unwrap_err(), error);

    // Raw codec equality is distinct from canonical admission: do not sort or
    // repair an invalid roster while decoding it.
    let encoded = encode_request(&request, SemanticMirWireVersionV1::V21, limits).unwrap();
    let mut decoder = CanonicalDecoderV1::with_expected_wire_version(
        &encoded,
        limits,
        Some(SemanticMirWireVersionV1::V21),
    );
    assert_eq!(decoder.request().unwrap(), request);
    decoder.finish().unwrap();
    assert_eq!(
        AdmittedInertSemanticMirV1::decode_exact_v21_canonical(&encoded, limits).unwrap_err(),
        SemanticMirDecodeErrorV1::Validation(error),
    );
}

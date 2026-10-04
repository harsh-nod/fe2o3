impl<'tcx> Extractor<'tcx> {
    fn shared_slice_capture(
        &mut self,
        ty: Ty<'tcx>,
        pointee: Ty<'tcx>,
        backend: &BackendRepresentationFacts,
        path: &str,
        depth: usize,
    ) -> Result<TypeLayoutKind, GeneralLayoutExtractError> {
        let unsupported = |detail| GeneralLayoutExtractError::UnsupportedType {
            path: path.to_owned(),
            rust_type: type_name(ty),
            detail,
        };
        let TyKind::Slice(element) = *pointee.kind() else {
            return Err(unsupported("shared slice capture has a non-slice pointee"));
        };
        if !matches!(
            element.kind(),
            TyKind::Bool | TyKind::Char | TyKind::Int(_) | TyKind::Uint(_) | TyKind::Float(_)
        ) || self.active.iter().any(|ancestor| {
            *ancestor != ty && matches!(ancestor.kind(), TyKind::Ref(..) | TyKind::RawPtr(..))
        }) {
            return Err(unsupported(
                "shared slice captures require scalar elements and no outer reference",
            ));
        }
        let BackendRepresentationFacts::ScalarPair {
            first,
            second,
            second_offset_bytes,
        } = backend
        else {
            return Err(unsupported(
                "shared slice capture requires a scalar-pair carrier",
            ));
        };
        if first.primitive != (ScalarPrimitiveFacts::Pointer { address_space: 0 })
            || first.size_bytes != 8
            || first.abi_alignment_bytes != 8
            || !first.initialized
            || second.primitive
                != (ScalarPrimitiveFacts::Integer {
                    bits: 64,
                    signed: false,
                })
            || second.size_bytes != 8
            || second.abi_alignment_bytes != 8
            || !second.initialized
            || *second_offset_bytes != 8
        {
            return Err(unsupported(
                "shared slice capture requires exact 64-bit data/length layout",
            ));
        }
        let element =
            self.extract_type(element, format!("{path}.element"), depth.saturating_add(1))?;
        Ok(TypeLayoutKind::SharedSliceReference {
            element: Box::new(element),
        })
    }
}

// Actual sequential operations used by both the native codec and its proof.
// Later errors and panics retain the effects of earlier successful operations.
macro_rules! distributed_codec_bytes_equal_body_v1 {
    ($syntax:ident, $left:ident, $right:ident, $index:ident, [$($invariants:tt)*]) => {
        $syntax!({
            if $left.len() != $right.len() {
                return false;
            }
            let mut $index = 0usize;
            while $index < $left.len()
                $($invariants)*
            {
                if $left[$index] != $right[$index] {
                    return false;
                }
                $index += 1;
            }
            true
        })
    };
}

macro_rules! distributed_codec_read_header_body_v1 {
    ($syntax:ident, $bytes:ident, $offset:ident, $domain:ident) => {
        $syntax!({
            let actual_domain = match take($bytes, $offset, $domain.len()) {
                Ok(value) => value,
                Err(error) => return Err(error),
            };
            if !bytes_equal(actual_domain, $domain) {
                return Err(DistributedPublicationContractErrorV1::WrongDomain);
            }
            let schema = match fixed::<2>($bytes, $offset) {
                Ok(value) => value,
                Err(error) => return Err(error),
            };
            if u16_from_le(schema) != DISTRIBUTED_PUBLICATION_CONTRACT_SCHEMA_V1 {
                return Err(DistributedPublicationContractErrorV1::WrongSchema);
            }
            let reserved = match fixed::<2>($bytes, $offset) {
                Ok(value) => value,
                Err(error) => return Err(error),
            };
            if reserved[0] != 0 || reserved[1] != 0 {
                return Err(DistributedPublicationContractErrorV1::NonzeroReserved);
            }
            Ok(())
        })
    };
}

macro_rules! distributed_codec_write_header_body_v1 {
    ($syntax:ident, $bytes:ident, $offset:ident, $domain:ident) => {
        $syntax!({
            put($bytes, $offset, $domain);
            put(
                $bytes,
                $offset,
                &u16_le(DISTRIBUTED_PUBLICATION_CONTRACT_SCHEMA_V1),
            );
            put($bytes, $offset, &[0; 2]);
        })
    };
}

macro_rules! distributed_codec_read_u64_body_v1 {
    ($syntax:ident, $bytes:ident, $offset:ident) => {
        $syntax!({
            let value = match fixed::<8>($bytes, $offset) {
                Ok(value) => value,
                Err(error) => return Err(error),
            };
            Ok(u64_from_le(value))
        })
    };
}

macro_rules! distributed_codec_write_u64_body_v1 {
    ($syntax:ident, $bytes:ident, $offset:ident, $value:ident) => {
        $syntax!({
            put($bytes, $offset, &u64_le($value));
        })
    };
}

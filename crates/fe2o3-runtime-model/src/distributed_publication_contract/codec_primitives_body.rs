// Executable bodies shared by the real codec and its bounded byte proof.
// Neither the reader nor the proof assumes an initially valid cursor.
macro_rules! distributed_codec_take_body_v1 {
    ($syntax:ident, $bytes:ident, $offset:ident, $count:ident) => {
        $syntax!({
            let start = *$offset;
            let end = match start.checked_add($count) {
                Some(end) => end,
                None => return Err(DistributedPublicationContractErrorV1::WrongLength),
            };
            if end > $bytes.len() {
                return Err(DistributedPublicationContractErrorV1::WrongLength);
            }
            let value = &$bytes[start..end];
            *$offset = end;
            Ok(value)
        })
    };
}

macro_rules! distributed_codec_fixed_body_v1 {
    ($syntax:ident, $bytes:ident, $offset:ident, $size:ident) => {
        $syntax!({
            let value = match take($bytes, $offset, $size) {
                Ok(value) => value,
                Err(error) => return Err(error),
            };
            let mut result = [0u8; $size];
            result.copy_from_slice(value);
            Ok(result)
        })
    };
}

macro_rules! distributed_codec_put_body_v1 {
    ($syntax:ident, $bytes:ident, $offset:ident, $value:ident) => {
        $syntax!({
            let start = *$offset;
            let end = start + $value.len();
            let (_, tail) = $bytes.split_at_mut(start);
            let (destination, _) = tail.split_at_mut($value.len());
            destination.copy_from_slice($value);
            *$offset = end;
        })
    };
}

macro_rules! distributed_codec_finish_body_v1 {
    ($syntax:ident, $bytes:ident, $offset:ident) => {
        $syntax!({
            if $offset == $bytes.len() {
                Ok(())
            } else {
                Err(DistributedPublicationContractErrorV1::WrongLength)
            }
        })
    };
}

macro_rules! distributed_codec_u16_le_body_v1 {
    ($syntax:ident, $value:ident) => {
        $syntax!({ [$value as u8, ($value >> 8) as u8] })
    };
}

macro_rules! distributed_codec_u16_from_le_body_v1 {
    ($syntax:ident, $bytes:ident) => {
        $syntax!({ ($bytes[0] as u16) | (($bytes[1] as u16) << 8) })
    };
}

macro_rules! distributed_codec_u64_le_body_v1 {
    ($syntax:ident, $value:ident) => {
        $syntax!({
            [
                $value as u8,
                ($value >> 8) as u8,
                ($value >> 16) as u8,
                ($value >> 24) as u8,
                ($value >> 32) as u8,
                ($value >> 40) as u8,
                ($value >> 48) as u8,
                ($value >> 56) as u8,
            ]
        })
    };
}

macro_rules! distributed_codec_u64_from_le_body_v1 {
    ($syntax:ident, $bytes:ident) => {
        $syntax!({
            ($bytes[0] as u64)
                | (($bytes[1] as u64) << 8)
                | (($bytes[2] as u64) << 16)
                | (($bytes[3] as u64) << 24)
                | (($bytes[4] as u64) << 32)
                | (($bytes[5] as u64) << 40)
                | (($bytes[6] as u64) << 48)
                | (($bytes[7] as u64) << 56)
        })
    };
}

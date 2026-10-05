use std::ffi::OsString;

pub const ELEMENTS: usize = 65;
pub const PAYLOAD_BYTES: usize = ELEMENTS * size_of::<u32>();
pub const GUARD_BYTES: usize = 32;
pub const FRAME_BYTES: usize = PAYLOAD_BYTES + 2 * GUARD_BYTES;
pub const SENTINELS: [u8; 2] = [0xa5, 0x5a];

pub fn parse_devices(args: impl IntoIterator<Item = OsString>) -> Result<Option<[u64; 2]>, String> {
    let mut args = args.into_iter();
    let Some(first) = args.next() else {
        return Ok(None);
    };
    let second = args
        .next()
        .ok_or("native execution requires exactly two GPU unique IDs")?;
    if args.next().is_some() {
        return Err("native execution requires exactly two GPU unique IDs".into());
    }
    let parse = |value: OsString| {
        let text = value.to_str().ok_or("GPU unique ID is not UTF-8")?;
        let digits = text
            .strip_prefix("0x")
            .ok_or("GPU unique ID requires 0x prefix")?;
        if digits.is_empty() || digits.len() > 16 || !digits.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("GPU unique ID must contain 1 to 16 hexadecimal digits".to_owned());
        }
        let id = u64::from_str_radix(digits, 16).map_err(|e| e.to_string())?;
        if id == 0 {
            return Err("GPU unique ID must be nonzero".into());
        }
        Ok(id)
    };
    let ids = [parse(first)?, parse(second)?];
    if ids[0] == ids[1] {
        return Err("native execution requires two distinct GPU unique IDs".into());
    }
    Ok(Some(ids))
}

pub fn check_fill(values: &[u32]) -> Result<(), String> {
    if values.len() != ELEMENTS {
        return Err(format!(
            "fill returned {} elements, expected {ELEMENTS}",
            values.len()
        ));
    }
    for (index, value) in values.iter().enumerate() {
        if *value != index as u32 {
            return Err(format!("fill mismatch at {index}: observed {value}"));
        }
    }
    Ok(())
}

pub fn destination_frame(device: usize) -> Vec<u8> {
    vec![SENTINELS[device]; FRAME_BYTES]
}

pub fn check_transport(
    payloads: &[Vec<u8>; 2],
    sources: &[Vec<u8>; 2],
    destinations: &[Vec<u8>; 2],
) -> Result<(), String> {
    for device in 0..2 {
        if payloads[device].len() != PAYLOAD_BYTES || sources[device] != payloads[device] {
            return Err(format!(
                "peer source {device} changed or has the wrong extent"
            ));
        }
        let mut expected = destination_frame(device);
        if payloads[1 - device].len() != PAYLOAD_BYTES {
            return Err("peer payload has the wrong extent".into());
        }
        expected[GUARD_BYTES..GUARD_BYTES + PAYLOAD_BYTES].copy_from_slice(&payloads[1 - device]);
        if destinations[device] != expected {
            return Err(format!(
                "peer destination {device} payload or guards differ"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Result<Option<[u64; 2]>, String> {
        parse_devices(values.iter().map(OsString::from))
    }

    #[test]
    fn admission_only_or_exact_distinct_hardware_ids() {
        assert_eq!(args(&[]).unwrap(), None);
        assert_eq!(
            args(&["0x1", "0xffffffffffffffff"]).unwrap(),
            Some([1, u64::MAX])
        );
        assert_eq!(args(&["0xAB", "0x02"]).unwrap(), Some([171, 2]));
        for values in [
            vec!["0x1"],
            vec!["0x1", "0x2", "0x3"],
            vec!["0x1", "0x01"],
            vec!["0x0", "0x1"],
            vec!["0x1", "0x0"],
            vec!["1", "0x2"],
            vec!["0X1", "0x2"],
            vec!["0x", "0x2"],
            vec!["0x-1", "0x2"],
            vec!["0x+1", "0x2"],
            vec!["0x10000000000000000", "0x2"],
            vec!["0x1 ", "0x2"],
            vec!["0x1", "0xg"],
        ] {
            assert!(args(&values).is_err(), "{values:?}");
        }
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt as _;
            assert!(parse_devices([OsString::from_vec(vec![255]), "0x2".into()]).is_err());
        }
    }

    #[test]
    fn fill_checks_every_value_and_the_exact_extent() {
        let values: Vec<u32> = (0..ELEMENTS as u32).collect();
        check_fill(&values).unwrap();
        assert!(check_fill(&values[..ELEMENTS - 1]).is_err());
        let mut longer = values.clone();
        longer.push(ELEMENTS as u32);
        assert!(check_fill(&longer).is_err());
        for index in 0..ELEMENTS {
            let mut changed = values.clone();
            changed[index] ^= 1;
            assert!(check_fill(&changed).is_err(), "index {index}");
        }
    }

    #[test]
    fn transport_rejects_noops_and_every_source_payload_or_guard_corruption() {
        let payload: Vec<u8> = (0..ELEMENTS as u32).flat_map(u32::to_le_bytes).collect();
        let payloads = [payload.clone(), payload];
        let sources = payloads.clone();
        let mut destinations = [destination_frame(0), destination_frame(1)];
        assert!(check_transport(&payloads, &sources, &destinations).is_err());
        for destination in &mut destinations {
            destination[GUARD_BYTES..GUARD_BYTES + PAYLOAD_BYTES].copy_from_slice(&payloads[0]);
        }
        check_transport(&payloads, &sources, &destinations).unwrap();
        for device in 0..2 {
            for byte in 0..FRAME_BYTES {
                let mut changed = destinations.clone();
                changed[device][byte] ^= 1;
                assert!(check_transport(&payloads, &sources, &changed).is_err());
            }
            for byte in 0..PAYLOAD_BYTES {
                let mut changed = sources.clone();
                changed[device][byte] ^= 1;
                assert!(check_transport(&payloads, &changed, &destinations).is_err());
            }
            let mut short = payloads.clone();
            short[device].pop();
            assert!(check_transport(&short, &sources, &destinations).is_err());
            let mut missing_direction = destinations.clone();
            missing_direction[device] = destination_frame(device);
            assert!(check_transport(&payloads, &sources, &missing_direction).is_err());
            for grow in [false, true] {
                let mut changed = destinations.clone();
                if grow {
                    changed[device].push(SENTINELS[device]);
                } else {
                    changed[device].pop();
                }
                assert!(check_transport(&payloads, &sources, &changed).is_err());
                let mut changed = sources.clone();
                if grow {
                    changed[device].push(0);
                } else {
                    changed[device].pop();
                }
                assert!(check_transport(&payloads, &changed, &destinations).is_err());
            }
        }
    }

    #[test]
    fn transport_compares_each_destination_to_the_opposite_source() {
        let payloads = [vec![0x17; PAYLOAD_BYTES], vec![0xe8; PAYLOAD_BYTES]];
        let mut destinations = [destination_frame(0), destination_frame(1)];
        for device in 0..2 {
            destinations[device][GUARD_BYTES..GUARD_BYTES + PAYLOAD_BYTES]
                .copy_from_slice(&payloads[1 - device]);
        }
        check_transport(&payloads, &payloads, &destinations).unwrap();
        for device in 0..2 {
            let mut wrong_source = destinations.clone();
            wrong_source[device][GUARD_BYTES..GUARD_BYTES + PAYLOAD_BYTES]
                .copy_from_slice(&payloads[device]);
            assert!(check_transport(&payloads, &payloads, &wrong_source).is_err());
        }
    }
}

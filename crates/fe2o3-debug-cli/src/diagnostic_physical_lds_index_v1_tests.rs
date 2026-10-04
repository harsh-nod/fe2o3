//! Direct actual typed-session export checks, never source/hardware evidence.
use super::super::tests::{Files, backend};
use super::*;
#[test]
fn index_joins_three_allocations_two_waves_and_actual_barrier_memory_records() {
    let files = Files::new(129, false);
    let (input, mut b, _) = backend(&files, true);
    let path = files.directory.join("index.json");
    export(&mut b, &input, &path).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    assert!(bytes.len() <= BYTES);
    let doc: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(doc["schema"], SCHEMA);
    // Exact byte slice, not parsed/re-encoded JSON, is the indexed payload.
    let needle = b",\"payload\":";
    let start = bytes
        .windows(needle.len())
        .position(|w| w == needle)
        .unwrap()
        + needle.len();
    assert_eq!(&bytes[bytes.len() - 2..], b"}\n");
    let payload = &bytes[start..bytes.len() - 2];
    assert_eq!(doc["identity"]["payload_bytes"], payload.len());
    let mut hash = Sha256::new();
    hash.update(DOMAIN);
    hash.update((payload.len() as u64).to_le_bytes());
    hash.update(payload);
    assert_eq!(
        doc["identity"]["sha256"],
        serde_json::to_value(IndexDigest(hash.finalize().into())).unwrap()
    );
    let p = &doc["payload"];
    assert_eq!(
        p["configuration_identity"],
        serde_json::to_value(b.configuration).unwrap()
    );
    assert_eq!(
        p["canonical"]["sha256"],
        serde_json::to_value(IndexDigest(*input.canonical().identity().digest())).unwrap()
    );
    assert_eq!(
        p["request"]["sha256"],
        serde_json::to_value(IndexDigest(*input.request_digest())).unwrap()
    );
    assert_eq!(p["simulated"], true);
    assert_eq!(p["hardware_observed"], false);
    assert_eq!(p["capture_stop"], serde_json::Value::Null);
    let allocations = p["allocations"].as_array().unwrap();
    assert_eq!(allocations.len(), 3);
    assert_eq!(
        allocations
            .iter()
            .filter(|a| a["address_space"] == "workgroup" && a["byte_length"] == 512)
            .count(),
        1
    );
    let records = p["records"].as_array().unwrap();
    assert_eq!(records.len(), b.session.records_len());
    let mut arrival = [false; 128];
    let mut releases = Vec::new();
    let mut writes = Vec::new();
    let mut reads = Vec::new();
    let mut pending_global = 0;
    let mut pending_lds = 0;
    for (i, row) in records.iter().enumerate() {
        let actual = b.session.record(i).unwrap();
        assert_eq!(row["sequence"], i + 1);
        assert_eq!(row["producer_ordinal"], actual.ordinal());
        assert_eq!(
            row["scope"]["local"],
            serde_json::to_value(actual.invocation().local).unwrap()
        );
        let local = row["scope"]["local"][0].as_u64().unwrap();
        assert_eq!(row["scope"]["logical_wave"], local / 64);
        assert_eq!(row["scope"]["logical_lane"], local % 64);
        let event = &row["payload"];
        match event["kind"].as_str().unwrap() {
            "checkpoint" => {
                for pending in event["pending"].as_array().unwrap() {
                    assert_eq!(pending["numeric_availability"], "not_represented");
                    assert!(pending.get("bits").is_none());
                    match pending["kind"].as_str().unwrap() {
                        "pending_global_read" => pending_global += 1,
                        "pending_lds_read" => pending_lds += 1,
                        _ => panic!("closed pending kind"),
                    }
                }
            }
            "barrier" => match event["action"].as_str().unwrap() {
                "arrive" => {
                    assert_eq!(event["participants"], 1);
                    assert!(!arrival[local as usize]);
                    arrival[local as usize] = true;
                }
                "release" => {
                    assert_eq!(event["participants"], 128);
                    assert!(arrival.into_iter().all(|x| x));
                    releases.push(i);
                }
                _ => panic!("closed barrier"),
            },
            "memory" if event["address_space"] == "workgroup" => {
                match event["access"].as_str().unwrap() {
                    "read" => reads.push(i),
                    "write_committed" => writes.push(i),
                    _ => panic!("memory"),
                }
            }
            "memory" => {}
            _ => panic!("closed row"),
        }
    }
    assert!(arrival.into_iter().all(|x| x));
    assert_eq!(releases.len(), 1);
    assert_eq!((writes.len(), reads.len()), (128, 128));
    assert!(*writes.last().unwrap() < releases[0] && releases[0] < reads[0]);
    assert!(pending_global >= 128 && pending_lds >= 128);
    assert_eq!((b.sequence(), b.revision), (0, 0));
}
#[test]
fn index_never_replaces_existing_files_or_follows_symlinks() {
    let files = Files::new(13, true);
    let (input, mut b, _) = backend(&files, true);
    let path = files.directory.join("index.json");
    std::fs::write(&path, b"sentinel").unwrap();
    assert_eq!(export(&mut b, &input, &path), Err(OUTPUT));
    assert_eq!(std::fs::read(&path).unwrap(), b"sentinel");
    let link = files.directory.join("index-link.json");
    std::os::unix::fs::symlink(&path, &link).unwrap();
    assert_eq!(export(&mut b, &input, &link), Err(OUTPUT));
    assert_eq!(std::fs::read(&path).unwrap(), b"sentinel");
    assert_eq!((b.sequence(), b.revision), (0, 0));
}
#[test]
fn insufficient_export_work_never_opens_output_or_changes_cursor() {
    let files = Files::new(129, false);
    let (input, mut b, _) = backend(&files, true);
    let used = b.session.usage().work;
    b.session
        .charge_query_work(WORK - used - (PREPAID_WORK - 1))
        .unwrap();
    let before = b.session.usage();
    let path = files.directory.join("index.json");
    assert_eq!(export(&mut b, &input, &path), Err(BUDGET));
    assert!(!path.exists());
    let after = b.session.usage();
    assert_eq!(after.work, before.work);
    assert_eq!(
        (
            after.entry_storage,
            after.retained_storage,
            after.peak_storage
        ),
        (
            before.entry_storage,
            before.retained_storage,
            before.peak_storage
        )
    );
    assert!(after.failed_work.is_some());
    assert_eq!((b.sequence(), b.revision), (0, 0));
}
#[test]
fn index_requires_export_configuration_not_an_arbitrary_backend() {
    let files = Files::new(129, false);
    let (input, b, _) = backend(&files, false);
    assert!(matches!(encode(&b, &input), Err(SHAPE)));
}
#[test]
fn bounded_writers_and_exact_envelope_limits_do_not_grow() {
    let mut b = Bounded::new(4).unwrap();
    b.write_all(b"1234").unwrap();
    let capacity = b.bytes.capacity();
    assert!(b.write_all(b"5").is_err());
    assert_eq!(b.bytes, b"1234");
    assert_eq!(b.bytes.capacity(), capacity);
    let mut row = RowBuffer::new();
    assert!(row.write_all(&vec![0; ROW_BYTES]).is_ok());
    assert!(row.write_all(&[1]).is_err());
    assert_eq!(row.len, ROW_BYTES);
    assert!(envelope(&vec![b' '; PAYLOAD_BYTES]).is_ok());
    assert!(matches!(
        envelope(&vec![b' '; PAYLOAD_BYTES + 1]),
        Err(LIMIT)
    ));
    assert_eq!(
        CONFIGURATION_CAPS,
        [8388608, 8387584, 16384, 8, 768, 8, 16384, 8454144, 50462720]
    );
}
#[test]
fn envelope_identity_is_bound_to_exact_bytes_not_equivalent_json() {
    let a = envelope(b"{\"a\":1}").unwrap();
    let b = envelope(b"{\"a\": 1}").unwrap();
    assert_ne!(a.slice(), b.slice());
}

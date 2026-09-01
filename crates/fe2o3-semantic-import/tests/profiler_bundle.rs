use fe2o3_semantic_import::*;
use fe2o3_semantic_trace::*;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::OnceLock;

fn opaque(byte: u8) -> OpaqueIdentityV1 {
    OpaqueIdentityV1::new([byte; 32]).unwrap()
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn replace_first_csv_field(source: &[u8], column: &str, replacement: &str) -> Vec<u8> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(source);
    let headers = reader.headers().unwrap().clone();
    let column = headers.iter().position(|header| header == column).unwrap();
    let mut records = reader.records().collect::<Result<Vec<_>, _>>().unwrap();
    let first = records.first_mut().unwrap();
    *first = csv::StringRecord::from(
        first
            .iter()
            .enumerate()
            .map(|(index, value)| {
                if index == column {
                    replacement.to_owned()
                } else {
                    value.to_owned()
                }
            })
            .collect::<Vec<_>>(),
    );
    let mut writer = csv::WriterBuilder::new()
        .has_headers(false)
        .from_writer(Vec::new());
    writer.write_record(&headers).unwrap();
    for record in records {
        writer.write_record(&record).unwrap();
    }
    writer.into_inner().unwrap()
}

fn content(byte: u8, len: u64) -> ContentIdentityRecordV1 {
    ContentIdentityRecordV1 {
        scheme: ContentSchemeV1::DomainSeparatedSha256,
        format_version: 1,
        digest: CaptureIdentityV1::new([byte; 32]).unwrap(),
        canonical_len: len,
    }
}

fn environment(devices: &[u8]) -> ProfilerEnvironmentBindingV4 {
    let source_agents = [17_u64, 19_u64];
    ProfilerEnvironmentBindingV4 {
        environment: content(10, 200),
        collector_tool: content(11, 50),
        collector_configuration: content(12, 80),
        stable_device_bindings: devices
            .iter()
            .enumerate()
            .map(|(index, byte)| ProfilerDeviceBindingV4 {
                source_agent_id: source_agents[index],
                stable_identity: content(*byte, 64),
            })
            .collect(),
    }
}

fn dispatch_binding(devices: &[u8]) -> ProfilerDispatchBindingV4 {
    ProfilerDispatchBindingV4 {
        environment: environment(devices),
        kernel_ir_claim: KernelIrIdentityClaimV1::canonical_v7_claim(opaque(1), 97).unwrap(),
        artifact: Some(ArtifactClaimV1 {
            identity: opaque(2),
            canonical_len: 4_096,
            format_version: 1,
        }),
        source_map: Some(
            ContentIdentityV1::new(
                ContentIdentitySchemeV1::RawCanonicalSha256,
                1,
                opaque(3),
                777,
            )
            .unwrap(),
        ),
        wave_width: WaveWidthV1::Wave64,
    }
}

fn json_dispatch_binding(devices: &[u8]) -> ProfilerDispatchBindingV4 {
    let mut binding = dispatch_binding(devices);
    for (device, node_id) in binding
        .environment
        .stable_device_bindings
        .iter_mut()
        .zip([7_u64, 8])
    {
        device.source_agent_id = node_id;
    }
    binding
}

fn import_json_bundle(
    source: &[u8],
    binding: ProfilerDispatchBindingV4,
) -> Result<SemanticProfilerBundleV4, ProfilerBundleErrorV4> {
    import_rocprofv3_json_profiler_bundle_v4(source, binding)
}

fn complete_installed_process(process: &mut serde_json::Value) {
    process["metadata"]["node"] = serde_json::json!({
        "id": 0, "hash": 0, "machine_id": "fixture", "system_name": "Linux",
        "hostname": "fixture", "release": "fixture", "version": "fixture",
        "hardware_name": "x86_64", "domain_name": "(none)"
    });
    let buffers = process["buffer_records"].as_object_mut().unwrap();
    for name in [
        "hip_api",
        "hsa_api",
        "rccl_api",
        "rocdecode_api",
        "rocjpeg_api",
        "marker_api",
        "memory_copy",
        "memory_allocation",
        "scratch_memory",
        "pc_sample_host_trap",
        "pc_sample_stochastic",
    ] {
        buffers.insert(name.to_owned(), serde_json::json!([]));
    }
    process["callback_records"] = serde_json::json!({"counter_collection": []});
    process["counters"] = serde_json::json!([]);
    process["code_objects"] = serde_json::json!([]);
    process["kernel_symbols"] = serde_json::json!([]);
    process["strings"] = serde_json::json!({
        "callback_records": [],
        "buffer_records": [],
        "marker_api": [],
        "correlation_id": {"external": []},
        "counters": {"dimension_ids": []},
        "pc_sample_instructions": [],
        "pc_sample_comments": [],
        "att_filenames": [],
        "code_object_snapshot_filenames": []
    });
    process["summary"] = serde_json::json!([]);
    process["host_functions"] = serde_json::json!([]);
}

fn complete_installed_agent(agent: &mut serde_json::Value) {
    let object = agent.as_object_mut().unwrap();
    for (name, value) in [
        ("size", serde_json::json!(312)),
        ("type", serde_json::json!(2)),
        ("gpu_index", serde_json::json!(0)),
        ("logical_node_id", serde_json::json!(1)),
        ("logical_node_type_id", serde_json::json!(2)),
        ("cpu_cores_count", serde_json::json!(0)),
        ("cpu_core_id_base", serde_json::json!(0)),
        ("simd_id_base", serde_json::json!(0)),
        ("max_waves_per_simd", serde_json::json!(8)),
        ("lds_size_in_kb", serde_json::json!(64)),
        ("gds_size_in_kb", serde_json::json!(0)),
        ("num_gws", serde_json::json!(64)),
        ("cu_count", serde_json::json!(304)),
        ("array_count", serde_json::json!(8)),
        ("num_shader_banks", serde_json::json!(4)),
        ("simd_arrays_per_engine", serde_json::json!(2)),
        ("cu_per_simd_array", serde_json::json!(19)),
        ("simd_per_cu", serde_json::json!(4)),
        ("max_slots_scratch_cu", serde_json::json!(32)),
        ("drm_render_minor", serde_json::json!(128)),
        ("num_sdma_engines", serde_json::json!(4)),
        ("num_sdma_xgmi_engines", serde_json::json!(0)),
        ("num_sdma_queues_per_engine", serde_json::json!(8)),
        ("num_cp_queues", serde_json::json!(8)),
        ("max_engine_clk_ccompute", serde_json::json!(2100)),
        ("max_engine_clk_fcompute", serde_json::json!(2100)),
        (
            "sdma_fw_version",
            serde_json::json!({"uCodeSDMA":1,"uCodeRes":0}),
        ),
        (
            "fw_version",
            serde_json::json!({"uCode":1,"Major":0,"Minor":0,"Stepping":0}),
        ),
        (
            "capability",
            serde_json::json!({"HotPluggable":0,"HSAMMUPresent":0,"SharedWithGraphics":0,"QueueSizePowerOfTwo":0,"QueueSize32bit":0,"QueueIdleEvent":0,"VALimit":0,"WatchPointsSupported":1,"WatchPointsTotalBits":2,"DoorbellType":2,"AQLQueueDoubleMap":0,"DebugTrapSupported":1,"WaveLaunchTrapOverrideSupported":1,"WaveLaunchModeSupported":1,"PreciseMemoryOperationsSupported":1,"DEPRECATED_SRAM_EDCSupport":0,"Mem_EDCSupport":1,"RASEventNotify":1,"ASICRevision":1,"SRAM_EDCSupport":1,"SVMAPISupported":1,"CoherentHostAccess":0,"DebugSupportedFirmware":1}),
        ),
        ("cu_per_engine", serde_json::json!(38)),
        ("max_waves_per_cu", serde_json::json!(32)),
        ("family_id", serde_json::json!(145)),
        ("workgroup_max_size", serde_json::json!(1024)),
        ("grid_max_size", serde_json::json!(4294967295_u64)),
        ("local_mem_size", serde_json::json!(65536)),
        ("hive_id", serde_json::json!(1)),
        (
            "workgroup_max_dim",
            serde_json::json!({"x":1024,"y":1024,"z":1024}),
        ),
        (
            "grid_max_dim",
            serde_json::json!({"x":2147483647_u64,"y":65535,"z":65535}),
        ),
        ("name", serde_json::json!("gfx942")),
        ("vendor_name", serde_json::json!("AMD")),
        ("product_name", serde_json::json!("MI300X")),
        ("model_name", serde_json::json!("MI300X")),
        (
            "uuid",
            serde_json::json!({"bytes":{"value0":1,"value1":2,"value2":3,"value3":4,"value4":5,"value5":6,"value6":7,"value7":8,"value8":0,"value9":0,"value10":0,"value11":0,"value12":0,"value13":0,"value14":0,"value15":0}}),
        ),
        ("mem_banks", serde_json::json!([])),
        ("mem_banks_count", serde_json::json!(0)),
        ("caches", serde_json::json!([])),
        ("caches_count", serde_json::json!(0)),
        ("io_links", serde_json::json!([])),
        ("io_links_count", serde_json::json!(0)),
        (
            "runtime_visibility",
            serde_json::json!({"hsa":1,"hip":1,"rccl":1,"rocdecode":1}),
        ),
    ] {
        object.entry(name.to_owned()).or_insert(value);
    }
}

fn json_source() -> &'static [u8] {
    static SOURCE: OnceLock<Vec<u8>> = OnceLock::new();
    SOURCE
        .get_or_init(|| {
            let mut value: serde_json::Value = serde_json::from_slice(
                br#"{"rocprofiler-sdk-tool":[{"metadata":{"node":{},"pid":1,"init_time":1,"fini_time":2,"command":[],"config":{}},"buffer_records":{"kernel_dispatch":[{"size":184,"kind":11,"operation":2,"thread_id":100,"correlation_id":{"internal":1,"external":0},"start_timestamp":100,"end_timestamp":180,"dispatch_info":{"size":72,"agent_id":{"handle":17},"queue_id":{"handle":1},"kernel_id":10,"dispatch_id":1,"private_segment_size":0,"group_segment_size":0,"workgroup_size":{"x":64,"y":1,"z":1},"grid_size":{"x":256,"y":1,"z":1}},"stream_id":{"handle":0}}]}},{"metadata":{"node":{},"pid":2,"init_time":1,"fini_time":2,"command":[],"config":{}},"buffer_records":{"kernel_dispatch":[{"size":184,"kind":11,"operation":2,"thread_id":101,"correlation_id":{"internal":2,"external":0},"start_timestamp":200,"end_timestamp":260,"dispatch_info":{"size":72,"agent_id":{"handle":19},"queue_id":{"handle":2},"kernel_id":11,"dispatch_id":2,"private_segment_size":0,"group_segment_size":0,"workgroup_size":{"x":32,"y":2,"z":1},"grid_size":{"x":128,"y":2,"z":1}},"stream_id":{"handle":0}}]}}]}"#,
            )
            .unwrap();
            for (process_index, process) in value["rocprofiler-sdk-tool"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .enumerate()
            {
                let handle = process["buffer_records"]["kernel_dispatch"][0]["dispatch_info"]
                    ["agent_id"]["handle"]
                    .as_u64()
                    .unwrap();
                let node_id = [7_u64, 8][process_index];
                let mut agent = serde_json::json!({
                    "id": {"handle": handle}, "node_id": node_id, "simd_count": 304,
                    "gpu_id": 42 + node_id, "vendor_id": 4098, "device_id": 29857,
                    "location_id": 1, "domain": 0, "gfx_target_version": 90402,
                    "wave_front_size": 64, "num_xcc": 8
                });
                complete_installed_agent(&mut agent);
                process["agents"] = serde_json::json!([agent]);
                complete_installed_process(process);
            }
            serde_json::to_vec(&value).unwrap()
        })
        .as_slice()
}

fn json_source_with_agent_catalog() -> Vec<u8> {
    let mut value = serde_json::json!({
        "rocprofiler-sdk-tool": [{
            "metadata": {"node": {}, "pid": 1, "init_time": 1, "fini_time": 2, "command": [], "config": {}},
            "agents": [{
                "id": {"handle": 7001},
                "type": 2,
                "gpu_index": 0,
                "node_id": 7,
                "simd_count": 304,
                "gpu_id": 42,
                "vendor_id": 4098,
                "device_id": 29857,
                "location_id": 1,
                "domain": 0,
                "gfx_target_version": 90402,
                "wave_front_size": 64,
                "num_xcc": 8
            }],
            "buffer_records": {"kernel_dispatch": [{
                "size": 184,
                "kind": 11,
                "operation": 2,
                "thread_id": 100,
                "correlation_id": {"internal": 1, "external": 0},
                "start_timestamp": 100,
                "end_timestamp": 180,
                "dispatch_info": {
                    "size": 72,
                    "agent_id": {"handle": 7001},
                    "queue_id": {"handle": 1},
                    "kernel_id": 10,
                    "dispatch_id": 1,
                    "private_segment_size": 0,
                    "group_segment_size": 0,
                    "workgroup_size": {"x": 64, "y": 1, "z": 1},
                    "grid_size": {"x": 256, "y": 1, "z": 1}
                },
                "stream_id": {"handle": 0}
            }]}
        }]
    });
    complete_installed_agent(&mut value["rocprofiler-sdk-tool"][0]["agents"][0]);
    complete_installed_process(&mut value["rocprofiler-sdk-tool"][0]);
    serde_json::to_vec(&value).unwrap()
}

fn strict_side_capture_source(fixture: &[u8], counter: bool) -> Vec<u8> {
    let mut value: serde_json::Value = serde_json::from_slice(fixture).unwrap();
    let process = &mut value["rocprofiler-sdk-tool"][0];
    let original_buffers = process["buffer_records"].clone();
    let raw_dispatches = if counter {
        process["callback_records"]["counter_collection"]
            .as_array()
            .unwrap()
            .iter()
            .map(|collection| collection["dispatch_data"].clone())
            .collect::<Vec<_>>()
    } else {
        original_buffers["kernel_dispatch"]
            .as_array()
            .unwrap()
            .clone()
    };
    let mut dispatches = Vec::new();
    for (ordinal, mut dispatch) in raw_dispatches.into_iter().enumerate() {
        let object = dispatch.as_object_mut().unwrap();
        object.insert("size".to_owned(), serde_json::json!(184));
        object.insert("kind".to_owned(), serde_json::json!(11));
        object.insert("operation".to_owned(), serde_json::json!(2));
        object.insert("thread_id".to_owned(), serde_json::json!(100 + ordinal));
        object
            .entry("correlation_id".to_owned())
            .or_insert_with(|| serde_json::json!({"internal": ordinal + 1, "external": 0}));
        object.insert("stream_id".to_owned(), serde_json::json!({"handle": 0}));
        let dispatch_info = object["dispatch_info"].as_object_mut().unwrap();
        dispatch_info
            .entry("size".to_owned())
            .or_insert_with(|| serde_json::json!(72));
        dispatch_info
            .entry("queue_id".to_owned())
            .or_insert_with(|| serde_json::json!({"handle": 1}));
        dispatch_info
            .entry("kernel_id".to_owned())
            .or_insert_with(|| serde_json::json!(10 + ordinal));
        dispatch_info
            .entry("private_segment_size".to_owned())
            .or_insert_with(|| serde_json::json!(0));
        dispatch_info
            .entry("group_segment_size".to_owned())
            .or_insert_with(|| serde_json::json!(0));
        dispatches.push(dispatch);
    }
    let source_agent_id = dispatches[0]["dispatch_info"]["agent_id"]["handle"]
        .as_u64()
        .unwrap();
    process["metadata"] = serde_json::json!({
        "node": {}, "pid": 41052, "init_time": 1, "fini_time": 2,
        "command": [], "config": {}
    });
    process["buffer_records"] = serde_json::json!({"kernel_dispatch": dispatches});
    let mut agent = serde_json::json!({
        "id": {"handle": source_agent_id}, "node_id": 7, "simd_count": 304,
        "gpu_id": 42, "vendor_id": 4098, "device_id": 29857,
        "location_id": 1, "domain": 0, "gfx_target_version": 90402,
        "wave_front_size": 64, "num_xcc": 8
    });
    complete_installed_agent(&mut agent);
    process["agents"] = serde_json::json!([agent]);
    let original_counters = process["counters"].clone();
    let original_callbacks = process["callback_records"].clone();
    complete_installed_process(process);
    if counter {
        process["counters"] = original_counters;
        process["callback_records"] = original_callbacks;
    } else {
        process["buffer_records"]["pc_sample_host_trap"] = original_buffers
            .get("pc_sample_host_trap")
            .cloned()
            .unwrap_or_else(|| serde_json::json!([]));
        process["buffer_records"]["pc_sample_stochastic"] =
            original_buffers["pc_sample_stochastic"].clone();
    }
    serde_json::to_vec(&value).unwrap()
}

fn side_capture_binding() -> RocprofCaptureBindingV1 {
    RocprofCaptureBindingV1 {
        kernel_ir_claim: KernelIrIdentityClaimV1::canonical_v7_claim(opaque(1), 97).unwrap(),
        artifact: Some(ArtifactClaimV1 {
            identity: opaque(2),
            canonical_len: 4_096,
            format_version: 1,
        }),
        source_map: Some(
            ContentIdentityV1::new(
                ContentIdentitySchemeV1::RawCanonicalSha256,
                1,
                opaque(3),
                777,
            )
            .unwrap(),
        ),
        wave_width: WaveWidthV1::Wave64,
    }
}

fn csv_source() -> &'static [u8] {
    include_bytes!("fixtures/rocprofv3-current-kernel-dispatch.csv")
}

#[test]
fn json_and_csv_bundles_are_canonical_bounded_and_identity_bound() {
    let json = import_json_bundle(json_source(), json_dispatch_binding(&[20, 21])).unwrap();
    let csv =
        import_rocprofv3_csv_profiler_bundle_v4(csv_source(), dispatch_binding(&[20, 21])).unwrap();

    for bundle in [&json, &csv] {
        let bytes = encode_profiler_bundle_v4(bundle).unwrap();
        assert_eq!(decode_profiler_bundle_v4(&bytes).unwrap(), *bundle);
        assert_eq!(bundle.run_identity_origin, TruthOriginV1::Inferred);
        assert_eq!(bundle.environment.origin, TruthOriginV1::Declared);
        assert_eq!(bundle.source.origin, TruthOriginV1::Observed);
        assert_eq!(bundle.coverage.imported_dispatches, 2);
        assert_eq!(bundle.coverage.loss.state, LossStateV1::Unknown);
        assert_eq!(
            bundle.devices[0].stable_identity.value,
            Some(content(20, 64))
        );
        assert!(
            bundle
                .unavailable
                .contains(&ProfilerUnavailableFactV4::WaitEvents)
        );
        assert!(
            bundle
                .unavailable
                .contains(&ProfilerUnavailableFactV4::DecodedAttEvents)
        );
    }
    let projection = project_rocprofv3_json_dispatch_agents_v4(json_source()).unwrap();
    assert_eq!(
        projection
            .agent_bindings()
            .iter()
            .map(|binding| binding.source_agent_id)
            .collect::<Vec<_>>(),
        [17, 19]
    );
    assert_eq!(
        projection
            .agent_bindings()
            .iter()
            .map(|binding| binding.node_id)
            .collect::<Vec<_>>(),
        [7, 8]
    );
    assert_ne!(json.source.value, json.normalized_projection.value);
    assert_eq!(
        json.normalized_projection.value,
        Some(json.dispatch_capture.as_ref().unwrap().runs[0].source)
    );
    assert_ne!(csv.source.value, csv.normalized_projection.value);
    assert_eq!(
        csv.normalized_projection.value,
        Some(csv.dispatch_capture.as_ref().unwrap().runs[0].source)
    );
}

#[test]
fn json_import_requires_the_exact_projection_and_absolute_node_bindings() {
    let source = json_source();
    let projection = project_rocprofv3_json_dispatch_agents_v4(source).unwrap();

    assert!(matches!(
        import_projected_rocprofv3_json_profiler_bundle_v4(
            source,
            &projection,
            dispatch_binding(&[20, 21]),
        ),
        Err(ProfilerBundleErrorV4::MissingDeviceBinding)
    ));

    let mut substituted_source: serde_json::Value = serde_json::from_slice(source).unwrap();
    substituted_source["rocprofiler-sdk-tool"][0]["agents"][0]["gpu_id"] = serde_json::json!(9_999);
    let substituted_source = serde_json::to_vec(&substituted_source).unwrap();
    assert!(matches!(
        import_projected_rocprofv3_json_profiler_bundle_v4(
            &substituted_source,
            &projection,
            json_dispatch_binding(&[20, 21]),
        ),
        Err(ProfilerBundleErrorV4::StaleReference)
    ));
}

#[test]
fn raw_source_relation_replays_projected_bundle_and_rejects_identity_domain_substitution() {
    let source = json_source();
    let bundle = import_json_bundle(source, json_dispatch_binding(&[20, 21])).unwrap();
    let relation = validate_rocprofv3_bundle_raw_source_relation_v1(
        source,
        &bundle,
        ImportLimitsV1::default(),
    )
    .unwrap();
    let v1_source =
        rocprofv3_json_source_content_identity_v1(source, ImportLimitsV1::default()).unwrap();
    let v4_source = rocprofv3_json_profiler_source_content_identity_v4(source).unwrap();
    assert_eq!(relation.source(), v1_source);
    assert_eq!(bundle.source.value, Some(v4_source));
    assert_ne!(v1_source, v4_source);
    assert_ne!(bundle.source.value, bundle.normalized_projection.value);

    let mut substituted = bundle;
    substituted.source.value = Some(v1_source);
    assert!(
        validate_rocprofv3_bundle_raw_source_relation_v1(
            source,
            &substituted,
            ImportLimitsV1::default(),
        )
        .is_err()
    );
}

#[test]
fn counter_relation_crosses_exact_raw_handle_to_projected_node_domains() {
    let source = strict_side_capture_source(
        include_bytes!("fixtures/rocprofv3-1.1-counter-collection.json"),
        true,
    );
    let bundle = import_json_bundle(&source, json_dispatch_binding(&[20])).unwrap();
    let admitted = validate_rocprofv3_bundle_raw_source_relation_v1(
        &source,
        &bundle,
        ImportLimitsV1::default(),
    )
    .unwrap();
    let counters = import_rocprofv3_counter_capture_v2(
        &source,
        side_capture_binding(),
        ImportLimitsV1::default(),
    )
    .unwrap();
    let relation = validate_rocprofv3_counter_bundle_relation_v1(
        &source,
        &bundle,
        admitted,
        &counters,
        ImportLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(relation.bundle_dispatch_ordinals(), [0, 1]);
    assert_ne!(
        counters.dispatches[0].device_identity,
        bundle.dispatch_capture.as_ref().unwrap().dispatches[0].device_identity
    );

    let mut catalog_substitution: serde_json::Value = serde_json::from_slice(&source).unwrap();
    catalog_substitution["rocprofiler-sdk-tool"][0]["agents"][0]["node_id"] = serde_json::json!(8);
    assert!(
        validate_rocprofv3_counter_bundle_relation_v1(
            &serde_json::to_vec(&catalog_substitution).unwrap(),
            &bundle,
            admitted,
            &counters,
            ImportLimitsV1::default(),
        )
        .is_err()
    );

    let mut ordinal_substitution = counters;
    ordinal_substitution.dispatches.swap(0, 1);
    assert!(
        validate_rocprofv3_counter_bundle_relation_v1(
            &source,
            &bundle,
            admitted,
            &ordinal_substitution,
            ImportLimitsV1::default(),
        )
        .is_err()
    );
}

#[test]
fn pc_relation_crosses_exact_raw_handle_to_projected_node_domains() {
    let source = strict_side_capture_source(
        include_bytes!("fixtures/rocprofv3-1.1-stochastic-pc-sampling.json"),
        false,
    );
    let bundle = import_json_bundle(&source, json_dispatch_binding(&[20])).unwrap();
    let admitted = validate_rocprofv3_bundle_raw_source_relation_v1(
        &source,
        &bundle,
        ImportLimitsV1::default(),
    )
    .unwrap();
    let pc = import_rocprofv3_pc_sample_capture_v3(
        &source,
        RocprofPcSampleCaptureBindingV3 {
            capture: side_capture_binding(),
            sampling_interval_cycles: 1_048_576,
        },
        ImportLimitsV1::default(),
    )
    .unwrap();
    let relation = validate_rocprofv3_pc_bundle_relation_v1(
        &source,
        &bundle,
        admitted,
        &pc,
        ImportLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(relation.bundle_dispatch_ordinals(), [0, 1]);
    assert_ne!(
        pc.dispatches[0].device_identity,
        bundle.dispatch_capture.as_ref().unwrap().dispatches[0].device_identity
    );

    let mut catalog_substitution: serde_json::Value = serde_json::from_slice(&source).unwrap();
    catalog_substitution["rocprofiler-sdk-tool"][0]["agents"][0]["node_id"] = serde_json::json!(8);
    assert!(
        validate_rocprofv3_pc_bundle_relation_v1(
            &serde_json::to_vec(&catalog_substitution).unwrap(),
            &bundle,
            admitted,
            &pc,
            ImportLimitsV1::default(),
        )
        .is_err()
    );

    let mut ordinal_substitution = pc;
    ordinal_substitution.dispatches.swap(0, 1);
    assert!(
        validate_rocprofv3_pc_bundle_relation_v1(
            &source,
            &bundle,
            admitted,
            &ordinal_substitution,
            ImportLimitsV1::default(),
        )
        .is_err()
    );
}

#[test]
fn json_dispatch_protocol_rejects_unknown_duplicate_and_trailing_input() {
    let value: serde_json::Value = serde_json::from_slice(json_source()).unwrap();
    for pointer in [
        "/unknown",
        "/rocprofiler-sdk-tool/0/unknown",
        "/rocprofiler-sdk-tool/0/buffer_records/unknown",
        "/rocprofiler-sdk-tool/0/buffer_records/kernel_dispatch/0/unknown",
        "/rocprofiler-sdk-tool/0/buffer_records/kernel_dispatch/0/dispatch_info/unknown",
        "/rocprofiler-sdk-tool/0/buffer_records/kernel_dispatch/0/dispatch_info/agent_id/unknown",
        "/rocprofiler-sdk-tool/0/buffer_records/kernel_dispatch/0/dispatch_info/grid_size/unknown",
    ] {
        let mut hostile = value.clone();
        let (parent, field) = pointer.rsplit_once('/').unwrap();
        hostile
            .pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert(field.to_owned(), serde_json::json!(1));
        assert!(matches!(
            import_json_bundle(
                &serde_json::to_vec(&hostile).unwrap(),
                json_dispatch_binding(&[20, 21])
            ),
            Err(ProfilerBundleErrorV4::InvalidRocprofJson)
        ));
    }

    for pointer in [
        "/rocprofiler-sdk-tool/0/buffer_records/kernel_dispatch/0/size",
        "/rocprofiler-sdk-tool/0/buffer_records/kernel_dispatch/0/kind",
        "/rocprofiler-sdk-tool/0/buffer_records/kernel_dispatch/0/operation",
        "/rocprofiler-sdk-tool/0/buffer_records/kernel_dispatch/0/thread_id",
        "/rocprofiler-sdk-tool/0/buffer_records/kernel_dispatch/0/correlation_id",
        "/rocprofiler-sdk-tool/0/buffer_records/kernel_dispatch/0/stream_id",
        "/rocprofiler-sdk-tool/0/buffer_records/kernel_dispatch/0/dispatch_info/size",
        "/rocprofiler-sdk-tool/0/buffer_records/kernel_dispatch/0/dispatch_info/queue_id",
        "/rocprofiler-sdk-tool/0/buffer_records/kernel_dispatch/0/dispatch_info/kernel_id",
        "/rocprofiler-sdk-tool/0/buffer_records/kernel_dispatch/0/dispatch_info/dispatch_id",
        "/rocprofiler-sdk-tool/0/buffer_records/kernel_dispatch/0/dispatch_info/private_segment_size",
        "/rocprofiler-sdk-tool/0/buffer_records/kernel_dispatch/0/dispatch_info/group_segment_size",
    ] {
        let mut hostile = value.clone();
        let (parent, field) = pointer.rsplit_once('/').unwrap();
        hostile
            .pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(matches!(
            import_json_bundle(
                &serde_json::to_vec(&hostile).unwrap(),
                json_dispatch_binding(&[20, 21])
            ),
            Err(ProfilerBundleErrorV4::InvalidRocprofJson)
        ));
    }

    let duplicate = json_source()
        .strip_suffix(b"}")
        .unwrap()
        .iter()
        .copied()
        .chain(br#",\"rocprofiler-sdk-tool\":[]}"#.iter().copied())
        .collect::<Vec<_>>();
    assert!(matches!(
        import_json_bundle(&duplicate, json_dispatch_binding(&[20, 21])),
        Err(ProfilerBundleErrorV4::InvalidRocprofJson)
    ));

    let mut trailing = json_source().to_vec();
    trailing.extend_from_slice(b"false");
    assert!(matches!(
        import_json_bundle(&trailing, json_dispatch_binding(&[20, 21])),
        Err(ProfilerBundleErrorV4::InvalidRocprofJson)
    ));
}

#[test]
fn device_bindings_join_by_absolute_kfd_node_not_position() {
    let mut binding = json_dispatch_binding(&[20, 21]);
    binding.environment.stable_device_bindings.reverse();
    binding
        .environment
        .stable_device_bindings
        .push(ProfilerDeviceBindingV4 {
            source_agent_id: 99,
            stable_identity: content(22, 64),
        });
    let bundle = import_json_bundle(json_source(), binding).unwrap();
    assert_eq!(
        bundle.devices[0].stable_identity.value,
        Some(content(20, 64))
    );
    assert_eq!(
        bundle.devices[1].stable_identity.value,
        Some(content(21, 64))
    );
    assert_eq!(bundle.devices.len(), 2);

    let missing = ProfilerDispatchBindingV4 {
        environment: json_dispatch_binding(&[20]).environment,
        ..json_dispatch_binding(&[20, 21])
    };
    assert!(matches!(
        import_json_bundle(json_source(), missing),
        Err(ProfilerBundleErrorV4::MissingDeviceBinding)
    ));

    let mut duplicate = json_dispatch_binding(&[20, 21]);
    duplicate.environment.stable_device_bindings[1].source_agent_id = 7;
    assert!(matches!(
        import_json_bundle(json_source(), duplicate),
        Err(ProfilerBundleErrorV4::DuplicateSourceAgentBinding)
    ));
}

#[test]
fn json_agent_catalog_maps_opaque_handle_to_node_and_rejects_collisions() {
    let source = json_source_with_agent_catalog();
    let projection = project_rocprofv3_json_dispatch_agents_v4(&source).unwrap();
    let bindings = projection.agent_bindings();
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].source_agent_id, 7001);
    assert_eq!(bindings[0].node_id, 7);
    assert_ne!(bindings[0].source_agent_id, u64::from(bindings[0].node_id));

    let base: serde_json::Value = serde_json::from_slice(&source).unwrap();
    let mut with_unused_gpu = base.clone();
    let mut unused = with_unused_gpu["rocprofiler-sdk-tool"][0]["agents"][0].clone();
    unused["id"]["handle"] = serde_json::json!(7002);
    unused["node_id"] = serde_json::json!(8);
    unused["gpu_id"] = serde_json::json!(43);
    with_unused_gpu["rocprofiler-sdk-tool"][0]["agents"]
        .as_array_mut()
        .unwrap()
        .push(unused);
    let projection =
        project_rocprofv3_json_dispatch_agents_v4(&serde_json::to_vec(&with_unused_gpu).unwrap())
            .unwrap();
    assert_eq!(projection.agent_bindings().len(), 1);
    assert_eq!(projection.agent_bindings()[0].node_id, 7);

    for (pointer, replacement, expected) in [
        (
            "/rocprofiler-sdk-tool/0/buffer_records/kernel_dispatch/0/dispatch_info/agent_id/handle",
            serde_json::json!(8),
            ProfilerBundleErrorV4::MissingDeviceBinding,
        ),
        (
            "/rocprofiler-sdk-tool/0/agents/0/simd_count",
            serde_json::json!(0),
            ProfilerBundleErrorV4::MissingDeviceBinding,
        ),
    ] {
        let mut hostile = base.clone();
        *hostile.pointer_mut(pointer).unwrap() = replacement;
        let actual =
            project_rocprofv3_json_dispatch_agents_v4(&serde_json::to_vec(&hostile).unwrap())
                .unwrap_err();
        assert!(
            matches!(
                (actual, expected),
                (
                    ProfilerBundleErrorV4::MissingDeviceBinding,
                    ProfilerBundleErrorV4::MissingDeviceBinding
                )
            ),
            "unexpected projection error variant"
        );
    }

    for field in ["id", "node_id"] {
        let mut hostile = base.clone();
        let mut duplicate = hostile["rocprofiler-sdk-tool"][0]["agents"][0].clone();
        if field == "id" {
            duplicate["node_id"] = serde_json::json!(8);
        } else {
            duplicate["id"]["handle"] = serde_json::json!(7002);
        }
        hostile["rocprofiler-sdk-tool"][0]["agents"]
            .as_array_mut()
            .unwrap()
            .push(duplicate);
        assert!(matches!(
            project_rocprofv3_json_dispatch_agents_v4(&serde_json::to_vec(&hostile).unwrap())
                .unwrap_err(),
            ProfilerBundleErrorV4::InvalidDevice
        ));
    }

    let mut hostile = base;
    hostile["rocprofiler-sdk-tool"][0]["agents"][0]["unknown"] = serde_json::json!(1);
    assert!(matches!(
        project_rocprofv3_json_dispatch_agents_v4(&serde_json::to_vec(&hostile).unwrap())
            .unwrap_err(),
        ProfilerBundleErrorV4::InvalidRocprofJson
    ));
}

#[test]
fn reviewed_json_dialects_are_distinct_and_remain_synthetic() {
    let manifest = include_str!("fixtures/rocprofv3-current-schema-fixture-v1.txt");
    let expected_manifest_lines = [
        "schema=fe2o3-rocprofv3-current-schema-fixture-v1",
        "synthetic_schema_fixture=true",
        "collector_invoked=false",
        "hardware_executed=false",
        "authenticated_observation=false",
        "installed_raw_json_sha256=c53f1812ad7953ff1d24e6f81cfe7672dc2dcadb0b47e58065b575e607017e8d",
        "installed_raw_json_len=3356",
        "installed_projection_sha256=88650f8a5ff86e4ecd6d80922f8e5e285009cdd57c8a5fa68184b0d2ba9e89ad",
        "installed_projection_len=241",
        "forward_raw_json_sha256=670fc75c363ec1e11afdcfd744039f5a188da4f4e43791dc80593d83e53936ce",
        "forward_raw_json_len=3479",
        "forward_projection_sha256=88650f8a5ff86e4ecd6d80922f8e5e285009cdd57c8a5fa68184b0d2ba9e89ad",
        "forward_projection_len=241",
        "raw_csv_sha256=4fa2c7bc1dab9236f9c73d6ea6d7738a2e461c5707a97f44d84754e9b74eda47",
        "raw_csv_len=503",
        "installed_schema_commit=97f5574fe2fdc7bef44fb01545347912ee9f1779",
        "installed_agent_info_blob=9ce6fbb8051bf450fd6b8f5fb2c3bc1360f68f17",
        "installed_agent_info_len=2650",
        "installed_agent_info_sha256=339a5b781d0b060e9f220a8d6028cc4e58e13915b65f0c40bc19299f75259037",
        "installed_generate_csv_blob=242781505021097df0887eb5795447a22b69f163",
        "installed_generate_csv_len=46481",
        "installed_generate_csv_sha256=4eaf257bb6f3da314478253f8cb4ab664f7a4875f2167bdc40e989516f96d967",
        "installed_generate_json_blob=7eb8b243ff13de5daaadef9ec849d167bf84be13",
        "installed_generate_json_len=9500",
        "installed_generate_json_sha256=eda7b2157a4e78ddac2d28eba02ad3b4fdbbf75e2eab719a39e705b0c68e4d02",
        "installed_metadata_blob=abe9e00b50b96e6fd8e3c7657df9136aaa2b1419",
        "installed_metadata_len=28610",
        "installed_metadata_sha256=bc3b42a85bc1f792187969b59fecd7eef2d0f3fd9de383e297a9a8d50bae03ed",
        "installed_stream_info_blob=990c586c0ebe117045ec69280fc7dae10f232663",
        "installed_stream_info_path=projects/rocprofiler-sdk/source/lib/output/stream_info.hpp",
        "installed_stream_info_len=7520",
        "installed_stream_info_sha256=649ea59d98edeeabb6d5d3c1607e9ada8d80a9603aa5adb3d3d14e5e61561f04",
        "installed_save_blob=c68eb20aa9fc8f2a8dd67154f1a660295cb4c6f0",
        "installed_save_len=44146",
        "installed_save_sha256=033630e3f821495eaa1ee941ec8341ff8c226e72265b06af2f3933dfd4388b8c",
        "forward_schema_commit=848868dc7b195d569afe6be615fd4954c87ab8cb",
        "forward_agent_info_blob=9ce6fbb8051bf450fd6b8f5fb2c3bc1360f68f17",
        "forward_agent_info_len=2650",
        "forward_agent_info_sha256=339a5b781d0b060e9f220a8d6028cc4e58e13915b65f0c40bc19299f75259037",
        "forward_generate_json_blob=3290b32873f662baddf214d1b405e0189cf84956",
        "forward_generate_json_len=10286",
        "forward_generate_json_sha256=7a0d2d5c9a148ba24b6b7f11c1e215a7b60c8d0a0d059c2aa2afa286257fa7a1",
        "forward_generate_csv_blob=92960a5ab7bf4758d240064a98d20d17d158ec02",
        "forward_generate_csv_len=47386",
        "forward_generate_csv_sha256=605742ed44a9a9baaf0912e533605dcce000f22c1963ae15a710663ef75590d0",
        "forward_metadata_blob=5fc309a4d7d25eda75f96b2961d0ff23ec554898",
        "forward_metadata_len=29982",
        "forward_metadata_sha256=9957618140d7b8bc65905fcc5e9dcda1093681dbc7e07e42154c445578d6a87b",
        "forward_stream_info_blob=bc93b710f99065c2f2cb7053dcfa6258ba0e5e58",
        "forward_stream_info_path=projects/rocprofiler-sdk/source/lib/output/stream_info.hpp",
        "forward_stream_info_len=8568",
        "forward_stream_info_sha256=2d66198e03e78dee0d20a78aaca906137335ec87ba38a1bc8eabaf2b29e1d9de",
        "forward_save_blob=d39ac7c800ffd3f286864b08dcfbf00d94b206db",
        "forward_save_path=projects/rocprofiler-sdk/source/include/rocprofiler-sdk/cxx/serialization/save.hpp",
        "forward_save_len=48952",
        "forward_save_sha256=dedb8fb50d09009f48c2b4d487dbcb55679d94eb5c35eebecc7c1d46029c7cd4",
        "fixture_note=closed synthetic serializer-shape fixtures and exact current 22-column CSV header; no collected result is represented",
    ];
    assert_eq!(
        manifest.lines().collect::<Vec<_>>(),
        expected_manifest_lines
    );
    let installed =
        include_bytes!("fixtures/rocprofv3-installed-97f5574-kernel-dispatch-schema.json");
    let forward = include_bytes!("fixtures/rocprofv3-forward-848868-kernel-dispatch-schema.json");
    let installed_projection = project_rocprofv3_json_dispatch_agents_v4(installed).unwrap();
    let forward_projection = project_rocprofv3_json_dispatch_agents_v4(forward).unwrap();
    assert_eq!(
        installed_projection.dialect(),
        RocprofDispatchSchemaDialectV4::InstalledRocprofv3_1_1_97f5574
    );
    assert_eq!(
        forward_projection.dialect(),
        RocprofDispatchSchemaDialectV4::ForwardRocprofv3_848868
    );
    for projection in [&installed_projection, &forward_projection] {
        assert_eq!(projection.agent_bindings().len(), 1);
        assert_eq!(projection.agent_bindings()[0].process_id, 100);
        assert_eq!(projection.agent_bindings()[0].source_agent_id, 7001);
        assert_eq!(projection.agent_bindings()[0].node_id, 7);
    }
    assert_eq!(installed.len(), 3356);
    assert_eq!(
        sha256_hex(installed),
        "c53f1812ad7953ff1d24e6f81cfe7672dc2dcadb0b47e58065b575e607017e8d"
    );
    assert_eq!(installed_projection.canonical_json().len(), 241);
    assert_eq!(
        sha256_hex(installed_projection.canonical_json()),
        "88650f8a5ff86e4ecd6d80922f8e5e285009cdd57c8a5fa68184b0d2ba9e89ad"
    );
    assert_eq!(forward.len(), 3479);
    assert_eq!(
        sha256_hex(forward),
        "670fc75c363ec1e11afdcfd744039f5a188da4f4e43791dc80593d83e53936ce"
    );
    assert_eq!(forward_projection.canonical_json().len(), 241);
    assert_eq!(
        sha256_hex(forward_projection.canonical_json()),
        "88650f8a5ff86e4ecd6d80922f8e5e285009cdd57c8a5fa68184b0d2ba9e89ad"
    );
    assert_eq!(csv_source().len(), 503);
    assert_eq!(
        sha256_hex(csv_source()),
        "4fa2c7bc1dab9236f9c73d6ea6d7738a2e461c5707a97f44d84754e9b74eda47"
    );

    let installed_value: serde_json::Value = serde_json::from_slice(installed).unwrap();
    for (parent, field) in [
        (
            "/rocprofiler-sdk-tool/0/callback_records",
            "spm_counter_collection",
        ),
        ("/rocprofiler-sdk-tool/0/buffer_records", "kfd"),
        ("/rocprofiler-sdk-tool/0/buffer_records", "hip_graph"),
        ("/rocprofiler-sdk-tool/0/buffer_records", "hipfile_api"),
        ("/rocprofiler-sdk-tool/0/buffer_records", "rocshmem_api"),
        (
            "/rocprofiler-sdk-tool/0/buffer_records/kernel_dispatch/0",
            "graph_exec_id",
        ),
        (
            "/rocprofiler-sdk-tool/0/buffer_records/kernel_dispatch/0",
            "graph_node_id",
        ),
    ] {
        let mut hostile = installed_value.clone();
        hostile
            .pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert(field.to_owned(), serde_json::Value::Null);
        assert!(matches!(
            project_rocprofv3_json_dispatch_agents_v4(&serde_json::to_vec(&hostile).unwrap()),
            Err(ProfilerBundleErrorV4::InvalidRocprofJson)
        ));
    }
    let oversized = "x".repeat(64 * 1024 + 1);
    let mut oversized_command = installed_value.clone();
    oversized_command["rocprofiler-sdk-tool"][0]["metadata"]["command"] =
        serde_json::json!([oversized.clone()]);
    assert!(matches!(
        project_rocprofv3_json_dispatch_agents_v4(&serde_json::to_vec(&oversized_command).unwrap()),
        Err(ProfilerBundleErrorV4::InvalidRocprofJson)
    ));
    let mut oversized_key = installed_value.clone();
    oversized_key["rocprofiler-sdk-tool"][0]["metadata"]["config"] =
        serde_json::json!({(oversized): 1});
    assert!(matches!(
        project_rocprofv3_json_dispatch_agents_v4(&serde_json::to_vec(&oversized_key).unwrap()),
        Err(ProfilerBundleErrorV4::InvalidRocprofJson)
    ));
    for (pointer, replacement) in [
        (
            "/rocprofiler-sdk-tool/0/agents/0/gpu_index",
            serde_json::json!("0"),
        ),
        (
            "/rocprofiler-sdk-tool/0/agents/0/type",
            serde_json::json!(1),
        ),
    ] {
        let mut hostile = installed_value.clone();
        *hostile.pointer_mut(pointer).unwrap() = replacement;
        assert!(
            project_rocprofv3_json_dispatch_agents_v4(&serde_json::to_vec(&hostile).unwrap())
                .is_err()
        );
    }
    let forward_value: serde_json::Value = serde_json::from_slice(forward).unwrap();
    for (pointer, replacement) in [
        (
            "/rocprofiler-sdk-tool/0/buffer_records/hipfile_api",
            serde_json::json!(7),
        ),
        (
            "/rocprofiler-sdk-tool/0/buffer_records/kernel_dispatch/0/graph_exec_id",
            serde_json::json!({"handle": 0}),
        ),
    ] {
        let mut hostile = forward_value.clone();
        *hostile.pointer_mut(pointer).unwrap() = replacement;
        assert!(
            project_rocprofv3_json_dispatch_agents_v4(&serde_json::to_vec(&hostile).unwrap())
                .is_err()
        );
    }
    let mut hybrid = forward_value;
    hybrid["rocprofiler-sdk-tool"][0]["buffer_records"]["kernel_dispatch"][0]
        .as_object_mut()
        .unwrap()
        .remove("graph_node_id");
    assert!(
        project_rocprofv3_json_dispatch_agents_v4(&serde_json::to_vec(&hybrid).unwrap()).is_err()
    );

    for source in [installed.as_slice(), forward.as_slice()] {
        let base: serde_json::Value = serde_json::from_slice(source).unwrap();
        for missing in ["agents", "kernel_dispatch"] {
            let mut hostile = base.clone();
            let mut auxiliary = hostile["rocprofiler-sdk-tool"][0].clone();
            auxiliary["metadata"]["pid"] = serde_json::json!(101);
            if missing == "agents" {
                auxiliary.as_object_mut().unwrap().remove("agents");
            } else {
                auxiliary["buffer_records"]
                    .as_object_mut()
                    .unwrap()
                    .remove("kernel_dispatch");
            }
            hostile["rocprofiler-sdk-tool"]
                .as_array_mut()
                .unwrap()
                .push(auxiliary);
            assert!(
                project_rocprofv3_json_dispatch_agents_v4(&serde_json::to_vec(&hostile).unwrap())
                    .is_err()
            );
        }
    }
    for (pointer, replacement) in [
        (
            "/rocprofiler-sdk-tool/0/agents/0/sdma_fw_version",
            serde_json::Value::Null,
        ),
        (
            "/rocprofiler-sdk-tool/0/agents/0/uuid/bytes/value0",
            serde_json::json!("1"),
        ),
        (
            "/rocprofiler-sdk-tool/0/agents/0/runtime_visibility",
            serde_json::json!(true),
        ),
        (
            "/rocprofiler-sdk-tool/0/metadata/node",
            serde_json::json!({}),
        ),
        (
            "/rocprofiler-sdk-tool/0/metadata/init_time",
            serde_json::json!("1"),
        ),
        (
            "/rocprofiler-sdk-tool/0/metadata/command",
            serde_json::json!({}),
        ),
        (
            "/rocprofiler-sdk-tool/0/metadata/config",
            serde_json::json!([]),
        ),
    ] {
        let mut hostile = installed_value.clone();
        *hostile.pointer_mut(pointer).unwrap() = replacement;
        assert!(
            project_rocprofv3_json_dispatch_agents_v4(&serde_json::to_vec(&hostile).unwrap())
                .is_err(),
            "accepted hostile mutation at {pointer}"
        );
    }
    for (pointer, field) in [
        ("/rocprofiler-sdk-tool/0/agents/0/fw_version", "uCode"),
        (
            "/rocprofiler-sdk-tool/0/agents/0/capability",
            "DebugSupportedFirmware",
        ),
    ] {
        let mut hostile = installed_value.clone();
        hostile
            .pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(
            project_rocprofv3_json_dispatch_agents_v4(&serde_json::to_vec(&hostile).unwrap())
                .is_err(),
            "accepted missing {field} at {pointer}"
        );
    }
    let mut extra_capability = installed_value.clone();
    extra_capability["rocprofiler-sdk-tool"][0]["agents"][0]["capability"]["Reserved"] =
        serde_json::json!(0);
    assert!(
        project_rocprofv3_json_dispatch_agents_v4(&serde_json::to_vec(&extra_capability).unwrap())
            .is_err()
    );

    let mut nonempty_strings = installed_value.clone();
    nonempty_strings["rocprofiler-sdk-tool"][0]["strings"]["correlation_id"]["external"] =
        serde_json::json!([{"key": 1, "value": "external-correlation"}]);
    nonempty_strings["rocprofiler-sdk-tool"][0]["strings"]["counters"]["dimension_ids"] =
        serde_json::json!([{"id": 2, "instance_size": 8, "name": "xcc"}]);
    assert!(
        project_rocprofv3_json_dispatch_agents_v4(&serde_json::to_vec(&nonempty_strings).unwrap())
            .is_ok()
    );
    for (pointer, replacement) in [
        (
            "/rocprofiler-sdk-tool/0/strings/correlation_id/external",
            serde_json::json!([{"key": "1", "value": "external-correlation"}]),
        ),
        (
            "/rocprofiler-sdk-tool/0/strings/counters/dimension_ids",
            serde_json::json!([{"id": 2, "instance_size": 8, "label": "xcc"}]),
        ),
        (
            "/rocprofiler-sdk-tool/0/agents/0/sdma_fw_version/uCodeSDMA",
            serde_json::json!(1024),
        ),
        (
            "/rocprofiler-sdk-tool/0/agents/0/fw_version/Major",
            serde_json::json!(64),
        ),
        (
            "/rocprofiler-sdk-tool/0/agents/0/capability/HotPluggable",
            serde_json::json!(2),
        ),
        (
            "/rocprofiler-sdk-tool/0/agents/0/capability/WatchPointsTotalBits",
            serde_json::json!(16),
        ),
        (
            "/rocprofiler-sdk-tool/0/agents/0/capability/DoorbellType",
            serde_json::json!(4),
        ),
        (
            "/rocprofiler-sdk-tool/0/agents/0/capability/ASICRevision",
            serde_json::json!(16),
        ),
        (
            "/rocprofiler-sdk-tool/0/agents/0/runtime_visibility/hsa",
            serde_json::json!(2),
        ),
        (
            "/rocprofiler-sdk-tool/0/agents/0/logical_node_id",
            serde_json::json!(2_147_483_648_u64),
        ),
        (
            "/rocprofiler-sdk-tool/0/buffer_records/kernel_dispatch/0/dispatch_info/grid_size/x",
            serde_json::json!(4_294_967_296_u64),
        ),
        (
            "/rocprofiler-sdk-tool/0/agents/0/mem_banks_count",
            serde_json::json!(1),
        ),
        (
            "/rocprofiler-sdk-tool/0/agents/0/caches_count",
            serde_json::json!(1),
        ),
        (
            "/rocprofiler-sdk-tool/0/agents/0/io_links_count",
            serde_json::json!(1),
        ),
    ] {
        let mut hostile = installed_value.clone();
        *hostile.pointer_mut(pointer).unwrap() = replacement;
        assert!(
            project_rocprofv3_json_dispatch_agents_v4(&serde_json::to_vec(&hostile).unwrap())
                .is_err(),
            "accepted hostile boundary at {pointer}"
        );
    }
}

#[test]
fn json_agent_projection_is_process_scoped_and_preserves_raw_source() {
    let base: serde_json::Value =
        serde_json::from_slice(&json_source_with_agent_catalog()).unwrap();
    let first = base["rocprofiler-sdk-tool"][0].clone();

    let mut reused_handle = first.clone();
    reused_handle["agents"][0]["node_id"] = serde_json::json!(8);
    reused_handle["agents"][0]["gpu_id"] = serde_json::json!(43);
    reused_handle["metadata"]["pid"] = serde_json::json!(2);
    let raw = serde_json::to_vec(&serde_json::json!({
        "rocprofiler-sdk-tool": [first.clone(), reused_handle]
    }))
    .unwrap();
    let projection = project_rocprofv3_json_dispatch_agents_v4(&raw).unwrap();
    assert_eq!(projection.agent_bindings().len(), 2);
    assert_eq!(projection.agent_bindings()[0].process_index, 0);
    assert_eq!(projection.agent_bindings()[0].process_id, 1);
    assert_eq!(projection.agent_bindings()[0].source_agent_id, 7001);
    assert_eq!(projection.agent_bindings()[0].node_id, 7);
    assert_eq!(projection.agent_bindings()[1].process_index, 1);
    assert_eq!(projection.agent_bindings()[1].process_id, 2);
    assert_eq!(projection.agent_bindings()[1].source_agent_id, 7001);
    assert_eq!(projection.agent_bindings()[1].node_id, 8);

    let mut binding = dispatch_binding(&[20, 21]);
    binding.environment.stable_device_bindings[0].source_agent_id = 7;
    binding.environment.stable_device_bindings[1].source_agent_id = 8;
    let bundle =
        import_projected_rocprofv3_json_profiler_bundle_v4(&raw, &projection, binding).unwrap();
    assert_ne!(bundle.source.value, bundle.normalized_projection.value);
    assert_eq!(
        bundle.dispatch_capture.as_ref().unwrap().dispatches.len(),
        2
    );
    assert_eq!(bundle.devices.len(), 2);

    let mut shared_node = first;
    shared_node["agents"][0]["id"]["handle"] = serde_json::json!(8001);
    shared_node["metadata"]["pid"] = serde_json::json!(2);
    shared_node["buffer_records"]["kernel_dispatch"][0]["dispatch_info"]["agent_id"]["handle"] =
        serde_json::json!(8001);
    let raw = serde_json::to_vec(&serde_json::json!({
        "rocprofiler-sdk-tool": [
            base["rocprofiler-sdk-tool"][0].clone(),
            shared_node
        ]
    }))
    .unwrap();
    let projection = project_rocprofv3_json_dispatch_agents_v4(&raw).unwrap();
    assert_eq!(projection.agent_bindings().len(), 2);
    assert_eq!(projection.agent_bindings()[0].node_id, 7);
    assert_eq!(projection.agent_bindings()[1].node_id, 7);
    let mut binding = dispatch_binding(&[20]);
    binding.environment.stable_device_bindings[0].source_agent_id = 7;
    let bundle =
        import_projected_rocprofv3_json_profiler_bundle_v4(&raw, &projection, binding).unwrap();
    assert_eq!(bundle.devices.len(), 1);
    assert_eq!(
        bundle.dispatch_capture.as_ref().unwrap().dispatches.len(),
        2
    );
}

#[test]
fn json_sequence_admission_is_bounded_before_projection() {
    let mut empty_process = serde_json::json!({
        "metadata": {"node": {}, "pid": 1, "init_time": 1, "fini_time": 2, "command": [], "config": {}},
        "agents": [],
        "buffer_records": {"kernel_dispatch": []}
    });
    complete_installed_process(&mut empty_process);
    let process_overflow = serde_json::to_vec(&serde_json::json!({
        "rocprofiler-sdk-tool": vec![empty_process; MAX_ROCPROF_PROCESSES_V1 + 1]
    }))
    .unwrap();
    assert!(matches!(
        project_rocprofv3_json_dispatch_agents_v4(&process_overflow),
        Err(ProfilerBundleErrorV4::InvalidRocprofJson)
    ));

    let mut agent = serde_json::json!({
        "id": {"handle": 1},
        "type": 2,
        "gpu_index": 0,
        "node_id": 1,
        "simd_count": 1,
        "gpu_id": 1,
        "vendor_id": 4098,
        "device_id": 1,
        "location_id": 1,
        "domain": 0,
        "gfx_target_version": 90402,
        "wave_front_size": 64,
        "num_xcc": 1
    });
    complete_installed_agent(&mut agent);
    let mut agents = Vec::new();
    for ordinal in 0..=MAX_PROFILER_DEVICE_BINDINGS_V4 {
        let value = u64::try_from(ordinal + 1).unwrap();
        agent["id"]["handle"] = serde_json::json!(value);
        agent["node_id"] = serde_json::json!(value);
        agent["gpu_id"] = serde_json::json!(value);
        agents.push(agent.clone());
    }
    let mut agent_overflow = serde_json::json!({
        "rocprofiler-sdk-tool": [{
            "metadata": {"node": {}, "pid": 1, "init_time": 1, "fini_time": 2, "command": [], "config": {}},
            "agents": agents,
            "buffer_records": {"kernel_dispatch": []}
        }]
    });
    complete_installed_process(&mut agent_overflow["rocprofiler-sdk-tool"][0]);
    let agent_overflow = serde_json::to_vec(&agent_overflow).unwrap();
    assert!(matches!(
        project_rocprofv3_json_dispatch_agents_v4(&agent_overflow),
        Err(ProfilerBundleErrorV4::InvalidRocprofJson)
    ));

    let dispatch = serde_json::json!({
        "size": 184,
        "kind": 11,
        "operation": 2,
        "thread_id": 1,
        "correlation_id": {"internal": 1, "external": 0},
        "start_timestamp": 1,
        "end_timestamp": 2,
        "dispatch_info": {
            "size": 72,
            "agent_id": {"handle": 1},
            "queue_id": {"handle": 1},
            "kernel_id": 1,
            "dispatch_id": 1,
            "private_segment_size": 0,
            "group_segment_size": 0,
            "workgroup_size": {"x": 1, "y": 1, "z": 1},
            "grid_size": {"x": 1, "y": 1, "z": 1}
        },
        "stream_id": {"handle": 0}
    });
    let mut per_process_overflow = serde_json::json!({
        "rocprofiler-sdk-tool": [{
            "metadata": {"node": {}, "pid": 1, "init_time": 1, "fini_time": 2, "command": [], "config": {}},
            "agents": [agent.clone()],
            "buffer_records": {
                "kernel_dispatch": vec![dispatch.clone(); MAX_PROFILER_DISPATCHES_V4 + 1]
            }
        }]
    });
    complete_installed_process(&mut per_process_overflow["rocprofiler-sdk-tool"][0]);
    let per_process_overflow = serde_json::to_vec(&per_process_overflow).unwrap();
    assert!(matches!(
        project_rocprofv3_json_dispatch_agents_v4(&per_process_overflow),
        Err(ProfilerBundleErrorV4::InvalidRocprofJson)
    ));

    let first_count = MAX_PROFILER_DISPATCHES_V4 / 2 + 1;
    let second_count = MAX_PROFILER_DISPATCHES_V4 - first_count + 1;
    let mut global_overflow = serde_json::json!({
        "rocprofiler-sdk-tool": [
            {
                "metadata": {"node": {}, "pid": 1, "init_time": 1, "fini_time": 2, "command": [], "config": {}},
                "agents": [agent.clone()],
                "buffer_records": {"kernel_dispatch": vec![dispatch.clone(); first_count]}
            },
            {
                "metadata": {"node": {}, "pid": 2, "init_time": 1, "fini_time": 2, "command": [], "config": {}},
                "agents": [agent],
                "buffer_records": {"kernel_dispatch": vec![dispatch; second_count]}
            }
        ]
    });
    for process in global_overflow["rocprofiler-sdk-tool"]
        .as_array_mut()
        .unwrap()
    {
        complete_installed_process(process);
    }
    let global_overflow = serde_json::to_vec(&global_overflow).unwrap();
    assert!(matches!(
        project_rocprofv3_json_dispatch_agents_v4(&global_overflow),
        Err(ProfilerBundleErrorV4::InvalidRocprofJson)
    ));
}

#[test]
fn csv_import_is_strict_about_schema_values_and_resource_bounds() {
    let bare_agent_ids = String::from_utf8(csv_source().to_vec())
        .unwrap()
        .replace("Agent 17", "17")
        .replace("Agent 19", "19");
    assert!(matches!(
        import_rocprofv3_csv_profiler_bundle_v4(
            bare_agent_ids.as_bytes(),
            dispatch_binding(&[20, 21])
        ),
        Err(ProfilerBundleErrorV4::InvalidRocprofCsv)
    ));

    for noncanonical in ["Agent 017", "Agent 0x11", "Agent 17 "] {
        let source =
            String::from_utf8(csv_source().to_vec())
                .unwrap()
                .replacen("Agent 17", noncanonical, 1);
        assert!(matches!(
            import_rocprofv3_csv_profiler_bundle_v4(source.as_bytes(), dispatch_binding(&[20, 21])),
            Err(ProfilerBundleErrorV4::InvalidRocprofCsv)
        ));
    }

    let u32_overflow = u64::from(u32::MAX) + 1;
    let hostile_agent =
        replace_first_csv_field(csv_source(), "Agent_Id", &format!("Agent {u32_overflow}"));
    assert!(matches!(
        import_rocprofv3_csv_profiler_bundle_v4(&hostile_agent, dispatch_binding(&[20, 21])),
        Err(ProfilerBundleErrorV4::InvalidRocprofCsv)
    ));
    for column in [
        "LDS_Block_Size",
        "Scratch_Size",
        "VGPR_Count",
        "Accum_VGPR_Count",
        "SGPR_Count",
        "Workgroup_Size_X",
        "Workgroup_Size_Y",
        "Workgroup_Size_Z",
        "Grid_Size_X",
        "Grid_Size_Y",
        "Grid_Size_Z",
    ] {
        let hostile = replace_first_csv_field(csv_source(), column, &u32_overflow.to_string());
        assert!(
            matches!(
                import_rocprofv3_csv_profiler_bundle_v4(&hostile, dispatch_binding(&[20, 21])),
                Err(ProfilerBundleErrorV4::InvalidRocprofCsv)
            ),
            "accepted u32 overflow for {column}"
        );
    }

    for (accepted, replacement) in [
        (true, ",4,0,100,1,"),
        (false, ",4,0x0,100,1,"),
        (false, ",4,0x11,100,1,"),
        (false, ",4,00,100,1,"),
        (false, ",4,+0,100,1,"),
        (false, ",4,-0,100,1,"),
        (false, ",4, 0,100,1,"),
        (false, ",4,0 ,100,1,"),
        (false, ",4,0X0,100,1,"),
        (false, ",4,0x00,100,1,"),
        (false, ",4,0xA,100,1,"),
        (false, ",4,18446744073709551616,100,1,"),
        (false, ",4,0x10000000000000000,100,1,"),
    ] {
        let source = String::from_utf8(csv_source().to_vec()).unwrap().replacen(
            ",4,0,100,1,",
            replacement,
            1,
        );
        assert_eq!(
            import_rocprofv3_csv_profiler_bundle_v4(source.as_bytes(), dispatch_binding(&[20, 21]))
                .is_ok(),
            accepted,
            "CSV integer spelling {replacement}"
        );
    }

    let unknown = String::from_utf8(csv_source().to_vec())
        .unwrap()
        .replacen("\"Kind\",", "\"Unknown\",\"Kind\",", 1)
        .replacen("\"KERNEL_DISPATCH\",", "x,\"KERNEL_DISPATCH\",", 2);
    assert!(matches!(
        import_rocprofv3_csv_profiler_bundle_v4(unknown.as_bytes(), dispatch_binding(&[20, 21])),
        Err(ProfilerBundleErrorV4::InvalidRocprofCsv)
    ));

    let duplicate = String::from_utf8(csv_source().to_vec())
        .unwrap()
        .replacen("\"Kind\",", "\"Kind\",\"Kind\",", 1)
        .replacen(
            "\"KERNEL_DISPATCH\",",
            "\"KERNEL_DISPATCH\",\"KERNEL_DISPATCH\",",
            2,
        );
    assert!(matches!(
        import_rocprofv3_csv_profiler_bundle_v4(duplicate.as_bytes(), dispatch_binding(&[20, 21])),
        Err(ProfilerBundleErrorV4::InvalidRocprofCsv)
    ));

    for hostile_header in [
        String::from_utf8(csv_source().to_vec()).unwrap().replacen(
            "\"Stream_Id\",",
            "\"Process_Id\",",
            1,
        ),
        String::from_utf8(csv_source().to_vec())
            .unwrap()
            .replacen("\"Stream_Id\",", "\"Stream_Id\",\"Process_Id\",", 1)
            .replacen(",4,0,100,", ",4,0,7,100,", 1)
            .replacen(",5,1,101,", ",5,1,9,101,", 1),
    ] {
        assert!(matches!(
            import_rocprofv3_csv_profiler_bundle_v4(
                hostile_header.as_bytes(),
                dispatch_binding(&[20, 21])
            ),
            Err(ProfilerBundleErrorV4::InvalidRocprofCsv)
        ));
    }

    let bad_number = String::from_utf8(csv_source().to_vec())
        .unwrap()
        .replacen(",100,180", ",wat,180", 1);
    assert!(matches!(
        import_rocprofv3_csv_profiler_bundle_v4(bad_number.as_bytes(), dispatch_binding(&[20, 21])),
        Err(ProfilerBundleErrorV4::InvalidRocprofCsv)
    ));

    let oversized = vec![b' '; MAX_PROFILER_SOURCE_BYTES_V4 as usize + 1];
    assert!(matches!(
        import_rocprofv3_csv_profiler_bundle_v4(&oversized, dispatch_binding(&[20])),
        Err(ProfilerBundleErrorV4::SourceSizeOutOfRange)
    ));

    let source = String::from_utf8(csv_source().to_vec()).unwrap();
    let long_kernel = "x".repeat(257);
    let long_name_source = source.replacen("generic,kernel", &long_kernel, 1);
    assert!(
        import_rocprofv3_csv_profiler_bundle_v4(
            long_name_source.as_bytes(),
            dispatch_binding(&[20, 21])
        )
        .is_ok()
    );
    let oversized_kernel = "x".repeat(MAX_PROFILER_CSV_KERNEL_NAME_BYTES_V4 + 1);
    let oversized_name_source = source.replacen("generic,kernel", &oversized_kernel, 1);
    assert!(matches!(
        import_rocprofv3_csv_profiler_bundle_v4(
            oversized_name_source.as_bytes(),
            dispatch_binding(&[20, 21])
        ),
        Err(ProfilerBundleErrorV4::InvalidRocprofCsv)
    ));

    let original =
        import_rocprofv3_csv_profiler_bundle_v4(csv_source(), dispatch_binding(&[20, 21])).unwrap();
    let changed_dispatch_id = source.replacen(",4,0,100,1,10,", ",4,0,100,9,10,", 1);
    let changed = import_rocprofv3_csv_profiler_bundle_v4(
        changed_dispatch_id.as_bytes(),
        dispatch_binding(&[20, 21]),
    )
    .unwrap();
    assert_ne!(
        original.normalized_projection,
        changed.normalized_projection
    );
    assert_ne!(
        original.dispatch_capture.as_ref().unwrap().dispatches[0].identity,
        changed.dispatch_capture.as_ref().unwrap().dispatches[0].identity
    );
}

#[test]
fn att_import_retains_only_safe_references_and_never_claims_decoding() {
    let source = br#"{"counter_names":[],"gfxip":9,"gfxv":"vega","global_begin_time":0,"is_pcs_stochastic":false,"pc_sampling":false,"thread_trace":true,"version":"3.0.0","wave_filenames":{"0":{"0":{"0":{"0":["waves/se0.json",10,20]}}}},"se_filenames":["se0.json"]}"#;
    let bundle = import_rocprofv3_att_profiler_bundle_v4(
        source,
        ProfilerAttBindingV4 {
            environment: environment(&[20]),
            source_agent_id: 17,
            referenced_artifacts: vec![ProfilerAttArtifactBindingV4 {
                reference: "se0.json".to_owned(),
                content: content(30, 400),
            }],
        },
    )
    .unwrap();
    let att = bundle.att.as_ref().unwrap();
    assert_eq!(bundle.coverage.att_references, 2);
    assert_eq!(att.decoder_output_origin, TruthOriginV1::Unavailable);
    assert_eq!(att.references[0].kind, AttReferenceKindV4::WaveTimeline);
    assert_eq!(att.references[0].content.origin, TruthOriginV1::Unavailable);
    assert_eq!(
        att.references[1].kind,
        AttReferenceKindV4::ShaderEngineMetadata
    );
    assert_eq!(att.references[1].content.value, Some(content(30, 400)));
    assert!(
        bundle
            .unavailable
            .contains(&ProfilerUnavailableFactV4::WaitEvents)
    );

    let installed = include_bytes!("fixtures/rocprofv3-1.1-att-manifest.json");
    assert!(
        import_rocprofv3_att_profiler_bundle_v4(
            installed,
            ProfilerAttBindingV4 {
                environment: environment(&[20]),
                source_agent_id: 17,
                referenced_artifacts: Vec::new(),
            }
        )
        .is_ok()
    );
}

#[test]
fn att_import_rejects_unsafe_duplicate_and_unrecognized_evidence() {
    for reference in [
        "/absolute.json",
        "../parent.json",
        "a/./b.json",
        "C:drive.json",
    ] {
        let source = serde_json::to_vec(&serde_json::json!({
            "thread_trace": true,
            "version": "3.0.0",
            "wave_filenames": {"0":{"0":{"0":{"0":[reference, 1, 2]}}}}
        }))
        .unwrap();
        assert!(matches!(
            import_rocprofv3_att_profiler_bundle_v4(
                &source,
                ProfilerAttBindingV4 {
                    environment: environment(&[20]),
                    source_agent_id: 17,
                    referenced_artifacts: Vec::new(),
                }
            ),
            Err(ProfilerBundleErrorV4::InvalidAttReference)
        ));
    }

    let unknown = br#"{"thread_trace":true,"version":"3.0.0","unknown":1,"wave_filenames":{"0":{"0":{"0":{"0":["wave.json",1,2]}}}}}"#;
    assert!(matches!(
        import_rocprofv3_att_profiler_bundle_v4(
            unknown,
            ProfilerAttBindingV4 {
                environment: environment(&[20]),
                source_agent_id: 17,
                referenced_artifacts: Vec::new(),
            }
        ),
        Err(ProfilerBundleErrorV4::InvalidAttManifest)
    ));
}

#[test]
fn decoder_rejects_noncanonical_and_stale_bundle_claims() {
    let bundle = import_json_bundle(json_source(), json_dispatch_binding(&[20, 21])).unwrap();
    let mut bytes = encode_profiler_bundle_v4(&bundle).unwrap();
    bytes.push(b'\n');
    assert!(matches!(
        decode_profiler_bundle_v4(&bytes),
        Err(ProfilerBundleErrorV4::NonCanonicalEncoding)
    ));

    let mut stale = bundle;
    stale.run_identity = CaptureIdentityV1::new([99; 32]).unwrap();
    assert!(matches!(
        decode_profiler_bundle_v4(&serde_json::to_vec(&stale).unwrap()),
        Err(ProfilerBundleErrorV4::StaleRunIdentity)
    ));

    for role in [
        "source",
        "environment",
        "collector_tool",
        "collector_configuration",
    ] {
        let mut substituted =
            import_json_bundle(json_source(), json_dispatch_binding(&[20, 21])).unwrap();
        let fact = match role {
            "source" => &mut substituted.source,
            "environment" => &mut substituted.environment,
            "collector_tool" => &mut substituted.collector_tool,
            "collector_configuration" => &mut substituted.collector_configuration,
            _ => unreachable!(),
        };
        fact.value.as_mut().unwrap().scheme = ContentSchemeV1::RawCanonicalSha256;
        assert!(
            matches!(
                decode_profiler_bundle_v4(&serde_json::to_vec(&substituted).unwrap()),
                Err(ProfilerBundleErrorV4::StaleRunIdentity)
            ),
            "run identity did not bind the {role} content-identity scheme"
        );
    }
}

#[test]
fn decoder_rejects_inexact_launches_and_hostile_multi_device_joins() {
    let bundle = import_json_bundle(json_source(), json_dispatch_binding(&[20, 21])).unwrap();

    let mut inconsistent = bundle.clone();
    inconsistent.dispatch_capture.as_mut().unwrap().dispatches[0]
        .launch
        .logical_grid[0] = 129;
    assert!(matches!(
        decode_profiler_bundle_v4(&serde_json::to_vec(&inconsistent).unwrap()),
        Err(ProfilerBundleErrorV4::InvalidDispatchCapture)
    ));

    let mut too_many_devices = bundle.clone();
    let template = too_many_devices.devices[0].clone();
    too_many_devices.devices = (0..=MAX_PROFILER_DEVICE_BINDINGS_V4)
        .map(|ordinal| {
            let mut device = template.clone();
            device.ordinal = u32::try_from(ordinal).unwrap();
            device
        })
        .collect();
    assert!(matches!(
        decode_profiler_bundle_v4(&serde_json::to_vec(&too_many_devices).unwrap()),
        Err(ProfilerBundleErrorV4::DeviceCountOutOfRange)
    ));

    let mut overflowing = bundle.clone();
    let launch = &mut overflowing.dispatch_capture.as_mut().unwrap().dispatches[0].launch;
    launch.logical_grid = [u64::from(u32::MAX), u64::from(u32::MAX), 2];
    launch.grid_workgroups = [u32::MAX, u32::MAX, 2];
    launch.workgroup_size = [1, 1, 1];
    assert!(matches!(
        decode_profiler_bundle_v4(&serde_json::to_vec(&overflowing).unwrap()),
        Err(ProfilerBundleErrorV4::InvalidDispatchCapture)
    ));

    let mut wrong_source_device = bundle.clone();
    wrong_source_device.devices.swap(0, 1);
    wrong_source_device.devices[0].ordinal = 0;
    wrong_source_device.devices[1].ordinal = 1;
    assert!(matches!(
        decode_profiler_bundle_v4(&serde_json::to_vec(&wrong_source_device).unwrap()),
        Err(ProfilerBundleErrorV4::StaleReference)
    ));

    let mut missing_source_device = bundle.clone();
    missing_source_device.devices[0].source_bound_identity = None;
    missing_source_device.devices[0].source_bound_origin = TruthOriginV1::Unavailable;
    assert!(matches!(
        decode_profiler_bundle_v4(&serde_json::to_vec(&missing_source_device).unwrap()),
        Err(ProfilerBundleErrorV4::StaleReference)
    ));

    let mut duplicate_source_device = bundle.clone();
    duplicate_source_device.devices[1].source_bound_identity =
        duplicate_source_device.devices[0].source_bound_identity;
    assert!(matches!(
        decode_profiler_bundle_v4(&serde_json::to_vec(&duplicate_source_device).unwrap()),
        Err(ProfilerBundleErrorV4::StaleReference)
    ));

    let mut duplicate_stable_device = bundle;
    duplicate_stable_device.devices[1].stable_identity =
        duplicate_stable_device.devices[0].stable_identity;
    assert!(matches!(
        decode_profiler_bundle_v4(&serde_json::to_vec(&duplicate_stable_device).unwrap()),
        Err(ProfilerBundleErrorV4::InvalidDevice)
    ));
}

#[test]
fn profiler_import_cli_emits_canonical_v4_without_paths_or_native_handles() {
    let id = |byte: u8, len: u64| format!("domain:1:{}:{len}", format!("{byte:02x}").repeat(32));
    let mut child = Command::new(env!("CARGO_BIN_EXE_fe2o3-profiler-import"))
        .args([
            "dispatch-csv-v4",
            "--environment",
            &id(10, 200),
            "--tool",
            &id(11, 50),
            "--config",
            &id(12, 80),
            "--device-binding",
            &format!("17={}", id(20, 64)),
            "--device-binding",
            &format!("19={}", id(21, 64)),
            "--kir-sha256",
            &"01".repeat(32),
            "--kir-len",
            "97",
            "--wave-width",
            "64",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(csv_source()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let bundle = decode_profiler_bundle_v4(&output.stdout).unwrap();
    assert_eq!(bundle.schema_version, 4);
    let encoded = String::from_utf8(output.stdout).unwrap();
    assert!(!encoded.contains("generic,kernel"));
    assert!(!encoded.contains("Queue_Id"));
}

#[test]
fn profiler_import_json_cli_keys_device_bindings_by_projected_kfd_node() {
    let id = |byte: u8, len: u64| format!("domain:1:{}:{len}", format!("{byte:02x}").repeat(32));
    let arguments = |first: u64, second: u64| {
        vec![
            "dispatch-json-v4".to_owned(),
            "--environment".to_owned(),
            id(10, 200),
            "--tool".to_owned(),
            id(11, 50),
            "--config".to_owned(),
            id(12, 80),
            "--device-binding".to_owned(),
            format!("{first}={}", id(20, 64)),
            "--device-binding".to_owned(),
            format!("{second}={}", id(21, 64)),
            "--kir-sha256".to_owned(),
            "01".repeat(32),
            "--kir-len".to_owned(),
            "97".to_owned(),
            "--wave-width".to_owned(),
            "64".to_owned(),
        ]
    };

    for (bindings, succeeds) in [(arguments(7, 8), true), (arguments(17, 19), false)] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_fe2o3-profiler-import"))
            .args(bindings)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(json_source())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.success(), succeeds);
        if succeeds {
            let bundle = decode_profiler_bundle_v4(&output.stdout).unwrap();
            assert_ne!(bundle.source.value, bundle.normalized_projection.value);
            assert_eq!(
                bundle.devices[0].stable_identity.value,
                Some(content(20, 64))
            );
        }
    }
}

#[test]
fn att_cli_argument_bound_covers_more_than_the_legacy_128_arguments() {
    let id = |byte: u8, len: u64| format!("domain:1:{}:{len}", format!("{byte:02x}").repeat(32));
    let references = (0..64)
        .map(|index| format!("se-{index}.json"))
        .collect::<Vec<_>>();
    let source = serde_json::to_vec(&serde_json::json!({
        "thread_trace": true,
        "version": "3.0.0",
        "wave_filenames": {"0":{"0":{"0":{"0":["wave.json", 1, 2]}}}},
        "se_filenames": references,
    }))
    .unwrap();
    let mut arguments = vec![
        "att-v4".to_owned(),
        "--environment".to_owned(),
        id(10, 200),
        "--tool".to_owned(),
        id(11, 50),
        "--config".to_owned(),
        id(12, 80),
        "--device-binding".to_owned(),
        format!("17={}", id(20, 64)),
        "--att-agent-id".to_owned(),
        "17".to_owned(),
    ];
    for reference in &references {
        arguments.push("--att-artifact".to_owned());
        arguments.push(format!("{reference}={}", id(30, 400)));
    }
    assert!(arguments.len() > 128);
    let mut child = Command::new(env!("CARGO_BIN_EXE_fe2o3-profiler-import"))
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&source).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bundle = decode_profiler_bundle_v4(&output.stdout).unwrap();
    assert_eq!(bundle.coverage.att_references, 65);
}

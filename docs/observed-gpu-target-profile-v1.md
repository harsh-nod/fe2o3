# Observed GPU Target Profile V1

`cargo fe2o3 profile` owns this bounded, authority-free observation record.
Only its direct-KFD topology discovery path constructs the record. The dry-run
plan and retained `fe2o3-profile-manifest-v1.txt` emit the same line for every
visible GPU, ordered with the stable device list:

```text
observed-gpu-target-profile-v1[<ordinal>]: schema=fe2o3-observed-gpu-target-profile-v1;origin=direct-kfd-properties;node=<u32>;stable-device-identity=raw:1:<lowercase-sha256>:<byte-length>;vendor-id=<u64>;gfx-target-version=<u64>;wave-width=<u64>;availability=<observed|unavailable>;profile=<gfx942|gfx950|unavailable>;unavailable-reason=<reason>
```

The payload is at most 512 ASCII bytes. Keys occur exactly in the displayed
order. Decimal integers are canonical and the fixed tokens need no escaping.
The stable identity covers the bounded KFD `unique_id`, `vendor_id`,
`device_id`, `domain`, `location_id`, `gfx_target_version`,
`wave_front_size`, and `num_xcc` property bytes. The complete record bytes are
also included in collection authorization and collector-configuration V2
content identity. Device equality rechecks them immediately before and after
collection.

## Admission

The only observed mappings are:

| KFD `vendor_id` | KFD `gfx_target_version` | KFD `wave_front_size` | Profile |
|---:|---:|---:|---|
| `4098` | `90402` | `64` | `gfx942` |
| `4098` | `90500` | `64` | `gfx950` |

Every other well-formed numeric combination is retained with
`availability=unavailable` and `profile=unavailable`. The reason is one of:

- `unknown-gfx-target-version`
- `vendor-contradicts-amd-target`
- `wave-width-contradicts-target`
- `vendor-and-wave-width-contradict-target`

Unavailable target observations do not block ordinary collection. They cannot
be used for source/IR/ISA association. Malformed, duplicate, missing, or
out-of-range required KFD properties reject discovery rather than producing a
record.

## Authority Boundary

This record says which target family the orchestrator observed for one stable
KFD node. It does not prove that rocprof observed a dispatch, that a dispatch
ran on the node, or that the profiled process executed any particular code
object. Bundle V4 import, dispatch-to-device association, executed artifact
identity, source-map identity, and KIR V7-to-V8 non-equivalence remain separate
unavailable contracts.

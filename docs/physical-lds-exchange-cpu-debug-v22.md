# V22 two-wave LDS CPU debugger

The explicit V22 route presents observations from the existing CPU simulator, not GPU execution, source-variable debugging, physical registers, or physical EXEC. It accepts the exact checked canonical V22 physical-LDS-exchange profile and a bounded simulation request. Reading exported canonical bytes does not recover the original Rust-source ownership.

Run the ordinary debugger binary on Linux with an existing canonical export and request:

```sh
cargo run --locked -p fe2o3-debug-cli --bin fe2o3-debug -- \
  sim --diagnostic-kir-v22 canonical-v22.bin --request request.json \
  --protocol jsonl --wave-width 64 --capture-index capture-index.json
```

The paths above are placeholders, not retained successful output. The index path must not already exist. Omit `--capture-index` to use only the JSONL interface. A single process creates the index from its own immutable capture, then serves the existing request/response V1 JSONL protocol on standard input/output. No index bytes are mixed into that protocol stream.

The request has exactly two logical buffer arguments: initialized/readable input of at least128 u32 elements and the output. Grid and workgroup are exactly `[128,1,1]`. At most two caller shared buffers are allowed. The simulator creates the third,512-byte initially uninitialized workgroup allocation itself; do not add an LDS buffer or ghost parameter. Short output views are useful CPU tail controls, but do not discharge the separate conservative native/formal output-allocation conditions. Same-backing/overlap refusals and input initialization checks remain active.

## What can be inspected

The shared protocol provides forward/reverse navigation, actual KIR SSA values, and allocation-relative logical memory with byte initialization state. Use the session's configuration/revision-qualified cursors and page tokens; stale or foreign tokens do not change session state.

For localX0..127, the displayed logical wave is `localX /64` and lane is `localX %64`. The full64-bit visualization mask is not a physical EXEC sample. Ready scalar bits are shown only when the actual Engine binding is ready. Pending global/LDS reads and other opaque symbolic values remain `NotRepresented`; waits update the same actual SSA binding. Reverse navigation selects stored observations, not resumed execution.

Source maps/variables, physical registers, hardware state, schedules, raw snapshots, and generic trace export remain unavailable. Non-checkpoint memory/barrier events have no invented checkpoint values. Existing V20/V21 routes, configuration/page domains, responses and refusal behavior are unchanged.

## Optional recorded capture index

`fe2o3-physical-lds-cpu-capture-index-v1` is a separate diagnostic JSON document. It contains:

- exact canonical/request content references and the same session configuration identity;
- actual allocation IDs, address spaces, lengths and first observed checkpoints;
- actual ordered record sequence/producer ordinal, KIR site and logical coordinates;
- checkpoint phases, memory access locations, and barrier actions/phases/participant counts;
- only pending-global-read and pending-LDS-read SSA tags, with numeric availability explicitly `not_represented`.

It does not dump all scalar values or allocation contents. Use normal JSONL value/memory queries at the indexed checkpoints. Allocation IDs are not labelled input/output by guessed numeric order. A complete exchange capture should show128 actual arrivals and a release with128 participants; a truncated capture is labelled with its real capture-stop reason, not repaired into a complete narrative. No pending-write queue or publication bitmap is exposed.

The ordered envelope fields are `schema`, `identity`, `payload`. Identity is the lower-case SHA256 of:

```text
"fe2o3-debug-physical-lds-v22-capture-index-v1\0"
|| little_endian_u64(payload_bytes)
|| exact UTF-8 payload bytes
```

The NUL in the domain is one zero byte. The identity object's `payload_bytes` excludes the outer envelope. Check the exact payload slice, not a parsed-and-reencoded approximation. This checksum is not source authentication or a signature.

A future recorded viewer must strictly check schema, byte/row caps, digest/length, unique ordered sequences, real scope/site/identity joins, closed event shapes and unavailable-value rules. Loading a JSON document is presentation only, never simulator/source/hardware authority. No website or live bridge route is enabled by this compiler change.

## Bounds and output behavior

The unchanged session account is512MiB storage and2^29 work units. The V22 route permits16384 records; V20/V21 remain8192. Existing protocol caps stay8192-byte requests,65536-byte responses,64-item pages and4096 commands.

Index limits are8MiB total JSON,16384 records,8 allocation identities,768 scanned bindings per checkpoint,8 pending tags per checkpoint and16KiB per row. Export prepays50,462,720 work units and8,454,144 storage bytes on the original ledger. The export buffer is dropped before JSONL serving, but its conservative reservation remains until the single normal session teardown; no additional public mutable-budget API is exposed. Export-enabled configuration identities include the flag and exact index-policy constants.

The complete bounded payload and header are encoded and hashed before opening a fresh exclusive no-follow output file. Existing files and symlinks are refused, never overwritten. A write/flush error terminates before serving; a partial output is not a qualified recording. Successful flush is not a durability guarantee. Retained qualification additionally requires successful producer exit and independent actual-source evidence; a checksum-valid user file alone proves neither.

## Qualification status

This implementation's new public route/index and authored tests require root integration and execution. The separately retained actual-source CPU capture qualification is not an observed recording of this new CLI. No native/GPU execution or full visualization milestone completion is claimed here.

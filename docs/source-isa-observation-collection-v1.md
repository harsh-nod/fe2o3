# Source/ISA observation collection V1

`cargo-fe2o3` build-config V2 enables one inert acceptance observation for
each configured Rust compilation unit. This protocol transports bounded
Source/MIR/KIR-to-sparse-ISA summary frames after the authenticated wrapper has
released its invocation authority. V1 and nonselected V2 invocations retain
the frozen ordinary release/ACK exchange. A selected V2 unit uses a distinct
prepared ACK on the same authenticated, ordered stream; the broker emits that
ACK only after it accepts the exact preceding configuration, unit, non-DIRECT
attempt, and broker session. The authenticated wrapper supplies attempt
legitimacy, as in V1. The broker does not independently register generations
or invocation identities, but the later frame must equal the ACKed request's
complete attempt. It does not grant compiler, artifact,
publication, load, launch, runtime, hardware-observation, or proof authority.

## Cargo output

After the Cargo child and invocation boundary have completed, the parent emits
one stderr line:

```text
[cargo-fe2o3] source-isa-observation-collection-v1 frames=<u64> missing=<u64> failure=<u16> encoding=hex:<lowercase-hex> authority=observation-only
```

Frames and missing units are ordered by their 32-byte selected-unit identity.
`failure=0` means that transport completed without a recorded failure. A
nonzero failure retains the first failure code while later valid, distinct
selected-unit frames remain collectable. Exact duplicate recovery frames are
deduplicated; conflicting duplicates retain the first frame and set a failure.

Stable failure codes are `1` collector-already-failed (reserved for the frozen
transport), `2` unit bound, `3` aggregate-byte bound, `4` conflicting
duplicate, `5` rejected frame, `6` missing selected units, and `7` broker
worker panic. Unknown codes are rejected.

The output is stderr telemetry rather than a generated artifact. Adding a
non-authoritative observer file to the generation directory would include it
in the authoritative artifact snapshot and make a nonfatal telemetry failure
change or stale that snapshot.

## Binary schema

All integers are little-endian. The fixed header is 80 bytes:

| Offset | Bytes | Field |
| ---: | ---: | --- |
| 0 | 8 | ASCII magic `F2SICOL1` |
| 8 | 2 | version `1` |
| 10 | 2 | header bytes `80` |
| 12 | 4 | total binary byte length, including the trailing identity |
| 16 | 4 | frame count |
| 20 | 4 | missing-unit count |
| 24 | 2 | first transport failure code, or zero |
| 26 | 2 | reserved zero |
| 28 | 4 | authority/truth claims, fixed zero |
| 32 | 32 | exact production configuration identity |
| 64 | 16 | exact non-DIRECT broker build session |

The header is followed by `frame_count` canonical 680-byte
`F2SISUM1` frames, then `missing_unit_count` 32-byte unit identities. The final
32 bytes are SHA-256 over the domain
`FE2O3/SOURCE-ISA-OBSERVATION-COLLECTION/V1\0` followed by all preceding
collection bytes.

The strict decoder rejects unknown versions or failure codes, truncated,
trailing, or oversized bytes, count/length disagreement, invalid embedded
frames, zero or unsorted unit identities, frame/missing overlap, missing units
without a failure, nonzero reserved truth claims, and collection-identity
disagreement. Every frame must carry the header configuration and broker
session, including collections where every selected unit is missing. It uses
the same checked arithmetic and fallible bounded
allocation as the encoder, so agents can consume the binary record directly
without parsing terminal presentation text.

Frames and missing identities are disjoint members of one selected-unit set,
so their combined count cannot exceed 1,024. The maximum binary record is
696,432 bytes and the maximum lowercase hexadecimal payload is 1,392,864
bytes. Length arithmetic, allocation, and encoding are fallible. The complete
stderr line has a compile-time two-mebibyte bound.

## Outcomes

An admitted frame carries exact correlation, artifact, target/KIR structural,
record/query-count, and optional canonical round-trip witness fields. Typed
unavailable values preserve compiler-carrier gaps, semantic-anchor gaps, KIR
V9 source-projection absence, and code `202` for load-ready recovery without
retained finalized evidence. Correlation admission errors use a separate
closed error-code namespace. No unavailable or error frame carries an admitted
payload, and all authority/truth fields remain zero.

An admitted frame accepts `1..=16,384` canonical functions, exactly one defined
body, and independent `1..=4,096` block and operation counts; empty blocks are
valid. For `O` structural operations, `S` source-anchored records, and `N`
no-source records, canonical admission requires `N <= O`, `covered = O - N`,
`(covered == 0) == (S == 0)`, and `S >= covered`. Multiple source records may
cover one operation. Before emitting zero truth claims, the mapper requires
acceptance-summary format V1 and verifies that all eight authority/coverage
claim accessors remain false.

## Error codes

Error codes are collision-free typed projections. Generic lossy codes `1` and
`2` are reserved and rejected. Direct correlation errors use `3..=10` in this
order: invalid KIR-to-LLVM replay, non-exact semantic map, artifact identity,
target/KIR identity, coordinate shape, source graph, resource limit, allocation
failure.

Nested finalized-map errors use `0x1001..=0x100f`; semantic-map errors use
`0x1101..=0x111c`; production-fragment errors use `0x1201..=0x120a`; and
semantic-anchor errors use `0x2001..=0x2013`. Within each range, values follow
these exact ordered labels:

- `0x1001`: production association; association mismatch; KIR-to-LLVM replay;
  replay target mismatch; LLVM-to-HSACO custody; bound source map; bound
  semantic MIR; bound correspondence V4; bound canonical KIR V8; bound
  canonical KIR V7; canonical KIR projection mismatch; correspondence identity;
  semantic correspondence; artifact inspection; allocation failure.
- `0x1101`: length; JSON; canonical encoding; encoding; binding; kernel ordinal
  basis; node; mapping; duplicate node; duplicate mapping; duplicate reference;
  unknown node; layer mismatch; contradictory mapping; orphan node; boundary;
  untyped boundary; resource limit; allocation failure; content binding;
  artifact binding; bound source map; bound canonical KIR; source-map/KIR
  binding; source location; MIR location; KIR location; ISA interval.
- `0x1201`: encoding; association; gap; schedule status; source map; canonical
  KIR; semantic map; axis mismatch; resource limit; allocation failure.
- `0x2001`: compiler attachment; production association; KIR-to-LLVM replay;
  target mismatch; LLVM; contradictory LLVM; binding; KIR coordinate;
  KIR-to-LLVM anchor; artifact; missing probe section; ambiguous probe section;
  probe encoding; probe descriptor; ambiguous entry symbol; unexpected probe;
  probe outside kernel; resource limit; allocation failure.

Each following label increments the starting code by one. Range gaps, unknown values, and the
retired generic codes are rejected even when a hostile producer recomputes the
frame identity. This preserves resource, allocation, integrity, binding, and
semantic distinctions without changing the fixed 680-byte frame.

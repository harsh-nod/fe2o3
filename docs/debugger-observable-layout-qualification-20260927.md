# Observable debugger layout qualification — 2026-09-27

The current private observable debugger passed actual-object DWARF inspection
for all nine required types, with 152 controls. This supersedes the layout
limitation recorded in the earlier disabled-type-anchor checkpoint; it does not
enable a public debugger route or establish startup or native capture.

## Measured layout

All entries were observed in the `amd_owned_one_stop_v1` namespace of the
newly built adapter object. Required members, member offsets and type references
were validated; no missing type size was inferred.

| Type | Bytes |
| --- | ---: |
| workspace | 1,936 |
| runtime_root | 40 |
| owner | 1,992 |
| physical_snapshot | 536 |
| snapshot_publication | 1,552 |
| sha256 | 184 |
| loaded_maintenance_scratch | 624 |
| native_adapter | 8,760 |
| owned_checkpoint_breakpoint | 232 |

The logical adapter reservation is 15,144 bytes, within the unchanged 65,536-byte
native cap. This is a defined logical reservation, not total debugger memory,
RSS or a claim that every descendant allocation has been metered.

The check used readelf on the adapter object, not a running debugger or GPU
target. Its output contained 43,592,376 bytes, 757,025 lines and 189,102 DIEs.
All 637 build-input metadata rows remained complete and ordered. Metadata for a
large build tool uses the existing 512-MiB compile-input domain; the ordinary
artifact validator and actual object-read cap did not change.

The static reader accounted for 1,545,334,198 bytes / 184,507 calls under its
unchanged 2-GiB / 300,000-call caps. The separately source-derived module-loader
bound is 10,330,531 bytes / 407 calls / 15 processes. Tool-internal and actual
module-loader I/O were not metered and are not claimed as such.

## Qualified evidence

All 152 controls passed: 87 inherited, 20 diagnostic and 45 loaded-maintenance.
The metadata-domain additions cover 108 scenarios / 120 assertions inside an
existing test. The complete source inverse includes the 213-byte type anchor;
the earlier missing-layout failure remains a failed historical attempt.

Actual static receipt:
`021eb18660b6e2867ab8f24c154a1cc5edd7e42e853e0eea21de61e5d32ea16a`.
Complete static report:
`1b6c4fe2615d37902b33a446a0f9d35415fbc5dfbe28e25add98eed9f2a10b73`.
Root full-output readback:
`7ec2ddfcf555deabdd43c932f31eab622c983081b7fb8349321a566ab79d797d`.
Independent metadata-domain source review:
`5b12d930ae0d66a318debcc23324f2a23e90447126a6400f71908495aa7ff06f`.

The four completed build phases also have a qualified complete custody handoff:
nine bounded records retain all 2,543 selected/observed input-row pairs, with
strict decoding of the full producer output. Admission receipt:
`1b7f8a30505e1d9be455154f53ae56c49970db698346155b58040369a7040858`.
Strict-decoder receipt:
`ff546438a1175ee0e9beb30c0939a1d1b5c4ce79bbed5bd48791b1a135a47b0d`.
These remain historical build provenance, not a fresh runtime product rehash.

## Remaining boundary

The current startup consumer must join the new custody and nine-type report,
retain or explicitly replace every old input role, derive current artifact
bindings, rebuild and qualify helpers, and complete a finite startup-only
deployment and postflight. Live GPU capture and native qualification follow
separately. There is no startup or native lease in this evidence, and no
register, memory, lane, canary or captured-kernel observation.

The disabled public package stays disabled. Unavailable visualizer data remains
unavailable. Accepted broad exits remain **M1/V1/V2/U1/U2/U3 (6/18)**; no global
compiler pin change or public activation is implied.

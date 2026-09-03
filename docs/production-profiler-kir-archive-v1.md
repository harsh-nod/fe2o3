# Production Profiler KIR Archive V1

`ProductionProfilerKirArchiveV1` is the self-contained restart format for the
production structural owners used by Profiler Variant V3. It is prepared only
from an already-finalized protected Worker V3 owner. It does not serialize a
catalog or bridge claim and later trust that claim; it serializes the complete
bounded inputs needed to derive them again.

## Canonical content

The archive header binds its magic, version, total length, exact build
generation/session/invocation, section count, and reserved fields. Ordered
tagged sections contain:

1. the outer semantic compiler-module handoff;
2. every external provider payload at its exact contiguous ordinal;
3. the compact Worker V3 finalizer replay transcript; and
4. the exact finalized HSACO.

A domain-separated checksum covers every preceding archive byte, and a
separate domain-separated identity covers the complete canonical archive.
Bounds are derived from the existing semantic-handoff, aggregate external
provider, compact-transcript, link-input, and HSACO maxima. The decoder retains
one owned byte buffer plus ranges into that buffer.

The decoder rejects invalid magic or version, nonzero reserved fields,
truncation, trailing data, checksum substitution, missing, duplicate, or
reordered tags, noncontiguous provider ordinals, empty components, excess
providers, per-component excess, and aggregate provider excess.

## Exact replay admission

Decoding returns `InertProductionProfilerKirArchiveV1`. It has no structural
query surface. `admit_exact_replay_v1` reruns the existing complete protected
Worker V3 finalizer validator over the exact archived components. That replay
revalidates the semantic handoff, requests and responses, compiler module,
link plan, intermediate identities, raw HSACO, descriptor finalization, and
the exact final HSACO.

Only after that replay succeeds does admission derive:

- `ProductionSourceIsaCatalogV1`;
- `ProductionKirV7StructuralBridgeV1`; and
- `ProductionSourceIsaCharacteristicCollectionV1`.

Successful admission returns those fresh owners and retains no compiler,
publication, module-load, launch, or collection handle. A valid finalizer
replay can still return a typed-unavailable semantic-debug carrier,
Source/ISA catalog, structural bridge, or Characteristic projection. Each gap
has a stable class and reason code; it is not collapsed into malformed input.

## Agent transport

`fe2o3-profiler-service variant-v3-jsonl` accepts an archive as canonical
lowercase hex together with its caller-pinned `ContentIdentityRecordV1`. The
service verifies that identity before replay and retains at most two admitted
owners for later `compare_variants` requests. Duplicate opens, unknown cited
identities, uppercase or malformed hex, stale revisions, duplicate request
IDs, and request or owner-budget exhaustion fail closed. Typed-unavailable
archives are reported but not retained.

Every response is content-identified under the V3 response domain. The public
response validator recomputes that identity without trusting the service
process. This supports deterministic authority-free queries in a fresh process
without treating separately supplied catalog or bridge bytes as production
owners.

## Trust boundary

The archive binds supplied content and proves that its structural owners can be
rederived by the repository's exact finalizer checks. Its checksum and content
identity are not a third-party signature, so it does not authenticate the
external origin of the compiler, worker, archive, or live profiler data. It
does not prove that a declared schedule executed, that a GPU observation was
captured, that an observed change was causal, or that unobserved partial data
means addition or removal.

Neither the archive nor the JSONL service grants execution, attach, pause,
scheduling, collection, decoder, publication, load, launch, dispatch, or
runtime authority. External executable custody, producer provenance, and OS
sandbox policy remain separate concerns.

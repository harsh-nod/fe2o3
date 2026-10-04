# Native same-stop resource accessor — 2026-09-23

This adds a read-only producer-backed accessor for the currently retained
native stop. It is infrastructure toward #281 V4, **not positive GPU register
qualification, a new CLI/bridge endpoint, or V4 acceptance**.

## Implemented ownership and value boundary

`RocgdbMiProcessV3::native_hardware_stop_resources_v1` returns an immutable
borrow only while its retained session, native stop identity and revision
match. The accessor sends no MI command and cannot import an uploaded DTO.
A sealed, move-only private owner is populated by the existing V5 native
register producer after fresh equality-checked KFD/ROCgdb correlation.

The target derives from the actual `CheckedGfx942XnackMinusDevice`, not an
architecture string, CPU declaration, browser setting or reported register count.
The projection retains exact artifact, association, queue occurrence, process,
dispatch, stop, thread and wave bindings. It exposes literal observed scalar
and predicate bits of at most 64 bits, with their runtime evidence identity.

Vector/special rows are unsupported, not manufactured per-lane VGPR values.
PC remains redacted; source, ISA and memory retain their named unavailable
reasons. No address or allocation authority is inferred from opaque register
bits. Copied or serialized projections are inert historical data; their
structural validator is not an authentication API.

Ordinary MI command attempts, controls, mutable adapter access, polling,
new stops, process transitions and errors clear retained resources, including
early refusal paths. The only preserving inspection is the existing optional
V5 simple-locals query, after successful complete parsing at the unchanged
native stop. The accessor makes no assertion about external events not yet
observed by the process.

Existing V4/V5 response schemas and default output are unchanged. No extra
MI command, GPU launch path, device admission or target expansion was added.
Limits remain 1,024 rows, 128-byte names and one evidence reference per copied
available value; copies use checked lengths and fallible reservation.

## Qualification

The MI350 pinned-toolchain CPU gate passed 358 unit/integration/doc tests
across debugger CLI and protocol, with three tests explicitly ignored.
Twenty new synthetic tests cover projection limits, exact bindings, invalidation,
same-stop locals preservation, early refusals, redaction and unchanged legacy
wire fields. Mock process tests use the existing deterministic MI fixture and
test-only owner injection; they do not admit a real checked device.

All-target Clippy completed without Rust lint diagnostics. The existing Cargo
fixture duplicate-target warning remains; this is not a warning-free workspace
claim. No ROCgdb attach, target launch or GPU observation was performed.

Receipt:
`logs/phase28-resume-r4-compiler-hardware-stop-resources-r1/receipt.json`,
21,922 bytes, SHA-256
`e12562790cac3af9e755d37317b97890a932c18290b29d1fe6da5c122017e55b`.
The tested source census was 6,759 files / 102,079,471 bytes, SHA-256
`b6656991caf1a3911d7e8fe8478ce827cac57def6f5a5d4b836214af5a0f59fa`.
Documentation updates afterward are not included in that snapshot.

## Remaining consuming work

The existing one-shot native launcher drops its process before writing its
outer response. A future capture output must obtain the view before that drop
and label the serialized result historical. A genuinely live browser service
must instead retain the actual process owner and handle events/liveness;
importing a DTO or repurposing the CPU bridge does not establish that ownership.

Positive hardware qualification still needs the actual amd-dbgapi
runtime-metadata handoff and a genuine correlated register observation.
The currently observed MI350 devices are gfx950; the existing checked native
route is gfx942-only. A separately typed gfx950 route is required, not a cast
or relabeling of its engineering device binding. Memory, source and ISA views
need their corresponding producers before their unavailable fields can change.

# Decoded ATT interchange V1

Status: experimental, authority-free admission and query contract for GitHub
issue #215.

## Scope

Decoded ATT V1 carries a canonical export of callbacks from the ROCprofiler SDK
7.2.4 experimental trace-decoder ABI. fe2o3 does not implement or fork the raw
decoder, load its library, read ATT paths, or collect beta ATT. An external
producer supplies the already decoded callback export, an exact Profiler Bundle
V4 ATT manifest, and content identities for the decoder library and exporter.

The ABI vocabulary is pinned to these installed headers:

| Header | SHA-256 | Bytes |
| --- | --- | ---: |
| `experimental/thread-trace/trace_decoder_types.h` | `1d3896cde4e533fb21a3ef74d3978a021465b7b0a456f5e40cc023c68af061fa` | 10,789 |
| `experimental/thread-trace/trace_decoder.h` | `2838cc93d5f7d20ad658667990a3a89f4e9b2ed28cd293278b7fd3e947ff006b` | 7,500 |

These hashes pin bytes. They are not signatures and do not authenticate the
decoder process, exporter, collection, or custody.

## Admission

The input export contains a safe relative raw-reference catalog, code-object
load declarations, and ordered callback batches. Each batch names the exact
wave-reference ordinal passed to that decode invocation. The first batch for
every decoded wave reference must be one nonzero GFXIP record. Record arrays,
nested wave arrays, references, code objects, strings, input bytes, and output
bytes have independent hard bounds and fallible output allocation.

The admitted interchange includes:

- occupancy start/end events with CU/WGP, SIMD, wave slot, timestamp, and PC;
- wave lifetime/context summaries and independently bounded state timelines;
- time-ordered instructions with category, stall, duration, and PC;
- four-bank performance-event values, shaderdata, realtime pairs/frequency;
- all defined INFO values: none, data lost, stitch incomplete, wave incomplete;
- exact Bundle V4, manifest, raw-reference, header, library, exporter, code
  object, export, callback, and record identities.

Raw numeric code-object load IDs are used only in the canonical export preimage.
The interchange publishes export-scoped opaque selectors. A PC with a nonzero
decoder code-object ID retains that selector and exact ELF virtual address. A
native PC with ID zero is redacted and typed unavailable.

Every decoded field is `external_decoder_declared`; reading its bytes is not an
independent hardware observation. Missing raw content identity takes precedence
over all other raw relation claims. If identities are exact but one or more
manifest wave files have no decoder invocation, the relation is explicitly
incomplete. INFO absence never implies completeness, and ATT selection never
implies full-grid wave coverage.

## Agent Query

`DecodedAttQuerySessionV1` provides content-bound pagination and filters over
raw references, code objects, occupancy, fixed-size wave summaries, wave
states, instructions, performance events, shaderdata, realtime records, and
INFO. Child cursors store raw flattened positions and seek through cumulative
per-wave lengths, avoiding a scan from child zero for every page.

The stdin/stdout service is:

```text
fe2o3-profiler-service decoded-att-jsonl
```

Requests use `fe2o3-decoded-att-agent-request-v1`; responses use
`fe2o3-decoded-att-agent-response-v1`. The protocol is canonical lowercase
JSONL with bounded lowercase-hex evidence, monotonic checked revisions, unique
nonzero request IDs, a request-attempt budget, content-bound cursors, and small
typed terminal errors for oversized input or output. It exposes no filesystem,
decoder, profiler, attach, launch, pause, or execution operation.

An additive, separately bound correlation service is also available:

```text
fe2o3-profiler-service decoded-att-source-isa-jsonl
```

Its `open` request supplies canonical Decoded ATT V1 bytes, one exact
export-scoped code-object identity, the exact HSACO bytes claimed by that code
object, and one canonical Characteristic V1 archive. Admission recomputes the
code-object and artifact identities, requires the decoded load size to equal
the HSACO `PT_LOAD` memory span, and uses the production metadata/descriptor/
ELF-symbol binder before treating metadata order as a kernel ordinal. A decoded
ELF virtual PC must fall in exactly one authenticated kernel entry symbol. The
response publishes only an opaque symbol identity and symbol-relative PC, never
the symbol name or address.

Every matching Characteristic interval occurrence is independently paged with
an explicit exact-relation kind. Source, MIR, neutral KIR, target KIR, LLVM, and
ISA coordinates are present only when the exact archive contains them. Each
item retains the decoded ATT loss/completeness and raw-decode relation. Native
PCs, unknown records, other code objects, PCs outside a symbol, and overlapping
symbols remain typed unavailable. Cursors bind all three supplied inputs and
cannot cross an artifact or Characteristic substitution.

## Remaining Boundaries

- No raw ATT decoder or exporter is shipped in this slice.
- No beta ATT collection or live decoded capture was run.
- Library/exporter identities do not prove process custody or authenticity.
- Source/MIR/KIR/LLVM/ISA correlation requires the separate exact supplied
  decoded-ATT/HSACO/Characteristic binding. The canonical archive is
  self-claimed evidence; it does not authenticate its producer.
- DEBUG callback payloads are unsupported because the pinned header defines no
  public payload ABI for them.
- Realtime pairs are declarations from the decoder; they do not establish a
  common direct-KFD/rocprof clock or dispatch identity.

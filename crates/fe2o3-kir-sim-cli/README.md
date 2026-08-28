# fe2o3-kir-sim

fe2o3-kir-sim is the standalone, Linux-only command-line boundary for bounded
deterministic CPU execution of exact verified canonical Kernel IR V7:

    fe2o3-kir-sim --kir-v7 kernel.kir --request request.json
    fe2o3-kir-sim --bundle kernel.fe2sim --request request.json
    fe2o3-kir-sim --kir-v7 kernel.kir --request request.json --output result.json
    fe2o3-kir-sim --bundle kernel.fe2sim --request request.json \
      --record-seeded-schedule schedule.json --schedule-seed 42
    fe2o3-kir-sim --bundle kernel.fe2sim --request request.json \
      --replay-schedule schedule.json
    fe2o3-kir-sim --kir-v7 kernel.kir --request request.json \
      --race-evidence
    fe2o3-kir-sim --bundle kernel.fe2sim --request request.json \
      --explore-seeded-schedules 64 --schedule-seed 42 \
      --schedule-max-decisions 1048576 \
      --exploration-max-retained-decisions 65536

It does not link or initialize HSA, HIP, KFD, ROCm, or a GPU. Simulation is an
observation only. It grants no source-refinement, proof, compiler, artifact,
load, launch, GPU-equivalence, race-freedom, timing, performance, or performance
prediction authority.

`--bundle` and `--kir-v7` are mutually exclusive. Bundle admission securely
captures one bounded regular file, strictly decodes and revalidates
`VerifiedSimulationBundleV1`, maps its exact admitted gfx942/gfx950 target to
the CPU target profile, and executes only its embedded canonical V7 bytes. It
never re-lowers source, invokes a compiler, launches hardware, or falls back
between execution modes. A separately supplied request retains the same strict
16 MiB boundary and preflight checks as raw KIR.

The library also exposes `load_debug_simulation_bundle_v2` for the debugger's
explicit V2 envelope route. It strictly verifies the outer V2 bytes, the exact
embedded V1 bundle, and the independently committed Source Map V2 payload.
It still executes only the embedded canonical KIR V7 and never authorizes or
performs source relowering. The standalone `fe2o3-kir-sim` command remains on
its frozen V1 bundle route.

The versioned `tutorial/fill-v1` known-answer fixture is directly runnable:

    cargo run --locked -q -p fe2o3-kir-sim-cli --bin fe2o3-kir-sim -- \
      --kir-v7 crates/fe2o3-kir-sim-cli/tutorial/fill-v1/kernel.kir \
      --request crates/fe2o3-kir-sim-cli/tutorial/fill-v1/request.json

Its canonical builder, exact KIR bytes, request, complete expected result, and
regression test are committed together. The fixture starts at exact KIR V7; it
does not claim that a Rust source program produced those bytes.

Inputs are regular files opened on Linux with openat2, O_NOFOLLOW, O_NONBLOCK,
and RESOLVE_NO_SYMLINKS|RESOLVE_NO_MAGICLINKS. FIFOs, devices, symlinks in any
path component, oversized files, and files changed while read are rejected.
Non-Linux platforms fail rather than using a weaker path.

An output path is published beneath a pinned no-symlink parent using a retained
anonymous 0600 O_TMPFILE inode, complete buffered write, file fsync,
linkat through a retained descriptor entry beneath a pinned, authenticated
procfs mount, and parent-directory fsync. The descriptor entry is matched to the
anonymous inode before publication. The final link inherently does not replace
an existing regular file or symlink. After a successful link, the CLI never
attempts a racy name-based rollback in a mutable parent. A later failure reports
whether durability is unknown or the published name is uncertain, and callers
must resolve that explicit state. There is no attacker-visible staging name;
filesystems or procfs setups without these primitives fail closed.

`--record-canonical-schedule PATH` and `--record-seeded-schedule PATH
--schedule-seed U64` publish the existing semantic CPU scheduler's successful
decision record. `--schedule-max-decisions COUNT` bounds recording and defaults
to 1,048,576. `--replay-schedule PATH` is mutually exclusive with recording.
Schedule input uses the same regular-file snapshot boundary; publication uses
the same private durable no-replace boundary as result output. The strict
`fe2o3-simulation-schedule-v1` document is canonical JSON capped at 256 MiB and
binds exact KIR, raw-versus-bundle route, complete verified bundle identity and
subject, request bytes, target, limits, context, transcript, seed, coverage, and
decisions.
Binding drift is rejected before execution; runnable-decision and transcript
validation remains in the simulator itself.

`--explore-seeded-schedules COUNT --schedule-seed FIRST_U64` is a separate,
bounded multi-schedule mode. It sweeps the contiguous wrapping seed interval,
uses `--schedule-max-decisions` as the per-schedule dynamic bound, and retains
at most `--exploration-max-retained-decisions` decisions across one race, one
no-race, and one incomplete witness. The CLI caps schedules at 4,096 and
retained witness decisions at 65,536; the simulator caps one schedule at
4,194,304 decisions. Zero and above-cap values fail as closed command-line
errors. Consuming the requested seed interval is reported separately from
schedule-space exhaustion. Exploration never claims to exhaust the schedule
space or prove race freedom.

Each retained witness contains its race assessment and an exact canonical
`fe2o3-simulation-schedule-v1` document encoded as a JSON string, together with
its byte length and SHA-256. Decode that string and write its UTF-8 bytes
unchanged to use it with `--replay-schedule`; object reserialization is neither
required nor trusted. The embedded document preserves the existing exact KIR,
raw-versus-bundle route, bundle identity and subject, request, target, limits,
context, transcript, seed, coverage, and decision binding. Substitution is
rejected before replay.

For recording, the schedule is the primary durable artifact and is published
before the ordinary result is delivered. Simulation or schedule-encoding
failure emits no result and creates no schedule name; a schedule-publication
failure before `linkat` does the same. Post-link schedule-publication failures
use the existing typed `publication_state` contract because the name may exist
with uncertain durability. If a later result-file publication or stdout write
fails after confirmed schedule durability, the schedule remains and the error
carries `"schedule_published":true`; two output sinks cannot share one
filesystem transaction.

## Request V1

The strict fe2o3-simulation-request-v1 JSON shape rejects duplicate and unknown
fields:

    {
      "schema": "fe2o3-simulation-request-v1",
      "kernel": "fill",
      "grid": [4, 1, 1],
      "workgroup": [2, 1, 1],
      "shared_buffers": [
        {
          "id": 7,
          "element": "u32",
          "access": "read_write",
          "alignment": 4,
          "bytes": "0x00000000000000000000000000000000"
        }
      ],
      "arguments": [
        {"kind": "scalar", "type": "u32", "bits": "0x0000002a"},
        {
          "kind": "buffer_view",
          "backing": 7,
          "element": "u32",
          "access": "read_write",
          "alignment": 4,
          "byte_offset": 0,
          "elements": 4
        }
      ]
    }

Scalar types are bool, signed and unsigned 8/16/32/64/128-bit integers,
F16/BF16/F32/F64, and 64-bit index. Bits use 0x plus exactly the type width in
lowercase hexadecimal; bool uses one digit. Floating-point scalars and buffer
elements are encoded as their exact IEEE-format bits, never decimal host
values. Buffer bytes use 0x followed by lowercase even-length hexadecimal.
initialized is optional; when present it is
an exact 0x-prefixed byte bitset, least-significant bit first, with bit N
describing buffer byte N and unused high bits zero. Omission means all bytes are
initialized. Shared buffers use the same exact codec and byte budgets. A
buffer_view names one shared backing plus an aligned byte offset and element
extent; multiple views may intentionally overlap.

Files are bounded to 16 MiB, arguments and shared buffers to 4,096 each, one
decoded buffer to 4 MiB, and all distinct and shared decoded buffers together
to 16 MiB. Success is streamed as bounded deterministic
fe2o3-simulation-result-v1 JSON. Additive evidence fields explicitly state that
the result was simulated, hardware was neither observed nor validated, no
performance prediction was made, and identify the scalar target profile,
scheduler, and exact canonical KIR. Every failure is stable
fe2o3-simulation-error-v1 JSON on stderr. Parsing failures use closed application
codes selected from private structural markers, while other malformed JSON is
classified by serde's closed syntax/data categories. Input failures identify
kir_v7, simulation_bundle, or request. Dynamic failures include exact invocation hierarchy and Kernel IR
site coordinates; overlong function identities carry an explicit bounded
prefix, original byte count, and truncation flag. Unsupported preflight failures
report exact total/emitted/truncated counts and a deterministic
encoded-byte-bounded prefix with closed feature codes. Post-publication failures
include a closed publication_state.

The immutable CLI simulation profile caps one allocation at 16 MiB, all live
allocations at 64 MiB, successfully admitted and accepted preflight/execution
resident peaks at 256 MiB, logical
invocations at 1,048,576, scheduled slots at 4,194,304, and execution steps at
134,217,728. Call depth is capped at 64 and live SSA values in one frame at
4,096 so their conservative resident-memory product remains within the host
budget. The 256 MiB setting is not enforced before canonical construction or
decode: verified-owner construction and a simulator decode/re-encode later
rejected by the post-decode resident check may transiently exceed it. Those
phases remain bounded by the 16 MiB canonical input limit and frozen KIR
wire/count/depth caps.

## Result V1

Success contains status ok, authority observation_only, the exact V7 SHA-256
and canonical byte length, all execution counters including padded scheduled
slots, the deterministic cooperative workgroup schedule identity, exact
semantic transcript SHA-256, complete decision/workgroup/barrier-release
coverage, bounded cross-invocation conflict assessment, copied argument values,
and copied shared backing buffers and views. With no schedule option the V1 CLI
retains canonical cooperative ordering. Recording and replay are command
policy, not request-document fields, so an unchanged request cannot silently
opt into a different execution order.
`--race-evidence` additively includes the bounded race assessment for one
ordinary run; without that flag the result remains byte-compatible with the
previous V1 output. Evidence distinguishes unordered races, conflicts ordered
by integer atomic serialization or a compatible same-workgroup global
acquire-release barrier, and incomplete assessment. Release/acquire atomic and
fence edges into ordinary memory are not fully modeled, so a potentially
affected ordinary conflict is reported as incomplete rather than as an exact
race or no-race result. Source sites in this agent-facing evidence carry an
explicit bounded function prefix, original byte count, and truncation flag.
Scalar bits, buffer bytes, and initialization bitsets retain their exact typed
lowercase hexadecimal encodings. Result bytes are measured exactly and capped
at 64 MiB before output publication begins, then emitted directly through a
bounded writer rather than assembled as one JSON string.

## Exploration V1

Exploration emits `fe2o3-simulation-exploration-v1`, not an ordinary copied-back
execution result. Its fixed authority fields state observation-only CPU
simulation, no hardware observation or validation, no performance prediction,
and no schedule-space exhaustion. The input block identifies exact raw KIR or
the complete simulation-bundle identity and subject. The exploration block
reports requested and hard bounds, seed wrapping, attempted/completed/failure
counts, race/no-race/incomplete counts, retained decisions, requested-budget
consumption, and witness-retention exhaustion. Dynamic failures retain the
seed, stable execution kind, invocation, bounded site, and structured wave
detail when applicable.

Race witnesses contain first byte-level access sites and invocation hierarchy,
atomic flags, ordered-conflict reason, incomplete record/synchronization flags,
and the exact replay schedule string. Valid retention exhaustion is a successful
bounded result with a null witness and `witness_retention_exhausted:true`, not
silent truncation. Per-schedule decision exhaustion is counted as a typed
dynamic failure. The 65,536 retained-decision cap leaves the maximally spelled,
JSON-escaped witness set within the existing 64 MiB response envelope; the
bounded writer and typed `output_too_large` error remain a defensive backstop.

Wave32/Wave64 logical collective failures are machine-readable. Incomplete
waves include width, logical wave ordinal, and fixed-width hexadecimal active
and required masks. Divergent waves identify the nonparticipating local lane;
mismatched waves identify the expected operation site; invalid tiled shuffles
include source lane and tile width. These are logical KIR diagnostics, not GPU
`EXEC` state, ISA simulation, or hardware-wave claims.

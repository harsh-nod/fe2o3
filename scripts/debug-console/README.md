# CPU debugger console

This additive Python 3.11+ / POSIX console speaks the existing
`fe2o3-debug-request-v1` / `fe2o3-debug-response-v1` JSONL protocol. It launches one
explicitly selected `fe2o3-debug sim` child. It adds no compiler, simulator,
debugger, wire, Resource V1, source-edit or private origin/lifecycle API.

Status: 37 pure unit controls and four actual CPU-process sessions passed on
2026-09-19. The actual-source profile was Bundle V5, wave32; the V6 usage below
is illustrative, not a V6 qualification claim. See the
[exact qualification and limits](../../docs/evidence/lifecycle-console-20260919.md).
No Cargo/package dependency or build step is added.

## Usage

Use already-exported input and its matching simulation request:

```sh
python3 scripts/debug-console/fe2o3_debug_console.py \
  --binary /absolute/path/to/fe2o3-debug \
  --kind bundle-v6 \
  --input /absolute/path/to/kernel.fe2sim \
  --request /absolute/path/to/request.json
```

Optional `--wave-width 32` or `64` defaults to 64. The admitted input switches are
`kir-v7`, `diagnostic-kir-v16`, `diagnostic-kir-v17`, `bundle`, and `bundle-v2`
through `bundle-v6`. Their backend-specific admission requirements are unchanged.

Use a terminal or pipe for stdin and stdout. Linux's default selector cannot
monitor regular-file descriptors; pipe input/output through a bounded consumer
instead of redirecting this candidate directly from/to a regular file.
Commands must end in LF (terminal CRLF is accepted). EOF requests orderly
termination; Ctrl-C invokes bounded cleanup.

```text
help
state
step 1
reverse 1
continue 100
break add 0 7 1 before
break list
break remove 1
watch add 1 0 0 4 write
watch list
watch remove 1
source 0 7 1
stack
memory 1 0 0 4
quit
```

The numbers above illustrate syntax only: they do not identify a real kernel,
allocation, breakpoint or generation. Use coordinates and IDs returned by the
current debugger for your input. `source FUNCTION BLOCK OPERATION` and
`break add FUNCTION BLOCK OPERATION [before|after]` use protocol roster ordinals:
BLOCK is not the runtime KIR `BlockId`. There is no fuzzy name/range lookup.

`memory ALLOCATION GENERATION OFFSET LENGTH` and
`watch add ALLOCATION GENERATION OFFSET LENGTH [read|write|atomic|any]` require
the allocation generation explicitly. Do not infer reuse, lifetime generation or
activation identity from a numeric address, depth or generation zero. Default
watch access is write; timing is the existing `after_commit` observation.

Each checked response is printed as compact JSON, including its session,
revision, snapshot anchor, stop and all returned unavailability facts. Strings
are JSON-escaped rather than interpreted as terminal controls. User-command
errors remain local and do not send protocol requests. Break/watch add replies
are acknowledgements; list afterwards to inspect the backend-assigned IDs.
Lists and stack request at most 16 entries; `next_cursor` means additional
entries were omitted, not that the displayed page was the whole collection.
This small console does not implement pagination traversal.

## Exact subset and truth boundaries

- `state` is GetState; `step`/`reverse` are operation-granularity Step with
  optional count 1..64; `continue` is forward Continue with a 1..65536 event
  budget (default 1024). No over/out, source stepping or implicit focus is added.
- Breakpoints are only exact KIR operation sites before/after; watchpoints are
  only explicit allocation/range/access requests. No expression evaluation,
  arbitrary predicates, hit conditions or automatic address discovery.
- `source` resolves exactly the requested site. It neither reads source files
  nor authenticates names/ranges/ownership. `stack` requests dispatch scope.
  The existing backend may report source/stack/memory facts unavailable.
- Snapshot anchors must equal the response session cursor; source coordinates
  and memory allocation/range must exactly match the request. Redacted,
  unavailable and truncated facts remain as returned. The client does not
  upgrade declaration/observation into source, ABI, hardware or proof authority.
- This is public-protocol replay only. Private runtime-origin/cursor/lifecycle
  candidates are not reachable. Existing frame/occurrence ABI limitations,
  allocation-generation unavailability and Resource V1 are unchanged.
- No hardware route, GPU launch, compilation, server, socket listener or network
  service is provided.

## Correlation and resource limits

One child and one in-flight request are allowed. Request IDs increase without
retry; every response must have the matching schema, ID and operation.
The first request is capability discovery at revision zero. Configuration
identity and CPU/simulation truth classification must stay fixed. Read-only
requests and error/unavailable responses must leave the whole session unchanged.
Filter mutations and successful termination require exactly one revision
increment. Control operations may retain or increment revision once; moving
the cursor requires an increment and events_advanced must match its exact delta.
A captured snapshot must anchor to that response's session cursor. Termination
must report terminated state. Rejection is fatal without committing the
candidate response to client state.

Python preserves u64 integers exactly and duplicate JSON keys are refused.
The client validates framing, correlation, revisions and the selected joins,
not every Rust protocol payload rule. Backend-owned payload details are
otherwise retained and displayed; this is not a new protocol validator owner.

| Limit | Bound |
| --- | --- |
| User command line / queued lines | 1024 UTF-8 bytes / 8 |
| Cumulative input / user lines | 64 KiB / 256 |
| Wire requests / each request | 256, last reserved for quit / 4096 bytes |
| Read chunk / response line | 4096 bytes / 256 KiB |
| Cumulative child stdout / stderr | 8 MiB / 64 KiB |
| Parsed graph | 32768 nodes, depth 32, collection length 4096 |
| Console pending / cumulative output | 2 MiB / 8 MiB |
| Memory/watch extent / page | 1..4096 bytes / 16 entries |
| Pending request / whole session | 30 seconds / 900 seconds including idle |
| Console-output / normal close-drain deadline | 5 seconds / 2 seconds |
| Cleanup | TERM + 1-second wait, then KILL + 1-second wait |
| Launch path bytes / file stat caps | 4096 / executable 512 MiB, input 64 MiB, request 1 MiB |

Byte/work limits are explicit client limits, not whole-process memory/CPU
guarantees. Graph checks happen after parsing one byte-bounded line; they are
not precharged parser-allocation accounting. JSON escaping and Python object
overhead can exceed input byte counts. The fixed child invocation preserves
the backend's own capture/execution/residency limits; these console caps do not
replace or lower simulator admission limits.

Paths must be absolute, regular, nonempty and canonical at initial stat.
Those checks are not descriptor custody or source authentication: path contents
can change before the backend opens them. Select a trusted executable and
retained inputs; the backend remains responsible for actual input admission.

Deadlines are checked by the selector loop and are not a hard real-time OS
guarantee. On a timeout or lost response, a mutating request's outcome may be
unknown: the console does not retry it. It attempts process-group termination
only while the owned child leader remains live, avoiding signals to a possibly
recycled group after the leader is reaped. It performs at most two one-second
waits, closes its pipes and reports if reaping was not confirmed. This does not
guarantee cleanup of escaped descendants, descendants holding inherited pipes
after leader exit, uninterruptible processes, or effects already performed.
The intended child is the selected CPU debugger, not arbitrary user shell code.

## Validation

Pure controls do not spawn a debugger, compiler or test executable:

```sh
python3 -B -m unittest discover -s scripts/debug-console -p 'test_debug_console_*.py' -v
```

They cover command grammar and exact requests, lossless u64/duplicate-key and
framing refusal, pending/cumulative bounds, correlation/revision/snapshot joins,
source/memory substitutions (including Python bool/int aliasing), reverse
cursor deltas, reserved quit, partial/blocked writes, deadlines and mocked
bounded cleanup. They are synthetic controls, not a live process qualification.
The 37 controls include aliased-descriptor cleanup, exact acknowledgements,
movement direction and terminate cursor preservation.

Actual-source qualification used a freshly exported ordinary LDS reduction
Bundle V5 and a retained reductionProfile request with the current measured
CPU debugger. Four paced sessions checked commands/quit, EOF, SIGINT and
shared-FD PTY quit. Source and stack resolved; snapshot not_captured facts
remained visible. Reverse/repeat preserved checkpoint values and bytes. One
real write watchpoint stopped, followed by the expected initialized first
result word 128. Breakpoint registration/list/removal was checked; no breakpoint
stop or whole-output/canary result is claimed by this console run.

Each observed backend exited and was reaped; the PTY blocking mode was restored.
Successful cleanup does not establish escaped-descendant closure on failures.
For other inputs, retain exact input/tool identities and requalify the applicable
profile. Pace commands after replies rather than overfilling the eight-line
queue; keep build/admission failures separate from semantic negative evidence.

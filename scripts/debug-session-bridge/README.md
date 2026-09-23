# Opt-in local CPU debugger bridge

This is a small, explicit connection from a local browser debugger to the existing
CPU simulator JSONL debugger. The owner selects one trusted debugger executable,
one already-exported KIR/bundle and one simulation request before the service
starts. The browser cannot select files, build code, set environment variables,
supply process arguments, invoke a shell, choose a GPU or change that input set.

**Qualification boundary:** Fresh normal-source exports and bounded real
loopback HTTP/debugger sessions are recorded in the
[2026-09-23 qualification report](../../docs/evidence/live-cpu-debugger-20260923.md).
That report separates actual browser coverage from routed mocks and identifies
remaining unqualified paths. These finite observations do not establish the
complete checklist below, broad #281 V2 closure, terminal-fault capture or
allocation reuse.

## Existing owners, not a new debugger protocol

The bridge imports the unchanged repository-local
[console command parser](../debug-console/debug_console_commands.py),
[framing/session validator](../debug-console/debug_console_protocol.py),
[bounded cleanup](../debug-console/debug_console_process.py), and
[fixed CPU launch argument builder](../debug-console/fe2o3_debug_console.py).
It does not introduce another KIR reader, rewrite a backend reply, infer a source
variable from an SSA name, manufacture a frame or fill an unavailable snapshot.

The console validator checks selected wire structure, exact request correlation,
revision/configuration/cursor transitions, CPU classification and applicable
source/stack/memory joins. It is **not the complete Rust wire schema validator**.
Other payload content remains backend-reported, not independently proved by
this bridge. Snapshot/source/memory unavailability and backend error replies
remain visible. A bridge success means a correlated backend reply was received;
it does not turn an inner backend refusal into a successful operation.

The initial command set is exactly the existing console grammar, except that
help/quit/empty input are not browser commands:

- `state`; `step [1..64]`; `reverse [1..64]`; `continue [1..65536]`.
- `break add F B O [before|after]`, `break list`, `break remove ID`.
- `watch add ALLOCATION GENERATION OFFSET LENGTH [read|write|atomic|any]`,
  `watch list`, `watch remove ID`.
- `source F B O`, `stack`, `memory ALLOCATION GENERATION OFFSET LENGTH`.

Source resolves an existing **KIR site**, not Source Variable V2 queries.
Coordinates are explicit ordinals or allocation/generation identities, never
guessed IDs. List/stack requests use the owner's limit of 16; a returned next
cursor means an incomplete page, not permission to infer the rest. Memory is
bounded to 4096 requested bytes. There is no step-over/out, expression evaluator,
source upload, target change, arbitrary backend operation or launch route.

## Additive bounded checkpoint queries

The bridge-local live-query adapter adds exactly three command spellings without
changing the existing console grammar, Rust protocol or simulator:

- `allocations`: first global ResourceV1 allocation page.
- `accesses ORDINAL 0`: first retained global access page for an actual returned
  allocation, with generation exactly zero.
- `variables 1`: first SourceVariableV2 page for the supported current frame.

An active, successful, stopped operation-step reply selects the captured
checkpoint. The three protocol schemas retain their own response envelopes and
share the same request counter, complete session projection and revision.
Resource queries use the selected unframed anchor. Source inspection requires
the current complete one-frame stack with a present `next_operation`, refining
that anchor with legacy frame 1/occurrence 1, not a dynamic activation.
Discovery, state replies and uncaptured watch stops do not select a checkpoint.

Inventory and access pages request at most 16 rows and scan at most 64 records.
The source page requests at most 16 rows. Callers cannot forward a cursor,
supply another scope/filter, query another generation or request arbitrary
frame selection. A returned cursor/token remains evidence of a partial page,
not authority to invent absent records or continue automatically.

The companion [live checkpoint dashboard guide](https://github.com/harsh-nod/fe2o3-kernels/blob/main/docs/live-checkpoint-dashboard-v1.md)
uses explicit refreshes: at most three calls for stack/source/inventory or five
when adding selected accesses/memory. A missing source prerequisite uses two
or four actual calls, preserving source unavailability without manufacturing a
SourceVariableV2 refusal. The browser reserves the applicable maximum and
retains at most 2 MiB of HTTP response text per collection; memory windows stay
within the existing 4096-byte request bound. No producer limit is relaxed.

Control/filter mutations, uncertainty and connection replacement invalidate the
selected checkpoint and derived observations. Source bindings and SSA values
remain separate; generation-zero inventory is not allocation-reuse evidence.
The table displays retained source identities/spans, not a fetched Rust body
or authenticated source. Existing recorded viewers remain independent.

The [separate qualification](../../docs/evidence/live-checkpoint-dashboard-20260923.md)
records the new ten-module runtime selection (six bridge and four console),
fresh mixed-query HTTP results in both forks, and actual desktop/mobile
dashboard sessions. Pure/mock tests remain separate; the earlier live-controls
report is unchanged. No broad V2, terminal-fault, allocation-lifetime or
protected-proof exit is claimed here.

## Owner launch

Use a trusted Linux/POSIX launcher and ordinary Python script execution. These
imports depend on the known script directory; `python -I /absolute/script.py`
is not the documented invocation. No dependency install is required.

Create a **new** secret file in an owner-controlled canonical directory; do not
reuse a repository fixture, a predictable string, a URL, a command-line secret,
a browser storage item or a checked-in file. For example, after replacing the
absolute file path:

```sh
python3 -B -c 'import os,secrets,sys; p=sys.argv[1]; fd=os.open(p,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o600); os.write(fd,secrets.token_hex(32).encode("ascii")+b"\n"); os.close(fd)' /absolute/private/bridge-token
```

The service requires a non-symlink regular secret file owned by the current UID,
exact mode 0600, one hard link, and exactly 64 lowercase hex characters with an
optional final LF. All-zero is refused. Entropy is a launcher obligation:
shape/permission checks cannot prove that a caller actually used a CSPRNG.
The service keeps the file descriptor and rechecks name, metadata and secret
bytes. A token-file change stops the service and attempts owned-child cleanup.
The secret is never printed or included in a response/error/access log.

Choose exact size/SHA-256 values from the owner's previously selected artifact
identities, not an unreviewed on-the-fly replacement. Replace every uppercase
slot below. These are templates, not qualified artifact values:

```sh
python3 -B scripts/debug-session-bridge/fe2o3_debug_bridge.py \
  --binary /absolute/qualified/fe2o3-debug \
  --binary-bytes BINARY_BYTES --binary-sha256 BINARY_SHA256 \
  --kind bundle-v6 --input /absolute/retained/source.bundle.json \
  --input-bytes INPUT_BYTES --input-sha256 INPUT_SHA256 \
  --request /absolute/retained/request.json \
  --request-bytes REQUEST_BYTES --request-sha256 REQUEST_SHA256 \
  --wave-width 64 \
  --token-file /absolute/private/bridge-token \
  --port 8741 --origin http://127.0.0.1:5173
```

The template is illustrative; the dated report identifies the actual qualified
Bundle V6 profile. Select the retained input kind and logical wave width
appropriate to your input. The fixed child argv is:

```text
SELECTED_BINARY sim --SELECTED_KIND SELECTED_INPUT --request SELECTED_REQUEST
  --wave-width SELECTED_WIDTH --protocol jsonl
```

The child inherits the **trusted launcher's environment**, with HIP, ROCr and
CUDA visible-device variables set empty. There is no browser-controlled
environment. This is not an OS sandbox or proof of dynamic-loader/dependency
closure; a malicious selected executable or launcher environment is out of the
trusted profile. The CPU route and the existing response validator's CPU flags
are the applicable execution classification, not a claim that environment
variables alone prevent arbitrary code from accessing hardware.

Only `127.0.0.1` is bound; hostname, wildcard and IPv6 listeners are absent.
The frontend must run at exactly the configured **HTTP loopback origin**.
A public/HTTPS hosted site is deliberately not admitted by this initial profile.
If an owner separately chooses SSH forwarding, both local forwarding binds must
also be loopback, and preserve the selected numeric ports/Host/origin; for
example `-L 127.0.0.1:8741:127.0.0.1:8741`. This document does not start tunnels,
open firewalls or authorize exposure beyond loopback.

In the browser, enter the loopback endpoint and paste the secret into the
memory-only password field. Connect is explicit. The client must retain the
original endpoint/token/connection handle across an uncertain reply so that an
explicit Disconnect still targets the original session. Changing UI fields
must not silently retarget cleanup, reconnect or retry a command.

## Closed HTTP contract

There are three exact HTTP/1.1 POST paths, without query, fragment, percent
aliases or redirects: `/v1/connect`, `/v1/command`, `/v1/disconnect`.
Content type is exactly `application/json`. Every request JSON value is an
ASCII string: there are no JSON numeric bridge counters, nested caller objects,
booleans or nulls. Duplicate JSON keys and extra keys are refused.

Every POST requires one exact Host, one configured Origin, one canonical
Content-Length and one `X-Fe2o3-Bridge-Token` header. Token comparison is
constant-time after shape validation. Transfer-Encoding, Expect,
Content-Encoding, cookies and Authorization are refused. Duplicate headers,
folding, HTTP/1.0, arbitrary routes and bodies over the cap are refused before
dispatch. No standard-handler access/error request-line logging is installed.

OPTIONS is a no-child preflight only: exact Host/Origin, requested method POST,
and precisely the Content-Type and X-Fe2o3-Bridge-Token requested-header names.
It cannot carry a token or nonempty body. CORS returns only the configured
loopback origin and `Vary: Origin`, never a wildcard or credentials.
Responses are no-store and each socket is closed after one request.

Request schema: `fe2o3-cpu-debug-bridge-request-v1`.

```json
{"schema":"fe2o3-cpu-debug-bridge-request-v1","action":"connect","connection_id":"CLIENT_RANDOM_64_LOWERCASE_HEX"}
```

The client creates the connection ID before dispatch using a CSPRNG. It is a
cleanup/correlation handle, **not authentication**. The server generates an
independent random bridge_session. A connect performs only the existing
discover_capabilities handshake. An already-active child causes session_exists,
never replacement. Reused connection IDs are refused even after cleanup.

```json
{"schema":"fe2o3-cpu-debug-bridge-request-v1","action":"command","bridge_session":"SERVER_RANDOM_64_LOWERCASE_HEX","sequence":"1","expected_revision":"0","command":"state"}
```

The next sequence must be exact; connect response sequence is `"0"` and its
backend request_id is 1. Command sequence n maps to backend request_id n+1.
The expected revision must equal the last accepted backend revision. An inner
backend unavailable/error reply consumes a request sequence while preserving
its backend revision/state. Stale session/sequence/revision refuses before
dispatch; the service never retries an uncertain command.

Success schema: `fe2o3-cpu-debug-bridge-response-v1`. Connect/command responses
have exactly these fields:

```text
schema, status:"ok", connection_id, bridge_session, sequence,
session, response_json, closed:false
```

The session object keeps the existing console shape and CPU booleans; only its
three u64 values are projected to canonical decimal strings:
`revision`, `cursor.event_sequence`, `cursor.state_revision`.
`response_json` is the **lossless re-encoded** complete backend reply, without
LF, not original transport bytes. Its u64 JSON integers must be parsed with the
site's lossless JSON parser, never ordinary JavaScript number conversion.
Original whitespace/key formatting is not retained or claimed as raw custody.

Disconnect requires only the exact original connection_id and normal HTTP
authentication; it deliberately ignores command revision/sequence:

```json
{"schema":"fe2o3-cpu-debug-bridge-request-v1","action":"disconnect","connection_id":"CLIENT_RANDOM_64_LOWERCASE_HEX"}
```

Its success has exactly `schema,status:"disconnected",connection_id,closed:true`.
It confirms the owned child leader was reaped after bounded cleanup, **not**
that a backend terminate request was acknowledged. No command is replayed.
A stale ID cannot close a newer child. A same-ID cleanup acknowledgement can be
repeated after known closure, but fresh connect always needs a new ID.

Non-2xx bridge errors have exactly `schema,status:"error",code,outcome,closed`.
Codes are closed to: invalid_request, authentication_failed, session_exists,
session_unavailable, stale_session, stale_sequence, stale_revision,
command_refused, resource_limit, backend_failed and cleanup_failed.
Outcome is `not_sent` or conservatively `unknown`; closed is factual, and may
be false. No token, supplied command, file path, stderr or session data is echoed.
An authentication failure does not alter an active child or its sequence.
Independent lifetime expiry may still happen while a client is unauthenticated.

The browser disables uncertain local state and must not automatically retry.
A request/socket timeout, disconnect during an in-flight request, extra
pipelined bytes, invalid backend reply or failed successful-response write
poisons that bridge session and attempts cleanup. A 35-second browser deadline
may conservatively expire before all server-side cooperative read/write bounds;
that is an unknown outcome, not evidence that the command did not run.

## Custody, limits and cleanup

The three selected files are absolute canonical regular files with explicit
expected sizes/SHA-256s. Each is opened O_NOFOLLOW with a retained descriptor.
Name/descriptor device, inode, mode, UID/GID, link count, size, mtime and ctime
are checked initially and around each backend request. Full bytes are hashed
initially, before each connect and after each confirmed cleanup. The original
selected metadata/identities are never refreshed to make a mutation acceptable.
Changed custody prevents reuse of the bridge instance.

The actual backend still opens the selected executable/input/request by their
**paths**. Descriptor checks do not make those opens atomic or bind all dynamic
dependencies; a between-check path/content race remains outside an immutable
trusted-input assumption. A metadata check is not a source signature,
authenticated provenance, compiler-closure proof or arbitrary hostile-filesystem
sandbox. Filesystem reads are bounded in bytes but not hard real-time OS I/O.

| Bound | Closed value |
|---|---:|
| Active child / pending backend operation / accepted active socket | 1 / 1 / 1 |
| Distinct connections per service / accepted sockets per service | 4 / 1024 |
| Child lifetime / service lifetime | 900 s / 1800 s |
| Backend request / HTTP header+body / response write deadlines | 30 s / 5 s / 5 s |
| HTTP headers / count / individual header / request line | 8192 B / 32 / 1024 chars / 2048 chars |
| HTTP body / response body | 4096 B / 1 MiB |
| Raw backend line / cumulative stdout per child / cumulative stderr | 256 KiB / 8 MiB / 64 KiB |
| Re-encoded inner backend JSON | 256 KiB |
| Backend requests per child | 1 discovery + at most 254 commands |
| Binary / KIR-bundle / simulation-request size | 512 MiB / 64 MiB / 1 MiB |
| Full-hash read work across service | 6 GiB |
| Cleanup waits | TERM + 1 s; KILL + 1 s |

The console reserves its 256th slot for terminate; this bridge does not expose
that command and closes through owned process cleanup. Each child uses the
console's bounded JSON graph checks (32768 nodes, depth 32, collections 4096).
Byte caps are not total process RSS or CPU guarantees; parsing/escaping/object
overhead consumes extra memory. The service's single accept loop runs idle
expiry checks even without new HTTP requests, but checks and filesystem work
are cooperative, not a hard real-time supervisor. A partially read socket can
delay a tick by its finite HTTP read bound.

Cleanup reuses the existing owner routine and retains an unreaped child handle;
no reconnect is allowed while that handle remains. Closed means the owned
leader was observed reaped. There is no guarantee for escaped descendants,
uninterruptible kernel waits, descendants surviving a reaped leader, or effects
already performed by an uncertain command. SIGINT/SIGTERM and normal service
exit attempt the same bounded cleanup. An outer owner supervisor remains
necessary for a qualified run's wall/storage/process-group bounds.

## Validation and remaining real-session coverage

Run the pure/mock controls without creating a socket/listener or debugger child:

```sh
python3 -B -m unittest discover -s scripts/debug-session-bridge -p 'test_bridge_*.py' -v
python3 -B -m unittest discover -s scripts/debug-console -p 'test_debug_console_*.py' -v
```

The 33 new groups cover session/sequence/revision joins, inner backend refusal,
closed command grammar, exact u64 projection, disconnect after uncertainty,
stale IDs, failed reap, idle expiry, invalid replies, response/input caps,
HTTP framing/Host/Origin/token/CSRF guard shapes, lost response writes,
unauthenticated resets, token mutation and selected-file identity checks.
They are synthetic/mock checks, not live debugger, socket or browser evidence.

The following is the full validation checklist, not a list of completed gates.
The dated report identifies the bounded observed coverage; real 900-second idle
expiry, uncertain-mutation abort and response-backpressure behavior remain
unqualified beyond mocks:


1. Fresh current-tool source export and an independently checked CPU simulation
   request; pin the exact debugger, bundle, request, console/bridge sources and
   launch environment. Do not relabel an old console run or test-only synthetic
   Session as new bridge execution.
2. Loopback socket connect and discovery, state, genuine step/reverse/continue,
   source site and stack availability, memory bytes/init and a real committed
   watchpoint stop. Show actual backend revisions and lossless u64 joins.
3. Real breakpoint set/list/remove and, if claimed, a real breakpoint stop.
   Registering a breakpoint alone is not an observed stop.
4. Wrong/missing/duplicate token/Host/Origin/header/body refusals, no child spawn
   on unauthenticated Connect, no active-session mutation by bad auth, exact
   preflight, no wildcard listener, and stale-sequence/session/revision refusal.
5. Abort/lost response and explicit cleanup using the original connection ID,
   no mutating retry, idle expiry, SIGINT/SIGTERM, response backpressure and
   observed child reaping. Failed/unavailable cleanup must remain a refusal.
6. A real local frontend connection with controls disabled while pending;
   preserved unavailable values; endpoint/token edits never retarget cleanup;
   explicit reconnect after known closure; no browser URL/storage secret.
7. Before/after selected byte identities and source census under the owner
   supervisor. Preserve failed runs; report which evidence is retained versus
   newly executed and do not imply hardware/proof authority.

Before-unwind terminal fault snapshots, actual same-session allocation
generation reuse, source-variable/SSA currentness beyond these existing
queries, protected capture/proof ownership and remaining #280/#282 authoring/
recipe exits stay with their existing owners. This bridge advances live CPU
interaction, not those independent acceptance requirements.

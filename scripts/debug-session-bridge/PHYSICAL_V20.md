# Local physical-entry V20 CPU bridge

This is a separate opt-in bridge/workbench for the existing typed V20 CPU-debug CLI. It does not widen the legacy bridge or console, admit V21, authenticate Rust sources, expose physical registers, or grant compilation/launch/hardware authority. No compiler pin or milestone maturity changes.

The local owner selects `--kind diagnostic-kir-v20 --wave-width 64`. The launcher checks the same explicit binary/input/request length+SHA-256 pins and retains existing file custody, token permissions, exact loopback origin, process ownership and cleanup rules. The V20 canonical input cap is 128 KiB and request cap 16 KiB. No runtime-observation, bundle fallback or capability-based version inference is allowed. The browser cannot supply paths, argv, environment, raw inner requests, scope, frame or continuation tokens.

The closed local memory choice is output allocation ordinal **1**, generation **0**. It is not discovered from a browser field or inferred to be a second input buffer. A successful memory reply must identify that same allocation and the same selected capture. SSA logical pointer results must also identify it. If an otherwise valid input uses another allocation arrangement, this profile refuses unsupported replies; it does not relabel an allocation. The profile choice does not prove host allocation ownership, alias safety or native ABI binding.

## Owner-selected launch

Use the existing bridge launcher and supply exact values from independently inspected files:

```text
python3 scripts/debug-session-bridge/fe2o3_debug_bridge.py
  --binary /absolute/pinned/fe2o3-debug --binary-bytes DECIMAL --binary-sha256 LOWERHEX64
  --kind diagnostic-kir-v20
  --input /absolute/export.kir --input-bytes DECIMAL --input-sha256 LOWERHEX64
  --request /absolute/request.json --request-bytes DECIMAL --request-sha256 LOWERHEX64
  --wave-width 64 --token-file /absolute/owner-only-token
  --port 8742 --origin http://127.0.0.1:5173
```

This is one command shown on multiple lines, not a verified launch receipt. The token file must meet the existing owner-only 0600/random nonzero 64-lowerhex requirements. Do not put its contents in the URL. The panel does not start a service. Open **V20 live CPU workbench**, enter the explicit loopback endpoint and token, acknowledge the V20 local profile, and connect. A changed input clears views; cleanup still uses the captured old endpoint/token/connection.

## Exact outer protocol

Both requests and responses use `schema: "fe2o3-physical-cpu-bridge-v20"`. HTTP paths remain `/v1/connect`, `/v1/command`, `/v1/disconnect`; local launch selects the schema, never the browser. Legacy clients reject it, and this client rejects the old outer schema.

Connect retains `action, connection_id`. A command retains `action, bridge_session, sequence, expected_revision, command`; all request fields are strings. Disconnect retains `action, connection_id`. Positive envelopes retain `status, connection_id, bridge_session, sequence, session, response_json, closed`; revision/cursor integers in the outer session are exact decimal strings. Inner `fe2o3-debug-{request,response}-v1` is unchanged. The bridge losslessly re-encodes inner replies: this is not their original wire byte stream.

Closed commands:

| Command | Meaning / bound |
| --- | --- |
| `state` | Inspect the current observation. |
| `step N`, `reverse N` | Move 1–64 recorded events, not operation/instruction execution. |
| `seek N` | Request cursor 0–8193; the actual capture may be shorter and refuse. |
| `values N` | Current lane/frame-1 SSA page, 1–64 rows. |
| `values next` | Use only the current accepted revision-bound continuation token. |
| `memory OFFSET LENGTH` | Read allocation 1/generation 0, 1–256 bytes. Browser offsets must fit an exact safe integer. |

Cursor 0 is entry. A completed capture's end cursor is record count + 1; it is not a captured event. Moving end→last or last→end advances zero recorded events. Every successful control increments revision, including a clamped/same-position control. Until the exact completed cursor is observed, the adapter validates the requested movement without inventing a total capture length.

Capabilities, exact operation/request ID, session identity/revision, selected logical scope and KIR site, SSA paths, page hash/position, memory range and initialization bits are checked. Returned active_mask is logical Wave64 residency, **not authored EXEC**. Symbolic pointer halves/carries remain `not_represented`; unavailable does not mean zero, redacted numeric bits or native addresses. Source locations remain `requires_authenticated_map`. Query errors/unavailability clear previous values. The UI never displays a past memory/SSA table as a current reply.

## Lifetime and limits

Existing bridge 255-command, four-connection, 30-second request, 900-second session, 8-MiB child stdout and 64-KiB stderr caps remain. V20 inner replies are <=64 KiB; the dedicated browser envelope cap is 128 KiB. The browser has a strict 35-second observation deadline, including decode/projection; it also checks after awaited work so late timer delivery is not positive evidence. Existing HTTP deadlines remain five seconds. A deadline is refusal on return to the controlling code, not preemptive execution control.

Unknown outcome, malformed/stale reply, abort or input replacement clears visible state. No retry occurs; the captured connection becomes cleanup-only. Failed cleanup retains custody. A response-send timeout does not claim already-sent bytes were rolled back. Reconnecting requires successful owned cleanup and a fresh nonce.

## Evidence limits

The five compiler-side JSON fixture rows are unchanged lines from the previously qualified actual V20 CLI recording, with exact source/line/hash provenance. Tests deliberately rebase correlation/revision and synthesize HTTP envelopes; those mutations are labeled synthetic, not a new live observation. Site tests reuse the existing actual recording and similarly synthesize only the transport controls. Empty/delayed/malformed cases are test-only. The empty-workbench browser tests do not exercise a service.

The recorded V20 viewer, original V19 viewer and legacy live bridge remain separate. Qualification below was performed by the integrating root agent, not the donor author.

## Root qualification — 2026-09-24

The integrated route passed root-run checks on \`mi350\`:

- Compiler Python bridge: 114 tests; legacy console: 37 tests.
- Site: lint, typecheck, 1,739 tests in 123 files, production build, 21 lab checks and all 196 desktop/mobile browser cases without retries. Browser HTTP fixtures are synthetic transport tests, not actual service observations.
- Actual source-derived V20 CLI → loopback bridge: four fresh bridge/CLI sessions (one, both diamond selectors, alternate registers); 380 inner responses matched the previously qualified recording after the explicit request-ID remapping. Configuration, revision, selected capture, typed values and memory payloads were not rebased.
- Actual HTTP rejection controls: 12 read-only authentication/origin/schema refusals and 16 stale-session/revision/sequence/unsupported-command refusals.
- Every actual session disconnected its owned CLI and reaped its direct bridge child (exit 130 after SIGTERM), with known CLI pidfd exit, no forced cleanup and no cleanup error. This is not a whole-family-reaping claim.

Actual-source report: 62,732 bytes, SHA-256
\`ea8b4fce7ff34eb927750990a41a27cf4eb6495e94dd2286e5d407ba231b63f2\`.
Root runner receipt: 102,954 bytes, SHA-256
\`6631562ebd07e793f0b6c71699a0c63f4f38b42612a32ef1de8b75a03c6ee8cb\`.
CLI ELF: 63,543,472 bytes, SHA-256
\`3d8c6d4061ae920a52e53efdc54d7b144b3bfceffc9af38aea84b8c13c7c9619\`.
The private qualification tooling also passed 32 pure/mock tests.

The first qualification attempt passed its first session and then refused during the second bridge constructor before any HTTP exchange. Its exact original cause remains unknown. The successor adds closed, token-screened startup diagnostics and truthful cleanup evidence; it does not relax process identity, retry a launch or alter production code. The four-session result above is a new complete run, not a reclassification of that failure.

Ten historical nonmutating inner refusals per case were omitted where this narrower outer protocol cannot express their raw requests. Termination was replaced by the bridge's owned disconnect. No actual successful continuation-page case is claimed by this matrix. These limitations are separate from the pure parser/transport tests.

This qualifies the closed local CPU route only: no authenticated Rust source map, physical register capture, GPU execution, resumable execution, native/protected authority, complete Python/ELF dependency closure, general profile support or additional milestone completion. Compiler pin and maturity are unchanged.

# Interrupted Aggregate Attribution Campaign

Status: failed and recovered. This is not a successful native qualification,
instrumentation-overhead result, or HIP/HSA performance-acceptance packet.

The source was signed commit
`f1fb5f4877bf888b3346318cb9d53e2757e7930b`, published to both remotes. Its
21-stage CPU qualification passed. The original prepared protocol is preserved
in `PROTOCOL.md`; the original controller tools, raw receipts, binding, ownership
marker, failure, and terminal state are unchanged.

The local native SSH receipt records exit 255 and
`RuntimeError: interrupted by signal 15`. Initial collection failed because
owned process 3440710 was still present. The controller did not delete remote
data while collection was incomplete. The remote runner later exited with
`BrokenPipeError: [Errno 32] Broken pipe`; its completion record explicitly says
`native_execution: false`. The cause of the external SIGTERM is not established.

## Recovery

`recover.py` authenticated the original signed tools before loading them. After
the owned runner exited, it obtained a complete remote inventory, collected every
listed byte, and compared the collected inventory before cleanup. It then used
the authenticated ownership helper to check recorded process-group absence,
remove only the marked directory, and verify path/process absence. The local
transport roster, binding, and all member hashes were checked before removing
that owned directory.

- Remote path: `/home/harsh/fe2o3-xgmi-aggregate-attribution-20260919.dbead0bf0008cee9`
- Local transport: `/home/harsh/.codex-tmp/fe2o3-peer-hot-payload-qd11ulbm`
- Recovery receipts: `recovery/raw/{inventory,collect,cleanup,absence}`
- Complete recovered artifacts: `recovery/remote`
- Recovery result: `recovery/status.json`

All four recovery commands passed with reaped local process groups. Remote
path/process absence and local transport absence were confirmed. No foreign
processes, GPU resets, or unrelated files were involved. The original failed
`state.json` was not rewritten to claim success.

`SHA256SUMS` is an integrity manifest over the entire preserved packet, not a
native-success certificate. The original complete-campaign `verify.py` is
retained unchanged and cannot certify this interrupted attempt. No performance
numbers from this attempt are accepted; a fresh complete campaign is required.

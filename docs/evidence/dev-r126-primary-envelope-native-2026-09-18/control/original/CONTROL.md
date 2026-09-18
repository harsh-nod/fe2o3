# Exact R126 Controller Review

This control packet is outside the reviewed frozen payload. `remote_control.py`
is sent over SSH stdin; it is never installed in the remote payload directory.
No native execution is authorized merely by creating these files.

## Exact Inputs

- Frozen local bundle:
  `/home/harsh/.codex-tmp/fe2o3-r126-primary-envelope-native-bundle-8b0021ba-20260918`.
- Payload manifest SHA-256:
  `b69190c2182feb82476d6be2ecd6e30e11d58c3b804e686d7ec338814d1ac4f6`.
- Root-created remote directory:
  `/tmp/fe2o3-r126-primary-8b0021ba-20260918.oVqzCv3c`.
- Host: `mi300x`; no alternate host or directory option is accepted.

Root reported creating that private marked directory successfully. An earlier
root SSH command failed shell quoting before Python or mkdir ran, exit 2.
That is narrated preparation history supplied by root, not a synthesized raw
receipt. This controller did not execute either preparation command.

## Ordering

1. Record local frozen protocol tests and payload authentication.
2. Require the exact remote directory to contain only its matching owner marker.
3. Upload the frozen bundle; authenticate payload bytes remotely and install the
   path/payload/commit-bound approval document only after explicit root READY.
4. Invoke the reviewed runner once under external timeout 1,200 seconds and
   KILL-after 15 seconds. The SSH controller wait is bounded at 1,300 seconds.
   Preserve its complete stdout/stderr and the remote outer-process receipt.
5. Obtain the complete remote file inventory and original recorded PID/group
   roster. Record their actual absence and accessible same-UID path references.
6. Collect the entire exact directory locally, including the ELF and every
   successful or failed result. Compare every relative filename and hash.
7. Only after that match and successful absence checks, request cleanup using
   the collected inventory digest and original PID roster. Cleanup independently
   recomputes both, checks visibility again, and deletes only the exact marked
   directory. It never signals processes.
8. Run a separate SSH command requiring path absence, recorded PID/group absence
   and the same bounded visible-reference scan. Report native failure separately
   even when collection and cleanup succeed.

The scanner covers accessible same-UID `/proc` exe, cwd, fd and maps references.
It reports unreadable entries and excludes other UIDs; it does not assert global
reference absence or isolation. An unreadable recorded owner/member is a refusal.
PID reuse, a surviving group, visible path reference, missing launch receipt,
changed collected bytes or incomplete observation also refuses removal.

SSH completion is not remote process absence. An interrupted transport may leave
the externally bounded remote runner alive until timeout. A refused inventory
or cleanup leaves the exact directory intact for root recovery; the controller
does not retry native execution, invent missing receipts, signal by name, or
delete files to hide a failed attempt. Local recorder timeouts clean only their
own local SSH/scp process groups. Native child cleanup remains the frozen
runner's responsibility, followed by independently checked remote absence.

## Review Commands

Already run, without SSH or GPU access:

```sh
python3 -B controller.py --qualify-only \
  --output /home/harsh/.codex-tmp/fe2o3-r126-primary-envelope-controller-local-20260918
```

Both recorded local commands passed with their process groups absent: four
protocol tests and authentication of all 20 frozen payload files. Raw command,
stdout/stderr, hashes, status and timing receipts are retained at that output.

Only after root explicitly approves the final controller/cleanup hashes:

```sh
python3 -B controller.py --root-ready-approved \
  --remote-directory /tmp/fe2o3-r126-primary-8b0021ba-20260918.oVqzCv3c \
  --output /home/harsh/.codex-tmp/fe2o3-r126-primary-envelope-native-run-20260918
```

The output path must be fresh. A successful native case still requires all
unchanged strict test, UID/BDF/PID/VRAM, immediate and fixed-delay rules in the
frozen runner. This wrapper does not relax those rules or introduce performance
or actual ioctl-failure claims.

## Rejected Draft

Root rejected controller draft SHA-256
`8417c2954861ef0ba0aa1604dfb32501eb29c0285b6dac8fb19e0b71d27e1abe`
before any native authorization: cleanup received the PID list intended for
absence, while absence received the cleanup PID/inventory object. The associated
remote-control draft hash was
`df894cf5b39aba6459ccf3bcf543916ecf9cdfd8f2b60d659c7e680b1347af98`.

The corrected controller uses one tested cleanup/absence call path. Four focused
CPU tests serialize its real SSH command arguments into the remote pure parsers,
reject the swapped payload types and malformed rosters/digests, and verify that
cleanup failure prevents the absence command. These tests perform no SSH or
filesystem removal. A fresh local-only controller qualification records them
alongside the unchanged frozen protocol tests and payload authentication.

# Supervised debugger startup — 2026-09-28

The built debugger has now completed a supervised MI2 startup on mi350.
This is startup-only qualification: no inferior was loaded, run or attached;
no kernel was dispatched and no physical register sample was captured.

The first attempt started and exited cleanly but was rejected when the final
process census exhausted its unchanged 64 MiB cumulative read budget. Its
failed gate and cleanup evidence are retained:
`ee1b7a38bd83cee5d0fa7eed55d31556eb9903c91c2adced5f5109538c85205d`.
A new attempt ran during a coordinated SSH-quiet window across the source
workers. It passed without changing source, resource limits or refusal guards.
The observations are consistent with process churn affecting the first scan;
they do not prove the identity of the process responsible.

Successful full gate:
`f5bafcaf01ad08faadf40985059a20773de65447ffbae57369d160f9801f82e7`.
Root protocol and loaded-file readback:
`cfdf237805e1c19024a749e38fd963653552579fae09c1f5d17a8200a7423447`.

All 1,024 selected inputs, tools and complete compiler source matched before
and after the successful command. The family protocol joined the request,
manager invocation, process identity, ready record and release. Both child
reaping and the owned cgroup's absence were checked separately; a manager's
cleanup status was not treated as proof of child reaping.

The service retained its 120-second runtime, 10-second stop, 256-task and
2 GiB memory bounds. Two fixed read-only process censuses each completed in
one attempt, with nine rows and 101 inspected FD links: 4,888,754 bytes before
and 4,880,624 bytes after, each under 0.5 seconds. These are sampled current
references, not historical cleanup or OS-enforced global writer exclusion.

The collector produced 14 MI2 records from two fixed commands, observed exit,
zero loaded objfiles and only inferior PID zero. Its initial snapshot contained
104 Python modules and its loaded maps contained 50 ELF paths. Root revalidated
193 file observations: 170 present files totaling 398,626,885 bytes and 23
absent paths. Complete contents, canonical paths and file identities were
checked again after the command.

The loaded snapshot contains 123 present paths beyond the static candidate
list; 37 static candidate paths were not observed. This distinction is expected
to require a separately reviewed loaded-file profile before a later consumer.
The snapshot is not complete import history, and a cache path is not proof of
which bytecode executed. System site hooks were not disabled by this profile.
No startup receipt grants runtime, source, artifact or launch authority.

The source generation and coordination expire at 2026-09-28 21:30 UTC. Historical
receipts do not extend that deadline or authorize replay. The earlier
[input-closure qualification](debugger-startup-input-closure-qualification-20260928.md)
remains a separate prerequisite, not a substitute for this startup observation.

Next work is the reviewed loaded-file profile and a separately scoped,
same-client stopped-wave capture through the owned adapter. The visualization
must consume actual source/PC-linked register and memory samples; none were
produced here. V4 and U4 remain open, and accepted broad exits remain
M1/V1/V2/U1/U2/U3 (6/18).

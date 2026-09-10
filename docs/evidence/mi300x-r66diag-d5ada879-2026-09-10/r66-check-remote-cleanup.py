"""Read-only absence and selected-device checks after this owned campaign."""
import json
import os
from pathlib import Path
import re
import subprocess
import sys

request = json.load(sys.stdin)
stage = request['stage']
assert re.fullmatch(r'/dev/shm/fe2o3-r66-owner\.[A-Za-z0-9]+', stage)
assert not os.path.lexists(stage), 'owned stage still exists'
absent = []
for pid in request['pids']:
    assert type(pid) is int and 0 < pid < 1 << 30
    assert not Path(f'/proc/{pid}').exists(), f'owned PID still exists or was reused: {pid}'
    absent.append(pid)
groups = set(request['process_groups'])
assert all(type(group) is int and 0 < group < 1 << 30 for group in groups)
for process in Path('/proc').iterdir():
    if not process.name.isdecimal():
        continue
    try:
        stat = (process / 'stat').read_text()
    except FileNotFoundError:
        continue
    fields = stat[stat.rindex(')') + 2:].split()
    assert int(fields[2]) not in groups, f'owned process group still has member: {process.name}'
    try:
        command = (process / 'cmdline').read_bytes()
    except FileNotFoundError:
        continue
    except PermissionError:
        assert process.stat().st_uid != os.getuid(), 'cannot inspect same-user process'
        continue
    assert stage.encode() not in command, f'process still references owned stage: {process.name}'
telemetry = json.loads(subprocess.check_output([
    '/opt/rocm/bin/rocm-smi', '--showuniqueid', '--showbus', '--showuse', '--showmemuse', '--json']))
selected = telemetry['card1']
assert selected['Unique ID'] == '0xab83d2ffef0d3cdf' and selected['PCI Bus'].lower() == '0000:26:00.0'
selected_idle = int(selected['GPU Memory Allocated (VRAM%)']) == 0 and int(selected['GPU use (%)']) <= 5
require_idle = request.get('require_selected_idle', True)
assert type(require_idle) is bool
if require_idle:
    assert selected_idle
print(json.dumps(dict(absent_stage=stage, absent_pids=sorted(absent), absent_groups=sorted(groups),
    selected_gpu_idle=selected_idle, selected_idle_required=require_idle, telemetry=telemetry), sort_keys=True))

timeout --signal=TERM --kill-after=5s 45s ssh -T -o BatchMode=yes -o ConnectTimeout=10 mi300x /opt/rocm/bin/rocm-smi --showuniqueid --showbus --showuse --showmeminfo vram --showpids --json

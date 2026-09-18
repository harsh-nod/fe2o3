#!/usr/bin/env bash
# Sourced by the runner; these observations are not a device reservation.
admit() {
    local status pids
    date -u +%FT%T.%NZ
    status=$(/usr/bin/timeout --kill-after=5s 20s /opt/rocm/bin/rocm-smi --showuse --showmeminfo vram --showuniqueid --showbus --json) || return $?
    printf '%s\n' "$status"
    jq -e '.card4 | .["Unique ID"] == "0x54f88318ca05093d" and .["PCI Bus"] == "0000:85:00.0" and (.["GPU use (%)"] | tonumber) == 0 and (.["VRAM Total Used Memory (B)"] | tonumber) < 536870912' <<< "$status" > /dev/null || return $?
    pids=$(/usr/bin/timeout --kill-after=5s 20s /opt/rocm/bin/rocm-smi --showpidgpus) || return $?
    printf '%s\n' "$pids"
    awk '
        /^PID [0-9]+ is using [0-9]+ DRM device\(s\)/ {
            count=$5; seen++;
            if (count > 0) {
                if (getline <= 0 || NF != count) { bad=1; exit }
                for (i=1; i<=NF; i++) if ($i !~ /^[0-9]+$/ || $i == 4) { bad=1; exit }
            }
            next
        }
        /^=|^[[:space:]]*$/ { next }
        { bad=1; exit }
        END { if (bad || seen == 0) exit 1 }
    ' <<< "$pids" || return $?
    printf 'admitted gpu=4 uid=0x54f88318ca05093d bdf=0000:85:00.0\n'
}

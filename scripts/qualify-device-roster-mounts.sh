# Qualification-only roster observations; no native or application authority.
# shellcheck shell=bash
# shellcheck disable=SC2154
validate_device_roster_case() {
  if [[ $campaign == genuine-device-roster ]]; then
    [[ -n ${FE2O3_GENUINE_GPU_ROSTER:-} && -z ${FE2O3_GENUINE_TWO_GPU_CASE+x} \
      && -z ${FE2O3_GENUINE_GPU_UID0+x} && -z ${FE2O3_GENUINE_GPU_UID1+x} ]] || {
      printf 'roster campaign requires explicit roster without pair controls\n' >&2; return 1;
    }
  elif [[ -n ${FE2O3_GENUINE_GPU_ROSTER+x} ]]; then
    printf 'GPU roster requires genuine-device-roster campaign\n' >&2; return 1
  fi
}

read_device_roster_selection() {
  "${FE2O3_GENUINE_JQ:?}" -Rse '
    split("\n") | map(select(startswith("FE2O3_GPU_ROSTER_SELECTION_V1="))
      | ltrimstr("FE2O3_GPU_ROSTER_SELECTION_V1=") | fromjson)
    | if length == 1 then .[0] else error("missing or duplicate GPU roster selection") end'
}

configure_device_roster_mounts() {
  FE2O3_GPU_MOUNTS=()
  local minors
  minors=$("${FE2O3_GENUINE_JQ:?}" -er '
    def uids: type == "array" and all(.[]; type == "string" and length == 18
      and test("^0x[0-9a-f]{16}$") and . != "0x0000000000000000")
      and (unique | length) == length;
    if keys == ["devices", "excluded_unselected_devices", "observed_host_devices",
        "occupancy", "render_minors", "schema", "scope"]
      and .schema == "fe2o3.genuine-gpu-roster-selection.v1"
      and .scope == "selected-admitted-context-only" and .occupancy == "not-measured"
      and (.devices | uids and length >= 2 and length <= 8)
      and (.observed_host_devices | uids and length >= 2 and length <= 256 and sort == .)
      and (.excluded_unselected_devices | uids and sort == .)
      and ((.devices - .observed_host_devices) | length) == 0
      and .excluded_unselected_devices == (.observed_host_devices - .devices)
      and (.render_minors | type == "array" and all(.[]; type == "number"
        and floor == . and . >= 128 and . <= 255) and (unique | length) == length)
      and (.render_minors | length) == (.devices | length)
    then .render_minors[] else error("invalid bounded roster selection") end
  ' <<< "${FE2O3_GENUINE_GPU_ROSTER_SELECTION:?}") || return 1
  local -a selected
  readarray -t selected <<< "$minors"
  [[ ${#selected[@]} -ge 2 && ${#selected[@]} -le 8 ]] || return 1
  local -a mounts=(--dev-bind /dev/kfd /dev/kfd --dir /dev/dri)
  local minor path
  for minor in "${selected[@]}"; do
    [[ $minor =~ ^[0-9]{3}$ ]] || return 1
    path="/dev/dri/renderD${minor}"
    mounts+=(--dev-bind "$path" "$path")
  done
  FE2O3_GPU_MOUNTS=("${mounts[@]}")
}

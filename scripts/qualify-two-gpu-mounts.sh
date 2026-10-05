# Qualification-only observation data, never application or device authority.
# Sourced by the resource campaign and its root-free parser tests.
read_two_gpu_selection() {
  "${FE2O3_GENUINE_JQ:?}" -Rse '
    split("\n") | map(select(startswith("FE2O3_GPU_SELECTION_V1="))
      | ltrimstr("FE2O3_GPU_SELECTION_V1=") | fromjson)
    | if length == 1 then .[0] else error("missing or duplicate GPU selection") end'
}

configure_two_gpu_mounts() {
  FE2O3_GPU_MOUNTS=()
  local minors
  minors=$("${FE2O3_GENUINE_JQ:?}" -er '
    if keys == ["devices", "render_minors", "schema"]
      and .schema == "fe2o3.genuine-gpu-selection.v1"
      and (.devices | type == "array" and length == 2
        and all(.[]; type == "string" and length == 18 and test("^0x[0-9a-f]{16}$")
          and . != "0x0000000000000000"))
      and .devices[0] != .devices[1]
      and (.render_minors | type == "array" and length == 2
        and all(.[]; type == "number" and floor == . and . >= 128 and . <= 255))
      and .render_minors[0] != .render_minors[1]
    then .render_minors[] else error("invalid two-GPU selection") end
  ' <<< "${FE2O3_GENUINE_GPU_SELECTION:?}") || return 1
  local -a selected
  readarray -t selected <<< "$minors"
  [[ ${#selected[@]} == 2 ]] || return 1
  FE2O3_GPU_MOUNTS=(--dev-bind /dev/kfd /dev/kfd --dir /dev/dri)
  local minor path
  for minor in "${selected[@]}"; do
    [[ $minor =~ ^[0-9]{3}$ ]] || return 1
    path="/dev/dri/renderD${minor}"
    FE2O3_GPU_MOUNTS+=(--dev-bind "$path" "$path")
  done
}

#!/usr/bin/bash
set -euf

case ${BASH_SOURCE[0]} in
  /*) script_dir=${BASH_SOURCE[0]%/*} ;;
  *) script_dir=$PWD/${BASH_SOURCE[0]%/*} ;;
esac
repo=$(/usr/bin/realpath --canonicalize-existing -- "$script_dir/../..")
cd "$repo"

if [[ -n $(/usr/bin/git status --porcelain=v1 --untracked-files=all) ]]; then
  printf 'launcher test requires a clean checkout\n' >&2
  exit 1
fi
if /usr/bin/find "$repo" -type d -name __pycache__ -print -quit | /usr/bin/grep -q . ||
  /usr/bin/find "$repo" -type f -name '*.pyc' -print -quit | /usr/bin/grep -q .; then
  printf 'clean checkout already contains Python bytecode\n' >&2
  exit 1
fi

# The exec is required: no same-scope parent may remain in the delegated cgroup
# when the controller creates its command sub-cgroups. The controller self-test
# performs the post-execution bytecode scan before reporting success.
exec scripts/gfx942-cov6-compiler-evidence.sh --self-test

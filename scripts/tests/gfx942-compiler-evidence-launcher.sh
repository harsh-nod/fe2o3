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

scripts/gfx942-cov6-compiler-evidence.sh --self-test

if /usr/bin/find "$repo" -type d -name __pycache__ -print -quit | /usr/bin/grep -q . ||
  /usr/bin/find "$repo" -type f -name '*.pyc' -print -quit | /usr/bin/grep -q .; then
  printf 'launcher created Python bytecode in the clean checkout\n' >&2
  exit 1
fi
if [[ -n $(/usr/bin/git status --porcelain=v1 --untracked-files=all) ]]; then
  printf 'launcher self-test dirtied the checkout\n' >&2
  exit 1
fi
printf 'gfx942 compiler-evidence clean launcher test: PASS\n'

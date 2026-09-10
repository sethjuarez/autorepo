#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "Usage: scripts/live-github.sh --create | --repo sethjuarez/fake-repo-example [--nocapture]" >&2
}

create=0
repo=""
nocapture=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --create)
      create=1
      shift
      ;;
    --repo)
      repo="${2:-}"
      shift 2
      ;;
    --nocapture)
      nocapture=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      usage
      exit 2
      ;;
  esac
done

if [[ "$create" == "1" && -n "$repo" ]] || [[ "$create" == "0" && -z "$repo" ]]; then
  usage
  exit 2
fi

if [[ -n "$repo" && ! "$repo" =~ ^sethjuarez/(fake-repo-|autorepo-test-).+ ]]; then
  echo "Live GitHub tests may target only sethjuarez/fake-repo-* or sethjuarez/autorepo-test-*" >&2
  exit 2
fi

export AUTOREPO_LIVE_GITHUB=1

if [[ "$create" == "1" ]]; then
  export AUTOREPO_LIVE_CREATE=1
  unset AUTOREPO_LIVE_REPO
else
  export AUTOREPO_LIVE_REPO="$repo"
  unset AUTOREPO_LIVE_CREATE
fi

args=(test --test live_github -- --ignored)
if [[ "$nocapture" == "1" ]]; then
  args+=(--nocapture)
fi

cargo "${args[@]}"

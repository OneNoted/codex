#!/usr/bin/env bash
set -euo pipefail

branch_name=${BRANCH_NAME:-main}
upstream_remote_name=${UPSTREAM_REMOTE_NAME:-upstream}
upstream_remote_url=${UPSTREAM_REMOTE_URL:-https://github.com/openai/codex.git}
dry_run=${DRY_RUN:-false}

git config user.name "${GIT_AUTHOR_NAME:-OneNoted Automation}"
git config user.email "${GIT_AUTHOR_EMAIL:-notes@madeingotland.com}"

if ! git remote get-url "$upstream_remote_name" >/dev/null 2>&1; then
  git remote add "$upstream_remote_name" "$upstream_remote_url"
fi

git fetch origin "$branch_name"
git fetch "$upstream_remote_name" "$branch_name"

git checkout -B "$branch_name" "origin/$branch_name"

before=$(git rev-parse HEAD)
upstream_ref="$upstream_remote_name/$branch_name"
base=$(git merge-base HEAD "$upstream_ref")
downstream_count=$(git rev-list --count "$base..HEAD")

if [[ "$downstream_count" == "0" ]]; then
  git reset --hard "$upstream_ref"
elif ! git rebase "$upstream_ref"; then
  conflicted_files=$(git diff --name-only --diff-filter=U || true)
  upstream_sha=$(git rev-parse "$upstream_ref")
  git rebase --abort || true
  {
    echo "sync_conflict=true"
    echo "upstream_sha=$upstream_sha"
    echo "conflicted_files<<EOF"
    printf '%s\n' "$conflicted_files"
    echo "EOF"
  } >> "${GITHUB_OUTPUT:-/dev/null}"
  {
    echo "Upstream sync needs conflict repair."
    echo
    echo "Conflicted files:"
    printf '%s\n' "$conflicted_files"
  } >> "${GITHUB_STEP_SUMMARY:-/dev/null}"
  echo "upstream sync has conflicts; repair workflow will handle this"
  exit 0
fi

after=$(git rev-parse HEAD)
if [[ "$before" == "$after" ]]; then
  echo "main is already current"
  exit 0
fi

if [[ "$dry_run" == "true" ]]; then
  echo "dry run: would update $branch_name from $before to $after"
  exit 0
fi

changed_files="$(git diff --name-only "$before..$after")"
if grep -qE '^\.github/workflows/' <<<"$changed_files"; then
  if [[ -z "${SYNC_PUSH_TOKEN:-}" || -z "${GITHUB_REPOSITORY:-}" ]]; then
    echo "sync needs CODEX_SYNC_WORKFLOW_TOKEN because upstream changed workflow files" >&2
    exit 1
  fi
  git remote set-url origin "https://x-access-token:${SYNC_PUSH_TOKEN}@github.com/${GITHUB_REPOSITORY}.git"
elif [[ -n "${GITHUB_TOKEN:-}" && -n "${GITHUB_REPOSITORY:-}" ]]; then
  git remote set-url origin "https://x-access-token:${GITHUB_TOKEN}@github.com/${GITHUB_REPOSITORY}.git"
fi

git push --force-with-lease="refs/heads/$branch_name:$before" origin "HEAD:$branch_name"

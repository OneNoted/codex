#!/usr/bin/env bash
set -euo pipefail

branch_name=${BRANCH_NAME:-main}
upstream_remote_name=${UPSTREAM_REMOTE_NAME:-upstream}
upstream_remote_url=${UPSTREAM_REMOTE_URL:-https://github.com/openai/codex.git}
repair_branch_prefix=${REPAIR_BRANCH_PREFIX:-automation/upstream-sync}
repair_model=${REPAIR_MODEL:-gpt-5.4}
repair_max_attempts=${REPAIR_MAX_ATTEMPTS:-8}
verify_command=${VERIFY_COMMAND:-}
auto_merge=${AUTO_MERGE_REPAIR_PR:-false}
dry_run=${DRY_RUN:-false}

git config user.name "${GIT_AUTHOR_NAME:-OneNoted Automation}"
git config user.email "${GIT_AUTHOR_EMAIL:-notes@madeingotland.com}"

if ! git remote get-url "$upstream_remote_name" >/dev/null 2>&1; then
  git remote add "$upstream_remote_name" "$upstream_remote_url"
fi

git fetch origin "$branch_name"
git fetch "$upstream_remote_name" "$branch_name"

upstream_ref="$upstream_remote_name/$branch_name"
upstream_sha=$(git rev-parse "$upstream_ref")
upstream_short=${upstream_sha:0:12}
repair_branch="${REPAIR_BRANCH:-$repair_branch_prefix-$upstream_short}"
body_file=$(mktemp)

if git merge-base --is-ancestor "$upstream_ref" "origin/$branch_name"; then
  echo "origin/$branch_name already contains $upstream_sha"
  exit 0
fi

push_token=${SYNC_PUSH_TOKEN:-${GH_TOKEN:-${GITHUB_TOKEN:-}}}
remote_url=${GITHUB_REPOSITORY:+https://x-access-token:${push_token}@github.com/${GITHUB_REPOSITORY}.git}
if [[ -n "${remote_url:-}" && "${remote_url}" != *":@github.com/"* ]]; then
  git remote set-url origin "$remote_url"
fi

git checkout -B "$repair_branch" "origin/$branch_name"

if git rebase "$upstream_ref"; then
  echo "upstream sync rebased cleanly; opening review PR"
else
  if [[ -z "${OPENAI_API_KEY:-}" ]]; then
    conflicted_files=$(git diff --name-only --diff-filter=U || true)
    git rebase --abort || true
    cat >&2 <<MSG
Upstream sync has merge conflicts and OPENAI_API_KEY is not configured.

Conflicted files:
$conflicted_files

Add an OPENAI_API_KEY repository secret to enable automatic conflict repair,
or run this workflow manually with a repaired branch.
MSG
    exit 1
  fi

  attempt=1
  while [[ -d .git/rebase-merge || -d .git/rebase-apply ]]; do
    conflicted_files=$(git diff --name-only --diff-filter=U || true)
    if [[ -z "$conflicted_files" ]]; then
      GIT_EDITOR=true git rebase --continue
      continue
    fi

    if ((attempt > repair_max_attempts)); then
      echo "exceeded repair attempt limit: $repair_max_attempts" >&2
      git status --short >&2
      exit 1
    fi

    cat > /tmp/upstream-sync-repair-prompt.txt <<PROMPT
You are repairing a fork's upstream rebase.

Goal:
- Resolve the current merge conflicts only.
- Preserve the fork behavior: raw reasoning traces are visible by default, and the TUI reasoning trace display remains available.
- Keep upstream changes unless they directly conflict with that fork behavior.
- Do not publish, push, tag, force-push, or create commits.
- After editing, leave the working tree with no unmerged paths.

Context:
- Target branch: $branch_name
- Upstream commit: $upstream_sha
- Conflicted files:
$conflicted_files

Run focused local inspection as needed, edit the conflicted files, and stop once the conflict markers/unmerged paths are resolved.
PROMPT

    codex exec \
      --dangerously-bypass-approvals-and-sandbox \
      --model "$repair_model" \
      --cd "$PWD" \
      --output-last-message /tmp/upstream-sync-repair-last-message.txt \
      - < /tmp/upstream-sync-repair-prompt.txt

    if git diff --name-only --diff-filter=U | grep -q .; then
      echo "conflicts remain after repair attempt $attempt" >&2
      git diff --name-only --diff-filter=U >&2
      attempt=$((attempt + 1))
      continue
    fi

    marker_files=$(
      while IFS= read -r path; do
        [[ -f "$path" ]] || continue
        grep -n '<<<<<<<\|=======\|>>>>>>>' -- "$path" || true
      done <<<"$conflicted_files"
    )
    if [[ -n "$marker_files" ]]; then
      echo "conflict markers remain after repair attempt $attempt" >&2
      printf '%s\n' "$marker_files" >&2
      attempt=$((attempt + 1))
      continue
    fi

    git add -A
    GIT_EDITOR=true git rebase --continue
    attempt=$((attempt + 1))
  done
fi

if [[ -n "$verify_command" ]]; then
  bash -lc "$verify_command"
fi

if [[ "$dry_run" == "true" ]]; then
  echo "dry run: repair branch $repair_branch is ready"
  git status --short
  exit 0
fi

git push --force-with-lease origin "HEAD:$repair_branch"

cat > "$body_file" <<BODY
Automated upstream sync repair.

Base branch: \`$branch_name\`
Upstream commit: \`$upstream_sha\`
Repair branch: \`$repair_branch\`

This PR is intentionally review-gated. Merge only after the required checks pass and the fork-specific reasoning trace behavior is confirmed.
BODY

existing_pr=$(
  gh pr list \
    --repo "${GITHUB_REPOSITORY:-}" \
    --head "$repair_branch" \
    --state open \
    --json number \
    --jq '.[0].number // empty'
)

if [[ -n "$existing_pr" ]]; then
  gh pr edit "$existing_pr" \
    --repo "${GITHUB_REPOSITORY:-}" \
    --title "ci: sync fork with upstream" \
    --body-file "$body_file"
  pr_number=$existing_pr
else
  pr_url=$(
    gh pr create \
      --repo "${GITHUB_REPOSITORY:-}" \
      --base "$branch_name" \
      --head "$repair_branch" \
      --title "ci: sync fork with upstream" \
      --body-file "$body_file"
  )
  pr_number=${pr_url##*/}
fi

echo "repair_pr=$pr_number" >> "${GITHUB_OUTPUT:-/dev/null}"

if [[ "$auto_merge" == "true" ]]; then
  gh pr merge "$pr_number" --repo "${GITHUB_REPOSITORY:-}" --auto --merge
fi

#!/usr/bin/env bash
set -euo pipefail

branch_name=${BRANCH_NAME:-main}
upstream_remote_name=${UPSTREAM_REMOTE_NAME:-upstream}
upstream_remote_url=${UPSTREAM_REMOTE_URL:-https://github.com/openai/codex.git}
patch_version=${PATCH_VERSION:-1}
verify_command=${VERIFY_COMMAND:-}
upstream_tag_override=${UPSTREAM_TAG:-}

git config user.name "${GIT_AUTHOR_NAME:-OneNoted Automation}"
git config user.email "${GIT_AUTHOR_EMAIL:-notes@madeingotland.com}"

if ! git remote get-url "$upstream_remote_name" >/dev/null 2>&1; then
  git remote add "$upstream_remote_name" "$upstream_remote_url"
fi

if [[ -n "$upstream_tag_override" ]]; then
  upstream_tag=$upstream_tag_override
else
  upstream_tag=$(
    git ls-remote --refs --tags "$upstream_remote_url" 'refs/tags/rust-v*' |
      awk -F/ '{print $NF}' |
      grep -E '^rust-v[0-9]+\.[0-9]+\.[0-9]+$' |
      sort -V |
      tail -n 1
  )
fi

if [[ -z "$upstream_tag" ]]; then
  echo "failed to determine upstream release tag" >&2
  exit 1
fi

if [[ ! "$upstream_tag" =~ ^rust-v([0-9]+\.[0-9]+\.[0-9]+)$ ]]; then
  echo "unexpected upstream release tag: $upstream_tag" >&2
  exit 1
fi

version=${BASH_REMATCH[1]}
fork_tag="aur-v${version}-reasoning.${patch_version}"

{
  echo "upstream_tag=$upstream_tag"
  echo "version=$version"
  echo "fork_tag=$fork_tag"
} >> "${GITHUB_OUTPUT:-/dev/null}"

if git ls-remote --exit-code --refs --tags origin "refs/tags/$fork_tag" >/dev/null 2>&1; then
  echo "should_release=false" >> "${GITHUB_OUTPUT:-/dev/null}"
  echo "$fork_tag already exists"
  exit 0
fi

echo "should_release=true" >> "${GITHUB_OUTPUT:-/dev/null}"

git fetch origin "$branch_name"
git fetch "$upstream_remote_name" "$branch_name" "refs/tags/$upstream_tag:refs/tags/$upstream_tag"

release_branch="release/$fork_tag"
git checkout -B "$release_branch" "$upstream_tag"

upstream_ref="$upstream_remote_name/$branch_name"
base=$(git merge-base "origin/$branch_name" "$upstream_ref")
mapfile -t downstream_commits < <(git rev-list --reverse "$base..origin/$branch_name")

for commit in "${downstream_commits[@]}"; do
  mapfile -t changed_paths < <(git diff-tree --no-commit-id --name-only -r "$commit")
  if ((${#changed_paths[@]} == 0)); then
    continue
  fi

  only_ci=true
  for path in "${changed_paths[@]}"; do
    if [[ "$path" != .github/* ]]; then
      only_ci=false
      break
    fi
  done

  if [[ "$only_ci" == "true" ]]; then
    echo "skipping CI-only downstream commit $commit"
    continue
  fi

  git cherry-pick "$commit"
done

if [[ -n "$verify_command" ]]; then
  bash -lc "$verify_command"
fi

git tag "$fork_tag"
git push origin "refs/tags/$fork_tag"

#!/usr/bin/env bash
set -euo pipefail

branch_name=${BRANCH_NAME:-main}
upstream_remote_name=${UPSTREAM_REMOTE_NAME:-upstream}
upstream_remote_url=${UPSTREAM_REMOTE_URL:-https://github.com/openai/codex.git}
patch_version=${PATCH_VERSION:-1}
verify_command=${VERIFY_COMMAND:-}
upstream_tag_override=${UPSTREAM_TAG:-}
base_tag_override=${BASE_TAG:-}
downstream_commits_override=${DOWNSTREAM_COMMITS:-}

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

base_ref="$upstream_tag"
if [[ -n "$base_tag_override" ]]; then
  git fetch origin "refs/tags/$base_tag_override:refs/tags/$base_tag_override"
  base_ref="$base_tag_override"
elif [[ "$patch_version" =~ ^[0-9]+$ ]]; then
  previous_fork_tag=$(
    git ls-remote --refs --tags origin "refs/tags/aur-v${version}-reasoning.*" |
      awk -F'reasoning[.]' -v patch_version="$patch_version" '
        NF == 2 && $2 ~ /^[0-9]+$/ && $2 < patch_version { print $2 }
      ' |
      sort -n |
      tail -n 1 |
      awk -v version="$version" '{ print "aur-v" version "-reasoning." $1 }'
  )

  if [[ -n "$previous_fork_tag" ]]; then
    git fetch origin "refs/tags/$previous_fork_tag:refs/tags/$previous_fork_tag"
    base_ref="$previous_fork_tag"
  fi
fi

release_branch="release/$fork_tag"
git checkout -B "$release_branch" "$base_ref"

upstream_ref="$upstream_remote_name/$branch_name"
base=$(git merge-base "origin/$branch_name" "$upstream_ref")
if [[ -n "$downstream_commits_override" ]]; then
  mapfile -t downstream_commits < <(tr '[:space:]' '\n' <<<"$downstream_commits_override" | sed '/^$/d')
else
  mapfile -t downstream_commits < <(git rev-list --reverse "$base..origin/$branch_name")
fi

commit_patch_id() {
  git show --format= --find-renames "$1" |
    git patch-id --stable |
    awk '{ print $1 }'
}

declare -A applied_patch_ids=()
if [[ "$base_ref" != "$upstream_tag" ]]; then
  while IFS= read -r commit; do
    patch_id=$(commit_patch_id "$commit")
    if [[ -n "$patch_id" ]]; then
      applied_patch_ids["$patch_id"]=1
    fi
  done < <(git rev-list "$upstream_tag..$base_ref")
fi

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

  patch_id=$(commit_patch_id "$commit")
  if [[ -n "$patch_id" && -n "${applied_patch_ids[$patch_id]:-}" ]]; then
    echo "skipping already-applied downstream commit $commit"
    continue
  fi

  git cherry-pick --empty=drop "$commit"
done

if [[ -n "$verify_command" ]]; then
  bash -lc "$verify_command"
fi

git tag "$fork_tag"
git push origin "refs/tags/$fork_tag"

#!/bin/sh
# Scan the staged git diff for secrets, using the same forge-core
# scanner the product uses (ADR 0007). Exits non-zero when a finding
# exists, so it can be wired as a pre-commit hook:
#
#   ln -s ../../scripts/scan-staged.sh .git/hooks/pre-commit
#
# Note: findings print line numbers *within the diff text*, not file
# line numbers - run `git diff --cached` to see the context.
set -eu

cd "$(git rev-parse --show-toplevel)"

# Test corpora under crates/*/tests/ contain deliberately fake secrets
# (the scanner's positive test cases); they are excluded, as with any
# scanner allowlist. Everything else - src, docs, scripts, config - is
# scanned.
diff_text="$(git diff --cached --no-color -- . ':(exclude)crates/*/tests/**')"
if [ -z "$diff_text" ]; then
  echo "scan-staged: nothing staged" >&2
  exit 0
fi

# Only added lines are new content worth scanning.
added="$(printf '%s\n' "$diff_text" | grep -E '^\+' | grep -vE '^\+\+\+' || true)"
if [ -z "$added" ]; then
  echo "scan-staged: no added lines" >&2
  exit 0
fi

if printf '%s\n' "$added" | cargo run --quiet -p forge-core --example secret_scan; then
  echo "scan-staged: clean" >&2
  exit 0
else
  echo "scan-staged: possible secrets in staged changes (kinds above; values are never printed)" >&2
  exit 1
fi

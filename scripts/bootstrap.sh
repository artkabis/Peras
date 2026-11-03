#!/bin/sh
# Installs the newest *mature* stable version of every JS dev dependency, then audits them.
# Mature = published longer ago than minimumReleaseAge (pnpm-workspace.yaml), which shields
# the project from freshly compromised releases.
# Run once before the first push; it creates pnpm-lock.yaml, which CI then installs frozen.
# Later updates come from Renovate pull requests.
set -eu

cd "$(dirname "$0")/.."

echo "==> Pinning the package manager"
pnpm pkg set "packageManager=pnpm@$(pnpm --version)"

echo "==> Picking the newest mature stable versions"
# typescript stays on 6.0.x until typescript-eslint supports 7 (its peer range is <6.1.0).
# shellcheck disable=SC2046 # word splitting is intended: one argument per package
set -- $(node scripts/latest-mature.mjs typescript@6.0 vitest vite eslint @eslint/js typescript-eslint prettier @types/node)
echo "    $*"
typescript="$1"
shift
vite=""
for pick in "$@"; do
  case "$pick" in vite@*) vite="$pick" ;; esac
done

echo "==> Installing them (workspace root)"
pnpm add --save-dev --workspace-root "$@"
pnpm add --save-dev --workspace-root "typescript@~${typescript#typescript@}"

echo "==> Declaring vite as a peer of the plugin"
# A peer is a range: projects on any later vite 8.x must still be accepted.
pnpm --filter vite-plugin-peras add --save-peer "vite@^${vite#vite@}"

echo "==> Refusing any bypass of the supply-chain policy"
if grep -q '^minimumReleaseAgeExclude' pnpm-workspace.yaml; then
  echo "pnpm-workspace.yaml gained a minimumReleaseAgeExclude entry: remove it." >&2
  exit 1
fi

echo "==> Auditing: any known vulnerability fails the bootstrap"
pnpm audit --audit-level low

echo "==> Running every check"
pnpm run check

echo "Done. Commit package.json files and pnpm-lock.yaml."

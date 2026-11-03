#!/bin/sh
# Installs the latest version of every JS dev dependency, then audits them.
# Run once before the first push; it creates pnpm-lock.yaml, which CI then installs frozen.
# Later updates come from Renovate pull requests.
set -eu

cd "$(dirname "$0")/.."

echo "==> Pinning the package manager"
pnpm pkg set "packageManager=pnpm@$(pnpm --version)"

echo "==> Installing the latest dev dependencies (workspace root)"
pnpm add --save-dev --workspace-root \
  typescript \
  vitest \
  vite \
  eslint \
  @eslint/js \
  typescript-eslint \
  prettier \
  @types/node

echo "==> Declaring vite as a peer of the plugin (latest major)"
pnpm --filter vite-plugin-peras add --save-peer vite

echo "==> Auditing: any known vulnerability fails the bootstrap"
pnpm audit --audit-level low

echo "==> Running every check"
pnpm run check

echo "Done. Commit package.json files and pnpm-lock.yaml."

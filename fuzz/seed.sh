#!/bin/sh
# Copies every Peras source of the repository into fuzz/seeds, the starting corpus.
set -eu
cd "$(dirname "$0")/.."
mkdir -p fuzz/seeds
find tests crates bench/backends examples -name '*.peras' -not -path '*/node_modules/*' | while read -r file; do
  cp "$file" "fuzz/seeds/$(echo "$file" | tr '/.' '__')"
done
echo "$(ls fuzz/seeds | wc -l) seeds"

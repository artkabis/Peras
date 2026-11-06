#!/bin/sh
# Copies every Peras source of the repository into fuzz/seeds, the starting corpus,
# and creates the corpus directory of each target.
set -eu
cd "$(dirname "$0")/.."
mkdir -p fuzz/seeds
# libFuzzer refuses a corpus directory that does not exist: on the first night,
# or after the cache expires, there is none yet.
for target in lex parse pipeline; do
  mkdir -p "fuzz/corpus/$target"
done
find tests crates bench/backends examples -name '*.peras' -not -path '*/node_modules/*' | while read -r file; do
  cp "$file" "fuzz/seeds/$(echo "$file" | tr '/.' '__')"
done
echo "$(ls fuzz/seeds | wc -l) seeds"

#!/usr/bin/env bash
# Fills VERSION and SHA256_* in dartr.rb (or in the formula given as $2) from
# the checksum files of the GitHub release v<version>.
# Usage: packaging/homebrew/update_formula.sh 0.1.0 [formula.rb]
# Needs: gh (authenticated), and the release must exist.
set -euo pipefail

version="${1:?usage: update_formula.sh <version> [formula.rb]}"
version="${version#v}"
here="$(cd "$(dirname "$0")" && pwd)"
formula="${2:-$here/dartr.rb}"
repo="orestesgaolin/dartr"
targets=(aarch64-apple-darwin x86_64-unknown-linux-gnu)

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

for target in "${targets[@]}"; do
  file="dartr-${version}-${target}.tar.gz.sha256"
  gh release download "v${version}" --repo "$repo" --pattern "$file" --dir "$tmp"
  sum="$(awk '{print $1}' "$tmp/$file")"
  if [[ ! "$sum" =~ ^[0-9a-f]{64}$ ]]; then
    echo "bad checksum in $file: '$sum'" >&2
    exit 1
  fi
  key="SHA256_$(echo "$target" | tr 'a-z-' 'A-Z_')"
  # Placeholder on the first run; on later runs replace the old value.
  if grep -q "$key" "$formula"; then
    sed -i.bak "s/$key/$sum/" "$formula"
  else
    echo "placeholder $key not found in $formula (already filled?)" >&2
    exit 1
  fi
done
sed -i.bak "s/^  version \"VERSION\"/  version \"$version\"/" "$formula"
rm -f "$formula.bak"
grep -nE 'version "|sha256 "' "$formula"

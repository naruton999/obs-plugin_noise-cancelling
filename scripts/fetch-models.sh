#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEST="${1:-$ROOT/models}"
BASE="https://raw.githubusercontent.com/Rikorose/DeepFilterNet/7b14b895fe7fc42cab72b4a4c1e49c982bd6404b/models"

mkdir -p "$DEST"

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

fetch_model() {
  local name="$1"
  local expected="$2"
  local target="$DEST/$name"

  if [[ -f "$target" ]]; then
    local actual
    actual="$(sha256_of "$target")"
    if [[ "$actual" == "$expected" ]]; then
      echo "ok: $name ($actual)"
      return 0
    fi
    echo "hash mismatch, re-downloading: $name" >&2
  fi

  local tmp="$target.part"
  rm -f "$tmp"
  echo "downloading: $name"
  curl -L --fail --retry 3 --output "$tmp" "$BASE/$name"

  local actual
  actual="$(sha256_of "$tmp")"
  if [[ "$actual" != "$expected" ]]; then
    rm -f "$tmp"
    echo "sha256 mismatch for $name: expected $expected, got $actual" >&2
    return 1
  fi

  mv "$tmp" "$target"
  echo "ok: $name ($actual)"
}

fetch_model "DeepFilterNet3_ll_onnx.tar.gz" "5998e58e8ba0e09bb76986ef97b84afa065a571ef282d4a1222f341e3251cf3a"
fetch_model "DeepFilterNet3_onnx.tar.gz" "c94d91f70911001c946e0fabb4aa9adc37045f45a03b56008cb0c8244cb63616"

echo "models ready: $DEST"

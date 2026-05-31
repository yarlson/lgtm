#!/usr/bin/env bash
set -euo pipefail

if [[ "$#" -ne 4 ]]; then
  echo "usage: $0 <formula-path> <version> <github-repository> <checksum-dir>" >&2
  exit 2
fi

formula_path="$1"
version="$2"
github_repository="$3"
checksum_dir="$4"
bin_name="lgtm"

artifact_sha() {
  local artifact="$1"
  local checksum_file="${checksum_dir}/${artifact}.sha256"

  if [[ ! -f "$checksum_file" ]]; then
    echo "missing checksum file: ${checksum_file}" >&2
    exit 1
  fi

  awk '{ print $1 }' "$checksum_file"
}

artifact_url() {
  local artifact="$1"
  printf 'https://github.com/%s/releases/download/v%s/%s' "$github_repository" "$version" "$artifact"
}

darwin_amd64="${bin_name}-v${version}-darwin-amd64.tar.gz"
darwin_arm64="${bin_name}-v${version}-darwin-arm64.tar.gz"
linux_amd64="${bin_name}-v${version}-linux-amd64.tar.gz"
linux_arm64="${bin_name}-v${version}-linux-arm64.tar.gz"

darwin_amd64_sha="$(artifact_sha "$darwin_amd64")"
darwin_arm64_sha="$(artifact_sha "$darwin_arm64")"
linux_amd64_sha="$(artifact_sha "$linux_amd64")"
linux_arm64_sha="$(artifact_sha "$linux_arm64")"

mkdir -p "$(dirname "$formula_path")"

# Older lgtm releases published a cask with the same token. Keeping both a
# formula and cask makes `brew upgrade lgtm` ambiguous and can leave the binary
# unlinked after upgrade. The formula owns the CLI now.
stale_cask_path="$(dirname "$formula_path")/../Casks/${bin_name}.rb"
rm -f "$stale_cask_path"

cat >"$formula_path" <<RUBY
class Lgtm < Formula
  desc "Plan and run Codex-backed local phase work"
  homepage "https://github.com/${github_repository}"
  version "${version}"
  link_overwrite "bin/${bin_name}"

  on_macos do
    if Hardware::CPU.arm?
      url "$(artifact_url "$darwin_arm64")"
      sha256 "${darwin_arm64_sha}"
    else
      url "$(artifact_url "$darwin_amd64")"
      sha256 "${darwin_amd64_sha}"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "$(artifact_url "$linux_arm64")"
      sha256 "${linux_arm64_sha}"
    else
      url "$(artifact_url "$linux_amd64")"
      sha256 "${linux_amd64_sha}"
    end
  end

  def install
    bin.install "${bin_name}"
  end

  test do
    system "#{bin}/${bin_name}", "--version"
  end
end
RUBY

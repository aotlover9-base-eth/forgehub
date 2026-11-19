# Homebrew formula TEMPLATE (Step 21) — not published.
# To publish: copy to a tap repository as Formula/forgehub.rb,
# fill the sha256 values from the release's SHA256SUMS file,
# and open the usual homebrew-core/tap PR. The version and
# URLs track the GitHub release asset names (ADR 0008).
class Forgehub < Formula
  desc "Cloud home for shell command snippets (CLI: forgehub / fg)"
  homepage "https://github.com/aotlover9-base-eth/forgehub"
  version "0.1.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/aotlover9-base-eth/forgehub/releases/download/v0.1.0/forgehub-v0.1.0-aarch64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_SHA256_FROM_RELEASE_SHA256SUMS"
    end
    on_intel do
      url "https://github.com/aotlover9-base-eth/forgehub/releases/download/v0.1.0/forgehub-v0.1.0-x86_64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_SHA256_FROM_RELEASE_SHA256SUMS"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/aotlover9-base-eth/forgehub/releases/download/v0.1.0/forgehub-v0.1.0-aarch64-unknown-linux-musl.tar.gz"
      sha256 "REPLACE_WITH_SHA256_FROM_RELEASE_SHA256SUMS"
    end
    on_intel do
      url "https://github.com/aotlover9-base-eth/forgehub/releases/download/v0.1.0/forgehub-v0.1.0-x86_64-unknown-linux-musl.tar.gz"
      sha256 "REPLACE_WITH_SHA256_FROM_RELEASE_SHA256SUMS"
    end
  end

  def install
    bin.install "forgehub"
    man1.install Dir["man/*.1"]
  end

  test do
    assert_match "forgehub #{version}", shell_output("#{bin}/forgehub --version")
  end
end

# Homebrew Formula for nimbus
# To use: brew tap ryoupr/tap && brew install nimbus

class Nimbus < Formula
  desc "High-performance EC2 SSM connection manager"
  homepage "https://github.com/ryoupr/nimbus"
  version "3.0.0"
  license "MIT"

  on_macos do
    on_intel do
      url "https://github.com/ryoupr/nimbus/releases/download/v#{version}/nimbus-darwin-x86_64.tar.gz"
      sha256 "REPLACE_WITH_ACTUAL_SHA256"
    end
    on_arm do
      url "https://github.com/ryoupr/nimbus/releases/download/v#{version}/nimbus-darwin-arm64.tar.gz"
      sha256 "REPLACE_WITH_ACTUAL_SHA256"
    end
  end

  def install
    bin.install "nimbus"
  end

  test do
    system "#{bin}/nimbus", "--version"
  end
end

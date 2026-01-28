# Homebrew Formula for ec2-connect
# To use: brew tap your-org/tap && brew install ec2-connect

class Ec2Connect < Formula
  desc "High-performance EC2 SSM connection manager"
  homepage "https://github.com/your-org/ec2-connect"
  version "3.0.0"
  license "MIT"

  on_macos do
    on_intel do
      url "https://github.com/your-org/ec2-connect/releases/download/v#{version}/ec2-connect-darwin-x86_64.tar.gz"
      sha256 "REPLACE_WITH_ACTUAL_SHA256"
    end
    on_arm do
      url "https://github.com/your-org/ec2-connect/releases/download/v#{version}/ec2-connect-darwin-arm64.tar.gz"
      sha256 "REPLACE_WITH_ACTUAL_SHA256"
    end
  end

  def install
    bin.install "ec2-connect"
  end

  test do
    system "#{bin}/ec2-connect", "--version"
  end
end

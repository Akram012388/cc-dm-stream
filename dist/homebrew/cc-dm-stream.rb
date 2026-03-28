class CcDmStream < Formula
  desc "Live streaming TUI for the cc-dm coordination bus"
  homepage "https://github.com/Akram012388/cc-dm-stream"
  license "MIT"
  version "0.1.9"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/Akram012388/cc-dm-stream/releases/download/v#{version}/cc-dm-stream-aarch64-apple-darwin"
      sha256 "PLACEHOLDER_ARM64_SHA256"
    else
      url "https://github.com/Akram012388/cc-dm-stream/releases/download/v#{version}/cc-dm-stream-x86_64-apple-darwin"
      sha256 "PLACEHOLDER_X86_64_SHA256"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/Akram012388/cc-dm-stream/releases/download/v#{version}/cc-dm-stream-aarch64-unknown-linux-gnu"
      sha256 "PLACEHOLDER_LINUX_ARM64_SHA256"
    else
      url "https://github.com/Akram012388/cc-dm-stream/releases/download/v#{version}/cc-dm-stream-x86_64-unknown-linux-gnu"
      sha256 "PLACEHOLDER_LINUX_X86_64_SHA256"
    end
  end

  def install
    binary_name = Dir.glob("cc-dm-stream-*").first || "cc-dm-stream"
    bin.install binary_name => "cc-dm-stream"
  end

  test do
    system "#{bin}/cc-dm-stream", "--version"
  end
end

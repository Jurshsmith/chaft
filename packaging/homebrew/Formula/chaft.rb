# Review template: publish only a separately rendered candidate with no
# UNRESOLVED release coordinates.
class Chaft < Formula
  desc "Native local-first peer-to-peer chat workspace"
  homepage "https://github.com/Jurshsmith/chaft"
  url "https://github.com/Jurshsmith/chaft.git",
      tag:      "UNRESOLVED_CHAFT_RELEASE_TAG",
      revision: "UNRESOLVED_CHAFT_RELEASE_COMMIT"
  version "UNRESOLVED_CHAFT_RELEASE_VERSION"
  license "AGPL-3.0-or-later"

  depends_on :macos
  depends_on "cmake" => :build
  depends_on "git" => :build
  depends_on "ninja" => :build
  depends_on "python@3.14" => :build
  depends_on "qtbase" => :build
  depends_on "qtdeclarative" => :build
  depends_on "rust" => :build

  def install
    # Homebrew's build environment intentionally omits brew from PATH. Bind the
    # shared workflow to this exact Homebrew installation for its read-only
    # dependency and prefix checks.
    ENV["CHAFT_HOMEBREW_EXECUTABLE"] = ENV.fetch("HOMEBREW_BREW_FILE")

    system "tools/macos/build-local.sh",
           "--yes",
           "--no-install-deps",
           "--install-dir", (libexec/"Applications").to_s,
           "--expected-commit", "UNRESOLVED_CHAFT_RELEASE_COMMIT",
           "--skip-launch"

    launcher = bin/"chaft"
    launcher.write <<~SH
      #!/bin/sh
      exec /usr/bin/open -n "#{opt_libexec}/Applications/Chaft.app"
    SH
    launcher.chmod 0755
  end

  def caveats
    <<~EOS
      This formula builds Chaft from source on this Mac.

      Chaft.app has a local ad-hoc signature. It is not Developer ID signed or
      Apple notarized, and it should not be redistributed as a trusted binary.

      Run `chaft` to open the app.
    EOS
  end

  test do
    app = opt_libexec/"Applications/Chaft.app"
    binary = app/"Contents/MacOS/Chaft"
    plist = app/"Contents/Info.plist"

    assert_predicate binary, :executable?
    assert_path_exists app/"Contents/Resources/Chaft.icns"
    assert_equal "Chaft",
                 shell_output("/usr/bin/plutil -extract CFBundleName raw -o - #{plist}").strip
    system "/usr/bin/codesign", "--verify", "--deep", "--strict", app
    assert_match "Signature=adhoc",
                 shell_output("/usr/bin/codesign --display --verbose=4 #{app} 2>&1")
  end
end

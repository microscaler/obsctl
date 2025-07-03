class Obsctl < Formula
  desc "High-performance S3-compatible CLI tool with OpenTelemetry observability"
  homepage "https://github.com/your-org/obsctl"
  license "MIT"
  head "https://github.com/your-org/obsctl.git", branch: "main"

  # Version and source configuration
  version "0.2.0" # x-release-please-version

  # Universal Binary for macOS (supports both Intel and Apple Silicon)
  if OS.mac?
    url "https://github.com/your-org/obsctl/releases/download/v#{version}/obsctl-#{version}-macos-universal.tar.gz"
    sha256 "PLACEHOLDER_UNIVERSAL_SHA256"
  end

  # Linux binaries
  if OS.linux?
    if Hardware::CPU.intel?
      url "https://github.com/your-org/obsctl/releases/download/v#{version}/obsctl-#{version}-linux-x64.tar.gz"
      sha256 "PLACEHOLDER_LINUX_X64_SHA256"
    elsif Hardware::CPU.arm?
      if Hardware::CPU.is_64_bit?
        url "https://github.com/your-org/obsctl/releases/download/v#{version}/obsctl-#{version}-linux-arm64.tar.gz"
        sha256 "PLACEHOLDER_LINUX_ARM64_SHA256"
      else
        url "https://github.com/your-org/obsctl/releases/download/v#{version}/obsctl-#{version}-linux-armv7.tar.gz"
        sha256 "PLACEHOLDER_LINUX_ARMV7_SHA256"
      end
    end
  end

  # Fallback to source compilation if pre-built binaries aren't available
  # or if installing from HEAD
  if build.head?
    depends_on "rust" => :build
  end

  def install
    if build.head?
      # Build from source when using HEAD
      system "cargo", "build", "--release"
      bin.install "target/release/obsctl"

      # Install additional files from source
      man1.install "packaging/obsctl.1"
      bash_completion.install "packaging/obsctl.bash-completion" => "obsctl"
      (share/"obsctl/dashboards").install Dir["packaging/dashboards/*.json"]
      (etc/"obsctl").install "packaging/debian/config"
    else
      # Install from pre-built binary
      bin.install "obsctl"

      # Install man page if present
      man1.install "obsctl.1" if File.exist?("obsctl.1")

      # Install bash completion if present
      if File.exist?("obsctl.bash-completion")
        bash_completion.install "obsctl.bash-completion" => "obsctl"
      end

      # Install dashboard files if present
      if Dir.exist?("dashboards")
        (share/"obsctl/dashboards").install Dir["dashboards/*.json"]
      end

      # Install configuration template if present
      if File.exist?("config")
        (etc/"obsctl").install "config"
      end
    end

    # Create symlink for easier access to dashboards
    (share/"obsctl").install_symlink share/"obsctl/dashboards" => "grafana-dashboards" if (share/"obsctl/dashboards").exist?
  end

  def post_install
    # Create AWS config directory if it doesn't exist
    aws_dir = "#{Dir.home}/.aws"
    Dir.mkdir(aws_dir) unless Dir.exist?(aws_dir)

    # Display installation success message
    puts <<~EOS
      🎉 obsctl installed successfully!

      📊 Dashboard Management:
        obsctl config dashboard install  - Install dashboards to Grafana
        obsctl config dashboard list     - List installed dashboards
        obsctl config dashboard info     - Show dashboard information

      📂 Dashboard files installed to: #{share}/obsctl/dashboards/
      📋 Configuration template: #{etc}/obsctl/config
      📖 Man page: man obsctl

      🚀 Quick Start:
        obsctl config configure         - Interactive setup
        obsctl ls s3://bucket          - List bucket contents
        obsctl config dashboard install - Install Grafana dashboards

      For more information: obsctl --help
    EOS
  end

  test do
    # Test that the binary was installed correctly
    assert_match version.to_s, shell_output("#{bin}/obsctl --version")

    # Test that help works
    assert_match "S3-compatible CLI tool", shell_output("#{bin}/obsctl --help")

    # Test config command
    assert_match "Configuration Commands", shell_output("#{bin}/obsctl config --help")

    # Test dashboard command
    assert_match "Dashboard Management", shell_output("#{bin}/obsctl config dashboard --help")

    # Test that dashboard files are installed (if they exist)
    if (share/"obsctl/dashboards").exist?
      assert_predicate share/"obsctl/dashboards/obsctl-unified.json", :exist?
    end

    # Test that man page is installed
    assert_predicate man1/"obsctl.1", :exist?

    # Test that bash completion is installed
    assert_predicate bash_completion/"obsctl", :exist?
  end
end

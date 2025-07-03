Name:           obsctl
Version:        0.3.0 # x-release-please-version
Release:        1%{?dist}
Summary:        S3-compatible CLI tool with OpenTelemetry observability

License:        MIT
URL:            https://github.com/your-org/obsctl
Source0:        %{name}-%{version}.tar.gz

BuildRequires:  rust cargo
Requires:       glibc

%description
obsctl is a high-performance S3-compatible CLI tool with built-in OpenTelemetry
observability and Grafana dashboard support. It provides comprehensive metrics,
tracing, and monitoring capabilities for S3 operations.

%prep
%setup -q

%build
cargo build --release

%install
rm -rf $RPM_BUILD_ROOT

# Create directories
mkdir -p $RPM_BUILD_ROOT/usr/bin
mkdir -p $RPM_BUILD_ROOT/usr/share/man/man1
mkdir -p $RPM_BUILD_ROOT/usr/share/bash-completion/completions
mkdir -p $RPM_BUILD_ROOT/usr/share/obsctl/dashboards
mkdir -p $RPM_BUILD_ROOT/etc/obsctl

# Install files
install -m 755 target/release/obsctl $RPM_BUILD_ROOT/usr/bin/obsctl
install -m 644 packaging/obsctl.1 $RPM_BUILD_ROOT/usr/share/man/man1/obsctl.1
install -m 644 packaging/obsctl.bash-completion $RPM_BUILD_ROOT/usr/share/bash-completion/completions/obsctl
install -m 644 packaging/dashboards/*.json $RPM_BUILD_ROOT/usr/share/obsctl/dashboards/
install -m 644 packaging/debian/config $RPM_BUILD_ROOT/etc/obsctl/config

%files
/usr/bin/obsctl
/usr/share/man/man1/obsctl.1
/usr/share/bash-completion/completions/obsctl
/usr/share/obsctl/dashboards/*.json
%config(noreplace) /etc/obsctl/config

%post
echo "obsctl installed."
echo ""
echo "obsctl Dashboard Management:"
echo "  obsctl config dashboard install  - Install dashboards to Grafana"
echo "  obsctl config dashboard list     - List installed dashboards"
echo "  obsctl config dashboard info     - Show dashboard information"
echo ""
echo "Dashboard files installed to: /usr/share/obsctl/dashboards/"

%changelog
* Thu Dec 19 2024 obsctl Team <team@obsctl.com> - 0.1.0-1
- Initial RPM package with dashboard support
- Added Grafana dashboard management commands
- Included comprehensive S3 CLI functionality
- Built-in OpenTelemetry observability support

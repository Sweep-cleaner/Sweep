# COPR / Fedora RPM spec: `sweep` CLI.
# Test: copr-cli build @sweep/sweep sweep.spec
Name:           sweep
Version:        0.1.0
Release:        1%{?dist}
Summary:        Fast, safe, cross-platform system cleaner
License:        GPL-3.0-or-later
URL:            https://github.com/sweep-cleaner/sweep
Source0:        %{url}/archive/v%{version}.tar.gz
BuildRequires:  cargo
BuildRequires:  python3

%description
Sweep is a BleachBit-inspired system and privacy cleaner written in Rust:
deep package-cache, kernel, journal, Snap/Flatpak/Docker cleaning with
dry-run previews, keep-list guards and size reports.

%prep
%autosetup -n %{name}-%{version}

%build
cargo build --release --locked

%install
install -Dm755 target/release/sweep %{buildroot}%{_bindir}/sweep
install -Dm644 README.md %{buildroot}%{_docdir}/%{name}/README.md
install -Dm644 packaging/systemd/sweep-clean.service %{buildroot}%{_unitdir}/sweep-clean.service
install -Dm644 packaging/systemd/sweep-clean.timer %{buildroot}%{_unitdir}/sweep-clean.timer

%check
python3 verify_tomls.py

%files
%{_bindir}/sweep
%{_unitdir}/sweep-clean.service
%{_unitdir}/sweep-clean.timer
%doc README.md

%changelog
* Thu Sep 04 2026 Sweep contributors <nebulawastaken.dev@proton.me> - 0.1.0-1
- Initial package

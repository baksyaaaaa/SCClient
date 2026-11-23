%global debug_package %{nil}
%global __strip /bin/true
%global _appname SCClient
%global _rpmfile %{_appname}_%{version}_linux_x86_64.rpm

Name:           scclient-bin
Version:        2.0.1
Release:        1%{?dist}
Summary:        Next-generation open source Minecraft launcher (prebuilt)

License:        GPL-3.0-only
URL:            https://github.com/baksyaaaaa/SCClient
Source0:        %{url}/releases/download/scclient-%{version}/%{_rpmfile}

ExclusiveArch:  x86_64
BuildRequires:  bsdtar
Requires:       libxdo
Requires:       dbus-libs
Provides:       scclient = %{version}-%{release}

%description
SCClient is a next-generation open source Minecraft launcher.
This package ships the prebuilt binaries from the upstream release.

%prep
%setup -q -c -T
# Repackages the upstream release rpm; bsdtar reads its cpio payload directly.
bsdtar -xf %{SOURCE0}

%install
mkdir -p %{buildroot}
cp -a usr %{buildroot}/
ln -s scclient_app %{buildroot}%{_bindir}/scclient

%files
%{_bindir}/scclient_app
%{_bindir}/scclient
%{_datadir}/applications/*.desktop
%{_datadir}/icons/hicolor/*/apps/*

%changelog
* Wed Sep 09 2026 SC <contact@atmofrost.org> - 2.0.1-1
- See upstream release notes.

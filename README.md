<div align="center">

<img src=".github/media/RepoBanner.png" alt="SC Client" />

# SC Client
The code for the SC Client launcher and its core backend.

SC Client is a Minecraft client featuring fully 100% open-source components, offering many packaged and pre-configured mods in one click.

</div>

## Installing

Download the latest Windows build: [SC-Client-Setup.exe](https://github.com/baksyaaaaa/SCClient/releases/latest/download/SC-Client-Setup.exe)
as well as our [GitHub releases](https://github.com/baksyaaaaa/SCClient/releases/latest).

| Windows (x86_64) | macOS (Intel & Apple Silicon) | Linux (x86_64)                                          |
|------------------|-------------------------------|---------------------------------------------------------|
| Installer 🔄      | DMG 🔄                        | AppImage 🔄                                             |
|                  | App Bundle 🔄                 | DEB                                                     |
|                  |                               | RPM                                                     |
|                  |                               | *[RPM (COPR)](https://copr.fedorainfracloud.org/coprs/sc/SCClient/) 🔄 |
|                  |                               | [AUR](https://aur.archlinux.org/packages/scclient-bin)|
|                  |                               | Flatpak|

> 🔄 = Has support for autoupdating built-in

<details>

<summary><h3>Flatpak Installation</h3></summary>

On most big distros, you can just double-click the .flatpak file to install it. You can also:
1. First, navigate to the folder the .flatpak was downloaded in: (e.g. Downloads)
```
cd Downloads
```
2. You can then install the package: (change filename with the one you downloaded)
```
flatpak install ./SCClient_2.11.0_linux_x86_64.flatpak 
```
</details>
<details>

<summary><h3>COPR Installation</h3></summary>

On supported distributions (RHEL, Fedora, etc.), you can make use of the Fedora COPR instead of using the AppImage or manually installing the RPM file.
1. First, you have to enable the repository:
```
sudo dnf copr enable sc/SCClient
```
2. You can then install the package:
```
sudo dnf install scclient-bin
```

</details>

## Contributing

We welcome contributions! Please read our [contributing guidelines](CONTRIBUTING.md) before getting started.


### Requirements

The project targets **Rust 1.97** or later. You can install Rust via [rustup](https://rustup.rs/).


### Building & Running

```sh
# Run the app
cargo run -p scclient_app

# Build a release binary
cargo build -p scclient_app --release
```


### Packaging / Releasing

Installers are produced with [**cargo-packager**](https://github.com/crabnebula-dev/cargo-packager)
(the standalone bundler spun out of the Tauri bundler). Config lives in
[`packages/scclient_app/Cargo.toml`](./packages/scclient_app/Cargo.toml) under
`[package.metadata.packager]`.

```sh
cargo install cargo-packager --locked

# Build the binary, then bundle it for the current OS:
cargo build --release -p scclient_app
cargo packager --release -p scclient_app --formats <targets>
#   Windows: nsis      macOS: app,dmg      Linux: deb,appimage
```


### Versioning

The workspace shares a single version, defined in the root [`Cargo.toml`](./Cargo.toml) under `[workspace.package]`.

Versions and release notes come from [Knope](https://knope.tech) change files in [`.changeset/`](./.changeset).
Add one per user-facing change with `knope document-change` (installed via `cargo install knope`). The
`SCClient Release Build` workflow consumes them, bumps the version, writes [`CHANGELOG.md`](./CHANGELOG.md),
and uses the new entry as the GitHub release body, which is what the launcher's changelog page shows.


## Code signing


<sub>Based on OneClient by Polyfrost, licensed under GPL-3.0.</sub>

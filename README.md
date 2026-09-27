# EchoFiles

A native Rust file manager for Linux with fast folder browsing, indexed filename search,
list and grid views, tabs, dual panes, previews, file operations and an `ef` search CLI.
Works on Omarchy and other Arch Linux desktops with Wayland or X11.

## Install on Arch Linux / Omarchy

**Distribution status:** the release workflow and AUR recipe are prepared. The package must
be published to the AUR before the `yay` and Omarchy commands below work.

Once the `echofiles` package is published to the AUR:

```bash
yay -S echofiles
```

On Omarchy, the equivalent command is:

```bash
omarchy pkg aur add echofiles
```

This is an **AUR source package**: the first installation compiles EchoFiles. Rust build
requirements are handled by the package manager. Subsequent updates arrive through `yay -Syu`.
Omarchy's AUR helper uses yay internally. EchoFiles is not in Arch's official repositories,
so `pacman -S echofiles` and `omarchy pkg add echofiles` are not the installation route.

### Install a prebuilt GitHub release

When a release is published, download its `echofiles-…-x86_64.pkg.tar.zst` and `SHA256SUMS`
from [Releases](https://github.com/adityavardhansharma/EchoFiles_Linux/releases).
In the download directory, verify the file and install the exact downloaded package:

```bash
sha256sum --check --ignore-missing SHA256SUMS
sudo pacman -U ./echofiles-0.1.0-1-x86_64.pkg.tar.zst
```

Replace the filename with the version you downloaded. This installs a tracked system
package, the `echofiles` and `ef` commands, a desktop launcher and an icon. Public builds
support baseline x86-64; they do not require the developer's AVX2 CPU. Install the Vulkan
driver appropriate for your GPU. Drive mounting uses optional `udisks2` and `polkit`.
The Arch package is for Arch-compatible systems, not Debian or Ubuntu.

### Build from this repository now

```bash
git clone https://github.com/adityavardhansharma/EchoFiles_Linux.git
cd EchoFiles_Linux
RUSTFLAGS='-C target-cpu=x86-64' bash scripts/install.sh
```

Requires Rust 1.90 or newer and the native dependencies listed in
[the package recipe](packaging/arch/PKGBUILD.in). This existing script builds and installs
for the current user under `~/.local`, without root. It is separate from pacman packaging.

### Move from the old user installation to a package

Quit EchoFiles, including background mode, then run this from the repository before
installing the package, so `~/.local/bin/echofiles` does not shadow `/usr/bin/echofiles`:

```bash
bash scripts/install.sh --uninstall
```

Settings and the search index are kept. If enabled, disable **Start at login** in the app
before removing the old installation; re-enable it after starting the packaged app.
AI agent integration can be enabled again in Settings after installation.

### Use and remove

Launch **EchoFiles** from your desktop menu or run `echofiles`. Run `ef --help` for search
commands. To remove the Arch package:

```bash
sudo pacman -Rns echofiles
# Or on Omarchy:
omarchy pkg drop echofiles
```

Personal settings and indexes remain in your home directory.

## Create a versioned release from GitHub Actions

1. Set `[workspace.package].version` in `Cargo.toml`, then run `cargo check` to update the
   workspace versions in `Cargo.lock`. Commit both files and the intended release changes.
2. Push the release workflow to the default branch so GitHub shows **Run workflow**.
3. Open **Actions → Release EchoFiles → Run workflow**. Select the release branch, enter
   the matching version (for example `0.1.0`), and choose whether to create a draft.
4. The workflow builds on Arch Linux and attaches the binary `.pkg.tar.zst`, source archive,
   checksums, `PKGBUILD` and `.SRCINFO`. Review the draft, then publish it.
5. Submit the generated `PKGBUILD` and `.SRCINFO` to the AUR for each published version.
   Existing version tags should not be reused; use a new version for a new release.

The workflow uses GitHub's built-in token; no personal access token is required. AUR
publication is a separate step requiring an AUR account and SSH access. A GitHub release
alone does **not** make a package available through yay.

See [AUR maintainer instructions](packaging/arch/README.md) for the first publication.

## Performance

[Open the visual performance report](design/docs/performance-report.html) or read
[the benchmark results](bench/RESULTS.md). Measurements include Nautilus, fd and find;
results specify hardware, cache state and whether an index was used.

## License

Workspace crates declare GPL-3.0-or-later. Vendored dependencies retain their own licenses.

# EchoFiles

A native Rust file manager for Linux with fast folder browsing, indexed filename search,
list and grid views, tabs, dual panes, previews, file operations and an `ef` search CLI.
Works on Omarchy and other Arch Linux desktops with Wayland or X11.

## Install from GitHub on Arch Linux / Omarchy

Clone the repository, install the build tools, then run the installer. This builds both the
graphical file manager (`echofiles`) and the command-line search tool (`ef`), and adds a
launcher entry for your desktop. It installs just for your user; no `sudo` is needed.

```bash
git clone https://github.com/adityavardhansharma/EchoFiles_Linux.git
cd EchoFiles_Linux
omarchy pkg add rust pkgconf fontconfig libxkbcommon wayland libx11 libxcursor libxi libxrandr vulkan-icd-loader
rustup toolchain install 1.90.0 # only if Rust 1.90+ is not already installed
bash scripts/install.sh
```

If Rust is managed by rustup and is older than 1.90, update it with `rustup update stable`.
On plain Arch, install the system packages with `sudo pacman -S rust pkgconf fontconfig libxkbcommon wayland libx11 libxcursor libxi libxrandr vulkan-icd-loader`; the `rust` package provides Cargo and rustc. Install the Vulkan driver for your GPU. Optional drive mounting uses `udisks2` and `polkit`.

On a non-Arch Linux distribution, install Rust 1.90+, pkg-config, Fontconfig, Vulkan, and
the Wayland/X11 development libraries using its package manager, then run the same clone and
installer commands. This source installer is intended for Linux desktop users.

The launcher appears as **EchoFiles**. The `echofiles` and `ef` commands are installed into
`~/.local/bin`. If that directory is missing from `PATH`, add it to your shell's PATH.
Updates are simple: `cd EchoFiles_Linux`, run `git pull`, then run `bash scripts/install.sh`
again. The installer rebuilds and replaces the user binaries.

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

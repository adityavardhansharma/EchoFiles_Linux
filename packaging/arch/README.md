# Publishing EchoFiles to the AUR

The repository contains a template. Release Actions generate a complete `PKGBUILD` and
`.SRCINFO` with the exact version and SHA-256 of the accompanying source archive.

1. Create an account at https://aur.archlinux.org and add your public SSH key.
2. Publish the GitHub release first: the source URL must be publicly downloadable.
3. Clone the AUR repository using your registered key:

   ```bash
   git clone ssh://aur@aur.archlinux.org/echofiles.git
   cd echofiles
   ```

4. Copy the release's `PKGBUILD` and `.SRCINFO` into this directory. Review them, then
   build in a clean Arch environment using `makepkg -s` (or Arch devtools).
5. Regenerate metadata and publish:

   ```bash
   makepkg --printsrcinfo > .SRCINFO
   git add PKGBUILD .SRCINFO
   git commit -m 'Release 0.1.0'
   git push
   ```

Repeat steps 4–5 for updates. Do not commit binary packages to the AUR. `yay -S echofiles`
and `omarchy pkg aur add echofiles` become available after successful AUR publication.
If the name is claimed before submission, resolve ownership/name choice first.

For local packaging of a committed revision:

```bash
python3 scripts/prepare-release.py 0.1.0
cd dist
makepkg -s
```

`makepkg` uses the already-generated local source archive, verifies its checksum, builds
with generic x86-64 flags and installs files only into its staging directory. It does not
modify user settings or enable agent integrations as root.

References: [AUR submission guidelines](https://wiki.archlinux.org/title/AUR_submission_guidelines),
[PKGBUILD specification](https://man.archlinux.org/man/PKGBUILD.5).

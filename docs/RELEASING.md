# Releasing DClutter for Windows

1. **Bump the version** in all three places, which must match:
   `package.json`, `src-tauri/tauri.conf.json` and the workspace `Cargo.toml`.
2. **Write the notes** in `docs/releases/vX.Y.Z.md`, and point the README's
   download links at the new version.
3. **Merge to main** through a pull request, with CI green.
4. **Build the installers** from an up-to-date `main`:

   ```bash
   npm ci
   ```

   ```bash
   npm run tauri build
   ```

   They land in `target/release/bundle/`:
   - `nsis/DClutter_X.Y.Z_x64-setup.exe`, the one most people download.
   - `msi/DClutter_X.Y.Z_x64_en-US.msi`, for managed installs.

5. **Check the build**: install the `.exe`, open it, and sort a few files.
   Release builds load the interface from inside the app, so this catches
   anything that only worked against the dev server.
6. **Publish** a GitHub release tagged `vX.Y.Z` on `main`, with the notes as
   its description and both installers attached:

   ```bash
   gh release create vX.Y.Z --target main --title "DClutter for Windows X.Y.Z" --notes-file docs/releases/vX.Y.Z.md target/release/bundle/nsis/DClutter_X.Y.Z_x64-setup.exe target/release/bundle/msi/DClutter_X.Y.Z_x64_en-US.msi
   ```

The installers aren't code-signed, so SmartScreen warns on first run. The
README explains this to users.

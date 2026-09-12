## Blocksmith v2.0.1

**Portable standalone release — no installer required.**

---

### Distribution

- Single `blocksmith.exe` — no installer, no admin rights, no companion files
- Icons and SkinMaster.exe are embedded directly in the executable
- SkinMaster is extracted to `%TEMP%\Blocksmith-SkinMaster` on demand when launched
- Settings persist at `%APPDATA%\blocksmith\settings.json`

---

### Changes in 2.0.1

Security and correctness hardening from a full repository audit:

- Safer archive path validation (rejects traversal, drive letters, and backslash paths)
- Zip extraction enforces per-entry and total uncompressed size limits
- Mash-up detection no longer retypes Behavior/Resource packs incorrectly
- Old packs on update go to Recycle Bin instead of permanent delete
- Recycle restore and 4D/premium import paths are allowlisted; symlinks are skipped
- Settings edits apply only on Save; scan/auto-scan races and false Update badges fixed
- SkinMaster temp dir is reused across launches instead of wiped every time

---

### Features

- Scan and extract `.mcpack`, `.mcaddon`, and `.mctemplate` files
- Automatically sorts packs by type: Behavior, Resource, Skin, World Template, Mash-up
- 4D skin pack detection and extraction for use with SkinMaster
- Install status detection — shows **Installed** or **Update Available** badges per pack
- Installed packs browser with delete, move, and rename support
- Statistics page showing pack counts and sizes per category
- Dark Red and Minecraft visual themes
- UI scaling with auto-detection for high-DPI and 4K+ displays
- Debug mode with detailed log output
- Recycle Bin, Find Duplicates, and Clean Up Names tools
- In-app confirmation dialog for destructive actions

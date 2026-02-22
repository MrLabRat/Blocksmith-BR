## Blocksmith v0.1.0

**Portable standalone release — no installer required.**

---

### Distribution

- Single `Blocksmith.exe` — no installer, no admin rights, no companion files
- Icons and SkinMaster.exe are embedded directly in the executable
- SkinMaster is extracted to `%TEMP%\Blocksmith\` on demand when launched
- Settings persist at `%APPDATA%\Roaming\blocksmith\settings.json`

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
- Shift+click trash to permanently delete source pack files from disk
- In-app confirmation dialog for destructive actions
- Tooltips on all action buttons

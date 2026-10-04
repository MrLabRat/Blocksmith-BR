## Blocksmith v2.1.0

**Portable standalone release — no installer required.**

---

### Highlights

- Status filters, version/attention badges, and clearer post-install results
- Premium Skin Packs page for importing extracted 4D skins
- Installed Packs multi-select and safer duplicate removal
- Scan accuracy and security hardening (skins.json multi-pack, path allowlists, semver updates)

See `CHANGELOG.md` for the full list.

---

### Distribution

- Single `blocksmith.exe` — no installer, no admin rights, no companion files
- Icons and SkinMaster.exe are embedded directly in the executable
- SkinMaster is extracted to `%TEMP%\Blocksmith-SkinMaster` on demand when launched
- Settings persist at `%APPDATA%\blocksmith\settings.json`

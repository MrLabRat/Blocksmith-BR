# Changelog

All notable changes to Blocksmith are documented in this file.

## [2.1.0]

### Added
- Install status filter chips (All / New / Updates / Installed) on the scan list, with updates sorted first.
- Version labels and Attention badges on pack rows (including child packs).
- Configurable **4D Skin Packs** destination path in Settings (auto-filled from scan location).
- **Premium Skin Packs** page to list marketplace premium packs and import extracted 4D skins.
- Installed Packs multi-select (grid + list), Select All / Clear, and Skin Packs (4D) filter tab.
- Duplicate removal confirmation dialog.
- Ctrl+Z keyboard shortcut for rollback.

### Fixed
- Archives with `skins.json` no longer hide nested behavior/resource packs in scans.
- Semver-aware version compare (`1.8` equals `1.8.0`); one-sided versions mark Update.
- Delete/rename allow packs that only have `skins.json` (no `manifest.json`).
- Old-pack recycle prefers UUID matches before base-name matching.
- Path allowlists for archive preview, folder sizes, icons, and MC file listing.
- Nested archive preview reads are size-capped; subfolder filtering uses component boundaries.
- UI scale saves are clamped to 50–300%.
- Successful extracts are removed from the scan list; recycled old paths show in results.
- Help text no longer claims unsupported `.zip` scanning; tips match real delete/rollback behavior.
- Launch Minecraft / Delete All failures surface in-app notifications.

## [2.0.0]

### Added
- **Clean Up Names**: bulk renamer for already-installed pack folders with messy,
  stacked legacy suffixes (e.g. `Dragons! Biomes (addon) - ppack1 (RESOURCE)` →
  `Dragons! Biomes (RESOURCE)`), with a review-and-apply modal plus a manual
  per-pack "Rename..." option in the context menu.
- **Recycle Bin**: deleted packs are moved to a recoverable recycle bin instead of
  being destroyed immediately — restore, permanently delete, or empty the bin from
  a dedicated page.
- **Find Duplicates**: scans installed packs for the same pack installed more than
  once (matched by UUID, or by name+type when no UUID is present) and lets you
  remove extras directly from the results.
- **Nested `.mcpack` support**: addons that bundle already-built `.mcpack` files
  directly inside a `.mcaddon` (instead of exploded folders) are now detected,
  previewed, and extracted correctly.
- **Auto-detect new packs**: files dropped into the configured scan folder while
  Blocksmith is open are automatically picked up without a manual rescan.
- **Marketplace icon fetching**: packs without an embedded icon can fetch their
  official icon from the Marketplace.
- **Multi-path scanning** for installed packs, covering every Minecraft Bedrock
  user profile on the machine, not just the primary one.
- **Minecraft visual theme**: grass/dirt background, squared UI corners, custom
  square window controls, and a bundled Minecraft-style font.
- Reworked packs grid layout with per-pack row backgrounds and grouping for
  addon/mash-up/world-template packs and their related resource/skin packs.

### Fixed
- Installed Packs scan progress bar no longer jumps backwards, and scanning no
  longer causes audible audio glitches (background-priority worker threads).
- The app window no longer goes transparent/freezes during heavy scans.
- Bulk "delete selected from disk" no longer reports false-positive "file does not
  exist" errors when a single archive contains more than one pack (e.g. an addon
  with both a Behavior Pack and Resource Pack).
- Pack names using bracket-style suffixes (`[BP]`, `[RP]`, etc.) and raw Minecraft
  `§` formatting codes are now cleaned correctly instead of showing raw codes.
- Various standalone/portable build fixes.

### Security
- Full audit of all Tauri commands for path-traversal, zip-slip, and command-
  injection risks; hardened destructive operations (delete, rename, extract) with
  strict destination-folder validation and filename sanitization.


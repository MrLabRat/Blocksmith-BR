# Changelog

All notable changes to Blocksmith are documented in this file.

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


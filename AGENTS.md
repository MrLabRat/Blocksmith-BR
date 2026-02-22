# Blocksmith - Minecraft Bedrock Pack Manager

## Project Overview

Blocksmith is a Minecraft Bedrock pack management application built with Tauri (Rust backend) and React (TypeScript frontend). It manages behavior packs, resource packs, skin packs, world templates, and mash-up packs.

## Architecture

- **Frontend**: React + TypeScript + Vite
- **Backend**: Rust + Tauri
- **Styling**: CSS with theme system (Dark Red / Minecraft)

## Key Features

1. Pack scanning and extraction (.mcpack, .mcaddon, .mctemplate)
2. Pack organization by type (BP, RP, Skin, World Template, Mash-up)
3. Installed packs management with delete functionality
4. UI scaling for high-resolution displays (5K, 8K, Ultrawide)
5. Statistics page with pack counts and sizes
6. Install status detection (already installed, update available)
7. Debug mode with detailed logs
8. In-app notifications for errors
9. **Theme system** (Dark Red / Minecraft)
10. **4D Skin Pack extraction** with SkinMaster integration

## Build Commands

```bash
npm run build        # Build frontend only
npm run tauri build  # Full Tauri build
npm run tauri dev    # Development mode
```

## Theme System

The app supports multiple visual themes:

### Dark Red (Default)
- Black and red color scheme
- Rounded corners (8px)
- Dark backgrounds with red accents

### Minecraft Theme
- Grass/dirt gradient background
- Green primary color (#5a8f32, #98fc03 for progress)
- Squared corners (0px border-radius)
- Minecraft font from CDNFonts
- Stone-gray panels
- Custom square window controls with X, -, □ icons

**Changing themes:**
- Go to Settings > Appearance > Theme
- Select "Dark Red" or "Minecraft"
- Theme applies immediately

**CSS Variables:**
- `--bg-primary`, `--bg-secondary`, `--bg-tertiary` - Background colors
- `--text-primary`, `--text-secondary`, `--text-muted` - Text colors
- `--primary-color`, `--primary-hover`, `--primary-light` - Accent colors
- `--border-radius` - Corner rounding (8px dark red, 0px minecraft)
- `--btn-bg`, `--btn-hover` - Button colors

## UI Scaling

- **Keyboard**: Ctrl+/- to zoom in/out, Ctrl+0 to reset
- **Settings**: UI Scale buttons (100%, 125%, 150%, 200%)
- **Auto-detection**: High DPI displays (>=2x) and 4K+ screens auto-scale to 130%/115%
- **CSS Variable**: `--ui-scale` applied via CSS zoom property to app-content

## 4D Skin Pack Handling

4D skin packs (containing `geometry.json`) require special handling:

1. **Detection**: Automatically detected by searching for geometry.json in archive
2. **Subfolder Detection**: Also detects skins.json in subfolders (not just root)
3. **Extraction**: Extracted to `SOURCE_FOLDER/4D Skin Packs/PACK_NAME` with contents at root (not nested)
4. **Path Display**: Results modal shows extracted path with copy button
5. **SkinMaster**: Launch SkinMaster directly from the results modal

**Implementation:**
- `PackType::SkinPack4D` enum variant
- `MoveOperation.skin_pack_4d_path` field stores extracted path
- `FileMover::process_pack` routes 4D skins to 4D folder
- Frontend displays paths with copy functionality

## Install Status Detection

When scanning packs, the app compares against installed packs by:
1. **UUID match** (most reliable) - matches pack UUIDs
2. **Base name match** - strips version numbers and suffixes from names for comparison

**Version detection sources (in order):**
1. Manifest version (from manifest.json header.version)
2. Pack name (e.g., "Pack Name 1.8")
3. Filename/path (e.g., "Pack Name 1.8.mcpack")
4. Folder name for installed packs (e.g., "Pack Name 1.8 (ADDON)")

**Version patterns detected:**
- ` 1.8`, ` 1.8.1` - space followed by version at end
- `v1.0.1`, `V.1.0.1` - v prefix versions
- ` 1.8 (` - version before parenthesized suffix
- ` 1.8 ` - version surrounded by spaces

**Badges shown:**
- **"Installed"** (green) - pack is already installed with same version
- **"Update"** (orange) - pack is installed but different version detected

**Fallback detection:**
- If no version found, compares folder sizes (>10% difference = update)

**World Template Updates:**
- When a world template is overwritten, shows warning in results modal
- Informs user they need to manually update existing worlds
- Instructions: Copy `behavior_packs/bp0` and `resource_packs/rp0` from new template to world folder

## Debug Mode

- **Toggle**: Settings > Options > Debug Mode
- **When enabled**: Shows all logs (DEBUG, INFO, WARNING, ERROR) in the log panel
- **When disabled**: Shows friendly Minecraft-themed messages instead of logs
- **Error notifications**: When an error occurs and debug mode is off, a notification pops up suggesting to enable debug mode or check Help & Feedback

## Window Controls

- Frameless titlebar with macOS-style buttons
- **Dark Red**: Circular buttons (12px diameter)
- **Minecraft**: Square buttons (24px) with beveled 3D effect
- **Important**: `.window-controls` must have `-webkit-app-region: no-drag` for clicks to work

## Code Conventions

- No comments unless explicitly requested
- Use CSS theme variables for all colors (no hardcoded values)
- Transparent panels to show animated background
- Custom frameless titlebar with macOS-style window controls
- Mash-up pack detection: (1) "mashup" in name, OR (2) matching WT + RP + BP
- World templates: internal behavior_packs/resource_packs are NOT scanned separately
- Log output uses monospace font, all other text uses theme font

## Recent Changes

1. Fixed window controls not working (added `-webkit-app-region: no-drag`)
2. Fixed SkinMaster launch (uses `cmd /C start` to open in new window)
3. Added 4D skin pack extraction to `SOURCE_FOLDER/4D Skin Packs`
4. Added `skin_pack_4d_path` to MoveOperation for displaying extracted paths
5. Minecraft theme now has grass/dirt background gradient
6. Minecraft progress bar uses #98fc03 color
7. Updated README.md with comprehensive documentation
8. Fixed taskbar icon - now applies saved icon immediately on startup via `.setup()` hook
9. Renamed icon settings: "Black & Red" → "Default", "Default" → "Minecraft"
10. Fixed 4D skin pack detection - now searches for skins.json in subfolders
11. 4D skin pack extraction strips subfolder prefix so contents are at root

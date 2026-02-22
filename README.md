# Blocksmith

A modern Minecraft Bedrock pack manager for Windows built with Tauri and React.

## Overview

Blocksmith is a desktop application that helps you organize, import, and manage Minecraft Bedrock Edition packs. It automatically detects your Minecraft Bedrock installation and provides an intuitive interface for handling Behavior Packs, Resource Packs, Skin Packs (including 4D geometry skins), World Templates, and Mash-up Packs.

## Features

### Core Functionality
- **Automatic Path Detection** - Finds Minecraft Bedrock pack directories automatically
- **Multi-format Support** - Import `.mcaddon`, `.mcpack`, `.mctemplate`, and `.zip` files
- **Smart Pack Detection** - Automatically identifies pack types and routes them correctly
- **Install Status Detection** - Shows which packs are already installed or need updating
- **Version Comparison** - Detects when a newer version of an installed pack is available
- **Rollback Support** - Undo the last batch of pack moves
- **Dry Run Mode** - Preview changes before applying them

### Pack Management
- View all installed packs with folder sizes
- Sort and filter packs by type, name, or size
- Batch select and delete multiple packs
- Export pack lists as CSV or JSON
- Context menu for quick actions

### Statistics Dashboard
- Pack counts by type
- Total storage usage
- Visual breakdown of installed packs

### Themes
- **Dark Red** - Default black and red color scheme with rounded corners
- **Minecraft** - Authentic Minecraft-style with squared corners, green accents, and grass/dirt background

### Additional Features
- **UI Scaling** - Adjustable for high-resolution displays (5K, 8K, Ultrawide)
- **Keyboard Shortcuts** - Ctrl+/- for zoom, Ctrl+0 to reset
- **Debug Mode** - Toggle detailed logging or friendly Minecraft-themed messages
- **In-app Notifications** - Error alerts when issues occur
- **Custom Icons** - Configurable taskbar and in-app icon styles

## Pack Types

| Type | Description | Destination |
|------|-------------|-------------|
| **Behavior Pack** | Custom gameplay mechanics, items, entities | `development_behavior_packs` |
| **Resource Pack** | Textures, models, sounds, UI elements | `development_resource_packs` |
| **Skin Pack** | Collections of character skins | `development_skin_packs` |
| **Skin Pack (4D)** | Advanced skins with custom geometry | Extracted to `4D Skin Packs` folder |
| **World Template** | Pre-made worlds for new creations | `minecraftWorlds` (premium) |
| **Mash-up Pack** | Combined WT + RP + BP + Skin | Detected and grouped automatically |

## 4D Skin Packs

4D skin packs contain custom geometry and require special handling:

1. **Detection** - Blocksmith automatically detects 4D skins by checking for `geometry.json`
2. **Extraction** - 4D skins are extracted to `SOURCE_FOLDER/4D Skin Packs/PACK_NAME`
3. **SkinMaster Integration** - Use the extracted path with [SkinMaster](https://github.com/MrLabRat/Blocksmith-BR) for full 4D skin functionality including encryption

## Installation

### From Release
1. Download the latest release from the [Releases](https://github.com/MrLabRat/Blocksmith-BR/releases) page
2. Launch Blocksmith by opening `Blocksmith.exe`

### From Source
```bash
# Clone the repository
git clone https://github.com/MrLabRat/Blocksmith-BR.git
cd "MC Bedrock Pack Mover"

# Install dependencies
npm install

# Run in development mode
npm run tauri dev

# Build for production
npm run tauri build
```

## Usage

### Importing Packs
1. Set your scan location in Settings
2. Click **Scan** to search for pack files
3. Select the packs you want to import
4. Click **Move Selected** to extract and install them
5. Use **Rollback Last** if needed to undo

### Managing Installed Packs
1. Open the menu and select **Installed Packs**
2. View, search, sort, and filter all installed packs
3. Use Ctrl+Click to select multiple packs
4. Right-click for context menu actions
5. Export the list for documentation

### Settings
- **Paths** - Configure destination folders for each pack type
- **Dry Run** - Preview changes without moving files
- **Delete Source** - Remove original files after successful import
- **UI Scale** - Adjust interface size for your display
- **Theme** - Switch between Dark Red and Minecraft themes
- **Debug Mode** - Toggle detailed logging

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Ctrl` + `+` | Zoom in (increase UI scale) |
| `Ctrl` + `-` | Zoom out (decrease UI scale) |
| `Ctrl` + `0` | Reset UI scale to 100% |

## Tech Stack

- **Frontend**: React 18, TypeScript, Vite
- **Backend**: Rust, Tauri 2
- **UI Components**: Lucide Icons
- **Styling**: CSS with theme system
- **Build**: Tauri bundler for Windows MSI/portable

## Project Structure

```
MC Bedrock Pack Mover/
├── src/                    # React frontend
│   ├── components/         # UI components
│   ├── styles/             # Component CSS
│   ├── App.tsx             # Main app component
│   ├── App.css             # Global styles + themes
│   └── types.ts            # TypeScript interfaces
├── src-tauri/              # Rust backend
│   ├── src/
│   │   ├── lib.rs          # Main Tauri commands
│   │   └── modules/        # Pack detection, file moving
│   ├── resources/          # Bundled resources (SkinMaster)
│   └── Cargo.toml          # Rust dependencies
├── public/                 # Static assets
│   └── icons/              # App icons
└── package.json            # Node dependencies
```

## Development

### Prerequisites
- [Node.js](https://nodejs.org/) v18+
- [Rust](https://www.rust-lang.org/)
- [Tauri CLI](https://tauri.app/)

### Commands
```bash
npm run dev          # Start Vite dev server
npm run build        # Build frontend only
npm run tauri dev    # Run in development mode
npm run tauri build  # Build production MSI/portable
```

### Adding New Features
1. Backend: Add new commands in `src-tauri/src/lib.rs` with `#[tauri::command]`
2. Frontend: Use `invoke('command_name')` to call from React
3. Types: Update interfaces in `src/types.ts` as needed

## Known Issues

- 4D skin pack encryption requires SkinMaster - Blocksmith extracts them but cannot encrypt
- World template updates require manual intervention for existing worlds
- Premium cache watching is experimental

## Contributing

Found a bug or have a feature request? Please open an issue on the [Issues](https://github.com/MrLabRat/Blocksmith-BR/issues) page.

## License

This project is provided as-is for the Minecraft Bedrock community.

## Credits

- Built with [Tauri](https://tauri.app/)
- Icons by [Lucide](https://lucide.dev/)
- Minecraft font from [CDNFonts](https://fonts.cdnfonts.com/)
- 4D Skin support powered by [SkinMaster] (Credits: xGG9 - RX Studio)

---

**Note**: Blocksmith is not affiliated with Mojang Studios or Microsoft. Minecraft is a trademark of Mojang Studios.

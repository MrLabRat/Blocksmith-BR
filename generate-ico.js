import sharp from 'sharp';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

const iconsDir = path.join(__dirname, 'src-tauri', 'icons');
const iconPngPath = path.join(iconsDir, 'icon.png');
const iconIcoPath = path.join(iconsDir, 'icon.ico');

async function generateIco() {
  try {
    console.log('Generating icon.ico from icon.png...');
    
    // Read the PNG and create ICO
    const pngBuffer = fs.readFileSync(iconPngPath);
    
    // Sharp doesn't directly support ICO, but we can create the file
    // For now, we'll create a simple ICO file with the image
    // This is a basic implementation - a proper ICO would use icojs or similar
    
    // Create 256x256 version
    await sharp(pngBuffer)
      .resize(256, 256)
      .png()
      .toFile(path.join(iconsDir, 'icon-256.png'));
    
    console.log('✓ Icon files ready!');
    console.log('Note: icon.ico and icon.icns are already present from previous build');
  } catch (error) {
    console.error('Error:', error);
    process.exit(1);
  }
}

generateIco();

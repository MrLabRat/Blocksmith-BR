import sharp from 'sharp';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// Function to create SVG with solid Minecraft-themed design
const createMinecraftIcon = () => {
  return `<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" width="256" height="256">
  <defs>
    <linearGradient id="grassGradient" x1="0%" y1="0%" x2="0%" y2="100%">
      <stop offset="0%" style="stop-color:#7cb342;stop-opacity:1" />
      <stop offset="100%" style="stop-color:#558b2f;stop-opacity:1" />
    </linearGradient>
  </defs>
  
  <!-- Main background - grass green -->
  <rect width="256" height="256" fill="#558b2f"/>
  
  <!-- Grass texture pattern -->
  <rect x="0" y="0" width="256" height="64" fill="url(#grassGradient)"/>
  
  <!-- Main dirt block -->
  <rect x="48" y="80" width="160" height="96" fill="#8d6e63" stroke="#705a46" stroke-width="3"/>
  
  <!-- Central chest icon -->
  <g transform="translate(128, 128)">
    <!-- Chest container -->
    <rect x="-45" y="-28" width="90" height="56" fill="#a1826d" stroke="#6b5436" stroke-width="2.5" rx="2"/>
    
    <!-- Chest lid/top -->
    <path d="M -40,-26 L -35,-36 L 35,-36 L 40,-26 Z" fill="#b8956a" stroke="#6b5436" stroke-width="2"/>
    
    <!-- Chest lock -->
    <circle cx="0" cy="-30" r="4" fill="#d4a574" stroke="#8b7355" stroke-width="1.5"/>
    
    <!-- Chest interior (darker) -->
    <rect x="-40" y="-20" width="80" height="30" fill="#6b5436"/>
    
    <!-- Glowing items inside -->
    <circle cx="-18" cy="-5" r="5" fill="#ffeb3b" opacity="0.9"/>
    <circle cx="0" cy="0" r="4" fill="#ffc107" opacity="0.8"/>
    <circle cx="18" cy="-6" r="5" fill="#ffeb3b" opacity="0.9"/>
  </g>
  
  <!-- Pickaxe symbol (top right) -->
  <g transform="translate(180, 40)">
    <rect x="0" y="12" width="3" height="30" fill="#6b5436" rx="1.5"/>
    <rect x="-10" y="0" width="20" height="14" fill="#929292" stroke="#4a4a4a" stroke-width="1.5" rx="1"/>
  </g>
</svg>`;
};

const iconsDir = path.join(__dirname, 'src-tauri', 'icons');

// Create icons directory if it doesn't exist
if (!fs.existsSync(iconsDir)) {
  fs.mkdirSync(iconsDir, { recursive: true });
}

async function generateIcons() {
  try {
    const svgBuffer = Buffer.from(createMinecraftIcon());
    
    // Generate main icon
    console.log('Generating icon.png (256x256)...');
    await sharp(svgBuffer)
      .png()
      .toFile(path.join(iconsDir, 'icon.png'));
    
    // Generate 128x128
    console.log('Generating 128x128.png...');
    await sharp(svgBuffer)
      .resize(128, 128)
      .png()
      .toFile(path.join(iconsDir, '128x128.png'));
    
    // Generate 32x32
    console.log('Generating 32x32.png...');
    await sharp(svgBuffer)
      .resize(32, 32)
      .png()
      .toFile(path.join(iconsDir, '32x32.png'));
    
    // Generate 128x128@2x (for retina)
    console.log('Generating 128x128@2x.png...');
    await sharp(svgBuffer)
      .resize(256, 256)
      .png()
      .toFile(path.join(iconsDir, '128x128@2x.png'));
    
    console.log('✓ All icons generated successfully!');
  } catch (error) {
    console.error('Error generating icons:', error);
    process.exit(1);
  }
}

generateIcons();

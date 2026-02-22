import { PackInfo, PackType } from '../types';
import { Puzzle, Palette, Shirt, Globe, Box } from 'lucide-react';

export function getFolderName(path: string): string {
  return path.split(/[/\\]/).pop() || path;
}

export function cleanDisplayName(name: string): string {
  let cleaned = name;
  cleaned = cleaned.replace(/\.(mcpack|mcaddon|mctemplate)$/i, '');

  const suffixes = [
    /\s*\(SKIN_PACK\)/i,
    /\s*\(SKIN\)/i,
    /\s*\(WORLD_TEMPLATE\)/i,
    /\s*\(MASHUP\)/i,
    /\s*\(MASH-UP\)/i,
    /\s*\(RESOURCES\)/i,
    /\s*\(RESOURCE\)/i,
    /\s*\(BEHAVIOR\)/i,
    /\s*\(BP\)/i,
    /\s*\(RP\)/i,
    /\s*\(ADDON\)/i,
    /\s*\(addon\)/i,
    /\s*\(TEMPLATE\)/i,
    /\s*-\s*ppack0/i,
    /\s*-\s*ppack1/i,
  ];

  for (const suffix of suffixes) {
    cleaned = cleaned.replace(suffix, '');
  }

  return cleaned.trim();
}

export function isRawInternalName(name: string): boolean {
  if (!name) return true;
  const lower = name.toLowerCase();
  if (/^pack\.[a-zA-Z_]+$/i.test(name)) return true;
  if (/^resourcePack\.[a-zA-Z_]+/i.test(name)) return true;
  if (/^behaviorPack\.[a-zA-Z_]+/i.test(name)) return true;
  if (/^skinpack\.[a-zA-Z_]+/i.test(name)) return true;
  if (/^[a-z]+\.[a-z_]+$/i.test(name)) return true;
  if (lower === name && name.includes('.')) return true;
  return false;
}

export function getBestDisplayName(pack: PackInfo): string {
  if (pack.name && pack.name.trim().length > 0 && !isRawInternalName(pack.name)) {
    return pack.name.trim();
  }
  const folderName = pack.subfolder ? pack.subfolder : getFolderName(pack.path);
  return cleanDisplayName(folderName);
}

export function getBaseNameForGrouping(folderName: string): string {
  let base = folderName;
  const suffixes = [
    /\.(mcpack|mcaddon|mctemplate)$/i,
    /\s*\(MASHUP\)/i,
    /\s*\(MASH-UP\)/i,
    /\s*\(TEMPLATE\)/i,
    /\s*\(WORLD_TEMPLATE\)/i,
    /\s*\(RESOURCES\)/i,
    /\s*\(RESOURCE\)/i,
    /\s*\(SKINS\)/i,
    /\s*\(SKIN\)/i,
    /\s*\(SKIN_PACK\)/i,
    /\s*\(ADDON\)/i,
    /\s*\(addon\)/i,
    /\s*\(BEHAVIOR\)/i,
    /\s*\(BEHAVIOUR\)/i,
    /\s*\(BP\)/i,
    /\s*\(RP\)/i,
    /\s*Resources$/i,
    /\s*Resource Pack$/i,
    /\s*Skins$/i,
    /\s*Skin Pack$/i,
    /\s*Addon$/i,
    /\s*Behavior Pack$/i,
    /\s*Behaviour Pack$/i,
    /\s*-\s*ppack\d/i,
  ];

  for (const suffix of suffixes) {
    base = base.replace(suffix, '');
  }

  return base.trim().toLowerCase();
}

export function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return Math.round((bytes / Math.pow(k, i)) * 100) / 100 + ' ' + sizes[i];
}

export function getIconForPackType(packType: PackType) {
  switch (packType) {
    case 'BehaviorPack':
      return Puzzle;
    case 'ResourcePack':
      return Palette;
    case 'SkinPack':
    case 'SkinPack4D':
      return Shirt;
    case 'WorldTemplate':
    case 'MashupPack':
      return Globe;
    default:
      return Box;
  }
}

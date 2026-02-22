export type PackType = 'BehaviorPack' | 'ResourcePack' | 'SkinPack' | 'SkinPack4D' | 'WorldTemplate' | 'MashupPack' | 'Unknown';

export interface PackInfo {
  path: string;
  name: string;
  pack_type: PackType;
  uuid?: string;
  version?: string;
  extracted: boolean;
  icon_base64?: string;
  subfolder?: string;
  folder_size?: number;
  folder_size_formatted?: string;
  needs_attention?: boolean;
  attention_message?: string;
  is_installed?: boolean;
  is_update?: boolean;
  installed_version?: string;
}

export type BackgroundStyle = 'embers' | 'matrix' | 'mc-terrain' | 'night-sky' | 'none';

export interface Settings {
  behavior_pack_path?: string;
  resource_pack_path?: string;
  skin_pack_path?: string;
  skin_pack_4d_path?: string;
  world_template_path?: string;
  scan_location?: string;
  dry_run: boolean;
  delete_source: boolean;
  disable_animations?: boolean;
  animation_speed_ms?: number;
  ui_scale?: number;
  taskbar_icon_style?: 'blackred' | 'default';
  taskbar_icon_border?: boolean;
  app_icon_style?: 'blackred' | 'default';
  app_icon_border?: boolean;
  debug_mode?: boolean;
  disable_tip_notifications?: boolean;
  theme?: 'darkred' | 'minecraft';
  background_style?: BackgroundStyle;
  background_smoke?: number;
  background_blobs?: number;
}

export type ThemeName = 'darkred' | 'minecraft';

export interface AppNotification {
  id: string;
  type: 'error' | 'warning' | 'info' | 'success';
  title: string;
  message: string;
  timestamp: number;
}

export interface MoveOperation {
  source: string;
  destination: string;
  pack_name: string;
  pack_type: PackType;
  success: boolean;
  error?: string;
  is_template_update?: boolean;
  skin_pack_4d_path?: string;
  deleted_old_path?: string;
}

export interface LogEntry {
  timestamp: string;
  level: string;
  message: string;
}

export interface ProgressEvent {
  current: number;
  total: number;
  message: string;
  estimated_seconds?: number;
}

export interface PremiumCachePack {
  folder_name: string;
  display_name: string;
  path: string;
}

export interface WatcherEvent {
  timestamp: string;
  event_type: string;
  path: string;
  details?: string;
}

export interface PackStats {
  pack_type: string;
  count: number;
  total_size: number;
  total_size_formatted: string;
}

export function getPackKey(pack: PackInfo): string {
  return `${pack.path}::${pack.subfolder || ''}`;
}

export const PackTypeLabels: Record<PackType, string> = {
  BehaviorPack: 'Addon',
  ResourcePack: 'Resource Pack',
  SkinPack: 'Skin Pack',
  SkinPack4D: 'Skin Pack (4D)',
  WorldTemplate: 'World Template',
  MashupPack: 'Mash-Up',
  Unknown: 'Unknown',
};

export const PackTypeColors: Record<PackType, string> = {
  BehaviorPack: '#8b0000',
  ResourcePack: '#2d5a3d',
  SkinPack: '#8b6914',
  SkinPack4D: '#5a3d6b',
  WorldTemplate: '#2d4a5a',
  MashupPack: '#5a2d5a',
  Unknown: '#4a4a4a',
};

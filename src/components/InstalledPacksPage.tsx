import { useState, useEffect, useMemo, useCallback, memo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { List } from 'react-window';
import type { RowComponentProps } from 'react-window';
import { PackInfo, PackType, PackTypeColors, AppNotification, DuplicateGroup, RenameSuggestion, RenameResult } from '../types';
import { getFolderName, cleanDisplayName, getBestDisplayName, getBaseNameForGrouping, formatBytes, getIconForPackType } from '../utils/packUtils';
import { X, Copy, Hash, FileText, Trash2, AlertTriangle, Bookmark, Wand2, Pencil } from 'lucide-react';
import '../styles/InstalledPacksPage.css';

/** Row height constants for the virtualized list view (react-window). */
const VIRTUAL_MAIN_ROW_HEIGHT = 76;
const VIRTUAL_CHILD_ROW_HEIGHT = 68;

interface InstalledPacksPageProps {
  onClose: () => void;
  addNotification: (type: AppNotification['type'], title: string, message: string) => void;
}

interface ContextMenuState {
  x: number;
  y: number;
  pack: PackInfo;
}

interface PackGroup {
  mainPack: PackInfo;
  resourcePacks: PackInfo[];
  skinPacks: PackInfo[];
  behaviorPacks: PackInfo[];
  worldTemplates: PackInfo[];
  totalSize: number;
  isAddon: boolean;
  isMashup: boolean;
  displayName: string;
}

const packTypeLabels: Record<string, string> = {
  'All': 'All Packs',
  'BehaviorPack': 'Addons',
  'ResourcePack': 'Resource Packs',
  'SkinPack': 'Skin Packs',
  'WorldTemplate': 'World Templates',
  'MashupPack': 'Mash-Ups',
};


const InstalledPackIcon = memo(function InstalledPackIcon({ pack, overrideIcon }: { pack: PackInfo; overrideIcon?: string | null }) {
  const IconComponent = getIconForPackType(pack.pack_type);
  const color = PackTypeColors[pack.pack_type];
  const iconSrc = overrideIcon ?? pack.icon_base64;
  
  if (iconSrc) {
    return (
      <img 
        src={iconSrc}
        alt={pack.name}
        className="pack-icon-img"
      />
    );
  }
  
  return (
    <div className="pack-icon-default" style={{ backgroundColor: `${color}20` }}>
      <IconComponent size={24} style={{ color }} />
    </div>
  );
});

/** Returns the best available icon from across all packs in a group. */
function getBestGroupIcon(group: PackGroup): string | null {
  const allPacks = [
    group.mainPack,
    ...group.worldTemplates,
    ...group.resourcePacks,
    ...group.skinPacks,
    ...group.behaviorPacks,
  ];
  for (const p of allPacks) {
    if (p.icon_base64) return p.icon_base64;
  }
  return null;
}

/** Skin packs commonly ship without their own icon; fall back to the world
 * template's or resource pack's icon from the same group so they don't show a generic glyph. */
function getSkinPackFallbackIcon(group: PackGroup): string | null {
  const candidates = [
    ...group.worldTemplates,
    ...group.resourcePacks,
    ...(group.mainPack.pack_type === 'WorldTemplate' || group.mainPack.pack_type === 'MashupPack' || group.mainPack.pack_type === 'ResourcePack'
      ? [group.mainPack]
      : []),
  ];
  for (const p of candidates) {
    if (p.icon_base64) return p.icon_base64;
  }
  return null;
}

const PackGridCard = memo(function PackGridCard({
  group,
  packSizes,
  onContextMenu,
  onDelete,
  isExpanded,
  onToggleExpand,
}: {
  group: PackGroup;
  packSizes: Record<string, { size: number; formatted: string }>;
  onContextMenu: (e: React.MouseEvent, pack: PackInfo) => void;
  onDelete: (pack: PackInfo) => void;
  isExpanded: boolean;
  onToggleExpand: () => void;
}) {
  const IconComponent = getIconForPackType(group.mainPack.pack_type);
  const color = PackTypeColors[group.mainPack.pack_type];
  const bestIcon = getBestGroupIcon(group);
  const childPacks: PackInfo[] = [
    ...group.worldTemplates.filter(wt => wt.path !== group.mainPack.path),
    ...group.resourcePacks,
    ...group.skinPacks,
    ...group.behaviorPacks,
  ];
  const hasChildren = childPacks.length > 0;

  return (
    <div
      className={`pack-grid-card${isExpanded ? ' grid-expanded' : ''}`}
      onContextMenu={(e) => onContextMenu(e, group.mainPack)}
      onClick={() => hasChildren && onToggleExpand()}
      style={hasChildren ? { cursor: 'pointer' } : undefined}
    >
      {/* Square image — no wrapper div, aspect-ratio on the img itself */}
      <div className="pack-grid-thumb">
        {bestIcon
          ? <img src={bestIcon} alt={group.displayName} className="pack-grid-thumb-img" />
          : <div className="pack-grid-thumb-fallback" style={{ backgroundColor: `${color}22` }}>
              <IconComponent style={{ color, width: '40%', height: '40%' }} />
            </div>
        }
        {hasChildren && (
          <span className="pack-grid-badge">+{childPacks.length}</span>
        )}
        <button
          className="pack-grid-del"
          onClick={(e) => { e.stopPropagation(); onDelete(group.mainPack); }}
          title="Delete"
        >
          <Trash2 size={11} />
        </button>
      </div>

      {/* Info strip */}
      <div className="pack-grid-info">
        <div className="pack-grid-name" title={group.displayName}>{group.displayName}</div>
        <div className="pack-grid-sub">
          <span style={{ color: PackTypeColors[group.mainPack.pack_type] || '#6b7280' }}>
            {packTypeLabels[group.mainPack.pack_type]}
          </span>
          <span className="pack-grid-size">{packSizes[group.mainPack.path]?.formatted ?? ''}</span>
        </div>
      </div>
    </div>
  );
});

type ChildKind = 'WorldTemplate' | 'BehaviorPack' | 'ResourcePack' | 'SkinPack';

const childTypeLabels: Record<ChildKind, string> = {
  WorldTemplate: 'World Template',
  BehaviorPack: 'Addon',
  ResourcePack: 'Resource Pack',
  SkinPack: 'Skin Pack',
};

/** Flattened representation of the list view — every visible group header and
 * (when expanded) its child rows become their own fixed-height virtual row so
 * react-window can render the whole thing without measuring dynamic heights. */
type VirtualRow =
  | { kind: 'main'; group: PackGroup; hasChildren: boolean }
  | { kind: 'child'; pack: PackInfo; childKind: ChildKind };

interface VirtualListRowProps {
  virtualRows: VirtualRow[];
  packSizes: Record<string, { size: number; formatted: string }>;
  expandedGroups: Set<string>;
  skinFallbackIcons: Record<string, string | null>;
  onToggle: (path: string) => void;
  onContextMenu: (e: React.MouseEvent, pack: PackInfo) => void;
  onDelete: (pack: PackInfo) => void;
  getBestDisplayName: (pack: PackInfo) => string;
}

function VirtualListRow({
  index,
  style,
  virtualRows,
  packSizes,
  expandedGroups,
  skinFallbackIcons,
  onToggle,
  onContextMenu,
  onDelete,
  getBestDisplayName,
}: RowComponentProps<VirtualListRowProps>) {
  const row = virtualRows[index];
  const wrapperStyle: React.CSSProperties = { ...style, paddingBottom: 4, boxSizing: 'border-box' };

  if (row.kind === 'child') {
    const { pack, childKind } = row;
    const isSkinPack = childKind === 'SkinPack';
    const icon = pack.icon_base64 ?? (isSkinPack ? skinFallbackIcons[pack.path] ?? null : null);
    return (
      <div style={wrapperStyle}>
        <div
          className="installed-pack-card child-pack"
          style={{ height: '100%' }}
          onContextMenu={(e) => onContextMenu(e, pack)}
        >
          {icon && <div className="pack-row-bg" style={{ backgroundImage: `url(${icon})` }} />}
          <InstalledPackIcon pack={pack} overrideIcon={icon} />
          <div className="pack-card-content">
            <div className="pack-card-name">{getBestDisplayName(pack)}</div>
            <div className="pack-card-details">
              <span className="pack-type" style={{ color: PackTypeColors[childKind] }}>
                {childTypeLabels[childKind]}
              </span>
              <span className="pack-card-size">
                {packSizes[pack.path]?.formatted || 'Unknown'}
              </span>
            </div>
          </div>
          <button
            className="btn btn-icon btn-delete"
            onClick={(e) => { e.stopPropagation(); onDelete(pack); }}
            title="Delete pack"
          >
            <Trash2 size={12} />
          </button>
        </div>
      </div>
    );
  }

  const { group, hasChildren } = row;
  const isExpanded = expandedGroups.has(group.mainPack.path);
  const groupIcon = getBestGroupIcon(group);

  return (
    <div style={wrapperStyle}>
      <div
        className={`installed-pack-card ${hasChildren ? 'has-children' : ''}`}
        style={{ height: '100%' }}
        onClick={() => hasChildren && onToggle(group.mainPack.path)}
        onContextMenu={(e) => onContextMenu(e, group.mainPack)}
      >
        {groupIcon && <div className="pack-row-bg" style={{ backgroundImage: `url(${groupIcon})` }} />}
        <InstalledPackIcon pack={group.mainPack} overrideIcon={groupIcon} />
        <div className="pack-card-content">
          <div className="pack-card-name">
            {group.displayName}
            {hasChildren && (
              <span className="expand-indicator">
                {isExpanded ? '▼' : '▶'}
              </span>
            )}
          </div>
          <div className="pack-card-details">
            <span className="pack-type" style={{ color: PackTypeColors[group.mainPack.pack_type] || '#6b7280' }}>
              {packTypeLabels[group.mainPack.pack_type]}
            </span>
            {(group.resourcePacks.length > 0 || group.skinPacks.length > 0 || group.behaviorPacks.length > 0) && (
              <span className="pack-group-count">
                +{group.resourcePacks.length + group.skinPacks.length + group.behaviorPacks.length} parts
              </span>
            )}
            <span className="pack-card-size">
              {formatBytes(group.totalSize)}
            </span>
          </div>
        </div>
        <button
          className="btn btn-icon btn-delete"
          onClick={(e) => { e.stopPropagation(); onDelete(group.mainPack); }}
          title="Delete pack"
        >
          <Trash2 size={12} />
        </button>
      </div>
    </div>
  );
}

function csvEscape(value: string): string {
  if (/^[=+\-@\t\r]/.test(value)) {
    value = "'" + value;
  }
  return '"' + value.replace(/"/g, '""') + '"';
}


function getAddonBaseName(folderName: string): string | null {
  const lower = folderName.toLowerCase();
  if (!lower.includes('(addon)')) return null;
  
  let base = folderName;
  const suffixes = [
    /\s*-\s*ppack0/i,
    /\s*-\s*ppack1/i,
    /\s*\(ADDON\)/i,
    /\s*\(addon\)/i,
    /\s*\(RESOURCE\)/i,
    /\s*\(BEHAVIOR\)/i,
    /\s*\(BP\)/i,
    /\s*\(RP\)/i,
  ];
  
  for (const suffix of suffixes) {
    base = base.replace(suffix, '');
  }
  
  return base.trim();
}


const SIZE_CACHE_KEY = 'blocksmith_folder_sizes_cache';
const CACHE_EXPIRY_MS = 24 * 60 * 60 * 1000;

const FILTER_PRESETS_KEY = 'blocksmith_filter_presets';

interface FilterPreset {
  name: string;
  selectedType: PackType | 'All';
  sortBy: 'name' | 'size' | 'type';
  sortOrder: 'asc' | 'desc';
  searchTerm: string;
}

function loadFilterPresets(): FilterPreset[] {
  try {
    const raw = localStorage.getItem(FILTER_PRESETS_KEY);
    if (!raw) return [];
    const parsed: unknown = JSON.parse(raw);
    return Array.isArray(parsed) ? (parsed as FilterPreset[]) : [];
  } catch (e) {
    console.warn('Failed to load filter presets:', e);
    return [];
  }
}

function saveFilterPresets(presets: FilterPreset[]) {
  try {
    localStorage.setItem(FILTER_PRESETS_KEY, JSON.stringify(presets));
  } catch (e) {
    console.warn('Failed to save filter presets:', e);
  }
}

type SizeCacheEntry = { size: number; formatted: string; timestamp: number };
type SizeCache = Record<string, SizeCacheEntry>;

function isSizeCacheEntry(v: unknown): v is SizeCacheEntry {
  return typeof v === 'object' && v !== null &&
    typeof (v as Record<string, unknown>).size === 'number' &&
    typeof (v as Record<string, unknown>).formatted === 'string' &&
    typeof (v as Record<string, unknown>).timestamp === 'number';
}

function loadSizeCache(): SizeCache {
  try {
    const raw = localStorage.getItem(SIZE_CACHE_KEY);
    if (raw) {
      const parsed: unknown = JSON.parse(raw);
      if (typeof parsed === 'object' && parsed !== null) {
        const result: SizeCache = {};
        for (const [key, value] of Object.entries(parsed as Record<string, unknown>)) {
          if (isSizeCacheEntry(value)) result[key] = value;
        }
        return result;
      }
    }
  } catch (e) {
    console.warn('Failed to load size cache:', e);
  }
  return {};
}

function saveSizeCache(sizes: Record<string, { size: number; formatted: string }>, existingCache?: SizeCache) {
  try {
    const cache: SizeCache = existingCache ? { ...existingCache } : loadSizeCache();
    const now = Date.now();
    Object.entries(sizes).forEach(([path, data]) => {
      cache[path] = { ...data, timestamp: now };
    });
    Object.keys(cache).forEach(key => {
      if (now - cache[key].timestamp > CACHE_EXPIRY_MS) {
        delete cache[key];
      }
    });
    localStorage.setItem(SIZE_CACHE_KEY, JSON.stringify(cache));
  } catch (e) {
    console.warn('Failed to save size cache:', e);
  }
}

export function InstalledPacksPage({ onClose, addNotification }: InstalledPacksPageProps) {
  const [packs, setPacks] = useState<PackInfo[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [loadingCount, setLoadingCount] = useState(0);
  const [loadingTotal, setLoadingTotal] = useState(0);
  const [selectedType, setSelectedType] = useState<PackType | 'All'>('All');
  const [searchTerm, setSearchTerm] = useState('');
  const [debouncedSearchTerm, setDebouncedSearchTerm] = useState('');
  const [packSizes, setPackSizes] = useState<Record<string, { size: number; formatted: string }>>({});
  const [sortBy, setSortBy] = useState<'name' | 'size' | 'type'>('name');
  const [sortOrder, setSortOrder] = useState<'asc' | 'desc'>('asc');
  const [selectedPacks, setSelectedPacks] = useState<Set<string>>(new Set());
  const [loadingProgress, setLoadingProgress] = useState({ loaded: 0, total: 0 });
  const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);
  const [expandedGroups, setExpandedGroups] = useState<Set<string>>(new Set());
  const [pendingDelete, setPendingDelete] = useState<PackInfo | null>(null);
  const [pendingDeleteSelected, setPendingDeleteSelected] = useState(false);
  const [viewMode, setViewMode] = useState<'list' | 'grid'>('grid');
  const [marketplaceStatus, setMarketplaceStatus] = useState<'idle' | 'loading' | 'done' | 'unavailable'>('idle');
  const [expandedGridCard, setExpandedGridCard] = useState<string | null>(null);
  const [duplicates, setDuplicates] = useState<DuplicateGroup[] | null>(null);
  const [isDuplicateLoading, setIsDuplicateLoading] = useState(false);
  const [renameSuggestions, setRenameSuggestions] = useState<RenameSuggestion[] | null>(null);
  const [isRenameScanning, setIsRenameScanning] = useState(false);
  const [selectedRenames, setSelectedRenames] = useState<Set<string>>(new Set());
  const [isApplyingRenames, setIsApplyingRenames] = useState(false);
  const [filterPresets, setFilterPresets] = useState<FilterPreset[]>(() => loadFilterPresets());
  const [selectedPresetName, setSelectedPresetName] = useState('');


  const groupedPacks = useMemo(() => {
    const groups: PackGroup[] = [];
    const processedPaths = new Set<string>();
    
    // Build lookup maps for O(1) access
    const byBaseName = new Map<string, PackInfo[]>();
    
    for (const pack of packs) {
      const folderName = getFolderName(pack.path);
      const baseName = getBaseNameForGrouping(folderName);
      
      if (!byBaseName.has(baseName)) {
        byBaseName.set(baseName, []);
      }
      byBaseName.get(baseName)!.push(pack);
    }
    
    // First, process all MashupPack types (detected by name containing "mashup")
    for (const pack of packs) {
      if (processedPaths.has(pack.path)) continue;
      if (pack.pack_type !== 'MashupPack') continue;
      
      const folderName = getFolderName(pack.path);
      const baseName = getBaseNameForGrouping(folderName);
      const packList = byBaseName.get(baseName) || [pack];
      
      const resourcePacks = packList.filter(p => p.pack_type === 'ResourcePack' && !processedPaths.has(p.path));
      const skinPacks = packList.filter(p => p.pack_type === 'SkinPack' && !processedPaths.has(p.path));
      const behaviorPacks = packList.filter(p => p.pack_type === 'BehaviorPack' && !processedPaths.has(p.path));
      const worldTemplates = packList.filter(p => (p.pack_type === 'WorldTemplate' || p.pack_type === 'MashupPack') && !processedPaths.has(p.path));
      
      packList.forEach(p => processedPaths.add(p.path));
      
      const allParts = [pack, ...resourcePacks, ...skinPacks, ...behaviorPacks, ...worldTemplates];
      const totalSize = allParts.reduce((sum, p) => sum + (packSizes[p.path]?.size || 0), 0);
      
      groups.push({
        mainPack: pack,
        resourcePacks,
        skinPacks,
        behaviorPacks,
        worldTemplates,
        totalSize,
        isAddon: false,
        isMashup: true,
        displayName: cleanDisplayName(folderName),
      });
    }
    
    // Then, detect mash-up packs by matching World Template + Resource Pack + Behavior Pack
    for (const [_baseName, packList] of byBaseName) {
      if (packList.every(p => processedPaths.has(p.path))) continue;
      if (packList.length < 2) continue;
      
      const hasWorldTemplate = packList.some(p => p.pack_type === 'WorldTemplate');
      const hasResourcePack = packList.some(p => p.pack_type === 'ResourcePack');
      const hasBehaviorPack = packList.some(p => p.pack_type === 'BehaviorPack');
      
      // It's a mash-up if it has world template + resource pack + behavior pack
      if (hasWorldTemplate && hasResourcePack && hasBehaviorPack) {
        const mainPack = packList.find(p => p.pack_type === 'WorldTemplate') || packList[0];
        
        const resourcePacks = packList.filter(p => p.pack_type === 'ResourcePack' && !processedPaths.has(p.path));
        const skinPacks = packList.filter(p => p.pack_type === 'SkinPack' && !processedPaths.has(p.path));
        const behaviorPacks = packList.filter(p => p.pack_type === 'BehaviorPack' && !processedPaths.has(p.path));
        const worldTemplates = packList.filter(p => p.pack_type === 'WorldTemplate' && !processedPaths.has(p.path));
        
        packList.forEach(p => processedPaths.add(p.path));
        
        const totalSize = packList.reduce((sum, p) => sum + (packSizes[p.path]?.size || 0), 0);
        
        groups.push({
          mainPack,
          resourcePacks,
          skinPacks,
          behaviorPacks,
          worldTemplates,
          totalSize,
          isAddon: false,
          isMashup: true,
          displayName: cleanDisplayName(getFolderName(mainPack.path)),
        });
      }
    }
    
    // Process addon behavior packs (with (addon) marker)
    for (const pack of packs) {
      if (processedPaths.has(pack.path)) continue;
      if (pack.pack_type !== 'BehaviorPack') continue;
      
      const folderName = getFolderName(pack.path);
      const baseName = getAddonBaseName(folderName);
      if (!baseName) continue;
      
      processedPaths.add(pack.path);
      
      const packList = byBaseName.get(baseName.toLowerCase()) || [];
      
      const matchingRPs = packList.filter(p => 
        p.pack_type === 'ResourcePack' && !processedPaths.has(p.path)
      );
      matchingRPs.forEach(rp => processedPaths.add(rp.path));
      
      const matchingSPs = packList.filter(p => 
        p.pack_type === 'SkinPack' && !processedPaths.has(p.path)
      );
      matchingSPs.forEach(sp => processedPaths.add(sp.path));
      
      const bpSize = packSizes[pack.path]?.size || 0;
      const rpsSize = matchingRPs.reduce((sum, rp) => sum + (packSizes[rp.path]?.size || 0), 0);
      const spsSize = matchingSPs.reduce((sum, sp) => sum + (packSizes[sp.path]?.size || 0), 0);
      
      groups.push({
        mainPack: pack,
        resourcePacks: matchingRPs,
        skinPacks: matchingSPs,
        behaviorPacks: [],
        worldTemplates: [],
        totalSize: bpSize + rpsSize + spsSize,
        isAddon: true,
        isMashup: false,
        displayName: baseName,
      });
    }
    
    // Add remaining standalone packs
    for (const pack of packs) {
      if (processedPaths.has(pack.path)) continue;
      
      processedPaths.add(pack.path);
      
      groups.push({
        mainPack: pack,
        resourcePacks: [],
        skinPacks: [],
        behaviorPacks: [],
        worldTemplates: [],
        totalSize: packSizes[pack.path]?.size || 0,
        isAddon: false,
        isMashup: false,
        displayName: getBestDisplayName(pack),
      });
    }
    
    return groups;
  }, [packs, packSizes]);

  const filteredGroupedPacks = useMemo(() => {
    let result = groupedPacks.filter(group => {
      const matchesType = selectedType === 'All' || group.mainPack.pack_type === selectedType;
      const matchesSearch = 
        group.displayName.toLowerCase().includes(debouncedSearchTerm.toLowerCase()) ||
        group.mainPack.path.toLowerCase().includes(debouncedSearchTerm.toLowerCase()) ||
        group.resourcePacks.some(rp => rp.name.toLowerCase().includes(debouncedSearchTerm.toLowerCase())) ||
        group.skinPacks.some(sp => sp.name.toLowerCase().includes(debouncedSearchTerm.toLowerCase()));
      return matchesType && matchesSearch;
    });

    result.sort((a, b) => {
      let comparison = 0;
      if (sortBy === 'name') {
        comparison = a.displayName.localeCompare(b.displayName);
      } else if (sortBy === 'size') {
        comparison = a.totalSize - b.totalSize;
      } else if (sortBy === 'type') {
        comparison = a.mainPack.pack_type.localeCompare(b.mainPack.pack_type);
      }
      return sortOrder === 'asc' ? comparison : -comparison;
    });

    return result;
  }, [groupedPacks, selectedType, debouncedSearchTerm, sortBy, sortOrder]);

  /** Flattens groups + their (expanded) children into fixed-height rows for
   * react-window virtualization of the list view. */
  const virtualRows = useMemo<VirtualRow[]>(() => {
    const rows: VirtualRow[] = [];
    for (const group of filteredGroupedPacks) {
      const hasChildren = group.resourcePacks.length > 0
        || group.skinPacks.length > 0
        || group.behaviorPacks.length > 0
        || group.worldTemplates.length > 1;
      rows.push({ kind: 'main', group, hasChildren });
      if (hasChildren && expandedGroups.has(group.mainPack.path)) {
        for (const wt of group.worldTemplates.filter(w => w.path !== group.mainPack.path)) {
          rows.push({ kind: 'child', pack: wt, childKind: 'WorldTemplate' });
        }
        for (const bp of group.behaviorPacks) {
          rows.push({ kind: 'child', pack: bp, childKind: 'BehaviorPack' });
        }
        for (const rp of group.resourcePacks) {
          rows.push({ kind: 'child', pack: rp, childKind: 'ResourcePack' });
        }
        for (const sp of group.skinPacks) {
          rows.push({ kind: 'child', pack: sp, childKind: 'SkinPack' });
        }
      }
    }
    return rows;
  }, [filteredGroupedPacks, expandedGroups]);

  /** Skin packs commonly ship without their own icon; precompute a fallback
   * per-pack so the virtualized child rows don't need the whole group object. */
  const skinFallbackIcons = useMemo(() => {
    const map: Record<string, string | null> = {};
    for (const group of filteredGroupedPacks) {
      if (group.skinPacks.length === 0) continue;
      const fallback = getSkinPackFallbackIcon(group);
      for (const sp of group.skinPacks) {
        map[sp.path] = fallback;
      }
    }
    return map;
  }, [filteredGroupedPacks]);

  const toggleGroup = useCallback((path: string) => {
    setExpandedGroups(prev => {
      const next = new Set(prev);
      if (next.has(path)) {
        next.delete(path);
      } else {
        next.add(path);
      }
      return next;
    });
  }, []);

  const handleContextMenu = useCallback((e: React.MouseEvent, pack: PackInfo) => {
    e.preventDefault();
    e.stopPropagation();
    setContextMenu({ x: e.clientX, y: e.clientY, pack });
  }, []);

  const closeContextMenu = useCallback(() => {
    setContextMenu(null);
  }, []);

  useEffect(() => {
    const handleClick = (e: MouseEvent) => {
      if (contextMenu) {
        const target = e.target as HTMLElement;
        if (!target.closest('.context-menu')) {
          closeContextMenu();
        }
      }
    };
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === 'Escape') closeContextMenu();
    };
    document.addEventListener('mousedown', handleClick);
    document.addEventListener('keydown', handleEscape);
    return () => {
      document.removeEventListener('mousedown', handleClick);
      document.removeEventListener('keydown', handleEscape);
    };
  }, [closeContextMenu, contextMenu]);

  const copyToClipboard = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
    } catch (err) {
      console.error('Failed to copy:', err);
    }
    closeContextMenu();
  };

  const handleDeletePack = async (pack: PackInfo) => {
    setPendingDelete(pack);
    closeContextMenu();
  };

  const confirmDeletePack = async () => {
    if (!pendingDelete) return;
    const pack = pendingDelete;
    setPendingDelete(null);
    try {
      await invoke('delete_pack', { path: pack.path });
      setPacks(packs.filter(p => p.path !== pack.path));
    } catch (error) {
      addNotification('error', 'Delete failed', `Failed to delete pack: ${error}`);
    }
  };

  // Debounce search input
  useEffect(() => {
    const timer = setTimeout(() => {
      setDebouncedSearchTerm(searchTerm);
    }, 300);
    return () => clearTimeout(timer);
  }, [searchTerm]);

  useEffect(() => {
    const loadPacks = async () => {
      try {
        setIsLoading(true);
        setLoadingCount(0);
        setLoadingTotal(0);

        // Stream packs in via event listener so we can show real scan progress.
        // The backend guarantees monotonically increasing values (a single
        // dedicated thread emits these), but we still clamp defensively here
        // in case an event from a previous scan arrives late.
        const unlisten = await listen<{ current: number; total: number }>('packs_scan_progress', (event) => {
          setLoadingCount(prev => Math.max(prev, event.payload.current));
          setLoadingTotal(event.payload.total);
        });

        const folderPacks = await invoke<PackInfo[]>('get_directory_folders');
        unlisten();
        setLoadingCount(folderPacks.length);
        setLoadingTotal(folderPacks.length);
        setPacks(folderPacks);
        setIsLoading(false);

        // ── Marketplace icon fetch (background, non-blocking) ────────
        setMarketplaceStatus('loading');
        const packsNeedingIcons = folderPacks
          .filter(p => !p.icon_base64 && p.uuid)
          .map(p => ({ path: p.path, uuid: p.uuid }));

        if (packsNeedingIcons.length > 0) {
          try {
            const marketIcons = await invoke<{ path: string; icon_base64: string }[]>(
              'fetch_marketplace_icons',
              { packs: packsNeedingIcons }
            );
            if (marketIcons.length > 0) {
              const iconMap = new Map(marketIcons.map(r => [r.path, r.icon_base64]));
              setPacks(prev => prev.map(p =>
                iconMap.has(p.path) ? { ...p, icon_base64: iconMap.get(p.path)! } : p
              ));
            }
            setMarketplaceStatus('done');
          } catch {
            setMarketplaceStatus('unavailable');
          }
        } else {
          setMarketplaceStatus('done');
        }

        // ── Size calculation ─────────────────────────────────────────
        const cache = loadSizeCache();
        const now = Date.now();
        const cachedSizes: Record<string, { size: number; formatted: string }> = {};
        const needsRefresh: string[] = [];

        folderPacks.forEach(pack => {
          const cached = cache[pack.path];
          if (cached && (now - cached.timestamp) < CACHE_EXPIRY_MS) {
            cachedSizes[pack.path] = { size: cached.size, formatted: cached.formatted };
          } else {
            needsRefresh.push(pack.path);
          }
        });

        setPackSizes(cachedSizes);

        if (needsRefresh.length === 0) return;

        setLoadingProgress({ loaded: folderPacks.length - needsRefresh.length, total: folderPacks.length });

        const results = await invoke<[string, number, string][]>('get_all_folder_sizes', { paths: needsRefresh });

        const newSizes: Record<string, { size: number; formatted: string }> = {};
        results.forEach(([path, size, formatted]) => {
          newSizes[path] = { size, formatted };
        });

        setPackSizes(prev => ({ ...prev, ...newSizes }));
        saveSizeCache(newSizes, cache);
        setLoadingProgress({ loaded: folderPacks.length, total: folderPacks.length });
      } catch (error) {
        console.error('Failed to load packs:', error);
        setIsLoading(false);
      }
    };

    loadPacks();
  }, []);

  const filteredPacks = useMemo(() => {
    let result = packs.filter((pack) => {
      const matchesType = selectedType === 'All' || pack.pack_type === selectedType;
      const matchesSearch = 
        pack.name.toLowerCase().includes(debouncedSearchTerm.toLowerCase()) ||
        pack.path.toLowerCase().includes(debouncedSearchTerm.toLowerCase());
      return matchesType && matchesSearch;
    });

    // Apply sorting
    result.sort((a, b) => {
      let comparison = 0;
      if (sortBy === 'name') {
        comparison = a.name.localeCompare(b.name);
      } else if (sortBy === 'size') {
        const sizeA = packSizes[a.path]?.size || 0;
        const sizeB = packSizes[b.path]?.size || 0;
        comparison = sizeA - sizeB;
      } else if (sortBy === 'type') {
        comparison = a.pack_type.localeCompare(b.pack_type);
      }
      return sortOrder === 'asc' ? comparison : -comparison;
    });

    return result;
  }, [packs, selectedType, debouncedSearchTerm, packSizes, sortBy, sortOrder]);

  const packCounts = useMemo(() => {
    const counts = {
      All: groupedPacks.length,
      BehaviorPack: 0,
      ResourcePack: 0,
      SkinPack: 0,
      WorldTemplate: 0,
      MashupPack: 0,
    };
    for (const g of groupedPacks) {
      const type = g.mainPack.pack_type;
      if (type in counts) counts[type as keyof typeof counts]++;
    }
    return counts;
  }, [groupedPacks]);

  const filteredTotalSize = useMemo(
    () => filteredGroupedPacks.reduce((sum, g) => sum + g.totalSize, 0),
    [filteredGroupedPacks]
  );

  const { parentFolderTotals, totalSize } = useMemo(() => {
    const totals = {
      BehaviorPack: 0,
      ResourcePack: 0,
      SkinPack: 0,
      WorldTemplate: 0,
      MashupPack: 0,
    };
    for (const g of groupedPacks) {
      const type = g.mainPack.pack_type;
      if (type in totals) totals[type as keyof typeof totals] += g.totalSize;
    }
    return {
      parentFolderTotals: totals,
      totalSize: Object.values(totals).reduce((sum, size) => sum + size, 0),
    };
  }, [groupedPacks]);

  // Pack management handlers
  const handleDeleteSelected = async () => {
    if (selectedPacks.size === 0) return;
    setPendingDeleteSelected(true);
  };

  const confirmDeleteSelected = async () => {
    setPendingDeleteSelected(false);
    try {
      await invoke('delete_packs', { paths: Array.from(selectedPacks) });
      setPacks(packs.filter(p => !selectedPacks.has(p.path)));
      setSelectedPacks(new Set());
    } catch (error) {
      addNotification('error', 'Delete failed', `Failed to delete packs: ${error}`);
    }
  };

  const handleExportList = () => {
    const data = filteredPacks.map(p => ({
      name: p.name,
      type: p.pack_type,
      path: p.path,
      size: packSizes[p.path]?.formatted || 'Unknown'
    }));
    
    const json = JSON.stringify(data, null, 2);
    const blob = new Blob([json], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = 'installed-packs.json';
    a.click();
    setTimeout(() => URL.revokeObjectURL(url), 100);
  };

  const handleExportCSV = () => {
    const headers = 'Name,Type,Path,Size\n';
    const rows = filteredPacks.map(p =>
      `${csvEscape(p.name)},${csvEscape(p.pack_type)},${csvEscape(p.path)},${csvEscape(packSizes[p.path]?.formatted || 'Unknown')}`
    ).join('\n');
    
    const csv = headers + rows;
    const blob = new Blob([csv], { type: 'text/csv' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = 'installed-packs.csv';
    a.click();
    setTimeout(() => URL.revokeObjectURL(url), 100);
  };

  const handleSavePreset = () => {
    const name = window.prompt('Name this filter preset:', selectedPresetName || '');
    if (!name) return;
    const newPreset: FilterPreset = { name, selectedType, sortBy, sortOrder, searchTerm };
    setFilterPresets(prev => {
      const next = [...prev.filter(p => p.name !== name), newPreset];
      saveFilterPresets(next);
      return next;
    });
    setSelectedPresetName(name);
  };

  const handleApplyPreset = (name: string) => {
    setSelectedPresetName(name);
    if (!name) return;
    const preset = filterPresets.find(p => p.name === name);
    if (!preset) return;
    setSelectedType(preset.selectedType);
    setSortBy(preset.sortBy);
    setSortOrder(preset.sortOrder);
    setSearchTerm(preset.searchTerm);
  };

  const handleDeletePreset = () => {
    if (!selectedPresetName) return;
    setFilterPresets(prev => {
      const next = prev.filter(p => p.name !== selectedPresetName);
      saveFilterPresets(next);
      return next;
    });
    setSelectedPresetName('');
  };

  const handleFindDuplicates = async () => {
    setIsDuplicateLoading(true);
    try {
      const groups = await invoke<DuplicateGroup[]>('find_duplicate_packs');
      setDuplicates(groups);
    } catch (error) {
      addNotification('error', 'Duplicate Scan Failed', `${error}`);
    } finally {
      setIsDuplicateLoading(false);
    }
  };

  const handleRemoveDuplicate = async (path: string) => {
    try {
      await invoke('delete_pack', { path });
      setPacks(prev => prev.filter(p => p.path !== path));
      setDuplicates(prev =>
        prev
          ? prev
              .map(g => ({ ...g, packs: g.packs.filter(p => p.path !== path) }))
              .filter(g => g.packs.length > 1)
          : prev
      );
    } catch (error) {
      addNotification('error', 'Delete Failed', `${error}`);
    }
  };

  /** Migrates a pack's path (and its cached folder size) after its on-disk
   * folder was renamed, without needing a full pack list reload. */
  const applyRenamedPath = (oldPath: string, newPath: string) => {
    setPacks(prev => prev.map(p => p.path === oldPath ? { ...p, path: newPath } : p));
    setPackSizes(prev => {
      if (!(oldPath in prev)) return prev;
      const next = { ...prev };
      next[newPath] = next[oldPath];
      delete next[oldPath];
      return next;
    });
  };

  const handleSuggestRenames = async () => {
    setIsRenameScanning(true);
    try {
      const suggestions = await invoke<RenameSuggestion[]>('suggest_pack_renames');
      setRenameSuggestions(suggestions);
      setSelectedRenames(new Set(suggestions.map(s => s.path)));
    } catch (error) {
      addNotification('error', 'Rename Scan Failed', `${error}`);
    } finally {
      setIsRenameScanning(false);
    }
  };

  const toggleRenameSelection = (path: string) => {
    setSelectedRenames(prev => {
      const next = new Set(prev);
      if (next.has(path)) {
        next.delete(path);
      } else {
        next.add(path);
      }
      return next;
    });
  };

  const handleApplyRenames = async () => {
    if (!renameSuggestions || selectedRenames.size === 0) return;
    setIsApplyingRenames(true);
    const toApply = renameSuggestions.filter(s => selectedRenames.has(s.path));
    try {
      const results = await invoke<RenameResult[]>('rename_installed_packs', {
        renames: toApply.map(s => ({ path: s.path, new_name: s.suggested_name })),
      });

      let successCount = 0;
      const failedPaths = new Set<string>();
      for (const result of results) {
        if (result.new_path) {
          successCount++;
          applyRenamedPath(result.path, result.new_path);
        } else {
          failedPaths.add(result.path);
        }
      }
      const failCount = results.length - successCount;

      setRenameSuggestions(prev => prev ? prev.filter(s => failedPaths.has(s.path)) : prev);
      setSelectedRenames(new Set());

      if (successCount > 0) {
        addNotification(
          'success',
          'Packs Renamed',
          `Renamed ${successCount} pack folder${successCount === 1 ? '' : 's'}.${failCount > 0 ? ` ${failCount} failed.` : ''}`
        );
      } else if (failCount > 0) {
        addNotification('error', 'Rename Failed', `Failed to rename ${failCount} pack folder${failCount === 1 ? '' : 's'}.`);
      }
    } catch (error) {
      addNotification('error', 'Rename Failed', `${error}`);
    } finally {
      setIsApplyingRenames(false);
    }
  };

  const handleRenamePack = async (pack: PackInfo) => {
    closeContextMenu();
    const currentFolderName = getFolderName(pack.path);
    const newName = window.prompt('Rename pack folder to:', currentFolderName);
    if (!newName || newName === currentFolderName) return;
    try {
      const newPath = await invoke<string>('rename_installed_pack', { path: pack.path, newName });
      applyRenamedPath(pack.path, newPath);
    } catch (error) {
      addNotification('error', 'Rename Failed', `${error}`);
    }
  };

  return (
    <>
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal modal-large" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h3>Installed Packs</h3>
          <button className="btn btn-icon" onClick={onClose}>
            <X size={20} />
          </button>
        </div>
        <div className="modal-content installed-packs-content">
          {isLoading ? (
            <div className="packs-loading-bar">
              <div className="packs-loading-label">
                {loadingTotal > 0
                  ? `Scanning packs — ${loadingCount}/${loadingTotal}`
                  : 'Scanning packs...'}
              </div>
              <div className="progress-bar-track">
                <div
                  className="progress-bar-fill"
                  style={{
                    width: loadingTotal > 0 ? `${Math.min(100, (loadingCount / loadingTotal) * 100)}%` : '15%',
                    transition: loadingTotal > 0 ? 'width 0.2s ease' : 'width 2s ease',
                  }}
                />
              </div>
            </div>
          ) : (
            <>
              {/* Filter tabs */}
              <div className="packs-filter-tabs">
                {['All', 'BehaviorPack', 'ResourcePack', 'SkinPack', 'WorldTemplate', 'MashupPack'].map((type) => (
                  <button
                    key={type}
                    className={`filter-tab ${selectedType === type ? 'active' : ''}`}
                    onClick={() => setSelectedType(type as PackType | 'All')}
                  >
                    {packTypeLabels[type]}
                    <span className="tab-count">{packCounts[type as keyof typeof packCounts]}</span>
                  </button>
                ))}
              </div>

               {/* Search */}
               <div className="packs-search">
                 <input
                   type="text"
                   placeholder="Search packs..."
                   value={searchTerm}
                   onChange={(e) => setSearchTerm(e.target.value)}
                   className="search-input"
                 />
               </div>

               {/* Sorting controls */}
               <div className="sort-controls">
                 <select 
                   value={sortBy} 
                   onChange={(e) => setSortBy(e.target.value as 'name' | 'size' | 'type')}
                   className="sort-select"
                 >
                   <option value="name">Sort by Name</option>
                   <option value="size">Sort by Size</option>
                   <option value="type">Sort by Type</option>
                 </select>
                 <button
                   className="sort-order-btn"
                   onClick={() => setSortOrder(sortOrder === 'asc' ? 'desc' : 'asc')}
                   title={sortOrder === 'asc' ? 'Ascending' : 'Descending'}
                 >
                   {sortOrder === 'asc' ? '↑' : '↓'}
                 </button>
                 <button
                   className={`view-toggle-btn ${viewMode === 'grid' ? 'active' : ''}`}
                   onClick={() => setViewMode(viewMode === 'grid' ? 'list' : 'grid')}
                   title={viewMode === 'grid' ? 'Switch to list view' : 'Switch to grid view'}
                 >
                   {viewMode === 'grid' ? '▦' : '☰'}
                 </button>
                 <select
                   value={selectedPresetName}
                   onChange={(e) => handleApplyPreset(e.target.value)}
                   className="sort-select"
                   title="Apply a saved filter preset"
                 >
                   <option value="">Filter Presets...</option>
                   {filterPresets.map(p => (
                     <option key={p.name} value={p.name}>{p.name}</option>
                   ))}
                 </select>
                 <button
                   className="sort-order-btn"
                   onClick={handleSavePreset}
                   title="Save current filters as a preset"
                 >
                   <Bookmark size={13} />
                 </button>
                 {selectedPresetName && (
                   <button
                     className="sort-order-btn"
                     onClick={handleDeletePreset}
                     title="Delete this preset"
                   >
                     <Trash2 size={13} />
                   </button>
                 )}
                 </div>

               {/* Action buttons */}
               <div className="pack-actions">
                 {selectedPacks.size > 0 && (
                   <button 
                     className="btn btn-danger btn-sm"
                     onClick={handleDeleteSelected}
                   >
                     Delete Selected ({selectedPacks.size})
                   </button>
                 )}
                 <button
                   className="btn btn-secondary btn-sm"
                   onClick={handleFindDuplicates}
                   disabled={isDuplicateLoading}
                   title="Find installed packs sharing the same UUID"
                 >
                   <AlertTriangle size={13} style={{ marginRight: 4 }} />
                   {isDuplicateLoading ? 'Scanning...' : 'Find Duplicates'}
                 </button>
                 <button
                   className="btn btn-secondary btn-sm"
                   onClick={handleSuggestRenames}
                   disabled={isRenameScanning}
                   title="Find installed pack folders with messy legacy names and suggest cleaner names"
                 >
                   <Wand2 size={13} style={{ marginRight: 4 }} />
                   {isRenameScanning ? 'Scanning...' : 'Clean Up Names'}
                 </button>
                 <button 
                   className="btn btn-secondary btn-sm"
                   onClick={handleExportCSV}
                   title="Export as CSV"
                 >
                   Export CSV
                 </button>
                 <button 
                   className="btn btn-secondary btn-sm"
                   onClick={handleExportList}
                   title="Export as JSON"
                 >
                   Export JSON
                 </button>
               </div>

                {/* Parent folder size info / All packs total */}
               {selectedType === 'All' && totalSize > 0 ? (
                 <div className="parent-folder-info all-packs-total">
                   <span className="info-label">Total all packs:</span>
                   <span className="info-value">
                     {formatBytes(totalSize)}
                   </span>
                 </div>
               ) : selectedType !== 'All' && selectedType in parentFolderTotals && parentFolderTotals[selectedType as keyof typeof parentFolderTotals] > 0 && (
                 <div className="parent-folder-info">
                   <span className="info-label">Total {packTypeLabels[selectedType]}:</span>
                   <span className="info-value">
                     {formatBytes(parentFolderTotals[selectedType as keyof typeof parentFolderTotals])}
                   </span>
                 </div>
               )}

                   {/* Packs list */}
                   {filteredGroupedPacks.length === 0 ? (
                     <div className="no-packs-found">
                       No packs found{debouncedSearchTerm && ` matching "${debouncedSearchTerm}"`}
                     </div>
                   ) : viewMode === 'grid' ? (
                     <div className="packs-grid">
                       {filteredGroupedPacks.map((group) => {
                         const isExpanded = expandedGridCard === group.mainPack.path;
                         return (
                           <PackGridCard
                             key={group.mainPack.path}
                             group={group}
                             packSizes={packSizes}
                             onContextMenu={handleContextMenu}
                             onDelete={handleDeletePack}
                             isExpanded={isExpanded}
                             onToggleExpand={() => setExpandedGridCard(isExpanded ? null : group.mainPack.path)}
                           />
                         );
                       })}
                       {/* Full-width expanded children panel — rendered outside card flow */}
                       {expandedGridCard && (() => {
                         const group = filteredGroupedPacks.find(g => g.mainPack.path === expandedGridCard);
                         if (!group) return null;
                         const childPacks: PackInfo[] = [
                           ...group.worldTemplates.filter(wt => wt.path !== group.mainPack.path),
                           ...group.resourcePacks,
                           ...group.skinPacks,
                           ...group.behaviorPacks,
                         ];
                         const skinFallbackIcon = getSkinPackFallbackIcon(group);
                         return (
                           <div className="packs-grid-children-panel">
                             <div className="packs-grid-children-header">
                               <span>{group.displayName} — related packs</span>
                               <button className="btn btn-icon" onClick={() => setExpandedGridCard(null)} title="Close"><X size={12} /></button>
                             </div>
                             <div className="packs-grid-children-list">
                               {childPacks.map(child => {
                                 const ChildIcon = getIconForPackType(child.pack_type);
                                 const childColor = PackTypeColors[child.pack_type];
                                 const isSkinPack = child.pack_type === 'SkinPack' || child.pack_type === 'SkinPack4D';
                                 const childIconSrc = child.icon_base64 ?? (isSkinPack ? skinFallbackIcon : null);
                                 return (
                                   <div key={child.path} className="packs-grid-child-row" onContextMenu={(e) => handleContextMenu(e, child)}>
                                     {childIconSrc
                                       ? <img src={childIconSrc} alt={child.name} className="packs-grid-child-thumb" />
                                       : <div className="packs-grid-child-thumb packs-grid-child-thumb-fallback" style={{ backgroundColor: `${childColor}22` }}>
                                           <ChildIcon size={16} style={{ color: childColor }} />
                                         </div>
                                     }
                                     <div className="packs-grid-child-info">
                                       <span className="packs-grid-child-name" title={child.name}>{child.name}</span>
                                       <span style={{ color: childColor, fontSize: '10px' }}>{packTypeLabels[child.pack_type]}</span>
                                     </div>
                                     <span className="packs-grid-child-size">{packSizes[child.path]?.formatted ?? ''}</span>
                                     <button className="btn btn-icon btn-delete" style={{ padding: '2px' }} onClick={(e) => { e.stopPropagation(); handleDeletePack(child); }} title="Delete"><Trash2 size={10} /></button>
                                   </div>
                                 );
                               })}
                             </div>
                           </div>
                         );
                       })()}
                     </div>
                   ) : (
                      <div className="installed-packs-list installed-packs-list-virtual">
                        {virtualRows.length > 0 && (
                          <List
                            className="installed-packs-list-scroller"
                            style={{ height: '100%', width: '100%' }}
                            rowCount={virtualRows.length}
                            rowHeight={(rowIndex) => virtualRows[rowIndex].kind === 'main' ? VIRTUAL_MAIN_ROW_HEIGHT : VIRTUAL_CHILD_ROW_HEIGHT}
                            overscanCount={6}
                            rowComponent={VirtualListRow}
                            rowProps={{
                              virtualRows,
                              packSizes,
                              expandedGroups,
                              skinFallbackIcons,
                              onToggle: toggleGroup,
                              onContextMenu: handleContextMenu,
                              onDelete: handleDeletePack,
                              getBestDisplayName,
                            }}
                          />
                        )}
                      </div>
                   )}

                {/* Summary */}
                 {filteredGroupedPacks.length > 0 && (
                   <div className="packs-summary">
                     <div className="summary-stats">
                       <span>Showing {filteredGroupedPacks.length} of {groupedPacks.length} packs</span>
                       {marketplaceStatus === 'loading' && (
                         <span className="loading-progress">Fetching marketplace icons...</span>
                       )}
                       {loadingProgress.loaded < loadingProgress.total && loadingProgress.total > 0 && (
                         <span className="loading-progress">
                           Calculating sizes: {loadingProgress.loaded}/{loadingProgress.total}
                         </span>
                       )}
                       {filteredTotalSize > 0 && (
                         <span className="total-size">
                           Total: {formatBytes(filteredTotalSize)}
                         </span>
                       )}
                     </div>
                  </div>
                 )}
             </>
           )}
         </div>

        {contextMenu && (
          <div 
            className="context-menu" 
            style={{ left: contextMenu.x, top: contextMenu.y }}
            onClick={(e) => e.stopPropagation()}
          >
            <div className="context-menu-item" onClick={() => copyToClipboard(contextMenu.pack.path)}>
              <Copy size={14} />
              Copy Path
            </div>
            {contextMenu.pack.uuid && (
              <div className="context-menu-item" onClick={() => copyToClipboard(contextMenu.pack.uuid!)}>
                <Hash size={14} />
                Copy UUID
              </div>
            )}
            <div className="context-menu-item" onClick={() => copyToClipboard(contextMenu.pack.name)}>
              <FileText size={14} />
              Copy Name
            </div>
            <div className="context-menu-item" onClick={() => handleRenamePack(contextMenu.pack)}>
              <Pencil size={14} />
              Rename...
            </div>
            <div className="context-menu-divider" />
            <div className="context-menu-item danger" onClick={() => handleDeletePack(contextMenu.pack)}>
              <Trash2 size={14} />
              Delete Pack
            </div>
          </div>
        )}

         <div className="modal-actions">
          <button className="btn btn-primary" onClick={onClose}>
            Close
          </button>
         </div>
       </div>
     </div>

     {pendingDelete && (
       <div className="modal-overlay" onClick={() => setPendingDelete(null)}>
         <div className="modal modal-small" onClick={(e) => e.stopPropagation()}>
           <div className="modal-header">
             <h3>Confirm Delete</h3>
           </div>
           <div className="modal-content">
             <p>Delete <strong>{pendingDelete.name}</strong>? This cannot be undone.</p>
           </div>
           <div className="modal-actions" style={{ gap: 8 }}>
             <button className="btn btn-secondary" onClick={() => setPendingDelete(null)}>Cancel</button>
             <button className="btn btn-danger" onClick={confirmDeletePack}>Delete</button>
           </div>
         </div>
       </div>
     )}

     {pendingDeleteSelected && (
       <div className="modal-overlay" onClick={() => setPendingDeleteSelected(false)}>
         <div className="modal modal-small" onClick={(e) => e.stopPropagation()}>
           <div className="modal-header">
             <h3>Confirm Delete</h3>
           </div>
           <div className="modal-content">
             <p>Delete {selectedPacks.size} selected pack(s)? This cannot be undone.</p>
           </div>
           <div className="modal-actions" style={{ gap: 8 }}>
             <button className="btn btn-secondary" onClick={() => setPendingDeleteSelected(false)}>Cancel</button>
             <button className="btn btn-danger" onClick={confirmDeleteSelected}>Delete</button>
           </div>
         </div>
       </div>
     )}

     {duplicates !== null && (
       <div className="modal-overlay" onClick={() => setDuplicates(null)}>
         <div className="modal modal-large" onClick={(e) => e.stopPropagation()}>
           <div className="modal-header">
             <h3>
               <AlertTriangle size={16} style={{ marginRight: 6, color: 'var(--warning-color, #f59e0b)' }} />
               Duplicate Packs
             </h3>
             <button className="btn btn-icon" onClick={() => setDuplicates(null)}><X size={20} /></button>
           </div>
           <div className="modal-content">
             {duplicates.length === 0 ? (
               <p style={{ padding: '16px 0', textAlign: 'center', color: 'var(--text-secondary)' }}>
                 No duplicates found — every installed pack is present only once.
               </p>
             ) : (
               <div className="duplicate-groups">
                 <p style={{ marginBottom: 12, color: 'var(--text-secondary)', fontSize: 13 }}>
                   Found {duplicates.length} pack{duplicates.length > 1 ? 's' : ''} installed more than once. Keep one copy and remove the rest.
                 </p>
                 {duplicates.map((group) => (
                   <div key={group.uuid} className="duplicate-group">
                     <div className="duplicate-group-header">
                       <span className="duplicate-group-name">{group.name}</span>
                       <span className="duplicate-group-uuid">{group.uuid}</span>
                     </div>
                     {group.packs.map((pack) => (
                       <div key={pack.path} className="duplicate-pack-row">
                         <div className="duplicate-pack-info">
                           <span className="duplicate-pack-type">{pack.pack_type}</span>
                           <span className="duplicate-pack-folder" title={pack.path}>{pack.folder_name}</span>
                         </div>
                         <button
                           className="btn btn-danger btn-sm"
                           onClick={() => handleRemoveDuplicate(pack.path)}
                           title={`Delete: ${pack.path}`}
                         >
                           <Trash2 size={12} style={{ marginRight: 4 }} />
                           Remove
                         </button>
                       </div>
                     ))}
                   </div>
                 ))}
               </div>
             )}
           </div>
           <div className="modal-actions">
             <button className="btn btn-primary" onClick={() => setDuplicates(null)}>Close</button>
           </div>
         </div>
       </div>
     )}

     {renameSuggestions !== null && (
       <div className="modal-overlay" onClick={() => setRenameSuggestions(null)}>
         <div className="modal modal-large" onClick={(e) => e.stopPropagation()}>
           <div className="modal-header">
             <h3>
               <Wand2 size={16} style={{ marginRight: 6, color: 'var(--primary-color)' }} />
               Clean Up Pack Names
             </h3>
             <button className="btn btn-icon" onClick={() => setRenameSuggestions(null)}><X size={20} /></button>
           </div>
           <div className="modal-content">
             {renameSuggestions.length === 0 ? (
               <p style={{ padding: '16px 0', textAlign: 'center', color: 'var(--text-secondary)' }}>
                 No messy names found — every installed pack folder already looks clean.
               </p>
             ) : (
               <div className="duplicate-groups">
                 <p style={{ marginBottom: 12, color: 'var(--text-secondary)', fontSize: 13 }}>
                   Found {renameSuggestions.length} pack folder{renameSuggestions.length > 1 ? 's' : ''} with leftover decorations from older installs. Review and apply the suggested names below.
                 </p>
                 {renameSuggestions.map((s) => (
                   <div key={s.path} className="duplicate-pack-row">
                     <label style={{ display: 'flex', alignItems: 'center', gap: 8, flex: 1, minWidth: 0, cursor: 'pointer' }}>
                       <input
                         type="checkbox"
                         checked={selectedRenames.has(s.path)}
                         onChange={() => toggleRenameSelection(s.path)}
                       />
                       <div className="duplicate-pack-info" style={{ minWidth: 0 }}>
                         <span className="duplicate-pack-type">{packTypeLabels[s.pack_type] ?? s.pack_type}</span>
                         <span className="duplicate-pack-folder" title={s.path} style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
                           <span style={{ textDecoration: 'line-through', opacity: 0.6 }}>{s.current_name}</span>
                           <span style={{ color: 'var(--primary-color)' }}>{s.suggested_name}</span>
                         </span>
                       </div>
                     </label>
                   </div>
                 ))}
               </div>
             )}
           </div>
           <div className="modal-actions" style={{ gap: 8 }}>
             <button className="btn btn-secondary" onClick={() => setRenameSuggestions(null)}>Close</button>
             {renameSuggestions.length > 0 && (
               <button
                 className="btn btn-primary"
                 onClick={handleApplyRenames}
                 disabled={selectedRenames.size === 0 || isApplyingRenames}
               >
                 {isApplyingRenames ? 'Renaming...' : `Apply Selected (${selectedRenames.size})`}
               </button>
             )}
           </div>
         </div>
       </div>
     )}
   </>
  );
}

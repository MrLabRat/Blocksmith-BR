import { useState, useEffect, useMemo, useCallback, memo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { PackInfo, PackType, PackTypeColors, AppNotification } from '../types';
import { getFolderName, cleanDisplayName, getBestDisplayName, getBaseNameForGrouping, formatBytes, getIconForPackType } from '../utils/packUtils';
import { X, Copy, Hash, FileText, Trash2 } from 'lucide-react';
import '../styles/InstalledPacksPage.css';

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


const InstalledPackIcon = memo(function InstalledPackIcon({ pack }: { pack: PackInfo }) {
  const IconComponent = getIconForPackType(pack.pack_type);
  const color = PackTypeColors[pack.pack_type];
  
  if (pack.icon_base64) {
    return (
      <img 
        src={pack.icon_base64}
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

const PackGroupItem = memo(function PackGroupItem({ 
  group, 
  isExpanded, 
  hasChildren, 
  packSizes, 
  onToggle, 
  onContextMenu,
  getBestDisplayName,
  onDelete
}: { 
  group: PackGroup; 
  isExpanded: boolean; 
  hasChildren: boolean; 
  packSizes: Record<string, { size: number; formatted: string }>;
  onToggle: () => void;
  onContextMenu: (e: React.MouseEvent, pack: PackInfo) => void;
  getBestDisplayName: (pack: PackInfo) => string;
  onDelete: (pack: PackInfo) => void;
}) {
  return (
    <div className="pack-group">
      <div 
        className={`installed-pack-card ${hasChildren ? 'has-children' : ''}`}
        onClick={() => hasChildren && onToggle()}
        onContextMenu={(e) => onContextMenu(e, group.mainPack)}
      >
        <InstalledPackIcon pack={group.mainPack} />
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
          <div className="pack-card-path" title={group.mainPack.path}>
            {group.mainPack.path}
          </div>
        </div>
        <button
          className="btn btn-icon btn-delete"
          onClick={(e) => {
            e.stopPropagation();
            onDelete(group.mainPack);
          }}
          title="Delete pack"
        >
          <Trash2 size={12} />
        </button>
      </div>
      
      {isExpanded && hasChildren && (
        <div className="pack-group-children">
          {group.worldTemplates.filter(wt => wt.path !== group.mainPack.path).map((wt) => (
            <div 
              key={wt.path}
              className="installed-pack-card child-pack"
              onContextMenu={(e) => onContextMenu(e, wt)}
            >
              <InstalledPackIcon pack={wt} />
              <div className="pack-card-content">
                <div className="pack-card-name">{getBestDisplayName(wt)}</div>
                <div className="pack-card-details">
                  <span className="pack-type" style={{ color: PackTypeColors['WorldTemplate'] }}>
                    World Template
                  </span>
                  <span className="pack-card-size">
                    {packSizes[wt.path]?.formatted || 'Unknown'}
                  </span>
                </div>
              </div>
              <button
                className="btn btn-icon btn-delete"
                onClick={(e) => {
                  e.stopPropagation();
                  onDelete(wt);
                }}
                title="Delete pack"
              >
                <Trash2 size={12} />
              </button>
            </div>
          ))}
          {group.behaviorPacks.map((bp) => (
            <div 
              key={bp.path}
              className="installed-pack-card child-pack"
              onContextMenu={(e) => onContextMenu(e, bp)}
            >
              <InstalledPackIcon pack={bp} />
              <div className="pack-card-content">
                <div className="pack-card-name">{getBestDisplayName(bp)}</div>
                <div className="pack-card-details">
                  <span className="pack-type" style={{ color: PackTypeColors['BehaviorPack'] }}>
                    Addon
                  </span>
                  <span className="pack-card-size">
                    {packSizes[bp.path]?.formatted || 'Unknown'}
                  </span>
                </div>
              </div>
              <button
                className="btn btn-icon btn-delete"
                onClick={(e) => {
                  e.stopPropagation();
                  onDelete(bp);
                }}
                title="Delete pack"
              >
                <Trash2 size={12} />
              </button>
            </div>
          ))}
          {group.resourcePacks.map((rp) => (
            <div 
              key={rp.path}
              className="installed-pack-card child-pack"
              onContextMenu={(e) => onContextMenu(e, rp)}
            >
              <InstalledPackIcon pack={rp} />
              <div className="pack-card-content">
                <div className="pack-card-name">{getBestDisplayName(rp)}</div>
                <div className="pack-card-details">
                  <span className="pack-type" style={{ color: PackTypeColors['ResourcePack'] }}>
                    Resource Pack
                  </span>
                  <span className="pack-card-size">
                    {packSizes[rp.path]?.formatted || 'Unknown'}
                  </span>
                </div>
              </div>
              <button
                className="btn btn-icon btn-delete"
                onClick={(e) => {
                  e.stopPropagation();
                  onDelete(rp);
                }}
                title="Delete pack"
              >
                <Trash2 size={12} />
              </button>
            </div>
          ))}
          {group.skinPacks.map((sp) => (
            <div 
              key={sp.path}
              className="installed-pack-card child-pack"
              onContextMenu={(e) => onContextMenu(e, sp)}
            >
              <InstalledPackIcon pack={sp} />
              <div className="pack-card-content">
                <div className="pack-card-name">{getBestDisplayName(sp)}</div>
                <div className="pack-card-details">
                  <span className="pack-type" style={{ color: PackTypeColors['SkinPack'] }}>
                    Skin Pack
                  </span>
                  <span className="pack-card-size">
                    {packSizes[sp.path]?.formatted || 'Unknown'}
                  </span>
                </div>
              </div>
              <button
                className="btn btn-icon btn-delete"
                onClick={(e) => {
                  e.stopPropagation();
                  onDelete(sp);
                }}
                title="Delete pack"
              >
                <Trash2 size={12} />
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
});


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
        const folderPacks = await invoke<PackInfo[]>('get_directory_folders');
        // Icons are now included in the response
        setPacks(folderPacks);
        setIsLoading(false);
        
        // Load cached sizes first
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
        
        // Fetch all sizes in one parallel call
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

  const packCounts = useMemo(() => ({
    All: groupedPacks.length,
    BehaviorPack: groupedPacks.filter(g => g.mainPack.pack_type === 'BehaviorPack').length,
    ResourcePack: groupedPacks.filter(g => g.mainPack.pack_type === 'ResourcePack').length,
    SkinPack: groupedPacks.filter(g => g.mainPack.pack_type === 'SkinPack').length,
    WorldTemplate: groupedPacks.filter(g => g.mainPack.pack_type === 'WorldTemplate').length,
    MashupPack: groupedPacks.filter(g => g.mainPack.pack_type === 'MashupPack').length,
  }), [groupedPacks]);

  const filteredTotalSize = useMemo(
    () => filteredGroupedPacks.reduce((sum, g) => sum + g.totalSize, 0),
    [filteredGroupedPacks]
  );

  const { parentFolderTotals, totalSize } = useMemo(() => {
    const totals = {
      BehaviorPack: groupedPacks
        .filter(g => g.mainPack.pack_type === 'BehaviorPack')
        .reduce((sum, g) => sum + g.totalSize, 0),
      ResourcePack: groupedPacks
        .filter(g => g.mainPack.pack_type === 'ResourcePack')
        .reduce((sum, g) => sum + g.totalSize, 0),
      SkinPack: groupedPacks
        .filter(g => g.mainPack.pack_type === 'SkinPack')
        .reduce((sum, g) => sum + g.totalSize, 0),
      WorldTemplate: groupedPacks
        .filter(g => g.mainPack.pack_type === 'WorldTemplate')
        .reduce((sum, g) => sum + g.totalSize, 0),
      MashupPack: groupedPacks
        .filter(g => g.mainPack.pack_type === 'MashupPack')
        .reduce((sum, g) => sum + g.totalSize, 0),
    };
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
            <div className="loading">Loading installed packs...</div>
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
                  ) : (
                    <div className="installed-packs-list">
                      {filteredGroupedPacks.map((group) => {
                        const isExpanded = expandedGroups.has(group.mainPack.path);
                        const hasChildren = group.resourcePacks.length > 0 
                          || group.skinPacks.length > 0 
                          || group.behaviorPacks.length > 0
                          || (group.worldTemplates.length > 1);
                        
                        return (
                           <PackGroupItem
                             key={group.mainPack.path}
                             group={group}
                             isExpanded={isExpanded}
                             hasChildren={hasChildren}
                             packSizes={packSizes}
                             onToggle={() => toggleGroup(group.mainPack.path)}
                             onContextMenu={handleContextMenu}
                             getBestDisplayName={getBestDisplayName}
                             onDelete={handleDeletePack}
                           />
                         );
                      })}
                    </div>
                  )}

                {/* Summary */}
                {filteredGroupedPacks.length > 0 && (
                  <div className="packs-summary">
                    <div className="summary-stats">
                      <span>Showing {filteredGroupedPacks.length} of {groupedPacks.length} packs</span>
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
   </>
  );
}

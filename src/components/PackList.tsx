import { useState, useEffect, useCallback, useMemo } from 'react';
import { PackInfo, PackTypeLabels, PackTypeColors, getPackKey } from '../types';
import { getFolderName, getBestDisplayName, getBaseNameForGrouping, getIconForPackType } from '../utils/packUtils';
import { Folder, Trash2, Box, Copy, FileText, Hash, ChevronDown, Check, Minus, RefreshCw, CheckCircle } from 'lucide-react';

interface PackListProps {
  packs: PackInfo[];
  selectedPacks: Set<string>;
  onTogglePack: (key: string) => void;
  onSelectAll: () => void;
  onDeselectAll: () => void;
  onRemove: (key: string) => void;
  onRemoveSelected: () => void;
  onDeleteFromDisk: (pack: PackInfo) => void;
  onDeleteSelectedFromDisk: () => void;
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
  worldTemplates: PackInfo[];
  displayName: string;
  isAddon: boolean;
  isMashup?: boolean;
}

function PackIcon({ pack }: { pack: PackInfo }) {
  if (pack.icon_base64) {
    return (
      <img
        src={pack.icon_base64}
        alt={pack.name}
        className="pack-icon-img"
      />
    );
  }
  
  const IconComponent = getIconForPackType(pack.pack_type);
  const color = PackTypeColors[pack.pack_type];
  
  return (
    <div className="pack-icon-default" style={{ backgroundColor: `${color}20` }}>
      <IconComponent size={24} style={{ color }} />
    </div>
  );
}


function InstallStatusBadge({ pack }: { pack: PackInfo }) {
  if (pack.is_update) {
    return (
      <span className="pack-status-badge update" title={`Update available (installed: v${pack.installed_version || '?'})`}>
        <RefreshCw size={10} />
        Update
      </span>
    );
  }
  if (pack.is_installed) {
    return (
      <span className="pack-status-badge installed" title="Already installed">
        <CheckCircle size={10} />
        Installed
      </span>
    );
  }
  return null;
}



function CustomCheckbox({ checked, indeterminate, onChange }: { checked: boolean; indeterminate?: boolean; onChange: () => void }) {
  return (
    <div
      className={`custom-checkbox ${checked ? 'checked' : ''} ${indeterminate ? 'indeterminate' : ''}`}
      onClick={(e) => {
        e.stopPropagation();
        onChange();
      }}
    >
      {checked && <Check size={12} />}
      {indeterminate && <Minus size={12} />}
    </div>
  );
}

export function PackList({
  packs,
  selectedPacks,
  onTogglePack,
  onSelectAll,
  onDeselectAll,
  onRemove,
  onRemoveSelected,
  onDeleteFromDisk,
  onDeleteSelectedFromDisk,
}: PackListProps) {
  const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);
  const [expandedGroups, setExpandedGroups] = useState<Set<string>>(new Set());

  const groupedPacks = useMemo(() => {
    const groups: PackGroup[] = [];
    const processedKeys = new Set<string>();
    
    // Group by base name (for mash-up packs and multi-pack archives)
    const baseNameGroups = new Map<string, PackInfo[]>();
    for (const pack of packs) {
      const baseName = getBaseNameForGrouping(getFolderName(pack.path));
      if (!baseNameGroups.has(baseName)) {
        baseNameGroups.set(baseName, []);
      }
      baseNameGroups.get(baseName)!.push(pack);
    }
    
    // Process each base name group
    for (const [, groupPacks] of baseNameGroups) {
      const behaviorPacks = groupPacks.filter(p => p.pack_type === 'BehaviorPack');
      const resourcePacks = groupPacks.filter(p => p.pack_type === 'ResourcePack');
      const skinPacks = groupPacks.filter(p => p.pack_type === 'SkinPack' || p.pack_type === 'SkinPack4D');
      const worldTemplates = groupPacks.filter(p => p.pack_type === 'WorldTemplate');
      
      // Determine if this is a mash-up pack (has multiple pack types)
      const hasMultipleTypes = [behaviorPacks.length > 0, resourcePacks.length > 0, skinPacks.length > 0, worldTemplates.length > 0].filter(Boolean).length > 1;
      
      // Determine main pack - prefer behavior pack, then world template, then resource pack
      let mainPack: PackInfo | null = behaviorPacks[0] || worldTemplates[0] || resourcePacks[0] || skinPacks[0];
      
      if (mainPack) {
        const mainKey = getPackKey(mainPack);
        if (!processedKeys.has(mainKey)) {
          processedKeys.add(mainKey);
          
          // Collect all other packs as children
          const allPacks = [...behaviorPacks, ...resourcePacks, ...skinPacks, ...worldTemplates];
          const childRPs: PackInfo[] = [];
          const childSPs: PackInfo[] = [];
          const childWTs: PackInfo[] = [];
          
          for (const p of allPacks) {
            const key = getPackKey(p);
            if (!processedKeys.has(key) && p !== mainPack) {
              processedKeys.add(key);
              if (p.pack_type === 'ResourcePack') childRPs.push(p);
              else if (p.pack_type === 'SkinPack' || p.pack_type === 'SkinPack4D') childSPs.push(p);
              else if (p.pack_type === 'WorldTemplate') childWTs.push(p);
            }
          }
          
          groups.push({
            mainPack,
            resourcePacks: mainPack.pack_type === 'ResourcePack' ? [] : childRPs,
            skinPacks: mainPack.pack_type === 'SkinPack' || mainPack.pack_type === 'SkinPack4D' ? [] : childSPs,
            worldTemplates: mainPack.pack_type === 'WorldTemplate' ? [] : childWTs,
            displayName: getBestDisplayName(mainPack),
            isAddon: behaviorPacks.length > 0 && resourcePacks.length > 0,
            isMashup: hasMultipleTypes && worldTemplates.length > 0,
          });
        }
      }
    }
    
    return groups;
  }, [packs]);

  const toggleGroup = useCallback((key: string) => {
    setExpandedGroups(prev => {
      const next = new Set(prev);
      if (next.has(key)) {
        next.delete(key);
      } else {
        next.add(key);
      }
      return next;
    });
  }, []);

  const handleGroupToggle = useCallback((group: PackGroup) => {
    const allKeys = [
      getPackKey(group.mainPack),
      ...group.resourcePacks.map(getPackKey),
      ...group.skinPacks.map(getPackKey),
      ...group.worldTemplates.map(getPackKey),
    ];
    
    const allSelected = allKeys.every(k => selectedPacks.has(k));
    
    if (allSelected) {
      allKeys.forEach(k => {
        if (selectedPacks.has(k)) onTogglePack(k);
      });
    } else {
      allKeys.forEach(k => {
        if (!selectedPacks.has(k)) onTogglePack(k);
      });
    }
  }, [selectedPacks, onTogglePack]);

  const handleContextMenu = useCallback((e: React.MouseEvent, pack: PackInfo) => {
    e.preventDefault();
    e.stopPropagation();
    setContextMenu({ x: e.clientX, y: e.clientY, pack });
  }, []);

  const closeContextMenu = useCallback(() => {
    setContextMenu(null);
  }, []);

  useEffect(() => {
    const handleClick = () => closeContextMenu();
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === 'Escape') closeContextMenu();
    };
    document.addEventListener('click', handleClick);
    document.addEventListener('keydown', handleEscape);
    return () => {
      document.removeEventListener('click', handleClick);
      document.removeEventListener('keydown', handleEscape);
    };
  }, [closeContextMenu]);

  const copyToClipboard = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
    } catch (err) {
      console.error('Failed to copy:', err);
    }
    closeContextMenu();
  };

  const handleDeletePack = (pack: PackInfo) => {
    const key = getPackKey(pack);
    if (selectedPacks.size > 1 && selectedPacks.has(key)) {
      onRemoveSelected();
    } else {
      onRemove(key);
    }
    closeContextMenu();
  };

  if (packs.length === 0) {
    return (
      <div className="pack-list-empty">
        <Box size={48} />
        <p>No pack files found.</p>
        <p className="hint">Scan a directory containing .mcpack, .mcaddon, or .mctemplate files</p>
      </div>
    );
  }

  const groupCount = groupedPacks.length;

  return (
    <div className="pack-list">
      <div className="pack-list-header">
        <h3>Found Packs ({groupCount} groups, {packs.length} total)</h3>
        <div className="pack-list-actions">
          <button className="btn btn-small" onClick={onSelectAll}>
            Select All
          </button>
          <button className="btn btn-small" onClick={onDeselectAll}>
            Deselect All
          </button>
          {selectedPacks.size > 0 && (
            <button
              className="btn btn-small btn-danger"
              onClick={(e) => {
                if (e.shiftKey) {
                  onDeleteSelectedFromDisk();
                } else {
                  onRemoveSelected();
                }
              }}
              data-tooltip={`Remove selected from list\nShift+Click to delete files from disk`}
            >
              <Trash2 size={14} />
              Remove ({selectedPacks.size})
            </button>
          )}
        </div>
      </div>
      <div className="pack-items">
        {groupedPacks.map((group) => {
          const mainKey = getPackKey(group.mainPack);
          const isExpanded = expandedGroups.has(mainKey);
          const hasChildren = group.resourcePacks.length > 0 || group.skinPacks.length > 0 || group.worldTemplates.length > 0;
          
          const allKeys = [
            mainKey,
            ...group.resourcePacks.map(getPackKey),
            ...group.skinPacks.map(getPackKey),
            ...group.worldTemplates.map(getPackKey),
          ];
          const allSelected = allKeys.every(k => selectedPacks.has(k));
          const someSelected = allKeys.some(k => selectedPacks.has(k));
          
          return (
            <div key={mainKey} className="pack-group">
              <div
                className={`pack-item ${someSelected ? 'selected' : ''} ${hasChildren ? 'has-children' : ''}`}
                onClick={() => hasChildren ? toggleGroup(mainKey) : onTogglePack(mainKey)}
                onContextMenu={(e) => handleContextMenu(e, group.mainPack)}
              >
                <CustomCheckbox 
                  checked={allSelected} 
                  indeterminate={someSelected && !allSelected}
                  onChange={() => handleGroupToggle(group)}
                />
                <PackIcon pack={group.mainPack} />
                <div className="pack-info">
                  <div className="pack-name">
                    {group.displayName}
                    <InstallStatusBadge pack={group.mainPack} />
                    {hasChildren && (
                      <span className="expand-indicator">
                        <ChevronDown 
                          size={16} 
                          style={{ 
                            transform: isExpanded ? 'rotate(0deg)' : 'rotate(-90deg)',
                            transition: 'transform 0.2s ease'
                          }} 
                        />
                      </span>
                    )}
                  </div>
                  <div className="pack-details">
                    <span className="pack-type" style={{ color: PackTypeColors[group.mainPack.pack_type] }}>
                      {PackTypeLabels[group.mainPack.pack_type]}
                    </span>
                    {hasChildren && (
                      <span className="pack-group-count">
                        +{group.resourcePacks.length + group.skinPacks.length + group.worldTemplates.length} parts
                      </span>
                    )}
                    {group.mainPack.uuid && <span className="pack-uuid">UUID: {group.mainPack.uuid.slice(0, 8)}...</span>}
                  </div>
                  <div className="pack-path">
                    <Folder size={12} />
                    {group.mainPack.path}
                    {group.mainPack.subfolder && <span className="pack-subfolder"> / {group.mainPack.subfolder}</span>}
                  </div>
                </div>
                <button
                  className="btn btn-icon btn-danger"
                  onClick={(e) => {
                    e.stopPropagation();
                    if (e.shiftKey) {
                      if (selectedPacks.has(mainKey) && selectedPacks.size > 1) {
                        onDeleteSelectedFromDisk();
                      } else {
                        onDeleteFromDisk(group.mainPack);
                      }
                    } else {
                      onRemove(mainKey);
                    }
                  }}
                  data-tooltip={`Remove from list\nShift+Click to delete file from disk`}
                >
                  <Trash2 size={16} />
                </button>
              </div>
              
              {isExpanded && hasChildren && (
                <div className="pack-group-children">
                  {group.resourcePacks.map((rp) => {
                    const rpKey = getPackKey(rp);
                    return (
                      <div
                        key={rpKey}
                        className={`pack-item child-pack ${selectedPacks.has(rpKey) ? 'selected' : ''}`}
                        onClick={() => onTogglePack(rpKey)}
                        onContextMenu={(e) => handleContextMenu(e, rp)}
                      >
                        <CustomCheckbox 
                          checked={selectedPacks.has(rpKey)}
                          onChange={() => onTogglePack(rpKey)}
                        />
                        <PackIcon pack={rp} />
                        <div className="pack-info">
                          <div className="pack-name">{getBestDisplayName(rp)}</div>
                          <div className="pack-details">
                            <span className="pack-type" style={{ color: PackTypeColors['ResourcePack'] }}>
                              Resource Pack
                            </span>
                          </div>
                          <div className="pack-path">
                            <Folder size={12} />
                            {rp.path}
                            {rp.subfolder && <span className="pack-subfolder"> / {rp.subfolder}</span>}
                          </div>
                        </div>
                        <button
                          className="btn btn-icon btn-danger"
                          onClick={(e) => {
                            e.stopPropagation();
                            if (e.shiftKey) {
                              if (selectedPacks.has(rpKey) && selectedPacks.size > 1) {
                                onDeleteSelectedFromDisk();
                              } else {
                                onDeleteFromDisk(rp);
                              }
                            } else {
                              onRemove(rpKey);
                            }
                          }}
                          data-tooltip={`Remove from list\nShift+Click to delete file from disk`}
                        >
                          <Trash2 size={16} />
                        </button>
                      </div>
                    );
                  })}
                  {group.skinPacks.map((sp) => {
                    const spKey = getPackKey(sp);
                    return (
                      <div
                        key={spKey}
                        className={`pack-item child-pack ${selectedPacks.has(spKey) ? 'selected' : ''}`}
                        onClick={() => onTogglePack(spKey)}
                        onContextMenu={(e) => handleContextMenu(e, sp)}
                      >
                        <CustomCheckbox 
                          checked={selectedPacks.has(spKey)}
                          onChange={() => onTogglePack(spKey)}
                        />
                        <PackIcon pack={sp} />
                        <div className="pack-info">
                          <div className="pack-name">{getBestDisplayName(sp)}</div>
                          <div className="pack-details">
                            <span className="pack-type" style={{ color: PackTypeColors[sp.pack_type] }}>
                              {PackTypeLabels[sp.pack_type]}
                            </span>
                          </div>
                          <div className="pack-path">
                            <Folder size={12} />
                            {sp.path}
                            {sp.subfolder && <span className="pack-subfolder"> / {sp.subfolder}</span>}
                          </div>
                        </div>
                        <button
                          className="btn btn-icon btn-danger"
                          onClick={(e) => {
                            e.stopPropagation();
                            if (e.shiftKey) {
                              if (selectedPacks.has(spKey) && selectedPacks.size > 1) {
                                onDeleteSelectedFromDisk();
                              } else {
                                onDeleteFromDisk(sp);
                              }
                            } else {
                              onRemove(spKey);
                            }
                          }}
                          data-tooltip={`Remove from list\nShift+Click to delete file from disk`}
                        >
                          <Trash2 size={16} />
                        </button>
                      </div>
                    );
                  })}
                  {group.worldTemplates.map((wt) => {
                    const wtKey = getPackKey(wt);
                    return (
                      <div
                        key={wtKey}
                        className={`pack-item child-pack ${selectedPacks.has(wtKey) ? 'selected' : ''}`}
                        onClick={() => onTogglePack(wtKey)}
                        onContextMenu={(e) => handleContextMenu(e, wt)}
                      >
                        <CustomCheckbox 
                          checked={selectedPacks.has(wtKey)}
                          onChange={() => onTogglePack(wtKey)}
                        />
                        <PackIcon pack={wt} />
                        <div className="pack-info">
                          <div className="pack-name">{getBestDisplayName(wt)}</div>
                          <div className="pack-details">
                            <span className="pack-type" style={{ color: PackTypeColors['WorldTemplate'] }}>
                              World Template
                            </span>
                          </div>
                          <div className="pack-path">
                            <Folder size={12} />
                            {wt.path}
                            {wt.subfolder && <span className="pack-subfolder"> / {wt.subfolder}</span>}
                          </div>
                        </div>
                        <button
                          className="btn btn-icon btn-danger"
                          onClick={(e) => {
                            e.stopPropagation();
                            if (e.shiftKey) {
                              if (selectedPacks.has(wtKey) && selectedPacks.size > 1) {
                                onDeleteSelectedFromDisk();
                              } else {
                                onDeleteFromDisk(wt);
                              }
                            } else {
                              onRemove(wtKey);
                            }
                          }}
                          data-tooltip={`Remove from list\nShift+Click to delete file from disk`}
                        >
                          <Trash2 size={16} />
                        </button>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
          );
        })}
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
            Remove from List
          </div>
          <div className="context-menu-item danger" onClick={() => { onDeleteFromDisk(contextMenu.pack); closeContextMenu(); }}>
            <Trash2 size={14} />
            Delete File from Disk
          </div>
        </div>
      )}
    </div>
  );
}

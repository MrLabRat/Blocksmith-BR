import { useState, useEffect, useCallback, useRef, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { save } from '@tauri-apps/plugin-dialog';
import { PackInfo, Settings as SettingsType, LogEntry, ProgressEvent, MoveOperation, getPackKey, PackType, AppNotification } from './types';
import { getBestDisplayName } from './utils/packUtils';
import { AnimatedLogViewer } from './components/AnimatedLogViewer';
import { PackList } from './components/PackList';
import { Settings, SettingsButton } from './components/Settings';
import { ScanControls } from './components/ScanControls';
import { HamburgerMenu } from './components/HamburgerMenu';
import { StatisticsPage } from './components/StatisticsPage';
import { InstalledPacksPage } from './components/InstalledPacksPage';
import { HelpPage } from './components/HelpPage';
import { RecycleBinPage } from './components/RecycleBinPage';
import { AnimatedBackground } from './components/AnimatedBackground';
import { Notifications, createNotification } from './components/Notifications';
import { ConfirmDialog } from './components/ConfirmDialog';
import { Package, X, CheckCircle, XCircle, FolderOpen, ExternalLink, Copy, ChevronDown, Globe, Eye, Download } from 'lucide-react';
import './App.css';

const friendlyLogMessages = [
  "Mining away at those packs...",
  "Crafting something awesome...",
  "Building your pack collection...",
  "Enchanting your experience...",
  "Smelting through the data...",
  "Exploring new lands...",
  "Gathering resources...",
  "Fighting off the creepers...",
  "No Iron Golems were harmed in this process!",
  "Just like redstone, everything is connected...",
];

const tipMessages = [
  "Hold Shift and click the trash icon to delete a pack file from disk.",
  "Ctrl + mouse wheel adjusts the UI scale.",
  "Use Dry Run to preview pack moves safely.",
  "4D skin packs are extracted to the 4D Skin Packs folder.",
  "Update packs are auto-replaced before extraction.",
  "Right-click any pack to copy its path, UUID, or name.",
  "Press Ctrl+A to select all visible packs at once.",
  "Press Delete to remove selected packs from the list.",
  "Press Escape to deselect all packs.",
  "Use the search bar to filter packs by name, path, or type.",
  "The pack type dropdown lets you view only one type at a time.",
  "Behavior packs and resource packs from the same file are grouped together.",
  "Press Ctrl+0 to reset the UI scale to 100%.",
  "Installed packs show a green badge — orange means an update is available.",
  "You can scan multiple times — rescanning replaces the current pack list.",
  "Mash-up packs contain a world template, resource pack, and behavior pack.",
  "Check the Statistics page for a breakdown of your pack collection sizes.",
  "The Installed Packs page lets you browse and delete packs already in Minecraft.",
  "Dry Run logs show exactly what would happen without touching any files.",
  "Debug Mode in Settings shows detailed logs for troubleshooting.",
  "The 'Delete Source' toggle removes the original .mcpack or .mcaddon after a successful extraction.",
  "Dry Run mode shows exactly what would happen without touching any files — useful before bulk installs.",
  "Installed packs can be deleted from the Installed Packs page — hold Shift to skip the confirmation.",
  "A .mcaddon file can contain both a behaviour pack and a resource pack — Blocksmith splits them automatically.",
  "World templates bundle their internal packs as a single unit and won't show duplicate BP/RP entries.",
  "The Statistics page breaks down total disk usage by pack type.",
  "You can open the destination folder directly from the results modal after an extraction.",
  "Ctrl+Z rolls back the last extraction if you change your mind.",
  "Mash-up packs are detected by their filename containing 'mashup' or by bundling a WT, RP, and BP together.",
  "Blocksmith detects 4D skin packs automatically by looking for geometry.json inside the archive.",
  "Version numbers are stripped from pack names when comparing installed versions, so '1.8' and '1.8.1' match correctly.",
  "The orange 'Update' badge means Blocksmith found the same pack installed but with a different version.",
];

const packTypes: (PackType | 'All')[] = ['All', 'BehaviorPack', 'ResourcePack', 'SkinPack', 'SkinPack4D', 'WorldTemplate'];
const packTypeLabels: Record<string, string> = {
  'All': 'All Packs',
  'BehaviorPack': 'Behavior Packs',
  'ResourcePack': 'Resource Packs',
  'SkinPack': 'Skin Packs',
  'SkinPack4D': 'Skin Packs (4D)',
  'WorldTemplate': 'World Templates',
};

const MAX_LOG_ENTRIES = 500;

function App() {
  const handleMinimize = async () => {
    try {
      await invoke('minimize_window');
    } catch (e) {
      console.error('Failed to minimize:', e);
    }
  };

  const handleMaximize = async () => {
    try {
      await invoke('maximize_window');
    } catch (e) {
      console.error('Failed to toggle maximize:', e);
    }
  };

  const handleClose = async () => {
    try {
      await invoke('close_window');
    } catch (e) {
      console.error('Failed to close:', e);
    }
  };
  // Core pack data
  const [packs, setPacks] = useState<PackInfo[]>([]);
  const [selectedPacks, setSelectedPacks] = useState<Set<string>>(new Set());
  const [settings, setSettings] = useState<SettingsType>({ dry_run: false, delete_source: false, delete_file_on_remove: true });
  const debugModeRef = useRef(false);
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [isScanning, setIsScanning] = useState(false);
  const [isMoving, setIsMoving] = useState(false);
  const [progress, setProgress] = useState<ProgressEvent | null>(null);
  const [searchFilter, setSearchFilter] = useState('');
  const [results, setResults] = useState<MoveOperation[] | null>(null);
  const [notifications, setNotifications] = useState<AppNotification[]>([]);
  
  // UI state
  const [showSettings, setShowSettings] = useState(false);
  const [showStats, setShowStats] = useState(false);
  const [showInstalledPacks, setShowInstalledPacks] = useState(false);
  const [showHelp, setShowHelp] = useState(false);
  const [showRecycleBin, setShowRecycleBin] = useState(false);
  const [logPanelOpen, setLogPanelOpen] = useState(true);
  const [selectedPackType, setSelectedPackType] = useState<PackType | 'All'>('All');
  const [packTypeDropdownOpen, setPackTypeDropdownOpen] = useState(false);
  const [copiedPath, setCopiedPath] = useState<string | null>(null);
  const [toolcoinInstalled, setToolcoinInstalled] = useState(false);
  const [confirmState, setConfirmState] = useState<{ title: string; message: string; detail?: string; confirmLabel?: string; onConfirm: () => void; onCancel?: () => void } | null>(null);
  const [showExportDropdown, setShowExportDropdown] = useState(false);
  const [previewPack, setPreviewPack] = useState<{ pack: { path: string; name: string; subfolder?: string } } | null>(null);
  const [previewFiles, setPreviewFiles] = useState<string[]>([]);
  const [previewLoading, setPreviewLoading] = useState(false);
  const [newPackPaths, setNewPackPaths] = useState<Set<string>>(new Set());
  const packsRef = useRef<PackInfo[]>([]);
  const newFilesDebounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const autoScanInFlightRef = useRef(false);
  const newPackNamesRef = useRef<Map<string, string>>(new Map()); // path → display name, accumulated
  const newPacksNotifIdRef = useRef<string | null>(null);

  const addNotification = useCallback((type: AppNotification['type'], title: string, message: string) => {
    setNotifications(prev => [...prev, createNotification(type, title, message)]);
  }, []);

  const addTipNotification = useCallback((message: string) => {
    if (settings.disable_tip_notifications) return;
    setNotifications(prev => [...prev, createNotification('info', 'Tip', message)]);
  }, [settings.disable_tip_notifications]);

  const dismissNotification = useCallback((id: string) => {
    setNotifications(prev => prev.filter(n => n.id !== id));
  }, []);

  // Initialize listeners
  useEffect(() => {
    const loadSavedSettings = async () => {
      try {
        const saved = await invoke<SettingsType>('load_settings');
        setSettings(saved);
        debugModeRef.current = !!saved.debug_mode;
        
        // Apply saved taskbar icon on startup
        try {
          await invoke('set_window_icon', { 
            style: saved.taskbar_icon_style || 'blackred', 
            bordered: saved.taskbar_icon_border !== false 
          });
        } catch (e) {
          console.error('Failed to set window icon:', e);
        }
        
        // Auto-scale for high DPI displays if no user preference
        if (saved.ui_scale === undefined) {
          const dpr = window.devicePixelRatio;
          const screenWidth = window.screen.width;
          
          // Auto-scale for high DPI or large screens
          if (dpr >= 2 || screenWidth >= 3840) {
            const autoScale = dpr >= 2 ? 130 : 115;
            handleSettingsChange({ ...saved, ui_scale: autoScale });
          }
        }
      } catch (error) {
        console.error('Failed to load settings:', error);
      }
    };

    const checkToolcoin = async () => {
      try {
        const installed = await invoke<boolean>('check_toolcoin_installed');
        setToolcoinInstalled(installed);
      } catch (error) {
        console.error('Failed to check ToolCoin:', error);
      }
    };

    loadSavedSettings();
    checkToolcoin();

    const unlistenLog = listen<LogEntry>('log', (event) => {
      const log = event.payload;
      setLogs((prev) => [...prev.slice(-(MAX_LOG_ENTRIES - 1)), log]);
      
      // Show notification for errors when debug mode is off
      if (log.level === 'ERROR' && !debugModeRef.current) {
        addNotification('error', 'Something went wrong!', 
          'An error occurred. Enable Debug Mode in Settings to see details, or check Help & Feedback.');
      }
    });

    const unlistenProgress = listen<ProgressEvent>('progress', (event) => {
      setProgress(event.payload);
    });

    return () => {
      unlistenLog.then((fn) => fn());
      unlistenProgress.then((fn) => fn());
    };
  }, []);

  // Close dropdowns when clicking outside
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      const target = e.target as HTMLElement;
      if (!target.closest('.pack-type-dropdown')) {
        document.querySelectorAll('.pack-type-dropdown-menu.open').forEach((menu) => {
          menu.classList.remove('open');
        });
      }
      if (!target.closest('.export-dropdown-wrapper')) {
        setShowExportDropdown(false);
      }
    };
    window.addEventListener('click', handleClickOutside);
    return () => window.removeEventListener('click', handleClickOutside);
  }, []);

  // Pack selection handlers
  const handleTogglePack = useCallback((key: string) => {
    setSelectedPacks((prev) => {
      const next = new Set(prev);
      if (next.has(key)) {
        next.delete(key);
      } else {
        next.add(key);
      }
      return next;
    });
  }, []);

  const handleDeselectAll = useCallback(() => {
    setSelectedPacks(new Set());
  }, []);

  const removeSelectedPacks = useCallback(() => {
    setPacks((prev) => prev.filter((p) => !selectedPacks.has(getPackKey(p))));
    setSelectedPacks(new Set());
  }, [selectedPacks]);

  // Detects when the currently selected packs contain multiple different versions
  // of the same pack (matched by UUID). Processing both would just overwrite one
  // with the other, so we warn and let the user confirm before continuing.
  const handleBeforeProcess = useCallback((selectedPacksList: PackInfo[]): Promise<boolean> => {
    const byUuid = new Map<string, PackInfo[]>();
    for (const p of selectedPacksList) {
      if (!p.uuid) continue;
      if (!byUuid.has(p.uuid)) byUuid.set(p.uuid, []);
      byUuid.get(p.uuid)!.push(p);
    }

    const conflicts = Array.from(byUuid.values()).filter((group) => {
      if (group.length < 2) return false;
      const versions = new Set(group.map((p) => p.version || 'unknown'));
      return versions.size > 1;
    });

    if (conflicts.length === 0) {
      return Promise.resolve(true);
    }

    return new Promise<boolean>((resolve) => {
      const detail = conflicts
        .map((group) => `${group[0].name}: ${group.map((p) => p.version || 'unknown').join(' vs ')}`)
        .join('\n');
      setConfirmState({
        title: 'Conflicting Pack Versions Selected',
        message: `${conflicts.length} selected pack${conflicts.length > 1 ? 's share' : ' shares'} the same UUID with a different version selected at the same time. Processing both may cause one to overwrite the other.`,
        detail,
        confirmLabel: 'Continue Anyway',
        onConfirm: () => {
          setConfirmState(null);
          resolve(true);
        },
        onCancel: () => resolve(false),
      });
    });
  }, []);

  const handleDeleteFromDisk = useCallback((pack: PackInfo) => {
    setConfirmState({
      title: 'Delete File from Disk',
      message: `Permanently delete "${pack.name}"? This cannot be undone.`,
      detail: pack.path,
      onConfirm: async () => {
        setConfirmState(null);
        try {
          await invoke('delete_source_file', { path: pack.path });
          const key = getPackKey(pack);
          setPacks((prev) => prev.filter((p) => getPackKey(p) !== key));
          setSelectedPacks((prev) => {
            const next = new Set(prev);
            next.delete(key);
            return next;
          });
          addNotification('success', 'File Deleted', `"${pack.name}" has been deleted from disk.`);
        } catch (error) {
          addNotification('error', 'Delete Failed', `Could not delete file: ${error}`);
        }
      },
    });
  }, [addNotification]);

  const handleDeleteSelectedFromDisk = useCallback(() => {
    const selectedList = packs.filter((p) => selectedPacks.has(getPackKey(p)));
    if (selectedList.length === 0) return;
    const paths = selectedList.map((p) => p.path).join('\n');
    setConfirmState({
      title: `Delete ${selectedList.length} File${selectedList.length > 1 ? 's' : ''} from Disk`,
      message: `Permanently delete ${selectedList.length} selected file${selectedList.length > 1 ? 's' : ''}? This cannot be undone.`,
      detail: paths,
      onConfirm: async () => {
        setConfirmState(null);
        // Multiple selected packs (e.g. the Behavior Pack + Resource Pack halves of
        // the same .mcaddon) can share the same underlying source file path -- only
        // attempt to delete each unique file once, or the 2nd attempt fails with
        // "File does not exist" since the 1st attempt already removed it.
        const uniquePaths = Array.from(new Set(selectedList.map((p) => p.path)));
        const failedPaths = new Set<string>();
        const errors: string[] = [];
        for (const path of uniquePaths) {
          try {
            await invoke('delete_source_file', { path });
          } catch (error) {
            failedPaths.add(path);
            const packName = selectedList.find((p) => p.path === path)?.name || path;
            errors.push(`${packName}: ${error}`);
          }
        }
        const deletedKeys = new Set(
          selectedList
            .filter((p) => !failedPaths.has(p.path))
            .map(getPackKey)
        );
        setPacks((prev) => prev.filter((p) => !deletedKeys.has(getPackKey(p))));
        setSelectedPacks((prev) => {
          const next = new Set(prev);
          deletedKeys.forEach((k) => next.delete(k));
          return next;
        });
        if (errors.length > 0) {
          addNotification('error', 'Some Deletions Failed', errors.join('\n'));
        } else {
          addNotification('success', 'Files Deleted', `${uniquePaths.length} file${uniquePaths.length > 1 ? 's' : ''} deleted from disk.`);
        }
      },
    });
  }, [packs, selectedPacks, addNotification]);

  const handleRemovePack = useCallback((key: string) => {
    setPacks((prev) => prev.filter((p) => getPackKey(p) !== key));
    setSelectedPacks((prev) => {
      const next = new Set(prev);
      next.delete(key);
      return next;
    });
  }, []);

  // Log handlers
  const handleClearLogs = useCallback(() => {
    setLogs([]);
  }, []);

  // UI Scale handlers
  const adjustScale = useCallback(async (delta: number) => {
    const currentScale = settings.ui_scale || 100;
    const newScale = Math.max(85, Math.min(200, Math.round((currentScale + delta) / 15) * 15));
    
    if (newScale === currentScale) return;
    
    setSettings(prev => ({ ...prev, ui_scale: newScale }));
    
    try {
      await invoke('save_ui_scale', { scale: newScale });
    } catch (e) {
      console.error('Failed to save UI scale:', e);
    }
  }, [settings.ui_scale]);

  const resetScale = useCallback(async () => {
    setSettings(prev => ({ ...prev, ui_scale: 100 }));
    
    try {
      await invoke('save_ui_scale', { scale: 100 });
    } catch (e) {
      console.error('Failed to save UI scale:', e);
    }
  }, []);

  // Scan/Move handlers
  const handleScanStart = useCallback(() => {
    setIsScanning(true);
    setProgress(null);
  }, []);

  const handleScanComplete = useCallback((newPacks: PackInfo[]) => {
    setPacks(newPacks);
    setSelectedPacks(new Set());
    setNewPackPaths(new Set());
    newPackNamesRef.current.clear();
    newPacksNotifIdRef.current = null;
    setProgress(null);
    setIsScanning(false);
    if (newPacks.length > 0) {
      invoke<PackInfo[]>('compute_pack_status', { packs: newPacks })
        .then((updated) => setPacks(updated))
        .catch((error) => console.error('Status check failed:', error));
    }
  }, []);

  const handleMoveStart = useCallback(() => {
    setIsMoving(true);
    setProgress(null);
    setResults(null);
  }, []);

  const handleMoveComplete = useCallback((ops?: MoveOperation[]) => {
    setIsMoving(false);
    setProgress(null);
    if (ops) {
      setResults(ops);
      // Auto-copy 4D skin paths to clipboard
      const fourDSkinPacks = ops.filter((r) => r.pack_type === 'SkinPack4D' && r.success);
      if (fourDSkinPacks.length > 0) {
        const path = fourDSkinPacks[0].destination.replace(/ \(4D SKIN\)$/, '');
        navigator.clipboard.writeText(path).catch(() => {});
      }
    }
  }, []);

  const handleSettingsChange = useCallback((newSettings: SettingsType) => {
    setSettings(newSettings);
    debugModeRef.current = !!newSettings.debug_mode;
  }, []);

  // File/Folder handlers
  const handleOpenFolder = useCallback(async (path: string) => {
    try {
      await invoke('open_folder', { path });
    } catch (error) {
      console.error('Failed to open folder:', error);
    }
  }, []);

  const handleCopyPath = useCallback(async (path: string) => {
    try {
      await navigator.clipboard.writeText(path);
      setCopiedPath(path);
      setTimeout(() => setCopiedPath(null), 2000);
    } catch (error) {
      console.error('Failed to copy to clipboard:', error);
    }
  }, []);

  // Pack preview
  const handlePreviewPack = useCallback(async (pack: PackInfo) => {
    setPreviewPack({ pack: { path: pack.path, name: pack.name, subfolder: pack.subfolder } });
    setPreviewFiles([]);
    setPreviewLoading(true);
    try {
      const files = await invoke<string[]>('list_archive_files', {
        path: pack.path,
        subfolder: pack.subfolder ?? null,
        nestedMcpack: pack.nested_mcpack ?? null,
      });
      setPreviewFiles(files);
    } catch (err) {
      addNotification('error', 'Preview Failed', `${err}`);
      setPreviewPack(null);
    } finally {
      setPreviewLoading(false);
    }
  }, [addNotification]);

  // 4D skin handling
  const handleOpenSkinMaster = async () => {
    try {
      await invoke('open_skinmaster');
    } catch (error) {
      console.error('Failed to open SkinMaster:', error);
    }
  };

  // Launch handlers
  const handleLaunchMinecraft = async () => {
    try {
      await invoke('launch_minecraft');
    } catch (error) {
      console.error('Failed to launch Minecraft:', error);
    }
  };

  const handleLaunchToolCoin = async () => {
    try {
      await invoke('launch_toolcoin');
    } catch (error) {
      addNotification('error', 'ToolCoin Not Found', `${error}\n\nInstall Blocksmith-BR from: https://github.com/MrLabRat/Blocksmith-BR`);
    }
  };

  // Menu handlers
  const handleDeleteAllPacks = async () => {
    try {
      await invoke('delete_all_packs');
      setPacks([]);
      setSelectedPacks(new Set());
    } catch (error) {
      console.error('Failed to delete all packs:', error);
    }
  };

  const handleRestart = async () => {
    // Reload the window
    window.location.reload();
  };

  // Filtering logic
  const filteredPacks = useMemo(() => packs.filter((pack) => {
    const matchesType = selectedPackType === 'All' || pack.pack_type === selectedPackType;
    const search = searchFilter.toLowerCase();
    const matchesSearch = (
      pack.name.toLowerCase().includes(search) ||
      pack.path.toLowerCase().includes(search) ||
      pack.pack_type.toLowerCase().includes(search)
    );
    return matchesType && matchesSearch;
  }), [packs, selectedPackType, searchFilter]);

  // Export pack list
  const exportPacksCSV = useCallback(async () => {
    const headers = 'Name,Type,UUID,Version,Size\n';
    const csvEscape = (value: string) => {
      const safeValue = /^[=+\-@]/.test(value.trimStart()) ? `'${value}` : value;
      return `"${safeValue.replace(/"/g, '""')}"`;
    };
    const rows = filteredPacks.map(p =>
      [
        csvEscape(getBestDisplayName(p)),
        csvEscape(p.pack_type),
        csvEscape(p.uuid || ''),
        csvEscape(p.version || ''),
        csvEscape(p.folder_size_formatted || ''),
      ].join(',')
    ).join('\n');
    const path = await save({
      defaultPath: 'packs.csv',
      filters: [{ name: 'CSV', extensions: ['csv'] }],
    });
    if (path) await invoke('write_export_file', { path, content: headers + rows, extension: 'csv' });
    setShowExportDropdown(false);
  }, [filteredPacks]);

  const exportPacksJSON = useCallback(async () => {
    const data = filteredPacks.map(p => ({
      name: getBestDisplayName(p),
      type: p.pack_type,
      uuid: p.uuid || null,
      version: p.version || null,
      size: p.folder_size_formatted || null,
      path: p.path,
    }));
    const path = await save({
      defaultPath: 'packs.json',
      filters: [{ name: 'JSON', extensions: ['json'] }],
    });
    if (path) await invoke('write_export_file', { path, content: JSON.stringify(data, null, 2), extension: 'json' });
    setShowExportDropdown(false);
  }, [filteredPacks]);

  const handleSelectAll = useCallback(() => {
    setSelectedPacks(new Set(filteredPacks.map(getPackKey)));
  }, [filteredPacks]);

  const handleSelectUpdates = useCallback(() => {
    setSelectedPacks(new Set(filteredPacks.filter((p) => p.is_update).map(getPackKey)));
  }, [filteredPacks]);

  // Keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;

      if (e.ctrlKey && (e.key === '=' || e.key === '+')) {
        e.preventDefault();
        adjustScale(15);
      } else if (e.ctrlKey && e.key === '-') {
        e.preventDefault();
        adjustScale(-15);
      } else if (e.ctrlKey && e.key === '0') {
        e.preventDefault();
        resetScale();
      } else if (e.ctrlKey && e.key === 'a') {
        e.preventDefault();
        handleSelectAll();
      } else if (e.key === 'Escape') {
        e.preventDefault();
        handleDeselectAll();
      } else if (e.key === 'Delete') {
        e.preventDefault();
        removeSelectedPacks();
      }
    };

    const handleWheel = (e: WheelEvent) => {
      if (!e.ctrlKey) return;
      e.preventDefault();
      if (e.deltaY < 0) {
        adjustScale(10);
      } else {
        adjustScale(-10);
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    window.addEventListener('wheel', handleWheel, { passive: false });
    return () => {
      window.removeEventListener('keydown', handleKeyDown);
      window.removeEventListener('wheel', handleWheel);
    };
  }, [adjustScale, resetScale, handleSelectAll, handleDeselectAll, removeSelectedPacks]);

  // Keep packsRef in sync for use inside interval callbacks
  useEffect(() => { packsRef.current = packs; }, [packs]);

  // Auto-detect new packs dropped into the scan folder
  useEffect(() => {
    if (!settings.scan_location) return;
    const scanLocation = settings.scan_location;

    const processNewFiles = async () => {
      if (autoScanInFlightRef.current) return;
      autoScanInFlightRef.current = true;
      newFilesDebounceRef.current = null;
      const knownPaths = new Set(packsRef.current.map(p => p.path));
      try {
        const allPacks = await invoke<PackInfo[]>('scan_packs', { directory: scanLocation });
        const newPacks = allPacks.filter(p => !knownPaths.has(p.path));
        if (newPacks.length === 0) return;

        setPacks(prev => {
          const existingKeys = new Set(prev.map(getPackKey));
          return [...prev, ...newPacks.filter(pack => !existingKeys.has(getPackKey(pack)))];
        });
        setNewPackPaths(prev => {
          const next = new Set(prev);
          newPacks.forEach(p => next.add(p.path));
          return next;
        });

        // Accumulate names across multiple detections
        const uniqueNewFiles = [...new Set(newPacks.map(p => p.path))];
        uniqueNewFiles.forEach(fp => {
          if (!newPackNamesRef.current.has(fp)) {
            const name = getBestDisplayName(newPacks.find(p => p.path === fp)!);
            newPackNamesRef.current.set(fp, name.length > 30 ? name.slice(0, 28) + '\u2026' : name);
          }
        });

        // Replace the single running notification with the full accumulated list
        const allNames = [...newPackNamesRef.current.values()];
        const shown = allNames.slice(0, 5);
        const overflow = allNames.length - shown.length;
        const messageBody = overflow > 0 ? `${shown.join(', ')} +${overflow} more` : shown.join(', ');
        const notif = createNotification(
          'info',
          `${allNames.length} New Pack${allNames.length !== 1 ? 's' : ''} Detected`,
          messageBody
        );
        const prevId = newPacksNotifIdRef.current;
        setNotifications(prev =>
          (prevId ? prev.filter(n => n.id !== prevId) : prev).concat(notif)
        );
        newPacksNotifIdRef.current = notif.id;
        invoke<PackInfo[]>('compute_pack_status', { packs: newPacks })
          .then(updated => {
            const updatedMap = new Map(updated.map(p => [p.path, p]));
            setPacks(prev => prev.map(p => updatedMap.get(p.path) ?? p));
          })
          .catch(() => {});
      } catch {
      } finally {
        autoScanInFlightRef.current = false;
      }
    };

    const interval = setInterval(async () => {
      if (autoScanInFlightRef.current) return;
      try {
        const files = await invoke<string[]>('list_mc_files_in_dir', { directory: scanLocation });
        const knownPaths = new Set(packsRef.current.map(p => p.path));
        const newFiles = files.filter(f => !knownPaths.has(f));
        if (newFiles.length === 0) return;

        // Reset debounce each time new files appear, so we wait for the full batch to arrive
        if (newFilesDebounceRef.current) clearTimeout(newFilesDebounceRef.current);
        newFilesDebounceRef.current = setTimeout(processNewFiles, 3000);
      } catch { }
    }, 2000);

    return () => {
      clearInterval(interval);
      if (newFilesDebounceRef.current) {
        clearTimeout(newFilesDebounceRef.current);
        newFilesDebounceRef.current = null;
      }
      autoScanInFlightRef.current = false;
    };
  }, [settings.scan_location, addNotification]);

  // Results calculations
  const selectedCount = selectedPacks.size;
  const totalCount = packs.length;
  const { successCount, failCount, has4dSkinPacks, fourDSkinPacks, templateUpdates } = useMemo(() => ({
    successCount: results?.filter((r) => r.success).length ?? 0,
    failCount: results?.filter((r) => !r.success).length ?? 0,
    has4dSkinPacks: results?.some((r) => r.pack_type === 'SkinPack4D' && r.success) ?? false,
    fourDSkinPacks: results?.filter((r) => r.pack_type === 'SkinPack4D' && r.success) ?? [],
    templateUpdates: results?.filter((r) => r.is_template_update && r.success) ?? [],
  }), [results]);
  const hasTemplateUpdates = templateUpdates.length > 0;

  // Filter logs based on debug mode - no performance impact, just filters display
  const filteredLogs = settings.debug_mode ? logs : [];

  // Only pick a new message when major action starts
  const [friendlyMessage, setFriendlyMessage] = useState(friendlyLogMessages[0]);
  
  useEffect(() => {
    if (!settings.debug_mode && (isScanning || isMoving)) {
      setFriendlyMessage(friendlyLogMessages[Math.floor(Math.random() * friendlyLogMessages.length)]);
    }
  }, [isScanning, isMoving, settings.debug_mode]);

  const tipQueueRef = useRef<string[]>([]);
  const getNextTip = useCallback(() => {
    if (tipQueueRef.current.length === 0) {
      const shuffled = [...tipMessages];
      for (let i = shuffled.length - 1; i > 0; i--) {
        const j = Math.floor(Math.random() * (i + 1));
        [shuffled[i], shuffled[j]] = [shuffled[j], shuffled[i]];
      }
      tipQueueRef.current = shuffled;
    }
    return tipQueueRef.current.shift()!;
  }, []);

  useEffect(() => {
    if (settings.disable_tip_notifications) return;
    const interval = setInterval(() => {
      if (Math.random() < 0.35) {
        addTipNotification(getNextTip());
      }
    }, 120000);
    return () => clearInterval(interval);
  }, [settings.disable_tip_notifications, addTipNotification, getNextTip]);

  const animationClass = useMemo(() => {
    if (settings.disable_animations) return 'animations-disabled';
    const speed = settings.animation_speed_ms ?? 300;
    if (speed <= 150) return 'animations-fast';
    if (speed >= 500) return 'animations-slow';
    return 'animations-normal';
  }, [settings.disable_animations, settings.animation_speed_ms]);

  const animationDuration = useMemo(() => {
    if (settings.disable_animations) return 0;
    return settings.animation_speed_ms ?? 300;
  }, [settings.disable_animations, settings.animation_speed_ms]);

  const effectiveScale = useMemo(() => {
    const uiScale = settings.ui_scale ?? 100;
    return Math.max(70, Math.min(200, uiScale));
  }, [settings.ui_scale]);

  const getAppIconPath = useCallback((style?: string, bordered?: boolean): string => {
    const prefix = style === 'default' ? 'default' : 'blackred';
    const suffix = bordered === false ? 'noborder' : 'border';
    return `/icons/${prefix}${suffix}.png`;
  }, []);

  const appIconStyle = settings.app_icon_style || 'default';
  const appIconBorder = settings.app_icon_border !== false;

  return (
    <div 
      className={`app ${animationClass}`}
      data-theme={settings.theme || 'darkred'}
      data-bg={settings.background_style ?? (settings.theme === 'minecraft' ? 'mc-terrain' : 'embers')}
       style={{ 
         '--animation-duration': `${animationDuration}ms`,
         '--ui-scale': effectiveScale / 100,
       } as React.CSSProperties}
    >
      <AnimatedBackground
        disabled={settings.disable_animations}
        style={settings.background_style ?? (settings.theme === 'minecraft' ? 'mc-terrain' : 'embers')}
        smokeIntensity={settings.background_smoke ?? 5}
        blobCount={settings.background_blobs ?? 5}
      />
      <Notifications notifications={notifications} onDismiss={dismissNotification} />
      <div className="app-content" style={{ zoom: effectiveScale / 100 }}>
        <header className="app-header" data-tauri-drag-region>
          <div className="app-title">
            <img 
              src={getAppIconPath(appIconStyle, appIconBorder)}
              alt="Blocksmith"
              className="app-title-icon"
            />
            <h1>Blocksmith</h1>
          </div>
          <div className="header-actions">
            <button className="btn btn-small" onClick={handleLaunchMinecraft} title="Launch Minecraft">
              Minecraft
            </button>
            <button className="btn btn-small" onClick={handleLaunchToolCoin} title={toolcoinInstalled ? "Launch ToolCoin" : "Install ToolCoin to get more packs"}>
              Get More Packs!
            </button>
            <SettingsButton onClick={() => setShowSettings(true)} />
            <HamburgerMenu 
              onDeleteAllPacks={handleDeleteAllPacks}
              onRestart={handleRestart}
              onShowStats={() => setShowStats(true)}
              onShowInstalledPacks={() => setShowInstalledPacks(true)}
              onShowHelp={() => setShowHelp(true)}
              onShowRecycleBin={() => setShowRecycleBin(true)}
            />
          </div>
          <div className="window-controls">
            <button 
              className="window-control minimize" 
              onClick={handleMinimize}
              title="Minimize"
            />
            <button 
              className="window-control maximize" 
              onClick={handleMaximize}
              title="Maximize"
            />
            <button 
              className="window-control close" 
              onClick={handleClose}
              title="Close"
            />
          </div>
        </header>

        <main className="app-main">
        <section className="controls-section">
          <ScanControls
            packs={packs}
            selectedPacks={selectedPacks}
            isScanning={isScanning}
            isMoving={isMoving}
            settings={settings}
            progress={progress}
            onScanStart={handleScanStart}
            onScanComplete={handleScanComplete}
            onMoveStart={handleMoveStart}
            onMoveComplete={handleMoveComplete}
            onError={(title, message) => addNotification('error', title, message)}
            onBeforeProcess={handleBeforeProcess}
          />

          {settings.dry_run && (
            <div className="dry-run-banner">
              Dry Run Mode Active - No files will be moved
            </div>
          )}
        </section>

        <section className="content-section">
          <div className="packs-panel">
            {/* Pack Type Dropdown */}
            <div className="pack-type-filter">
              <div className="pack-type-dropdown">
                <button 
                  className="pack-type-dropdown-btn"
                  onClick={() => setPackTypeDropdownOpen(prev => !prev)}
                >
                  <span>{packTypeLabels[selectedPackType]}</span>
                  <ChevronDown size={16} />
                </button>
                {packTypeDropdownOpen && (
                  <div className="pack-type-dropdown-menu open">
                    {packTypes.map((type) => (
                      <button
                        key={type}
                        className={`pack-type-option ${selectedPackType === type ? 'selected' : ''}`}
                        onClick={() => {
                          setSelectedPackType(type);
                          setPackTypeDropdownOpen(false);
                        }}
                      >
                        {packTypeLabels[type]}
                      </button>
                    ))}
                  </div>
                )}
              </div>
            </div>

            {/* Pack List */}
            <div className="panel-header">
              <h2>Packs</h2>
              <div className="pack-header-actions">
                <input
                  type="text"
                  className="search-input"
                  placeholder="Search packs..."
                  value={searchFilter}
                  onChange={(e) => setSearchFilter(e.target.value)}
                />
                {totalCount > 0 && (
                  <span className="selection-info">
                    {selectedCount} of {filteredPacks.length} selected
                  </span>
                )}
                {filteredPacks.length > 0 && (
                  <div className="export-dropdown-wrapper">
                    <button
                      className="btn btn-small"
                      onClick={() => setShowExportDropdown(prev => !prev)}
                      title="Export pack list"
                    >
                      <Download size={13} style={{ marginRight: 4 }} />
                      Export
                    </button>
                    {showExportDropdown && (
                      <div className="export-dropdown-menu">
                        <button className="export-dropdown-item" onClick={exportPacksCSV}>
                          Export as CSV
                        </button>
                        <button className="export-dropdown-item" onClick={exportPacksJSON}>
                          Export as JSON
                        </button>
                      </div>
                    )}
                  </div>
                )}
              </div>
            </div>
            <PackList
              packs={filteredPacks}
              selectedPacks={selectedPacks}
              onTogglePack={handleTogglePack}
              onSelectAll={handleSelectAll}
              onDeselectAll={handleDeselectAll}
              onSelectUpdates={handleSelectUpdates}
              onRemove={handleRemovePack}
              onRemoveSelected={removeSelectedPacks}
              onDeleteFromDisk={handleDeleteFromDisk}
              onDeleteSelectedFromDisk={handleDeleteSelectedFromDisk}
              onPreview={handlePreviewPack}
              newPackPaths={newPackPaths}
              deleteFileOnRemove={settings.delete_file_on_remove ?? true}
            />
          </div>

          {/* Collapsible Log Panel */}
          <div className={`logs-panel ${logPanelOpen ? 'open' : 'collapsed'}`}>
            <div className="log-panel-header">
              <div className="log-panel-title" onClick={() => setLogPanelOpen(!logPanelOpen)}>
                <h3>Logs</h3>
                <button className="btn btn-icon">
                  <ChevronDown size={18} style={{ 
                    transform: logPanelOpen ? 'rotate(0deg)' : 'rotate(-90deg)',
                    transition: 'transform var(--transition)'
                  }} />
                </button>
              </div>
              {logPanelOpen && (
                <button className="btn btn-small" onClick={handleClearLogs} disabled={logs.length === 0}>
                  Clear
                </button>
              )}
            </div>
            {logPanelOpen ? (
              settings.debug_mode ? (
                <AnimatedLogViewer logs={filteredLogs} />
              ) : (
                <div className="friendly-log-panel">
                  <div className="friendly-message">
                    {isScanning || isMoving ? friendlyMessage : "Let's organize this chest!"}
                  </div>
                  <div className="friendly-hint">
                    Enable Debug Mode in Settings to see detailed logs
                  </div>
                </div>
              )
            ) : (
              isMoving && progress && (
                <div className="simplified-progress">
                  <div className="progress-info">
                    <span className="progress-message">{progress.message}</span>
                    <span className="progress-count">{progress.current} / {progress.total}</span>
                  </div>
                  <div className="progress-bar-container">
                    <div 
                      className="progress-bar-fill" 
                      style={{ width: `${progress.total > 0 ? (progress.current / progress.total) * 100 : 0}%` }}
                    />
                  </div>
                  <span className="progress-percentage">
                    {progress.total > 0 ? Math.round((progress.current / progress.total) * 100) : 0}%
                  </span>
                </div>
              )
            )}
          </div>
        </section>
      </main>

      {/* Settings Modal */}
      <Settings 
        settings={settings} 
        onSettingsChange={handleSettingsChange} 
        isOpen={showSettings} 
        onClose={() => setShowSettings(false)} 
      />
      </div>

      {/* Results Modal */}
      {results && (
        <div className="modal-overlay" onClick={() => setResults(null)}>
          <div className="modal modal-large" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h3>Processing Complete</h3>
              <button className="btn btn-icon" onClick={() => setResults(null)}>
                <X size={20} />
              </button>
            </div>
            <div className="modal-content">
              <div className="results-summary">
                <div className="result-stat success">
                  <CheckCircle size={24} />
                  <span>{successCount} Successful</span>
                </div>
                {failCount > 0 && (
                  <div className="result-stat fail">
                    <XCircle size={24} />
                    <span>{failCount} Failed</span>
                  </div>
                )}
              </div>

              {has4dSkinPacks && (
                <div className="skinmaster-notice">
                  <div className="skinmaster-notice-header">
                    <Package size={20} />
                    <span>4D Skin Packs Extracted ({fourDSkinPacks.length})</span>
                  </div>
                  <p>
                    4D skin packs have been extracted to a "4D Skin Packs" folder. Use these paths with SkinMaster:
                  </p>
                  <div className="four-d-paths-list">
                    {fourDSkinPacks.map((pack, idx) => (
                      <div key={idx} className="four-d-path-item">
                        <span className="four-d-path-name">{pack.pack_name}</span>
                        <code className="four-d-path-value">{pack.skin_pack_4d_path || pack.destination}</code>
                        <div className="four-d-path-actions">
                          <button 
                            className="btn btn-small"
                            onClick={() => handleCopyPath(pack.skin_pack_4d_path || pack.destination)}
                            title="Copy path to clipboard"
                          >
                            {copiedPath === (pack.skin_pack_4d_path || pack.destination) ? <CheckCircle size={14} /> : <Copy size={14} />}
                          </button>
                        </div>
                      </div>
                    ))}
                  </div>
                  <div className="skinmaster-buttons">
                    <button className="btn btn-primary" onClick={handleOpenSkinMaster}>
                      <ExternalLink size={16} />
                      Open SkinMaster
                    </button>
                  </div>
                </div>
              )}

              {hasTemplateUpdates && (
                <div className="template-update-notice">
                  <div className="skinmaster-notice-header">
                    <Globe size={20} />
                    <span>World Template Updated ({templateUpdates.length})</span>
                  </div>
                  <p>
                    World templates have been updated. If you have existing worlds using these templates, you'll need to manually update them.
                  </p>
                  <p className="hint">
                    Copy the <code>behavior_packs/bp0</code> and <code>resource_packs/rp0</code> folders from the new template to your world's folder in:<br/>
                    <code>minecraftWorlds/[world_id]/</code>
                  </p>
                </div>
              )}

              <div className="results-list">
                {results.map((result, idx) => (
                  <div key={idx} className={`result-item ${result.success ? 'success' : 'fail'}`}>
                    <div className="result-icon">
                      {result.success ? <CheckCircle size={18} /> : <XCircle size={18} />}
                    </div>
                    <div className="result-info">
                      <div className="result-name">
                        {result.pack_name}
                        {result.pack_type === 'SkinPack4D' && result.success && (
                          <span className="badge-4d">4D</span>
                        )}
                      </div>
                      {result.success ? (
                        <div className="result-dest">{result.destination}</div>
                      ) : (
                        <div className="result-error">{result.error}</div>
                      )}
                    </div>
                    {result.success && (
                      <div className="result-actions">
                        <button
                          className="btn btn-small"
                          onClick={() => handleCopyPath(result.destination)}
                          title="Copy path to clipboard"
                        >
                          {copiedPath === result.destination ? <CheckCircle size={16} /> : <Copy size={16} />}
                        </button>
                         <button
                           className="btn btn-small"
                           onClick={() => handleOpenFolder(result.destination)}
                           title="Open folder"
                         >
                           <FolderOpen size={16} />
                         </button>
                       </div>
                     )}
                  </div>
                ))}
              </div>
            </div>
            <div className="modal-actions">
              <button className="btn btn-primary" onClick={() => setResults(null)}>
                Close
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Statistics Page Modal */}
      {showStats && <StatisticsPage onClose={() => setShowStats(false)} />}

      {/* Installed Packs Modal */}
      {showInstalledPacks && <InstalledPacksPage onClose={() => setShowInstalledPacks(false)} addNotification={addNotification} />}

      {/* Help & Feedback Modal */}
      {showHelp && <HelpPage onClose={() => setShowHelp(false)} />}

      {/* Recycle Bin Modal */}
      {showRecycleBin && <RecycleBinPage onClose={() => setShowRecycleBin(false)} addNotification={addNotification} />}

      {/* Confirm Dialog */}
      {confirmState && (
        <ConfirmDialog
          title={confirmState.title}
          message={confirmState.message}
          detail={confirmState.detail}
          confirmLabel={confirmState.confirmLabel ?? 'Delete'}
          onConfirm={confirmState.onConfirm}
          onCancel={() => {
            confirmState.onCancel?.();
            setConfirmState(null);
          }}
        />
      )}

      {/* Pack Preview Modal */}
      {previewPack && (
        <div className="modal-overlay" onClick={() => setPreviewPack(null)}>
          <div className="modal modal-large" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h3>
                <Eye size={16} style={{ marginRight: 8 }} />
                Preview: {previewPack.pack.name}
              </h3>
              <button className="btn btn-icon" onClick={() => setPreviewPack(null)}>
                <X size={20} />
              </button>
            </div>
            <div className="modal-content">
              {previewLoading ? (
                <div style={{ padding: '24px', textAlign: 'center', color: 'var(--text-secondary)' }}>
                  Reading archive...
                </div>
              ) : (
                <PreviewFileTree files={previewFiles} />
              )}
            </div>
            <div className="modal-actions">
              <button className="btn btn-primary" onClick={() => setPreviewPack(null)}>Close</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

interface TreeNode {
  name: string;
  children: Map<string, TreeNode>;
  isFile: boolean;
}

function buildTree(files: string[]): TreeNode {
  const root: TreeNode = { name: '', children: new Map(), isFile: false };
  for (const file of files) {
    const parts = file.split('/');
    let node = root;
    for (let i = 0; i < parts.length; i++) {
      const part = parts[i];
      if (!node.children.has(part)) {
        node.children.set(part, { name: part, children: new Map(), isFile: i === parts.length - 1 });
      }
      node = node.children.get(part)!;
    }
  }
  return root;
}

function TreeNodeRow({ node, depth }: { node: TreeNode; depth: number }) {
  const [expanded, setExpanded] = useState(depth < 1);
  const isDir = !node.isFile && node.children.size > 0;
  const children = Array.from(node.children.values()).sort((a, b) => {
    if (a.isFile !== b.isFile) return a.isFile ? 1 : -1;
    return a.name.localeCompare(b.name);
  });

  return (
    <>
      <div
        className={`preview-tree-row${isDir ? ' preview-tree-dir' : ''}`}
        style={{ paddingLeft: depth * 16 + 8 }}
        onClick={() => isDir && setExpanded(e => !e)}
      >
        <span className="preview-tree-icon">
          {isDir ? (expanded ? '▼' : '▶') : '·'}
        </span>
        <span className="preview-tree-name">{node.name}{!node.isFile && node.children.size > 0 ? '/' : ''}</span>
      </div>
      {isDir && expanded && children.map(child => (
        <TreeNodeRow key={child.name} node={child} depth={depth + 1} />
      ))}
    </>
  );
}

function PreviewFileTree({ files }: { files: string[] }) {
  const tree = useMemo(() => buildTree(files), [files]);
  const topLevel = Array.from(tree.children.values()).sort((a, b) => {
    if (a.isFile !== b.isFile) return a.isFile ? 1 : -1;
    return a.name.localeCompare(b.name);
  });

  if (files.length === 0) {
    return <div style={{ padding: '16px', color: 'var(--text-secondary)' }}>Archive is empty.</div>;
  }

  return (
    <div className="preview-file-tree">
      <div className="preview-file-count">{files.filter(f => !f.endsWith('/')).length} files</div>
      {topLevel.map(node => (
        <TreeNodeRow key={node.name} node={node} depth={0} />
      ))}
    </div>
  );
}

export default App;

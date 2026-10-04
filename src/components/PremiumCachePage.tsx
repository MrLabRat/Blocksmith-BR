import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { PremiumCachePack, AppNotification } from '../types';
import { X, FolderOpen, Download, ExternalLink, RefreshCw } from 'lucide-react';

interface PremiumCachePageProps {
  onClose: () => void;
  addNotification: (type: AppNotification['type'], title: string, message: string) => void;
  scanLocation?: string;
}

export function PremiumCachePage({ onClose, addNotification, scanLocation }: PremiumCachePageProps) {
  const [packs, setPacks] = useState<PremiumCachePack[]>([]);
  const [loading, setLoading] = useState(true);
  const [importingPath, setImportingPath] = useState<string | null>(null);

  const loadPacks = useCallback(async () => {
    setLoading(true);
    try {
      const result = await invoke<PremiumCachePack[]>('get_premium_cache_packs');
      setPacks(result);
    } catch (error) {
      addNotification('error', 'Premium Cache', `${error}`);
      setPacks([]);
    } finally {
      setLoading(false);
    }
  }, [addNotification]);

  useEffect(() => {
    void loadPacks();
  }, [loadPacks]);

  const handleOpenCache = async () => {
    try {
      await invoke('open_premium_cache');
    } catch (error) {
      addNotification('error', 'Open Failed', `${error}`);
    }
  };

  const handleImport = async (premium: PremiumCachePack) => {
    const defaultPath = scanLocation
      ? `${scanLocation.replace(/[\\/]+$/, '')}/4D Skin Packs`
      : undefined;
    const selected = await open({
      directory: true,
      multiple: false,
      defaultPath,
      title: 'Select extracted 4D Skin Pack folder',
    });
    if (!selected || typeof selected !== 'string') return;

    setImportingPath(premium.path);
    try {
      await invoke('import_4d_skin_to_premium', {
        skinPackPath: selected,
        premiumPackPath: premium.path,
      });
      addNotification(
        'success',
        '4D Skin Imported',
        `Imported into "${premium.display_name || premium.folder_name}".`
      );
    } catch (error) {
      addNotification('error', 'Import Failed', `${error}`);
    } finally {
      setImportingPath(null);
    }
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal modal-large" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h3>Premium Skin Packs</h3>
          <button className="btn btn-icon" onClick={onClose}>
            <X size={20} />
          </button>
        </div>
        <div className="modal-content">
          <p style={{ marginBottom: 12, color: 'var(--text-secondary)', fontSize: 13 }}>
            Import an extracted 4D Skin Pack into a marketplace premium skin pack folder. Choose a pack below, then pick the folder under your scan location&apos;s 4D Skin Packs directory.
          </p>
          <div className="settings-buttons-row" style={{ marginBottom: 12 }}>
            <button className="btn btn-small" onClick={() => void loadPacks()} disabled={loading}>
              <RefreshCw size={14} />
              Refresh
            </button>
            <button className="btn btn-small" onClick={() => void handleOpenCache()}>
              <ExternalLink size={14} />
              Open Premium Cache
            </button>
          </div>
          {loading ? (
            <p style={{ color: 'var(--text-secondary)' }}>Loading premium skin packs...</p>
          ) : packs.length === 0 ? (
            <p style={{ color: 'var(--text-secondary)' }}>
              No premium skin packs found. Open Minecraft Bedrock once so the premium cache is created.
            </p>
          ) : (
            <div className="duplicate-groups">
              {packs.map((pack) => (
                <div key={pack.path} className="duplicate-pack-row">
                  <div className="duplicate-pack-info">
                    <span className="duplicate-pack-folder" title={pack.path}>
                      {pack.display_name || pack.folder_name}
                    </span>
                    <span className="duplicate-pack-type">{pack.folder_name}</span>
                  </div>
                  <div style={{ display: 'flex', gap: 6 }}>
                    <button
                      className="btn btn-small"
                      onClick={() => void invoke('open_folder', { path: pack.path }).catch((error) => addNotification('error', 'Open Failed', `${error}`))}
                      title="Open folder"
                    >
                      <FolderOpen size={12} />
                    </button>
                    <button
                      className="btn btn-primary btn-sm"
                      onClick={() => void handleImport(pack)}
                      disabled={importingPath === pack.path}
                      title="Import a 4D skin pack into this premium pack"
                    >
                      <Download size={12} style={{ marginRight: 4 }} />
                      {importingPath === pack.path ? 'Importing...' : 'Import 4D'}
                    </button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
        <div className="modal-actions">
          <button className="btn btn-primary" onClick={onClose}>
            Close
          </button>
        </div>
      </div>
    </div>
  );
}

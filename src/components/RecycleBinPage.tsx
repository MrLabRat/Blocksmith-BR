import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { RecycledPackInfo, AppNotification } from '../types';
import { formatBytes } from '../utils/packUtils';
import { X, RotateCcw, Trash2, Trash } from 'lucide-react';
import { ConfirmDialog } from './ConfirmDialog';
import '../styles/RecycleBinPage.css';

interface RecycleBinPageProps {
  onClose: () => void;
  addNotification: (type: AppNotification['type'], title: string, message: string) => void;
}

function formatDeletedAt(ms: number): string {
  if (!ms) return 'Unknown';
  const date = new Date(ms);
  return date.toLocaleString();
}

export function RecycleBinPage({ onClose, addNotification }: RecycleBinPageProps) {
  const [items, setItems] = useState<RecycledPackInfo[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [pendingDelete, setPendingDelete] = useState<RecycledPackInfo | null>(null);
  const [pendingEmpty, setPendingEmpty] = useState(false);
  const [busyPath, setBusyPath] = useState<string | null>(null);

  const loadItems = useCallback(async () => {
    setIsLoading(true);
    try {
      const result = await invoke<RecycledPackInfo[]>('list_recycled_packs');
      setItems(result);
    } catch (error) {
      addNotification('error', 'Recycle Bin', `Failed to load recycle bin: ${error}`);
    } finally {
      setIsLoading(false);
    }
  }, [addNotification]);

  useEffect(() => {
    loadItems();
  }, [loadItems]);

  const handleRestore = async (item: RecycledPackInfo) => {
    setBusyPath(item.recycle_path);
    try {
      await invoke('restore_recycled_pack', { recyclePath: item.recycle_path });
      addNotification('success', 'Pack Restored', `${item.name} was restored to its original location.`);
      setItems(prev => prev.filter(i => i.recycle_path !== item.recycle_path));
    } catch (error) {
      addNotification('error', 'Restore Failed', `${error}`);
    } finally {
      setBusyPath(null);
    }
  };

  const confirmPermanentDelete = async () => {
    if (!pendingDelete) return;
    const item = pendingDelete;
    setPendingDelete(null);
    setBusyPath(item.recycle_path);
    try {
      await invoke('permanently_delete_recycled', { recyclePath: item.recycle_path });
      setItems(prev => prev.filter(i => i.recycle_path !== item.recycle_path));
    } catch (error) {
      addNotification('error', 'Delete Failed', `${error}`);
    } finally {
      setBusyPath(null);
    }
  };

  const confirmEmpty = async () => {
    setPendingEmpty(false);
    try {
      await invoke('empty_recycle_bin');
      setItems([]);
    } catch (error) {
      addNotification('error', 'Recycle Bin', `Failed to empty recycle bin: ${error}`);
    }
  };

  return (
    <>
      <div className="modal-overlay" onClick={onClose}>
        <div className="modal modal-large" onClick={(e) => e.stopPropagation()}>
          <div className="modal-header">
            <h3>Recycle Bin</h3>
            <button className="btn btn-icon" onClick={onClose}>
              <X size={18} />
            </button>
          </div>
          <div className="modal-content recycle-bin-content">
            <div className="recycle-bin-toolbar">
              <span className="recycle-bin-count">
                {items.length} item{items.length === 1 ? '' : 's'}
              </span>
              <button
                className="btn btn-danger btn-sm"
                onClick={() => setPendingEmpty(true)}
                disabled={items.length === 0}
              >
                <Trash size={13} style={{ marginRight: 4 }} />
                Empty Recycle Bin
              </button>
            </div>

            {isLoading ? (
              <div className="no-packs-found">Loading recycle bin...</div>
            ) : items.length === 0 ? (
              <div className="no-packs-found">Recycle bin is empty.</div>
            ) : (
              <div className="recycle-bin-list">
                {items.map(item => (
                  <div key={item.recycle_path} className="recycle-bin-row">
                    <div className="recycle-bin-info">
                      <div className="recycle-bin-name" title={item.name}>{item.name}</div>
                      <div className="recycle-bin-path" title={item.original_path}>{item.original_path}</div>
                      <div className="recycle-bin-meta">
                        <span>{formatBytes(item.size)}</span>
                        <span>Deleted {formatDeletedAt(item.deleted_at)}</span>
                      </div>
                    </div>
                    <div className="recycle-bin-actions">
                      <button
                        className="btn btn-secondary btn-sm"
                        onClick={() => handleRestore(item)}
                        disabled={busyPath === item.recycle_path}
                        title="Restore to original location"
                      >
                        <RotateCcw size={13} style={{ marginRight: 4 }} />
                        Restore
                      </button>
                      <button
                        className="btn btn-danger btn-sm"
                        onClick={() => setPendingDelete(item)}
                        disabled={busyPath === item.recycle_path}
                        title="Delete forever"
                      >
                        <Trash2 size={13} />
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
      </div>

      {pendingDelete && (
        <ConfirmDialog
          title="Delete Forever"
          message={`Permanently delete "${pendingDelete.name}"?`}
          detail="This cannot be undone."
          confirmLabel="Delete Forever"
          onConfirm={confirmPermanentDelete}
          onCancel={() => setPendingDelete(null)}
        />
      )}

      {pendingEmpty && (
        <ConfirmDialog
          title="Empty Recycle Bin"
          message={`Permanently delete all ${items.length} item${items.length === 1 ? '' : 's'} in the recycle bin?`}
          detail="This cannot be undone."
          confirmLabel="Empty Recycle Bin"
          onConfirm={confirmEmpty}
          onCancel={() => setPendingEmpty(false)}
        />
      )}
    </>
  );
}

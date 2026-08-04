import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { PackInfo, Settings, MoveOperation, ProgressEvent, getPackKey } from '../types';
import { Scan, Package, Undo2, Loader2, XCircle } from 'lucide-react';

function formatTime(seconds: number): string {
  if (seconds < 60) {
    return `${seconds}s`;
  }
  const minutes = Math.floor(seconds / 60);
  const secs = seconds % 60;
  if (minutes < 60) {
    return `${minutes}m ${secs}s`;
  }
  const hours = Math.floor(minutes / 60);
  const mins = minutes % 60;
  return `${hours}h ${mins}m`;
}

interface ScanControlsProps {
  packs: PackInfo[];
  selectedPacks: Set<string>;
  isScanning: boolean;
  isMoving: boolean;
  settings: Settings;
  progress: ProgressEvent | null;
  onScanStart: () => void;
  onScanComplete: (packs: PackInfo[]) => void;
  onMoveStart: () => void;
  onMoveComplete: (results?: MoveOperation[]) => void;
  onError?: (title: string, message: string) => void;
  onBeforeProcess?: (packs: PackInfo[]) => Promise<boolean>;
}

export function ScanControls({
  packs,
  selectedPacks,
  isScanning,
  isMoving,
  settings,
  progress,
  onScanStart,
  onScanComplete,
  onMoveStart,
  onMoveComplete,
  onError,
  onBeforeProcess,
}: ScanControlsProps) {
  const [sourcePath, setSourcePath] = useState<string>('');
  const [isDragOver, setIsDragOver] = useState(false);

  useEffect(() => {
    if (settings.scan_location) {
      setSourcePath(settings.scan_location);
    }
  }, [settings.scan_location]);

  // Allow dropping a folder (or one or more pack files) anywhere on the app window to
  // trigger a scan, instead of requiring the user to Browse for a directory every time.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;

    (async () => {
      const webview = getCurrentWebview();
      const stop = await webview.onDragDropEvent((event) => {
        if (event.payload.type === 'over' || event.payload.type === 'enter') {
          setIsDragOver(true);
        } else if (event.payload.type === 'leave') {
          setIsDragOver(false);
        } else if (event.payload.type === 'drop') {
          setIsDragOver(false);
          const paths = event.payload.paths;
          if (paths.length === 0 || isScanning || isMoving) return;
          void handleDroppedPaths(paths);
        }
      });
      if (cancelled) {
        stop();
      } else {
        unlisten = stop;
      }
    })();

    return () => {
      cancelled = true;
      unlisten?.();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isScanning, isMoving]);

  const handleDroppedPaths = async (paths: string[]) => {
    try {
      const directory = await invoke<string>('resolve_scan_directory', { paths });
      setSourcePath(directory);
      await performScan(directory);
    } catch (error) {
      console.error('Failed to resolve dropped path:', error);
      onError?.('Drag & Drop Failed', `${error}`);
    }
  };

  const handleSelectDirectory = async () => {
    const selected = await open({
      directory: true,
      multiple: false,
      title: 'Select Directory to Scan for Pack Files',
    });

    if (selected && typeof selected === 'string') {
      setSourcePath(selected);
    }
  };

  const handleScan = async () => {
    if (!sourcePath) {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Select Directory to Scan for Pack Files',
      });

      if (selected && typeof selected === 'string') {
        setSourcePath(selected);
        await performScan(selected);
      }
    } else {
      await performScan(sourcePath);
    }
  };

  const performScan = async (path: string) => {
    onScanStart();
    try {
      const result = await invoke<PackInfo[]>('scan_packs', { directory: path });
      onScanComplete(result);
    } catch (error) {
      console.error('Scan failed:', error);
      onScanComplete([]);
    }
  };

  const handleProcess = async () => {
    if (selectedPacks.size === 0) return;

    const selectedPacksList = packs.filter((p) => selectedPacks.has(getPackKey(p)));

    if (onBeforeProcess) {
      const shouldContinue = await onBeforeProcess(selectedPacksList);
      if (!shouldContinue) return;
    }

    onMoveStart();
    try {
      const results = await invoke<MoveOperation[]>('process_packs', { packs: selectedPacksList });
      onMoveComplete(results);
    } catch (error) {
      console.error('Process failed:', error);
      onMoveComplete();
    }
  };

  const handleCancel = async () => {
    try {
      await invoke('request_cancel');
    } catch (error) {
      console.error('Cancel request failed:', error);
    }
  };

  const handleRollback = async () => {
    onMoveStart();
    try {
      const result = await invoke<MoveOperation | null>('rollback_last');
      onMoveComplete(result ? [result] : undefined);
    } catch (error) {
      console.error('Rollback failed:', error);
      onError?.('Rollback failed', `${error}`);
      onMoveComplete();
    }
  };

  return (
    <div className={`scan-controls${isDragOver ? ' scan-controls-drag-over' : ''}`}>
      {isDragOver && (
        <div className="scan-drag-overlay">
          <Package size={28} />
          <span>Drop a folder or pack files to scan</span>
        </div>
      )}
      <div className="scan-input">
        <input
          type="text"
          value={sourcePath}
          onChange={(e) => setSourcePath(e.target.value)}
          placeholder="Select directory with .mcpack/.mcaddon/.mctemplate files..."
        />
        <button className="btn" onClick={handleSelectDirectory} disabled={isScanning || isMoving}>
          Browse
        </button>
      </div>

      <div className="scan-actions">
        <button className="btn btn-primary" onClick={handleScan} disabled={isScanning || isMoving}>
          {isScanning ? (
            <>
              <Loader2 className="spin" size={18} />
              Scanning...
            </>
          ) : (
            <>
              <Scan size={18} />
              Scan
            </>
          )}
        </button>

        <button
          className="btn btn-success"
          onClick={handleProcess}
          disabled={isMoving || isScanning || selectedPacks.size === 0}
        >
          {isMoving ? (
            <>
              <Loader2 className="spin" size={18} />
              Processing...
            </>
          ) : (
            <>
              <Package size={18} />
              Extract & Move ({selectedPacks.size})
            </>
          )}
        </button>

        <button
          className="btn btn-warning"
          onClick={handleRollback}
          disabled={isMoving}
          title="Undo last operation"
        >
          <Undo2 size={18} />
          Rollback
        </button>

        {(isScanning || isMoving) && (
          <button
            className="btn btn-danger"
            onClick={handleCancel}
            title="Cancel the current scan or process operation"
          >
            <XCircle size={18} />
            Cancel
          </button>
        )}
      </div>

      {progress && (isScanning || isMoving) && progress.total > 0 && (
        <div className="progress-bar">
          <div className="progress-text">
            {progress.message}
            <span className="progress-count">
              {' '}({progress.current}/{progress.total})
            </span>
            {progress.estimated_seconds && progress.estimated_seconds > 0 && (
              <span className="progress-time">
                {' '}~{formatTime(progress.estimated_seconds)} remaining
              </span>
            )}
          </div>
          <div className="progress-track">
            <div
              className="progress-fill"
              style={{ width: `${Math.min(100, (progress.current / progress.total) * 100)}%` }}
            />
          </div>
        </div>
      )}
    </div>
  );
}

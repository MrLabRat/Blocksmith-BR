import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { PackInfo, Settings, MoveOperation, ProgressEvent, getPackKey } from '../types';
import { Scan, Package, Undo2, Loader2 } from 'lucide-react';

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
}: ScanControlsProps) {
  const [sourcePath, setSourcePath] = useState<string>('');

  useEffect(() => {
    if (settings.scan_location) {
      setSourcePath(settings.scan_location);
    }
  }, [settings.scan_location]);

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

    onMoveStart();
    try {
      const selectedPacksList = packs.filter((p) => selectedPacks.has(getPackKey(p)));
      const results = await invoke<MoveOperation[]>('process_packs', { packs: selectedPacksList });
      onMoveComplete(results);
    } catch (error) {
      console.error('Process failed:', error);
      onMoveComplete();
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
    <div className="scan-controls">
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

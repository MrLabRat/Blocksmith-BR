import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { Settings as SettingsType, PackType, BackgroundStyle } from '../types';
import { Settings as SettingsIcon, FolderOpen, Search, Play, X } from 'lucide-react';

interface SettingsProps {
  settings: SettingsType;
  onSettingsChange: (settings: SettingsType) => void;
  isOpen: boolean;
  onClose: () => void;
  onError?: (title: string, message: string) => void;
}

const pathConfigs: { key: keyof SettingsType; label: string; packType: PackType }[] = [
  { key: 'behavior_pack_path', label: 'Behavior Packs', packType: 'BehaviorPack' },
  { key: 'resource_pack_path', label: 'Resource Packs', packType: 'ResourcePack' },
  { key: 'skin_pack_path', label: 'Skin Packs', packType: 'SkinPack' },
  { key: 'world_template_path', label: 'World Templates', packType: 'WorldTemplate' },
];

const uiScaleOptions = [100, 125, 150, 200];
const iconStyleOptions = [
  { value: 'blackred', label: 'Default' },
  { value: 'default', label: 'Minecraft' },
];
const themeOptions = [
  { value: 'darkred', label: 'Dark Red' },
  { value: 'minecraft', label: 'Minecraft' },
];

function getIconPath(style?: string, bordered?: boolean): string {
  const prefix = style === 'default' ? 'default' : 'blackred';
  const suffix = bordered === false ? 'noborder' : 'border';
  return `/icons/${prefix}${suffix}.png`;
}

export function Settings({ settings, onSettingsChange, isOpen, onClose, onError }: SettingsProps) {
  const [draft, setDraft] = useState(settings);

  useEffect(() => {
    if (isOpen) {
      setDraft(settings);
    }
  }, [isOpen, settings]);

  const handleSelectPath = async (key: keyof SettingsType) => {
    const selected = await open({
      directory: true,
      multiple: false,
      title: `Select ${pathConfigs.find(p => p.key === key)?.label || ''} Directory`,
    });

    if (selected && typeof selected === 'string') {
      setDraft({
        ...draft,
        [key]: selected,
      });
    }
  };

  const handleDryRunToggle = () => {
    setDraft({
      ...draft,
      dry_run: !draft.dry_run,
    });
  };

  const handleDeleteSourceToggle = () => {
    setDraft({
      ...draft,
      delete_source: !draft.delete_source,
    });
  };

  const handleDeleteOldOnUpdateToggle = () => {
    setDraft({
      ...draft,
      delete_old_on_update: !(draft.delete_old_on_update ?? true),
    });
  };

  const handleDeleteFileOnRemoveToggle = () => {
    setDraft({
      ...draft,
      delete_file_on_remove: !(draft.delete_file_on_remove ?? true),
    });
  };

  const handleRememberScanLocationToggle = () => {
    const next = !(draft.remember_scan_location ?? true);
    setDraft({
      ...draft,
      remember_scan_location: next,
      scan_location: next ? draft.scan_location : undefined,
    });
  };

  const handleAnimationsToggle = () => {
    setDraft({
      ...draft,
      disable_animations: !draft.disable_animations,
    });
  };

  const handleAnimationSpeedChange = (speedMs: number) => {
    setDraft({
      ...draft,
      animation_speed_ms: speedMs,
    });
  };

  const handleUiScaleChange = (scale: number) => {
    setDraft({
      ...draft,
      ui_scale: scale,
    });
  };

  const handleTaskbarIconStyleChange = (style: 'blackred' | 'default') => {
    setDraft({
      ...draft,
      taskbar_icon_style: style,
    });
  };

  const handleTaskbarIconBorderToggle = () => {
    const newBordered = draft.taskbar_icon_border === false;
    setDraft({
      ...draft,
      taskbar_icon_border: newBordered,
    });
  };

  const handleAppIconStyleChange = (style: 'blackred' | 'default') => {
    setDraft({
      ...draft,
      app_icon_style: style,
    });
  };

  const handleAppIconBorderToggle = () => {
    setDraft({
      ...draft,
      app_icon_border: draft.app_icon_border === false,
    });
  };

  const handleDebugModeToggle = () => {
    setDraft({
      ...draft,
      debug_mode: !draft.debug_mode,
    });
  };

  const handleThemeChange = (theme: 'darkred' | 'minecraft') => {
    const defaultBg: BackgroundStyle = theme === 'minecraft' ? 'mc-terrain' : 'embers';
    setDraft({ ...draft, theme, background_style: defaultBg });
  };

  const handleBackgroundStyleChange = (bg: BackgroundStyle) => {
    setDraft({ ...draft, background_style: bg });
  };

  const handleSmokeChange = (v: number) => {
    setDraft({ ...draft, background_smoke: v });
  };

  const handleBlobsChange = (v: number) => {
    setDraft({ ...draft, background_blobs: v });
  };

  const updateWindowIcon = async (style: string, bordered: boolean) => {
    try {
      await invoke('set_window_icon', { style, bordered });
    } catch (error) {
      console.error('Failed to update window icon:', error);
    }
  };

  const handleAutoDetect = async () => {
    try {
      const detected = await invoke<SettingsType>('auto_detect_paths');
      setDraft({
        ...draft,
        behavior_pack_path: detected.behavior_pack_path ?? draft.behavior_pack_path,
        resource_pack_path: detected.resource_pack_path ?? draft.resource_pack_path,
        skin_pack_path: detected.skin_pack_path ?? draft.skin_pack_path,
        world_template_path: detected.world_template_path ?? draft.world_template_path,
        scan_location: detected.scan_location ?? draft.scan_location,
      });
    } catch (error) {
      onError?.('Auto-Detect Failed', `${error}`);
    }
  };

  const handleSave = async () => {
    try {
      await invoke('save_settings', { settings: draft });
      onSettingsChange(draft);
      await updateWindowIcon(
        draft.taskbar_icon_style || 'blackred',
        draft.taskbar_icon_border !== false
      );
      onClose();
    } catch (error) {
      onError?.('Save Failed', `${error}`);
    }
  };

  if (!isOpen) return null;

  return (
    <div className="settings-modal-overlay" onClick={onClose}>
      <div className="settings-modal" onClick={(e) => e.stopPropagation()}>
        <div className="settings-modal-header">
          <h2>Settings</h2>
          <button className="btn btn-icon" onClick={onClose}>
            <X size={20} />
          </button>
        </div>

        <div className="settings-modal-content">
          <div className="settings-section">
            <h3>Destination Paths</h3>
            <div className="settings-buttons-row">
              <button className="btn btn-small" onClick={handleAutoDetect}>
                <Search size={14} />
                Auto-Detect
              </button>
            </div>
            {pathConfigs.map(({ key, label }) => (
              <div key={key} className="settings-row">
                <label>{label}</label>
                <div className="path-input">
                  <input
                    type="text"
                    value={(draft[key] as string) || ''}
                    onChange={(e) =>
                      setDraft({
                        ...draft,
                        [key]: e.target.value || undefined,
                      })
                    }
                    placeholder="Not configured"
                  />
                  <button className="btn btn-small" onClick={() => handleSelectPath(key)}>
                    <FolderOpen size={16} />
                  </button>
                </div>
              </div>
            ))}
          </div>

          <div className="settings-section">
            <h3>Options</h3>
            <div className="settings-row">
              <label>
                Remember Last Scan Directory
                <span className="hint">Prefill the scan folder from your last scan on startup</span>
              </label>
              <label className="toggle">
                <input type="checkbox" checked={draft.remember_scan_location ?? true} onChange={handleRememberScanLocationToggle} />
                <span className="toggle-slider"></span>
              </label>
            </div>
            <div className="settings-row">
              <label>
                Dry Run Mode
                <span className="hint">Preview without extracting/moving files</span>
              </label>
              <label className="toggle">
                <input type="checkbox" checked={draft.dry_run} onChange={handleDryRunToggle} />
                <span className="toggle-slider"></span>
              </label>
            </div>
            <div className="settings-row">
              <label>
                Delete Source Files
                <span className="hint">Remove .mcpack/.mcaddon after successful extraction</span>
              </label>
              <label className="toggle">
                <input type="checkbox" checked={draft.delete_source} onChange={handleDeleteSourceToggle} />
                <span className="toggle-slider"></span>
              </label>
            </div>
            <div className="settings-row">
              <label>
                Replace Old on Update
                <span className="hint">Remove old pack folder when installing an update</span>
              </label>
              <label className="toggle">
                <input type="checkbox" checked={draft.delete_old_on_update ?? true} onChange={handleDeleteOldOnUpdateToggle} />
                <span className="toggle-slider"></span>
              </label>
            </div>
            <div className="settings-row">
              <label>
                Delete File When Removing from List
                <span className="hint">Clicking the trash icon on a found pack also deletes the file from disk, so it won't reappear in future scans. Hold Shift to do the opposite.</span>
              </label>
              <label className="toggle">
                <input type="checkbox" checked={draft.delete_file_on_remove ?? true} onChange={handleDeleteFileOnRemoveToggle} />
                <span className="toggle-slider"></span>
              </label>
            </div>
            <div className="settings-row">
              <label>
                Debug Mode
                <span className="hint">Show detailed logs for troubleshooting</span>
              </label>
              <label className="toggle">
                <input type="checkbox" checked={draft.debug_mode || false} onChange={handleDebugModeToggle} />
                <span className="toggle-slider"></span>
              </label>
            </div>
            <div className="settings-row">
              <label>
                Tip Notifications
                <span className="hint">Show occasional tips in notifications</span>
              </label>
              <label className="toggle">
                <input
                  type="checkbox"
                  checked={!(draft.disable_tip_notifications || false)}
                  onChange={() =>
                    setDraft({
                      ...draft,
                      disable_tip_notifications: !draft.disable_tip_notifications,
                    })
                  }
                />
                <span className="toggle-slider"></span>
              </label>
            </div>
          </div>

          <div className="settings-section">
            <h3>Appearance</h3>
            <div className="settings-row">
              <label>
                Theme
                <span className="hint">Choose the visual style</span>
              </label>
              <div className="theme-buttons">
                {themeOptions.map((option) => (
                  <button
                    key={option.value}
                    className={`btn ${draft.theme === option.value || (!draft.theme && option.value === 'darkred') ? 'btn-primary' : 'btn-secondary'}`}
                    onClick={() => handleThemeChange(option.value as 'darkred' | 'minecraft')}
                  >
                    {option.label}
                  </button>
                ))}
              </div>
            </div>
            <div className="settings-row">
              <label>
                Disable Animations
                <span className="hint">Turn off all UI animations</span>
              </label>
              <label className="toggle">
                <input type="checkbox" checked={draft.disable_animations || false} onChange={handleAnimationsToggle} />
                <span className="toggle-slider"></span>
              </label>
            </div>
            {!draft.disable_animations && (
              <div className="settings-row animation-speed-row">
                <label>
                  Animation Speed
                  <span className="speed-value">{draft.animation_speed_ms ?? 300}ms</span>
                </label>
                <div className="animation-controls">
                  <input
                    type="range"
                    min="50"
                    max="600"
                    step="10"
                    value={draft.animation_speed_ms ?? 300}
                    onChange={(e) => handleAnimationSpeedChange(parseInt(e.target.value))}
                    className="speed-slider"
                  />
                  <button 
                    className="btn btn-small test-animation-btn"
                    onClick={() => {
                      const btn = document.querySelector('.test-animation-btn');
                      btn?.classList.add('animate-test');
                      setTimeout(() => btn?.classList.remove('animate-test'), draft.animation_speed_ms ?? 300);
                    }}
                  >
                    <Play size={14} />
                  </button>
                </div>
              </div>
            )}
            <div className="settings-row ui-scale-row">
              <label>
                Background
                <span className="hint">Choose the animated background style</span>
              </label>
              <div className="theme-buttons">
                {(draft.theme === 'minecraft'
                  ? [
                      { value: 'mc-terrain'  as BackgroundStyle, label: 'Terrain' },
                      { value: 'night-sky' as BackgroundStyle, label: 'Night Sky' },
                      { value: 'none'         as BackgroundStyle, label: 'None' },
                    ]
                  : [
                      { value: 'embers' as BackgroundStyle, label: 'Embers' },
                      { value: 'matrix' as BackgroundStyle, label: 'Matrix' },
                      { value: 'none'   as BackgroundStyle, label: 'None' },
                    ]
                ).map((opt) => (
                  <button
                    key={opt.value}
                    className={`btn ${(draft.background_style ?? (draft.theme === 'minecraft' ? 'mc-terrain' : 'embers')) === opt.value ? 'btn-primary' : 'btn-secondary'}`}
                    onClick={() => handleBackgroundStyleChange(opt.value)}
                  >
                    {opt.label}
                  </button>
                ))}
              </div>
            </div>
            {(draft.background_style ?? (draft.theme === 'minecraft' ? 'mc-terrain' : 'embers')) === 'embers' && (
              <>
                <div className="settings-row animation-speed-row">
                  <label>
                    Smoke Intensity
                    <span className="speed-value">{draft.background_smoke ?? 5}/10</span>
                  </label>
                  <input
                    type="range"
                    min="0"
                    max="10"
                    step="1"
                    value={draft.background_smoke ?? 5}
                    onChange={(e) => handleSmokeChange(parseInt(e.target.value))}
                    className="speed-slider"
                  />
                </div>
                <div className="settings-row animation-speed-row">
                  <label>
                    Red Blobs
                    <span className="speed-value">{draft.background_blobs ?? 5}/10</span>
                  </label>
                  <input
                    type="range"
                    min="0"
                    max="10"
                    step="1"
                    value={draft.background_blobs ?? 5}
                    onChange={(e) => handleBlobsChange(parseInt(e.target.value))}
                    className="speed-slider"
                  />
                </div>
              </>
            )}
            <div className="settings-row ui-scale-row">
              <label>
                UI Scale
                <span className="hint">Ctrl +/- to zoom, Ctrl+0 to reset</span>
              </label>
              <div className="ui-scale-buttons">
                {uiScaleOptions.map((scale) => (
                  <button
                    key={scale}
                    className={`btn ${draft.ui_scale === scale || (!draft.ui_scale && scale === 100) ? 'btn-primary' : 'btn-secondary'}`}
                    onClick={() => handleUiScaleChange(scale)}
                  >
                    {scale}%
                  </button>
                ))}
              </div>
            </div>
          </div>

          <div className="settings-section">
            <h3>Taskbar Icon</h3>
            <p className="settings-section-hint">Changes the icon shown in the Windows taskbar</p>
            <div className="settings-row">
              <label>
                Icon Style
              </label>
              <div className="icon-style-buttons">
                {iconStyleOptions.map((option) => (
                  <button
                    key={option.value}
                    className={`btn ${draft.taskbar_icon_style === option.value || (!draft.taskbar_icon_style && option.value === 'blackred') ? 'btn-primary' : 'btn-secondary'}`}
                    onClick={() => handleTaskbarIconStyleChange(option.value as 'blackred' | 'default')}
                  >
                    {option.label}
                  </button>
                ))}
              </div>
            </div>
            <div className="settings-row">
              <label>
                Icon Border
              </label>
              <label className="toggle">
                <input type="checkbox" checked={draft.taskbar_icon_border !== false} onChange={handleTaskbarIconBorderToggle} />
                <span className="toggle-slider"></span>
              </label>
            </div>
            <div className="icon-preview-row">
              <label>Preview</label>
              <div className="icon-preview">
                <img 
                  src={getIconPath(draft.taskbar_icon_style, draft.taskbar_icon_border)}
                  alt="Taskbar icon preview"
                  className="icon-preview-img"
                />
              </div>
            </div>
          </div>

          <div className="settings-section">
            <h3>In-App Icon</h3>
            <p className="settings-section-hint">Changes the icon shown inside the app header</p>
            <div className="settings-row">
              <label>
                Icon Style
              </label>
              <div className="icon-style-buttons">
                {iconStyleOptions.map((option) => (
                  <button
                    key={option.value}
                    className={`btn ${draft.app_icon_style === option.value || (!draft.app_icon_style && option.value === 'blackred') ? 'btn-primary' : 'btn-secondary'}`}
                    onClick={() => handleAppIconStyleChange(option.value as 'blackred' | 'default')}
                  >
                    {option.label}
                  </button>
                ))}
              </div>
            </div>
            <div className="settings-row">
              <label>
                Icon Border
              </label>
              <label className="toggle">
                <input type="checkbox" checked={draft.app_icon_border !== false} onChange={handleAppIconBorderToggle} />
                <span className="toggle-slider"></span>
              </label>
            </div>
            <div className="icon-preview-row">
              <label>Preview</label>
              <div className="icon-preview">
                <img 
                  src={getIconPath(draft.app_icon_style, draft.app_icon_border)}
                  alt="In-app icon preview"
                  className="icon-preview-img"
                />
              </div>
            </div>
          </div>
        </div>

        <div className="settings-modal-actions">
          <button className="btn btn-secondary" onClick={onClose}>
            Cancel
          </button>
          <button className="btn btn-primary" onClick={handleSave}>
            Save Settings
          </button>
        </div>
      </div>
    </div>
  );
}

export function SettingsButton({ onClick }: { onClick: () => void }) {
  return (
    <button className="btn btn-icon" onClick={onClick} title="Settings">
      <SettingsIcon size={20} />
    </button>
  );
}

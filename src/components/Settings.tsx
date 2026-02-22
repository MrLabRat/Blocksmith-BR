import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { Settings as SettingsType, PackType, BackgroundStyle } from '../types';
import { Settings as SettingsIcon, FolderOpen, Search, Play, X } from 'lucide-react';

interface SettingsProps {
  settings: SettingsType;
  onSettingsChange: (settings: SettingsType) => void;
  isOpen: boolean;
  onClose: () => void;
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

export function Settings({ settings, onSettingsChange, isOpen, onClose }: SettingsProps) {
  const handleSelectPath = async (key: keyof SettingsType) => {
    const selected = await open({
      directory: true,
      multiple: false,
      title: `Select ${pathConfigs.find(p => p.key === key)?.label || ''} Directory`,
    });

    if (selected && typeof selected === 'string') {
      onSettingsChange({
        ...settings,
        [key]: selected,
      });
    }
  };

  const handleDryRunToggle = () => {
    onSettingsChange({
      ...settings,
      dry_run: !settings.dry_run,
    });
  };

  const handleDeleteSourceToggle = () => {
    onSettingsChange({
      ...settings,
      delete_source: !settings.delete_source,
    });
  };

  const handleAnimationsToggle = () => {
    onSettingsChange({
      ...settings,
      disable_animations: !settings.disable_animations,
    });
  };

  const handleAnimationSpeedChange = (speedMs: number) => {
    onSettingsChange({
      ...settings,
      animation_speed_ms: speedMs,
    });
  };

  const handleUiScaleChange = (scale: number) => {
    onSettingsChange({
      ...settings,
      ui_scale: scale,
    });
  };

  const handleTaskbarIconStyleChange = (style: 'blackred' | 'default') => {
    const newSettings = {
      ...settings,
      taskbar_icon_style: style,
    };
    onSettingsChange(newSettings);
    updateWindowIcon(style, settings.taskbar_icon_border !== false);
  };

  const handleTaskbarIconBorderToggle = () => {
    const newBordered = settings.taskbar_icon_border === false;
    const newSettings = {
      ...settings,
      taskbar_icon_border: newBordered,
    };
    onSettingsChange(newSettings);
    updateWindowIcon(settings.taskbar_icon_style || 'blackred', newBordered);
  };

  const handleAppIconStyleChange = (style: 'blackred' | 'default') => {
    onSettingsChange({
      ...settings,
      app_icon_style: style,
    });
  };

  const handleAppIconBorderToggle = () => {
    onSettingsChange({
      ...settings,
      app_icon_border: settings.app_icon_border === false,
    });
  };

  const handleDebugModeToggle = () => {
    onSettingsChange({
      ...settings,
      debug_mode: !settings.debug_mode,
    });
  };

  const handleThemeChange = (theme: 'darkred' | 'minecraft') => {
    const defaultBg: BackgroundStyle = theme === 'minecraft' ? 'mc-terrain' : 'embers';
    onSettingsChange({ ...settings, theme, background_style: defaultBg });
  };

  const handleBackgroundStyleChange = (bg: BackgroundStyle) => {
    onSettingsChange({ ...settings, background_style: bg });
  };

  const handleSmokeChange = (v: number) => {
    onSettingsChange({ ...settings, background_smoke: v });
  };

  const handleBlobsChange = (v: number) => {
    onSettingsChange({ ...settings, background_blobs: v });
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
      onSettingsChange({
        ...settings,
        behavior_pack_path: detected.behavior_pack_path ?? settings.behavior_pack_path,
        resource_pack_path: detected.resource_pack_path ?? settings.resource_pack_path,
        skin_pack_path: detected.skin_pack_path ?? settings.skin_pack_path,
        world_template_path: detected.world_template_path ?? settings.world_template_path,
        scan_location: detected.scan_location ?? settings.scan_location,
      });
    } catch (error) {
      console.error('Auto-detect failed:', error);
    }
  };

  const handleSave = async () => {
    try {
      await invoke('save_settings', { settings });
      onClose();
    } catch (error) {
      console.error('Failed to save settings:', error);
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
                    value={(settings[key] as string) || ''}
                    onChange={(e) =>
                      onSettingsChange({
                        ...settings,
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
                Dry Run Mode
                <span className="hint">Preview without extracting/moving files</span>
              </label>
              <label className="toggle">
                <input type="checkbox" checked={settings.dry_run} onChange={handleDryRunToggle} />
                <span className="toggle-slider"></span>
              </label>
            </div>
            <div className="settings-row">
              <label>
                Delete Source Files
                <span className="hint">Remove .mcpack/.mcaddon after successful extraction</span>
              </label>
              <label className="toggle">
                <input type="checkbox" checked={settings.delete_source} onChange={handleDeleteSourceToggle} />
                <span className="toggle-slider"></span>
              </label>
            </div>
            <div className="settings-row">
              <label>
                Debug Mode
                <span className="hint">Show detailed logs for troubleshooting</span>
              </label>
              <label className="toggle">
                <input type="checkbox" checked={settings.debug_mode || false} onChange={handleDebugModeToggle} />
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
                  checked={!(settings.disable_tip_notifications || false)}
                  onChange={() =>
                    onSettingsChange({
                      ...settings,
                      disable_tip_notifications: !settings.disable_tip_notifications,
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
                    className={`btn ${settings.theme === option.value || (!settings.theme && option.value === 'darkred') ? 'btn-primary' : 'btn-secondary'}`}
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
                <input type="checkbox" checked={settings.disable_animations || false} onChange={handleAnimationsToggle} />
                <span className="toggle-slider"></span>
              </label>
            </div>
            {!settings.disable_animations && (
              <div className="settings-row animation-speed-row">
                <label>
                  Animation Speed
                  <span className="speed-value">{settings.animation_speed_ms ?? 300}ms</span>
                </label>
                <div className="animation-controls">
                  <input
                    type="range"
                    min="50"
                    max="600"
                    step="10"
                    value={settings.animation_speed_ms ?? 300}
                    onChange={(e) => handleAnimationSpeedChange(parseInt(e.target.value))}
                    className="speed-slider"
                  />
                  <button 
                    className="btn btn-small test-animation-btn"
                    onClick={() => {
                      const btn = document.querySelector('.test-animation-btn');
                      btn?.classList.add('animate-test');
                      setTimeout(() => btn?.classList.remove('animate-test'), settings.animation_speed_ms ?? 300);
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
                {(settings.theme === 'minecraft'
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
                    className={`btn ${(settings.background_style ?? (settings.theme === 'minecraft' ? 'mc-terrain' : 'embers')) === opt.value ? 'btn-primary' : 'btn-secondary'}`}
                    onClick={() => handleBackgroundStyleChange(opt.value)}
                  >
                    {opt.label}
                  </button>
                ))}
              </div>
            </div>
            {(settings.background_style ?? (settings.theme === 'minecraft' ? 'mc-terrain' : 'embers')) === 'embers' && (
              <>
                <div className="settings-row animation-speed-row">
                  <label>
                    Smoke Intensity
                    <span className="speed-value">{settings.background_smoke ?? 5}/10</span>
                  </label>
                  <input
                    type="range"
                    min="0"
                    max="10"
                    step="1"
                    value={settings.background_smoke ?? 5}
                    onChange={(e) => handleSmokeChange(parseInt(e.target.value))}
                    className="speed-slider"
                  />
                </div>
                <div className="settings-row animation-speed-row">
                  <label>
                    Red Blobs
                    <span className="speed-value">{settings.background_blobs ?? 5}/10</span>
                  </label>
                  <input
                    type="range"
                    min="0"
                    max="10"
                    step="1"
                    value={settings.background_blobs ?? 5}
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
                    className={`btn ${settings.ui_scale === scale || (!settings.ui_scale && scale === 100) ? 'btn-primary' : 'btn-secondary'}`}
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
                    className={`btn ${settings.taskbar_icon_style === option.value || (!settings.taskbar_icon_style && option.value === 'blackred') ? 'btn-primary' : 'btn-secondary'}`}
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
                <input type="checkbox" checked={settings.taskbar_icon_border !== false} onChange={handleTaskbarIconBorderToggle} />
                <span className="toggle-slider"></span>
              </label>
            </div>
            <div className="icon-preview-row">
              <label>Preview</label>
              <div className="icon-preview">
                <img 
                  src={getIconPath(settings.taskbar_icon_style, settings.taskbar_icon_border)}
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
                    className={`btn ${settings.app_icon_style === option.value || (!settings.app_icon_style && option.value === 'blackred') ? 'btn-primary' : 'btn-secondary'}`}
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
                <input type="checkbox" checked={settings.app_icon_border !== false} onChange={handleAppIconBorderToggle} />
                <span className="toggle-slider"></span>
              </label>
            </div>
            <div className="icon-preview-row">
              <label>Preview</label>
              <div className="icon-preview">
                <img 
                  src={getIconPath(settings.app_icon_style, settings.app_icon_border)}
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

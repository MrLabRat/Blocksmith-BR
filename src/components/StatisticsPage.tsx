import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { X } from 'lucide-react';
import { PackStats, PackTypeColors } from '../types';
import { formatBytes } from '../utils/packUtils';
import '../styles/StatisticsPage.css';

interface StatisticsPageProps {
  onClose: () => void;
}

const packTypeLabels: Record<string, string> = {
  'BehaviorPack': 'Behavior Packs',
  'ResourcePack': 'Resource Packs',
  'SkinPack': 'Skin Packs',
  'SkinPack4D': 'Skin Packs (4D)',
  'WorldTemplate': 'World Templates',
  'MashupPack': 'Mash-Up Packs',
};

export function StatisticsPage({ onClose }: StatisticsPageProps) {
  const [stats, setStats] = useState<PackStats[]>([]);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    const loadStats = async () => {
      try {
        const data = await invoke<PackStats[]>('get_installed_packs_stats');
        setStats(data);
      } catch (error) {
        console.error('Failed to load statistics:', error);
      } finally {
        setIsLoading(false);
      }
    };

    loadStats();
  }, []);

  const totalPacks = stats.reduce((sum, s) => sum + s.count, 0);
  const totalSize = stats.reduce((sum, s) => sum + s.total_size, 0);

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal modal-large" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h3>Pack Statistics</h3>
          <button className="btn btn-icon" onClick={onClose}>
            <X size={20} />
          </button>
        </div>
        <div className="modal-content statistics-content">
          {isLoading ? (
            <div className="loading">Loading statistics...</div>
          ) : stats.length > 0 ? (
            <>
              <div className="stats-summary">
                <div className="stat-card">
                  <div className="stat-label">Total Packs</div>
                  <div className="stat-value">{totalPacks}</div>
                </div>
                <div className="stat-card">
                  <div className="stat-label">Total Size</div>
                  <div className="stat-value" style={{ fontSize: '16px' }}>
                    {formatBytes(totalSize)}
                  </div>
                </div>
              </div>

              <div className="stats-list">
                {stats.map((stat) => (
                  <div key={stat.pack_type} className="stat-row">
                    <div 
                     className="stat-row-icon"
                     style={{ backgroundColor: PackTypeColors[stat.pack_type as keyof typeof PackTypeColors] || '#6b7280' }}
                   />
                   <div className="stat-row-info">
                     <div className="stat-row-name">{packTypeLabels[stat.pack_type] || stat.pack_type}</div>
                      <div className="stat-row-size">{stat.total_size_formatted}</div>
                    </div>
                    <div className="stat-row-count">{stat.count}</div>
                  </div>
                ))}
              </div>
            </>
          ) : (
            <div className="no-stats">No packs installed yet.</div>
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


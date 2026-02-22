import { useState } from 'react';
import { Menu, X, HelpCircle, Info, RotateCcw, Trash2, Package } from 'lucide-react';
import '../styles/HamburgerMenu.css';

interface HamburgerMenuProps {
  onDeleteAllPacks: () => void;
  onRestart: () => void;
  onShowStats: () => void;
  onShowInstalledPacks: () => void;
  onShowHelp: () => void;
}

export function HamburgerMenu({ onDeleteAllPacks, onRestart, onShowStats, onShowInstalledPacks, onShowHelp }: HamburgerMenuProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [showConfirm, setShowConfirm] = useState(false);

  const handleDeleteAllClick = () => {
    setShowConfirm(true);
  };

  const handleConfirmDelete = () => {
    setShowConfirm(false);
    setIsOpen(false);
    onDeleteAllPacks();
  };

  const handleRestart = () => {
    setIsOpen(false);
    onRestart();
  };

  const handleShowStats = () => {
    setIsOpen(false);
    onShowStats();
  };

  const handleShowInstalledPacks = () => {
    setIsOpen(false);
    onShowInstalledPacks();
  };

  const handleShowHelp = () => {
    setIsOpen(false);
    onShowHelp();
  };

  return (
    <div className="hamburger-menu">
      <button 
        className="hamburger-toggle"
        onClick={() => setIsOpen(!isOpen)}
        title="Menu"
      >
        {isOpen ? <X size={24} /> : <Menu size={24} />}
      </button>

      {isOpen && (
        <>
          <div className="hamburger-overlay" onClick={() => setIsOpen(false)} />
          <div className="hamburger-dropdown">
            <button className="menu-item" onClick={handleShowInstalledPacks}>
              <Package size={18} />
              <span>Installed Packs</span>
            </button>
            <button className="menu-item" onClick={handleShowStats}>
              <Info size={18} />
              <span>Statistics</span>
            </button>
            <button className="menu-item" onClick={handleShowHelp}>
              <HelpCircle size={18} />
              <span>Help & Feedback</span>
            </button>
            <button className="menu-item" onClick={handleRestart}>
              <RotateCcw size={18} />
              <span>Restart App</span>
            </button>
            <button className="menu-item menu-item-danger" onClick={handleDeleteAllClick}>
              <Trash2 size={18} />
              <span>Delete All Packs</span>
            </button>
          </div>
        </>
      )}

      {showConfirm && (
        <div className="modal-overlay" onClick={() => setShowConfirm(false)}>
          <div className="modal modal-small" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h3>Confirm Delete All Packs</h3>
            </div>
            <div className="modal-content">
              <p style={{ marginBottom: '16px' }}>
                This will delete ALL packs from all folders. This action cannot be undone.
              </p>
              <p style={{ color: 'var(--error-color)', fontWeight: '500' }}>
                Are you absolutely sure?
              </p>
            </div>
            <div className="modal-actions">
              <button className="btn" onClick={() => setShowConfirm(false)}>
                Cancel
              </button>
              <button className="btn btn-danger" onClick={handleConfirmDelete}>
                Delete All Packs
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

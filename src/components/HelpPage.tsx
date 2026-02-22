import { X, ExternalLink, HelpCircle, Package, Users, Globe, Lock } from 'lucide-react';
import { openUrl } from '@tauri-apps/plugin-opener';
import '../styles/HelpPage.css';

interface HelpPageProps {
  onClose: () => void;
}

export function HelpPage({ onClose }: HelpPageProps) {
  const openGitHub = async (path: string = '') => {
    try {
      await openUrl(`https://github.com/MrLabRat/BlockBench-BR${path}`);
    } catch (error) {
      console.error('Failed to open URL:', error);
    }
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal modal-large" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h3>Help & Feedback</h3>
          <button className="btn btn-icon" onClick={onClose}>
            <X size={20} />
          </button>
        </div>
        <div className="modal-content help-content">
          <div className="help-section">
            <h4><HelpCircle size={18} /> What is Blocksmith?</h4>
            <p>
              Blocksmith is a Minecraft Bedrock pack manager that helps you organize, move, and manage your 
              Behavior Packs, Resource Packs, Skin Packs, and World Templates. It automatically detects 
              your Minecraft Bedrock installation and provides an easy interface to import new packs from 
              .mcaddon, .mcpack, or .zip files.
            </p>
          </div>

          <div className="help-section">
            <h4><Package size={18} /> Pack Types Explained</h4>
            <div className="pack-type-list">
              <div className="pack-type-item">
                <div className="pack-type-badge" style={{ backgroundColor: '#4f46e5' }}>BP</div>
                <div className="pack-type-info">
                  <strong>Behavior Packs</strong>
                  <p>Add custom gameplay mechanics, items, entities, and game logic. They modify how the game behaves.</p>
                </div>
              </div>
              <div className="pack-type-item">
                <div className="pack-type-badge" style={{ backgroundColor: '#059669' }}>RP</div>
                <div className="pack-type-info">
                  <strong>Resource Packs</strong>
                  <p>Change the look and sound of the game - textures, models, sounds, animations, and UI elements.</p>
                </div>
              </div>
              <div className="pack-type-item">
                <div className="pack-type-badge" style={{ backgroundColor: '#dc2626' }}>SP</div>
                <div className="pack-type-info">
                  <strong>Skin Packs</strong>
                  <p>Collections of character skins that can be used in-game. Regular skin packs work directly.</p>
                </div>
              </div>
              <div className="pack-type-item">
                <div className="pack-type-badge" style={{ backgroundColor: '#9333ea' }}>4D</div>
                <div className="pack-type-info">
                  <strong>Skin Packs (4D Geometry)</strong>
                  <p>Advanced skins with custom geometry/models. These require special handling - use SkinMaster for full support.</p>
                </div>
              </div>
              <div className="pack-type-item">
                <div className="pack-type-badge" style={{ backgroundColor: '#0891b2' }}>WT</div>
                <div className="pack-type-info">
                  <strong>World Templates</strong>
                  <p>Pre-made worlds that can be used as starting points for new creations. Include custom terrain, structures, and settings.</p>
                </div>
              </div>
            </div>
          </div>

          <div className="help-section">
            <h4><Lock size={18} /> What is SkinMaster?</h4>
            <p>
              SkinMaster is a specialized tool for handling 4D geometry skin packs with encryption support. 
              Blocksmith cannot encrypt 4D skins the way SkinMaster does. For full 4D skin pack functionality, 
              please use SkinMaster directly.
            </p>
            <button className="btn btn-secondary btn-sm" onClick={() => openGitHub('')}>
              <ExternalLink size={14} /> Learn More
            </button>
          </div>

          <div className="help-section">
            <h4><Users size={18} /> Frequently Asked Questions</h4>
            <div className="faq-list">
              <details className="faq-item">
                <summary>Where are my packs stored?</summary>
                <p>
                  Packs are stored in your Minecraft Bedrock directory, typically at:
                  <code>%APPDATA%\Minecraft Bedrock\Users\[USER_ID]\games\com.mojang\</code>
                </p>
              </details>
              <details className="faq-item">
                <summary>Why isn't my pack showing up in-game?</summary>
                <p>
                  Make sure the pack was imported correctly. Check the Installed Packs page to verify 
                  it's in the right folder. For world templates, create a new world and select the template.
                </p>
              </details>
              <details className="faq-item">
                <summary>Can I undo pack moves?</summary>
                <p>
                  Yes! After processing packs, you can use the "Rollback Last" button to undo the most recent 
                  batch of moves. This only works for the last operation.
                </p>
              </details>
              <details className="faq-item">
                <summary>What file formats are supported?</summary>
                <p>
                  Blocksmith supports .mcaddon, .mcpack, and .zip files. These are automatically scanned 
                  and organized by pack type.
                </p>
              </details>
              <details className="faq-item">
                <summary>Why do 4D skins need special handling?</summary>
                <p>
                  4D geometry skins use encrypted geometry files that require specific tools to properly 
                  encrypt and import. Use SkinMaster for full 4D skin support.
                </p>
              </details>
            </div>
          </div>

          <div className="help-section feedback-section">
            <h4><Globe size={18} /> Feedback & Support</h4>
            <p>Found a bug or have a feature request? Visit the GitHub repository:</p>
            <div className="feedback-buttons">
              <button className="btn btn-primary" onClick={() => openGitHub('/issues')}>
                <ExternalLink size={16} /> Report an Issue
              </button>
              <button className="btn btn-secondary" onClick={() => openGitHub('/discussions')}>
                <ExternalLink size={16} /> Discussions
              </button>
              <button className="btn btn-secondary" onClick={() => openGitHub()}>
                <ExternalLink size={16} /> View Repository
              </button>
            </div>
          </div>
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

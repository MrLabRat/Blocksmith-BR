import { useEffect, useRef, useState } from 'react';
import { LogEntry } from '../types';
import '../styles/AnimatedLogViewer.css';

interface AnimatedLogViewerProps {
  logs: LogEntry[];
}

const CHARS_PER_FRAME = 3;

export function AnimatedLogViewer({ logs }: AnimatedLogViewerProps) {
  const [displayedLogs, setDisplayedLogs] = useState<(LogEntry & { displayedText: string })[]>([]);
  const stateRef = useRef<{
    fullTexts: string[];
    charPositions: number[];
    processedCount: number;
    rafId: number | null;
  }>({ fullTexts: [], charPositions: [], processedCount: 0, rafId: null });

  useEffect(() => {
    if (logs.length === 0) {
      const st = stateRef.current;
      if (st.rafId !== null) {
        cancelAnimationFrame(st.rafId);
        st.rafId = null;
      }
      st.fullTexts = [];
      st.charPositions = [];
      st.processedCount = 0;
      setDisplayedLogs([]);
      return;
    }

    const st = stateRef.current;
    const newLogs = logs.slice(st.processedCount);
    if (newLogs.length === 0) return;

    for (const newLog of newLogs) {
      st.fullTexts.push(`[${newLog.timestamp}] ${newLog.level}: ${newLog.message}`);
      st.charPositions.push(0);
    }
    st.processedCount = logs.length;

    setDisplayedLogs((prev) => {
      const updated = [...prev];
      for (let i = prev.length; i < logs.length; i++) {
        updated[i] = { ...logs[i], displayedText: '' };
      }
      return updated;
    });

    if (st.rafId !== null) return;

    const tick = () => {
      let anyActive = false;
      const updates: { index: number; text: string }[] = [];

      for (let i = 0; i < st.fullTexts.length; i++) {
        const full = st.fullTexts[i];
        if (st.charPositions[i] < full.length) {
          st.charPositions[i] = Math.min(st.charPositions[i] + CHARS_PER_FRAME, full.length);
          updates.push({ index: i, text: full.slice(0, st.charPositions[i]) });
          anyActive = true;
        }
      }

      if (updates.length > 0) {
        setDisplayedLogs((prev) => {
          const updated = [...prev];
          for (const { index, text } of updates) {
            if (updated[index]) {
              updated[index] = { ...updated[index], displayedText: text };
            }
          }
          return updated;
        });
      }

      if (anyActive) {
        st.rafId = requestAnimationFrame(tick);
      } else {
        st.rafId = null;
      }
    };

    st.rafId = requestAnimationFrame(tick);

    return () => {
      if (st.rafId !== null) {
        cancelAnimationFrame(st.rafId);
        st.rafId = null;
      }
    };
  }, [logs]);

  return (
    <div className="log-viewer">
      <div className="log-container">
        {displayedLogs.length === 0 ? (
          <div className="log-empty">Waiting for logs...</div>
        ) : (
          displayedLogs.map((log, idx) => (
            <div key={idx} className="log-entry">
              <span className={`log-level log-${log.level.toLowerCase()}`}>
                {log.level}
              </span>
              <span className="log-message">{log.displayedText}</span>
            </div>
          ))
        )}
      </div>
    </div>
  );
}

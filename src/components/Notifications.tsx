import { useCallback, useEffect, useState } from 'react';
import { AppNotification } from '../types';
import { X, AlertCircle, AlertTriangle, Info, CheckCircle } from 'lucide-react';

interface NotificationsProps {
  notifications: AppNotification[];
  onDismiss: (id: string) => void;
}

export function Notifications({ notifications, onDismiss }: NotificationsProps) {
  return (
    <div className="notifications-container">
      {notifications.map((notification) => (
        <NotificationItem 
          key={notification.id} 
          notification={notification} 
          onDismiss={onDismiss} 
        />
      ))}
    </div>
  );
}

function NotificationItem({ notification, onDismiss }: { notification: AppNotification; onDismiss: (id: string) => void }) {
  const [isExiting, setIsExiting] = useState(false);

  const handleDismiss = useCallback(() => {
    setIsExiting(true);
    setTimeout(() => onDismiss(notification.id), 300);
  }, [onDismiss, notification.id]);

  useEffect(() => {
    const timer = setTimeout(() => {
      handleDismiss();
    }, 8000);

    return () => clearTimeout(timer);
  }, [handleDismiss]);

  const icons = {
    error: AlertCircle,
    warning: AlertTriangle,
    info: Info,
    success: CheckCircle,
  };

  const Icon = icons[notification.type];

  return (
    <div className={`notification notification-${notification.type} ${isExiting ? 'notification-exit' : ''}`}>
      <div className="notification-icon">
        <Icon size={20} />
      </div>
      <div className="notification-content">
        <div className="notification-title">{notification.title}</div>
        <div className="notification-message">{notification.message}</div>
      </div>
      <button className="notification-close" onClick={handleDismiss}>
        <X size={16} />
      </button>
    </div>
  );
}

export function createNotification(type: AppNotification['type'], title: string, message: string): AppNotification {
  return {
    id: `${Date.now()}-${Math.random().toString(36).substr(2, 9)}`,
    type,
    title,
    message,
    timestamp: Date.now(),
  };
}

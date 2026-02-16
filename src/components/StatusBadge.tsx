import { Item } from '../types';

interface StatusBadgeProps {
  status: Item['status'];
}

export function StatusBadge({ status }: StatusBadgeProps) {
  const icons: Record<Item['status'], string> = {
    waiting: '⏸️',
    in_progress: '⏳',
    updated: '✅',
    completed: '✔️',
    failed: '❌',
  };

  return (
    <span className={`status-badge status-${status}`}>
      {icons[status]} {status.replace('_', ' ')}
    </span>
  );
}

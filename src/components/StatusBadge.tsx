import { Item } from '../types';

interface StatusBadgeProps {
  status: Item['status'];
}

export function StatusBadge({ status }: StatusBadgeProps) {
  return (
    <span className={`status-badge status-${status}`}>
      {status.replace('_', ' ')}
    </span>
  );
}

import { Item } from '../types';

interface StatusBadgeProps {
  status: Item['status'];
}

const statusLabels: Partial<Record<Item['status'], string>> = {
  input_needed: 'input needed',
};

export function StatusBadge({ status }: StatusBadgeProps) {
  const label = statusLabels[status] ?? status.replace('_', ' ');
  return (
    <span className={`status-badge status-${status}`}>
      {label}
    </span>
  );
}

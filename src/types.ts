export type ItemType = 
  | 'slack_thread' 
  | 'github_action' 
  | 'github_pr' 
  | 'copilot_agent' 
  | 'cli_session'
  | 'opencode_session';

export type ItemStatus = 
  | 'waiting' 
  | 'in_progress' 
  | 'input_needed'
  | 'updated' 
  | 'approved'
  | 'merged'
  | 'completed' 
  | 'failed'
  | 'archived';

export interface Item {
  id: string;
  type: ItemType;
  title: string;
  url?: string;
  status: ItemStatus;
  previous_status?: ItemStatus;
  metadata: Record<string, any>;
  last_checked_at?: string;
  last_updated_at?: string;
  created_at: string;
  archived: boolean;
  archived_at?: string;
  polling_interval_override?: number;
  checked: boolean;
}

export interface Credentials {
  slack_token?: string;
  github_token?: string;
  opencode_url?: string;
  opencode_password?: string;
}

export interface Settings {
  polling_interval: number;
  screen_width: number;
}

export interface AddItemRequest {
  url: string;
  custom_title?: string;
}

export interface ParsedUrl {
  type: ItemType;
  metadata: Record<string, any>;
  suggested_title: string;
}

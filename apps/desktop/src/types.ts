export interface Commit {
  oid: string;
  parents: string[];
  tree_oid: string;
  author_name: string;
  author_email: string;
  committer_name: string;
  committer_email: string;
  author_date: string;
  subject: string;
  blockers: string[];
}
export interface Scan {
  id: string;
  repository_path: string;
  repository_name: string;
  head: string;
  branch: string;
  commits: Commit[];
  total_commits: number;
  blockers: string[];
  scanned_at: string;
  synthetic?: boolean;
  import_source?: {
    repository_full_name: string;
    fetched_at: number;
  };
}
export interface Plan {
  id: string;
  scan_id: string;
  plan_hash: string;
  selected_oids: string[];
  affected_oids: string[];
  baseline_oid: string;
  blockers: string[];
  edits: { oid: string; field: string; before: string; after: string }[];
}
export interface ValidationCheck {
  name: string;
  passed: boolean;
  detail: string;
}
export interface Operation {
  id: string;
  plan_id: string;
  repository_name: string;
  state: string;
  created_at: string;
  selected_count: number;
  rewritten_count: number;
  baseline_oid: string;
  candidate_oid?: string;
  backup_path?: string;
  validation_checks: ValidationCheck[];
  blockers: string[];
  report_path?: string;
  repository_status?: string;
  calendar_status?: string;
}
export interface AuthStatus {
  connected: boolean;
  login?: string;
  message?: string;
  client_id_configured: boolean;
  retry_after_seconds?: number;
}
export interface DeviceFlow {
  flow_id: string;
  user_code: string;
  verification_uri: string;
  interval: number;
  expires_in: number;
}
export interface Dashboard {
  auth: AuthStatus;
  operations: Operation[];
  toolchain: { name: string; version: string; available: boolean }[];
  data_directory?: string;
  writer_enabled: boolean;
}
export interface ContributionDay {
  date: string;
  count: number | null;
  level: 0 | 1 | 2 | 3 | 4 | null;
}
export interface ContributionSnapshot {
  year: number;
  fetched_at: string;
  days: ContributionDay[];
  partial: boolean;
  coverage_note: string;
  years?: number[];
  restricted?: number | null;
  stale?: boolean;
  fetch_error?: string;
}
export interface DaySnapshot {
  date: string;
  fetched_at?: string | number;
  stale?: boolean;
  fetch_error?: string;
  observed_total: number | null;
  partial: boolean;
  issues?: { code: string; message: string }[];
  coverage: string;
  totals: {
    commits: number | null;
    issues: number | null;
    pull_requests: number | null;
    reviews: number | null;
    repositories_created: number | null;
    restricted: number | null;
  };
  repositories: {
    full_name: string;
    private: boolean;
    observed_commit_count: number | null;
    partial: boolean;
  }[];
}
export interface IdentityEvidence {
  emails: {
    email: string;
    verified: boolean;
    primary: boolean;
    visibility: string | null;
  }[];
  fetched_at: string;
  coverage_note: string;
}
export interface RepositoryList {
  partial: boolean;
  issues: { code: string; message: string }[];
  installations: {
    id: number;
    account: { login: string };
    repository_selection: string;
    suspended: boolean;
    permissions?: Record<string, string>;
  }[];
  repositories: {
    id: number;
    full_name: string;
    private: boolean;
    fork: boolean;
    archived: boolean;
    default_branch: string | null;
    contents_read: boolean;
    contents_write: boolean;
    permission_status: string;
  }[];
}

import { invoke, isTauri } from "@tauri-apps/api/core";
import type {
  AuthStatus,
  ContributionSnapshot,
  Dashboard,
  DaySnapshot,
  DeviceFlow,
  IdentityEvidence,
  Operation,
  Plan,
  RepositoryList,
  Scan,
} from "./types";
export const isNative = () => isTauri();
async function call<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (!isNative())
    throw new Error(
      "Open the native un-git app to access local repositories and macOS Keychain. Browser preview does not run Git commands.",
    );
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw new Error(
      typeof error === "string"
        ? error
        : error && typeof error === "object" && "message" in error
          ? String(error.message)
          : JSON.stringify(error),
    );
  }
}
export const bridge = {
  dashboard: () => call<Dashboard>("get_dashboard"),
  demo: () => call<{ repository_path: string; label: string }>("create_demo"),
  scan: (repositoryPath: string) =>
    call<Scan>("scan_repository", { repositoryPath }),
  plan: (scanId: string, selectedOids: string[], confirmedEmail: string) =>
    call<Plan>("plan_selection", { scanId, selectedOids, confirmedEmail }),
  prepare: (planId: string) => call<Operation>("prepare_plan", { planId }),
  operation: (operationId: string) =>
    call<Operation>("get_operation", { operationId }),
  verify: (operationId: string) =>
    call<Operation>("verify_operation", { operationId }),
  cancel: (operationId: string) =>
    call<void>("cancel_operation", { operationId }),
  reportPreview: (operationId: string) =>
    call<Record<string, unknown>>("preview_report", { operationId }),
  export: (operationId: string) =>
    call<{ path: string }>("export_report", { operationId }),
  authLogout: () => call<AuthStatus>("auth_logout"),
  authStatus: () => call<AuthStatus>("auth_status"),
  authStart: (clientId: string) => call<DeviceFlow>("auth_start", { clientId }),
  authPoll: (flowId: string) => call<AuthStatus>("auth_poll", { flowId }),
  authCancel: (flowId: string) => call<void>("auth_cancel", { flowId }),
  calendar: (year: number) =>
    call<ContributionSnapshot>("fetch_calendar", { year }),
  day: (date: string) => call<DaySnapshot>("fetch_day", { date }),
  repositories: () => call<RepositoryList>("github_repositories"),
  importRepository: (repositoryId: number) =>
    call<Scan>("import_github_repository", { repositoryId }),
  identities: () => call<IdentityEvidence>("github_identity_evidence"),
  openAuthorization: () => call<void>("open_github_device"),
  openRepositoryAccess: () => call<void>("open_github_repository_access"),
};

import { useRef, useState } from "react";
import {
  Download,
  ExternalLink,
  Github,
  LockKeyhole,
  RefreshCw,
} from "lucide-react";
import { bridge } from "../bridge";
import type { RepositoryList } from "../types";
import { Button } from "./ui/button";

export function RemoteRepositories({
  busy,
  onImport,
}: {
  busy: boolean;
  onImport: (repositoryId: number) => Promise<void>;
}) {
  const [data, setData] = useState<RepositoryList | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [openingAccess, setOpeningAccess] = useState(false);
  const [accessError, setAccessError] = useState("");
  const [importingId, setImportingId] = useState<number | null>(null);
  const [importError, setImportError] = useState("");
  const importActive = useRef(false);
  const controlsBusy = busy || importingId !== null;
  async function importRepository(
    repositoryId: number,
    repositoryName: string,
  ) {
    if (controlsBusy || loading || importActive.current) return;
    importActive.current = true;
    setImportingId(repositoryId);
    setImportError("");
    try {
      await onImport(repositoryId);
    } catch (e) {
      setImportError(
        `Could not import ${repositoryName}: ${e instanceof Error ? e.message : String(e)}`,
      );
    } finally {
      importActive.current = false;
      setImportingId(null);
    }
  }
  async function manageAccess() {
    setOpeningAccess(true);
    setAccessError("");
    try {
      await bridge.openRepositoryAccess();
    } catch (e) {
      setAccessError(e instanceof Error ? e.message : String(e));
    } finally {
      setOpeningAccess(false);
    }
  }
  async function load() {
    setLoading(true);
    setError("");
    try {
      setData(await bridge.repositories());
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }
  return (
    <section className="remote-repositories">
      <div className="section-title">
        <h2>
          <Github size={17} /> Installed repository access
        </h2>
        <div className="flex flex-wrap justify-end gap-2">
          <Button
            variant="secondary"
            disabled={controlsBusy || openingAccess}
            onClick={() => void manageAccess()}
          >
            <ExternalLink size={14} />
            {openingAccess ? "Opening GitHub…" : "Manage repository access"}
          </Button>
          <Button
            variant="secondary"
            disabled={controlsBusy || loading}
            onClick={() => void load()}
          >
            <RefreshCw size={14} />
            {loading ? "Checking…" : "Refresh access"}
          </Button>
        </div>
      </div>
      <p className="subtle">
        To add repositories, open Manage repository access, choose Configure
        beside your Inspector app, select the repositories, and save. Then
        return here and choose Refresh access. Account authorization and
        repository installation are separate.
      </p>
      <p className="subtle">
        Import and inspect creates an isolated local snapshot using read-only
        GitHub access. Your repository files and history are preserved. You can
        also open an existing local clone using the path above.
      </p>
      {accessError && (
        <p role="alert" className="text-warning">
          Could not open GitHub settings: {accessError}. Try Manage repository
          access again, or visit github.com/settings/installations.
        </p>
      )}
      {error && (
        <p role="alert" className="text-warning">
          {error} Choose Refresh access to retry.
          {data && " The list below is from the last successful check."}
        </p>
      )}
      {importError && (
        <p role="alert" className="text-warning">
          {importError} Choose Import and inspect on that repository to retry.
        </p>
      )}
      {data && (
        <>
          {data.installations.some((installation) =>
            Object.values(installation.permissions ?? {}).some(
              (permission) => permission === "write" || permission === "admin",
            ),
          ) && (
            <div className="notice notice-warning" role="alert">
              This installation has write permissions. The Inspector only needs
              read access. Review the App permissions in GitHub settings; this
              build cannot publish repository changes.
            </div>
          )}
          <p>
            {data.installations.length} installation(s) ·{" "}
            {data.repositories.length} visible repositories
            {data.partial ? " · Incomplete coverage" : ""}
          </p>
          {data.installations.length === 0 && (
            <div className="notice">
              Install your Inspector GitHub App on selected repositories in
              GitHub, then choose Refresh access.
            </div>
          )}
          {data.issues.map((issue) => (
            <p className="text-warning" key={issue.code}>
              {issue.message}
            </p>
          ))}
          <div className="remote-repository-list">
            {data.repositories.map((repo) => (
              <div className="setting-row" key={repo.id}>
                <LockKeyhole size={16} />
                <div>
                  <strong>{repo.full_name}</strong>
                  <p>
                    {repo.private ? "Private" : "Public"} ·{" "}
                    {repo.default_branch || "No default branch"}
                    {repo.fork ? " · Fork" : ""}
                    {repo.archived ? " · Archived" : ""}
                  </p>
                </div>
                <span className="badge">
                  {repo.contents_write
                    ? "Contents read and write"
                    : repo.contents_read
                      ? "Contents read-only"
                      : "No contents access"}
                </span>
                <Button
                  variant="secondary"
                  aria-label={`Import and inspect ${repo.full_name}`}
                  disabled={
                    controlsBusy ||
                    loading ||
                    !repo.contents_read ||
                    !repo.default_branch
                  }
                  onClick={() => void importRepository(repo.id, repo.full_name)}
                >
                  <Download size={14} />
                  {importingId === repo.id
                    ? "Importing…"
                    : "Import and inspect"}
                </Button>
              </div>
            ))}
          </div>
        </>
      )}
    </section>
  );
}

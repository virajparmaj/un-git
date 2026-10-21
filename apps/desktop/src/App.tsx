import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  ArrowRight,
  CalendarDays,
  Check,
  ChevronRight,
  ExternalLink,
  FolderGit2,
  GitBranch,
  Github,
  Info,
  Loader2,
  LockKeyhole,
  LogOut,
  RefreshCw,
  Search,
  Settings2,
  ShieldCheck,
  Terminal,
  Workflow,
  X,
  AlertTriangle,
  Download,
  BookOpen,
} from "lucide-react";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { RemoteRepositories } from "./components/RemoteRepositories";
import { HistoryGraph } from "./components/HistoryGraph";
import { IdentityEvidence } from "./components/IdentityEvidence";
import { bridge, isNative } from "./bridge";
import type {
  AuthStatus,
  ContributionSnapshot,
  Dashboard,
  DeviceFlow,
  Operation,
  Plan,
  Scan,
} from "./types";
import { Button } from "./components/ui/button";
import { Dialog } from "./components/ui/dialog";
import { Disconnect } from "./components/Brand";
import { Calendar } from "./components/Calendar";
import { CommitTable } from "./components/CommitTable";
import { CommitDayFilter } from "./components/CommitDayFilter";
import { OperationDetail, stateTitle } from "./components/OperationDetail";
import { Instructions } from "./components/Instructions";
type Page =
  | "Calendar"
  | "Repositories"
  | "Operations"
  | "Settings"
  | "Instructions";
const nav = [
  { name: "Calendar", icon: CalendarDays },
  { name: "Repositories", icon: FolderGit2 },
  { name: "Operations", icon: Workflow },
  { name: "Settings", icon: Settings2 },
  { name: "Instructions", icon: BookOpen },
] as const;
const emptyAuth: AuthStatus = { connected: false, client_id_configured: false };
export default function App() {
  const [page, setPage] = useState<Page>("Calendar");
  const [dashboard, setDashboard] = useState<Dashboard | null>(null);
  const [auth, setAuth] = useState<AuthStatus>(emptyAuth);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState("");
  const [path, setPath] = useState("");
  const [scan, setScan] = useState<Scan | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [search, setSearch] = useState("");
  const [commitDay, setCommitDay] = useState("");
  const [email, setEmail] = useState("");
  const [confirmed, setConfirmed] = useState(false);
  const [plan, setPlan] = useState<Plan | null>(null);
  const [operation, setOperation] = useState<Operation | null>(null);
  const [report, setReport] = useState("");
  const [connect, setConnect] = useState(false);
  const [clientId, setClientId] = useState("");
  const [device, setDevice] = useState<DeviceFlow | null>(null);
  const [year, setYear] = useState(new Date().getFullYear());
  const [snapshot, setSnapshot] = useState<ContributionSnapshot | null>(null);
  const calendarRequest = useRef(0);
  const [confirmPrepare, setConfirmPrepare] = useState(false);
  const [reportPreview, setReportPreview] = useState<Record<
    string,
    unknown
  > | null>(null);
  const [pollAfter, setPollAfter] = useState(0);
  const [clock, setClock] = useState(Date.now());
  const refresh = useCallback(async () => {
    const result = await bridge.dashboard();
    setDashboard(result);
    setAuth(result.auth);
    return result;
  }, []);
  useEffect(() => {
    if (isNative()) void refresh().catch((e) => setError(e.message));
  }, [refresh]);
  useEffect(() => {
    const interval = setInterval(() => setClock(Date.now()), 1000);
    return () => clearInterval(interval);
  }, []);
  useEffect(() => {
    if (!isNative()) return;
    let disposed = false;
    let cleanup: (() => void) | undefined;
    void listen<Operation>("operation-progress", (event) => {
      setOperation(event.payload);
      setPage("Operations");
    })
      .then((unlisten) => {
        if (disposed) unlisten();
        else cleanup = unlisten;
      })
      .catch((error: unknown) => {
        if (!disposed)
          setError(`Operation progress is unavailable: ${String(error)}`);
      });
    return () => {
      disposed = true;
      cleanup?.();
    };
  }, []);
  const run = useCallback(async (label: string, fn: () => Promise<void>) => {
    setBusy(label);
    setError("");
    try {
      await fn();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy("");
    }
  }, []);
  async function scanPath(repositoryPath: string) {
    const result = await bridge.scan(repositoryPath);
    showScan(result);
  }
  function createDemo() {
    return run("Creating synthetic fixture", async () => {
      const demo = await bridge.demo();
      setCommitDay("");
      await scanPath(demo.repository_path);
    });
  }
  function showScan(result: Scan) {
    setScan(result);
    setPath(result.repository_path);
    setSelected(new Set());
    setSearch("");
    setPlan(null);
    setEmail("");
    setConfirmed(false);
    setPage("Repositories");
  }
  async function importRepository(repositoryId: number) {
    setBusy("Importing and inspecting GitHub repository");
    setError("");
    try {
      showScan(await bridge.importRepository(repositoryId));
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
      throw cause;
    } finally {
      setBusy("");
    }
  }
  const loadCalendar = (nextYear = year) => {
    const request = ++calendarRequest.current;
    return run("Loading contributions", async () => {
      const result = await bridge.calendar(nextYear);
      if (request === calendarRequest.current) setSnapshot(result);
    });
  };
  async function prepare() {
    setConfirmPrepare(false);
    await run("Preparing and validating candidate", async () => {
      try {
        const result = await bridge.prepare(plan!.id);
        setOperation(result);
        setPage("Operations");
        setPlan(null);
        setReport("");
        await refresh();
      } catch (e) {
        await refresh().catch(() => {});
        throw e;
      }
    });
  }
  function toggle(oid: string) {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(oid)) next.delete(oid);
      else next.add(oid);
      return next;
    });
    setPlan(null);
    setConfirmed(false);
  }
  const filtered =
    scan?.commits.filter(
      (c) =>
        (!commitDay || c.author_date.slice(0, 10) === commitDay) &&
        `${c.oid} ${c.subject} ${c.author_name} ${c.author_email}`
          .toLowerCase()
          .includes(search.toLowerCase()),
    ) || [];
  const emails = [
    ...new Set(
      scan?.commits
        .filter((c) => selected.has(c.oid))
        .map((c) => c.author_email),
    ),
  ];
  const matched =
    scan?.commits.filter((c) => selected.has(c.oid) && c.author_email === email)
      .length || 0;
  const hasUnmatched = !!selected.size && matched !== selected.size;
  const operations = dashboard?.operations || [];
  const planSelection = useMemo(() => {
    const dates: Record<string, { selected: number; inspected: number }> = {};
    if (!plan || !scan) return dates;
    const selected = new Set(plan.selected_oids);
    for (const commit of scan.commits) {
      const day = commit.author_date.slice(0, 10);
      if (!/^\d{4}-\d{2}-\d{2}$/.test(day)) continue;
      dates[day] ??= { selected: 0, inspected: 0 };
      dates[day].inspected++;
      if (selected.has(commit.oid)) dates[day].selected++;
    }
    return Object.fromEntries(
      Object.entries(dates).filter(([, value]) => value.selected > 0),
    );
  }, [plan, scan]);
  function changeConnect(open: boolean) {
    setConnect(open);
    if (!open && device) {
      const flowId = device.flow_id;
      setDevice(null);
      setPollAfter(0);
      void bridge
        .authCancel(flowId)
        .catch((cause: unknown) =>
          setError(cause instanceof Error ? cause.message : String(cause)),
        );
    }
  }
  return (
    <div className="app-shell">
      <aside className="sidebar">
        <a
          className="brand"
          href="#"
          onClick={(e) => {
            e.preventDefault();
            setPage("Calendar");
          }}
          aria-label="un-git home"
        >
          <Disconnect />
          <span>
            un-git<span className="brand-period">.</span>
          </span>
        </a>
        <div className="workspace-label">WORKSPACE</div>
        <nav aria-label="Main navigation">
          {nav.map(({ name, icon: Icon }) => (
            <button
              key={name}
              className={`nav-item ${page === name ? "active" : ""}`}
              onClick={() => setPage(name)}
              aria-current={page === name ? "page" : undefined}
            >
              <Icon size={17} />
              <span>{name}</span>
              {name === "Operations" && operations.length > 0 && (
                <span className="nav-count">{operations.length}</span>
              )}
            </button>
          ))}
        </nav>
        <div className="sidebar-bottom">
          <div className="local-badge">
            <span className="status-dot" />
            <span>Local by design</span>
            <LockKeyhole size={12} />
          </div>
          <p>
            Your history stays
            <br />
            on your machine.
          </p>
          <button className="account-button" onClick={() => setConnect(true)}>
            <span className="account-icon">
              <Github size={17} />
            </span>
            <span>
              <strong>{auth.connected ? auth.login : "Connect GitHub"}</strong>
              <small>
                {auth.connected ? "Account connected" : "No account connected"}
              </small>
            </span>
            <ChevronRight size={14} />
          </button>
          <div className="sidebar-version">
            <span>ENGINE PROTOTYPE</span>
            <span className="mono">0.1.0</span>
          </div>
        </div>
      </aside>
      <div className="workspace">
        <header className="topbar">
          <div className="breadcrumb">
            Workspace <ChevronRight size={12} />
            <strong>{page}</strong>
          </div>
          <div className="topbar-right">
            <span className="mode-label">
              <ShieldCheck size={13} />
              Local preparation only
            </span>
            <span className="divider-vertical" />
            <button className="topbar-account" onClick={() => setConnect(true)}>
              <Github size={15} />
              {auth.connected ? auth.login : "Not connected"}
            </button>
          </div>
        </header>
        <main>
          {!isNative() && (
            <div className="native-banner">
              <Terminal size={16} />
              <span>
                <strong>Browser preview</strong> · Open the desktop app to
                inspect repositories. Local Git and Keychain are unavailable
                here.
              </span>
            </div>
          )}
          {error && (
            <div className="error-banner" role="alert">
              <AlertTriangle size={17} />
              <span>{error}</span>
              <Button
                variant="ghost"
                size="icon"
                onClick={() => setError("")}
                aria-label="Dismiss error"
              >
                <X size={16} />
              </Button>
            </div>
          )}
          {busy && (
            <div className="progress-banner" role="status">
              <Loader2 size={16} className="spin" />
              {busy}… <span>The engine determines the result.</span>
            </div>
          )}
          {page === "Calendar" && (
            <>
              <Calendar
                snapshot={snapshot}
                year={year}
                onYear={(next) => {
                  setYear(next);
                  setSnapshot(null);
                  if (auth.connected) void loadCalendar(next);
                }}
                onRefresh={() => void loadCalendar()}
                onConnect={() => setConnect(true)}
                onInspectDay={(date) => {
                  setCommitDay(date);
                  setSearch("");
                  setPage("Repositories");
                }}
                connected={auth.connected}
                busy={!!busy}
                planSelection={planSelection}
              />
              <section className="local-entry">
                <div>
                  <Terminal size={19} />
                  <div>
                    <h3>Explore the local engine</h3>
                    <p>
                      Inspect an existing repository or create an isolated
                      synthetic fixture.
                    </p>
                  </div>
                </div>
                <Button
                  variant="secondary"
                  onClick={() => setPage("Repositories")}
                >
                  Open repository inspector <ArrowRight size={15} />
                </Button>
              </section>
            </>
          )}
          {page === "Instructions" && (
            <Instructions
              onDemo={() => void createDemo()}
              onRepositories={() => setPage("Repositories")}
              onCalendar={() => setPage("Calendar")}
              disabled={!!busy || !isNative()}
              dataDirectory={dashboard?.data_directory}
            />
          )}
          {page === "Repositories" && (
            <>
              <div className="page-heading">
                <div>
                  <div className="eyebrow">LOCAL HISTORY</div>
                  <h1>Repository inspector</h1>
                  <p>
                    Understand exact commit objects before preparing any change.
                  </p>
                </div>
                <span className="badge">
                  <LockKeyhole size={12} /> Local only
                </span>
              </div>
              {commitDay && !scan && (
                <div className="notice">
                  <Info size={17} />
                  <div>
                    <strong>Inspect commits authored on {commitDay}</strong>
                    <p>
                      Import a GitHub repository or choose an existing local
                      clone below. The date filter will use each commit’s
                      original author date and time zone.
                    </p>
                  </div>
                </div>
              )}
              <form
                className="repository-input"
                onSubmit={(e) => {
                  e.preventDefault();
                  void run("Scanning repository", () => scanPath(path.trim()));
                }}
              >
                <FolderGit2 size={18} />
                <input
                  aria-label="Local repository path"
                  placeholder="/Users/you/projects/repository"
                  className="mono"
                  value={path}
                  onChange={(e) => setPath(e.target.value)}
                  autoComplete="off"
                />
                <Button
                  type="button"
                  variant="secondary"
                  disabled={!!busy || !isNative()}
                  onClick={() =>
                    void run("Choosing repository", async () => {
                      const folder = await open({
                        directory: true,
                        multiple: false,
                      });
                      if (folder) await scanPath(folder);
                    })
                  }
                >
                  Browse
                </Button>
                <Button
                  type="submit"
                  disabled={!path.trim() || !!busy || !isNative()}
                >
                  <Search size={15} />
                  Inspect repository
                </Button>
              </form>
              {auth.connected && (
                <RemoteRepositories busy={!!busy} onImport={importRepository} />
              )}
              {!scan ? (
                <section className="repository-empty">
                  <div className="empty-visual">
                    <Disconnect large />
                  </div>
                  <span className="eyebrow">A SAFE PLACE TO START</span>
                  <h2>Know exactly what changes.</h2>
                  <p>
                    Scan a repository to inspect authors, timestamps, and commit
                    ancestry.
                    <br />
                    Preparation happens in a separate local candidate.
                  </p>
                  <Button
                    onClick={() => void createDemo()}
                    disabled={!!busy || !isNative()}
                  >
                    <GitBranch size={16} />
                    Create local demo <ArrowRight size={15} />
                  </Button>
                  <span className="empty-caption">
                    Synthetic history · no GitHub connection · no remote changes
                  </span>
                  <div className="empty-checks">
                    <span>
                      <Check size={14} />
                      Files preserved
                    </span>
                    <span>
                      <Check size={14} />
                      Dates preserved
                    </span>
                    <span>
                      <Check size={14} />
                      Backup verified
                    </span>
                  </div>
                </section>
              ) : (
                <>
                  <div className="repository-heading">
                    <div>
                      <FolderGit2 size={22} />
                      <div>
                        <h2>{scan.repository_name}</h2>
                        <p className="mono">
                          {scan.branch} <span>·</span> {scan.head.slice(0, 12)}
                        </p>
                      </div>
                    </div>
                    <div className="repository-tags">
                      {scan.synthetic && (
                        <span className="badge badge-warning">
                          Synthetic local fixture
                        </span>
                      )}
                      <span className="badge mono">
                        {scan.total_commits.toLocaleString()} commits
                      </span>
                    </div>
                  </div>
                  {scan.import_source && (
                    <p className="subtle" role="status">
                      Local snapshot imported{" "}
                      {new Date(
                        scan.import_source.fetched_at * 1000,
                      ).toLocaleString()}
                      . Import again to inspect newer GitHub history. Your
                      selections and prepared operations use this saved
                      snapshot.
                    </p>
                  )}
                  {scan.blockers.length > 0 && (
                    <div className="notice notice-warning">
                      <Info size={17} />
                      <div>
                        <strong>
                          Inspection is available. Preparation restrictions
                          apply.
                        </strong>
                        {scan.blockers.map((b, i) => (
                          <p key={i}>{b}</p>
                        ))}
                      </div>
                    </div>
                  )}
                  <div className="table-toolbar">
                    <div className="search-input">
                      <Search size={15} />
                      <input
                        aria-label="Search commits"
                        placeholder="Search commits, author, or SHA"
                        value={search}
                        onChange={(e) => setSearch(e.target.value)}
                      />
                    </div>
                    <span className="subtle">{selected.size} selected</span>
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => {
                        setSelected(new Set());
                        setPlan(null);
                        setConfirmed(false);
                      }}
                      disabled={!selected.size || !!busy}
                    >
                      Clear selection
                    </Button>
                  </div>
                  <CommitDayFilter
                    commits={scan.commits}
                    selected={selected}
                    date={commitDay}
                    onDate={setCommitDay}
                    onSelect={(oids) => {
                      setSelected(new Set(oids));
                      setEmail("");
                      setPlan(null);
                      setConfirmed(false);
                    }}
                    disabled={!!busy}
                  />
                  <CommitTable
                    commits={filtered}
                    selected={selected}
                    onToggle={toggle}
                    disabled={!!busy}
                  />
                  <div className="table-footer">
                    <span>
                      <Info size={13} />
                      Author dates retain their original offsets.
                    </span>
                    <span className="mono">
                      {filtered.length} / {scan.total_commits} objects
                    </span>
                  </div>
                  {selected.size > 0 && (
                    <section className="selection-panel">
                      <div className="section-title">
                        <h2>Attribution change</h2>
                        <span className="badge badge-coral">
                          {selected.size} selected commits
                        </span>
                      </div>
                      <p>
                        Only the selected author email will change. Names,
                        committer metadata, files, dates, and topology must
                        remain intact.
                      </p>
                      <p>
                        The commits’ code changes stay in the history. This does
                        not revert files or delete commits. GitHub’s
                        contribution counts may still include other activity.
                      </p>
                      <div className="identity-fields">
                        <label>
                          Exact author email
                          <select
                            aria-label="Confirmed author email"
                            value={email}
                            disabled={!!busy}
                            onChange={(e) => {
                              setEmail(e.target.value);
                              setConfirmed(false);
                              setPlan(null);
                            }}
                          >
                            <option value="">Choose an identity</option>
                            {emails.map((value) => (
                              <option key={value} value={value}>
                                {value}
                              </option>
                            ))}
                          </select>
                        </label>
                        <ArrowRight size={17} />
                        <label>
                          Replacement email
                          <input
                            aria-label="Replacement email"
                            className="mono"
                            readOnly
                            value="removed@un-git.invalid"
                          />
                        </label>
                      </div>
                      <IdentityEvidence
                        email={email}
                        connected={auth.connected}
                        accountKey={auth.login}
                      />
                      {hasUnmatched && email && (
                        <p className="text-warning small">
                          {selected.size - matched} selected commit(s) use a
                          different author email. Select commits belonging to
                          the chosen identity.
                        </p>
                      )}
                      <label className="confirm-checkbox">
                        <input
                          type="checkbox"
                          checked={confirmed}
                          disabled={!!busy}
                          onChange={(e) => {
                            setConfirmed(e.target.checked);
                            setPlan(null);
                          }}
                        />
                        <span>
                          {scan.synthetic
                            ? "I confirm this is the synthetic test identity I intend to change."
                            : "I confirm this exact email is my identity and these are the commits I intend to change."}
                          <small>
                            Confirmation applies to this exact identity and
                            selection. Account evidence, when available, is
                            shown separately above.
                          </small>
                        </span>
                      </label>
                      <Button
                        disabled={
                          !!busy || !confirmed || !email || hasUnmatched
                        }
                        onClick={() =>
                          void run("Computing exact rewrite plan", async () => {
                            setPlan(
                              await bridge.plan(scan.id, [...selected], email),
                            );
                          })
                        }
                      >
                        Review local plan <ArrowRight size={15} />
                      </Button>
                      <p className="small subtle" style={{ marginTop: 12 }}>
                        This build supports exact author-email edits only.
                        Coauthor trailers and committer metadata remain
                        unchanged. Signed, multi-ref, and other unsupported
                        histories remain inspection-only; engine blockers are
                        authoritative.
                      </p>
                    </section>
                  )}
                  {plan && (
                    <section className="plan-panel">
                      <div className="section-title">
                        <div>
                          <span className="eyebrow">LOCAL PLAN</span>
                          <h2>A precise change. A visible impact.</h2>
                        </div>
                        <span className="badge">Not prepared</span>
                      </div>
                      <div className="plan-counts">
                        <div>
                          <strong>{plan.selected_oids.length}</strong>
                          <span>Selected commits</span>
                        </div>
                        <ArrowRight size={22} />
                        <div>
                          <strong>{plan.affected_oids.length}</strong>
                          <span>Commit objects affected</span>
                        </div>
                      </div>
                      <p>
                        Descendants receive new object IDs because their parent
                        references change. Commit history remains connected.
                      </p>
                      <div className="edit-preview">
                        {plan.edits.map((edit, i) => (
                          <div key={i}>
                            <code>{edit.oid.slice(0, 12)}</code>
                            <span>{edit.field}</span>
                            <code>{edit.before}</code>
                            <ArrowRight size={14} />
                            <code className="text-coral">{edit.after}</code>
                          </div>
                        ))}
                      </div>
                      <HistoryGraph
                        commits={scan.commits}
                        selected={new Set(plan.selected_oids)}
                        affected={new Set(plan.affected_oids)}
                      />
                      <div className="notice">
                        <Info size={16} />
                        <span>
                          Manual email detachment is an account-wide
                          alternative. This local scan cannot establish its
                          effect on other repositories or wanted contributions.
                          No email settings are changed by un-git.
                        </span>
                      </div>
                      <div className="plan-hash">
                        <span>Immutable plan</span>
                        <code>{plan.plan_hash}</code>
                      </div>
                      {plan.blockers.length > 0 ? (
                        <div className="notice notice-warning">
                          <AlertTriangle size={16} />
                          <div>
                            {plan.blockers.map((b, i) => (
                              <p key={i}>{b}</p>
                            ))}
                          </div>
                        </div>
                      ) : (
                        <Button
                          onClick={() => setConfirmPrepare(true)}
                          disabled={!!busy}
                        >
                          <ShieldCheck size={16} />
                          Prepare local candidate
                        </Button>
                      )}
                      <p className="subtle small">
                        Preparation creates and restores a backup, rewrites a
                        separate candidate, and validates every supported
                        invariant. It does not publish.
                      </p>
                    </section>
                  )}
                </>
              )}
            </>
          )}
          {page === "Operations" && (
            <>
              <div className="page-heading">
                <div>
                  <div className="eyebrow">DURABLE HISTORY</div>
                  <h1>Operations</h1>
                  <p>
                    Every checkpoint, validation result, and recovery artifact
                    in one place.
                  </p>
                </div>
                <Button
                  variant="secondary"
                  disabled={!!busy || !isNative()}
                  onClick={() =>
                    void run("Refreshing operations", async () => {
                      await refresh();
                    })
                  }
                >
                  <RefreshCw size={14} />
                  Refresh
                </Button>
              </div>
              {operation ? (
                <>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => setOperation(null)}
                    className="back-link"
                  >
                    ← All operations
                  </Button>
                  <OperationDetail
                    operation={operation}
                    busy={!!busy}
                    exportPath={report}
                    onRefresh={() =>
                      void run("Reading operation", async () =>
                        setOperation(await bridge.operation(operation.id)),
                      )
                    }
                    onVerify={() =>
                      void run("Revalidating preserved artifacts", async () => {
                        try {
                          setOperation(await bridge.verify(operation.id));
                        } finally {
                          setOperation(await bridge.operation(operation.id));
                          await refresh();
                        }
                      })
                    }
                    onCancel={() =>
                      void run("Requesting safe cancellation", async () =>
                        bridge.cancel(operation.id),
                      )
                    }
                    onExport={() =>
                      void run("Previewing redacted report", async () =>
                        setReportPreview(
                          await bridge.reportPreview(operation.id),
                        ),
                      )
                    }
                  />
                </>
              ) : operations.length ? (
                <div className="operations-list">
                  {operations.map((op) => (
                    <button
                      key={op.id}
                      onClick={() =>
                        void run("Reading operation", async () => {
                          setOperation(await bridge.operation(op.id));
                          setReport("");
                        })
                      }
                    >
                      <span className="operation-list-icon">
                        <Workflow size={20} />
                      </span>
                      <span>
                        <strong>{op.repository_name}</strong>
                        <small className="mono">{op.id}</small>
                      </span>
                      <span className="operation-list-state">
                        {stateTitle(op.state)}
                        <small>
                          {new Date(op.created_at).toLocaleString()}
                        </small>
                      </span>
                      <ChevronRight size={17} />
                    </button>
                  ))}
                </div>
              ) : (
                <div className="simple-empty">
                  <Workflow size={32} />
                  <h2>No operations yet</h2>
                  <p>
                    Prepare a local candidate from the repository inspector.
                    <br />
                    Its checkpoint history and verification results will appear
                    here.
                  </p>
                  <Button
                    variant="secondary"
                    onClick={() => setPage("Repositories")}
                  >
                    Inspect a repository <ArrowRight size={15} />
                  </Button>
                </div>
              )}
            </>
          )}
          {page === "Settings" && (
            <>
              <div className="page-heading">
                <div>
                  <div className="eyebrow">YOUR WORKSPACE</div>
                  <h1>Settings</h1>
                  <p>
                    Access, local storage, and the tools behind each operation.
                  </p>
                </div>
              </div>
              <section className="settings-section">
                <h2>GitHub access</h2>
                <div className="setting-row">
                  <Github size={20} />
                  <div>
                    <strong>
                      {auth.connected ? auth.login : "No account connected"}
                    </strong>
                    <p>
                      {auth.message ||
                        "A GitHub App device flow keeps account credentials in macOS Keychain."}
                    </p>
                  </div>
                  <Button
                    variant="secondary"
                    disabled={!!busy}
                    onClick={() =>
                      auth.connected
                        ? void run("Disconnecting account", async () => {
                            setAuth(await bridge.authLogout());
                            calendarRequest.current += 1;
                            setSnapshot(null);
                            await refresh();
                          })
                        : setConnect(true)
                    }
                  >
                    {auth.connected ? (
                      <>
                        <LogOut size={14} />
                        Disconnect
                      </>
                    ) : (
                      "Connect GitHub"
                    )}
                  </Button>
                </div>
                <div className="setting-row">
                  <ShieldCheck size={20} />
                  <div>
                    <strong>Writer capabilities</strong>
                    <p>
                      Remote publication is unavailable. This build prepares and
                      validates local candidates.
                    </p>
                  </div>
                  <span className="badge">Disabled</span>
                </div>
              </section>
              <section className="settings-section">
                <h2>Storage and privacy</h2>
                <div className="setting-row">
                  <LockKeyhole size={20} />
                  <div>
                    <strong>Local application data</strong>
                    <p className="mono full-value">
                      {dashboard?.data_directory ||
                        "Available in the native application"}
                    </p>
                    <p>
                      Operation artifacts and private backups are stored
                      separately from scan caches. No automatic cleanup of
                      unresolved backups.
                    </p>
                  </div>
                </div>
                <div className="setting-row">
                  <Download size={20} />
                  <div>
                    <strong>Diagnostic reports</strong>
                    <p>
                      Redacted exports are created locally from an operation. No
                      telemetry or report uploads.
                    </p>
                  </div>
                </div>
              </section>
              <section className="settings-section">
                <h2>Toolchain</h2>
                {dashboard?.toolchain.length ? (
                  dashboard.toolchain.map((tool) => (
                    <div className="setting-row" key={tool.name}>
                      <Terminal size={19} />
                      <div>
                        <strong>{tool.name}</strong>
                        <p className="mono">
                          {tool.version || "Version unavailable"}
                        </p>
                      </div>
                      <span
                        className={`badge ${tool.available ? "" : "badge-warning"}`}
                      >
                        {tool.available ? "Available" : "Unavailable"}
                      </span>
                    </div>
                  ))
                ) : (
                  <p className="subtle">
                    Open the desktop app to run toolchain checks.
                  </p>
                )}
              </section>
              <section className="settings-section">
                <h2>Appearance</h2>
                <div className="setting-row">
                  <Disconnect />
                  <div>
                    <strong>Surgical Precision, refined</strong>
                    <p>
                      Graphite, mineral lime, and The Disconnect. Geist Sans
                      with the redistributable Geist Mono fallback. System
                      reduced-motion settings are respected.
                    </p>
                  </div>
                </div>
              </section>
            </>
          )}
          <footer className="main-footer">
            <span>
              <span className="status-dot" />
              Inspect. Understand. Validate.
            </span>
            <span>Files and dates are invariants.</span>
          </footer>
        </main>
      </div>
      <Dialog
        open={connect}
        onOpenChange={changeConnect}
        title={auth.connected ? "GitHub account" : "Connect GitHub"}
        description="Authorize your account, install the Inspector GitHub App on selected repositories, then verify available capabilities."
      >
        {auth.connected ? (
          <>
            <div className="connected-identity">
              <Github size={30} />
              <div>
                <h3>{auth.login}</h3>
                <p>
                  Credentials are stored in Keychain. GitHub access is verified
                  on each request and is limited by the app installation.
                </p>
              </div>
            </div>
            <Button
              onClick={() => {
                setConnect(false);
                void loadCalendar();
              }}
            >
              Load contribution calendar <ArrowRight size={15} />
            </Button>
          </>
        ) : (
          <>
            <div className="notice">
              <LockKeyhole size={17} />
              <span>
                A public GitHub App client ID is required. No client secret or
                app private key belongs in this app.
              </span>
            </div>
            <label className="form-label">
              Inspector GitHub App client ID
              <input
                className="mono"
                value={clientId}
                onChange={(e) => setClientId(e.target.value)}
                placeholder="Iv1.…"
                aria-label="GitHub App client ID"
                autoComplete="off"
              />
            </label>
            {!device ? (
              <Button
                disabled={!!busy || !clientId.trim() || !isNative()}
                onClick={() =>
                  void run("Requesting device authorization", async () =>
                    setDevice(await bridge.authStart(clientId.trim())),
                  )
                }
              >
                <Github size={16} />
                Start device authorization
              </Button>
            ) : (
              <div className="device-auth">
                <span>Enter this code on GitHub</span>
                <strong className="mono">{device.user_code}</strong>
                <Button
                  variant="secondary"
                  onClick={() =>
                    void run("Opening GitHub authorization", () =>
                      bridge.openAuthorization(),
                    )
                  }
                >
                  Open GitHub authorization <ExternalLink size={14} />
                </Button>
                <p>
                  After authorizing, check the result. Codes expire after{" "}
                  {Math.round(device.expires_in / 60)} minutes.
                </p>
                <Button
                  disabled={!!busy || clock < pollAfter}
                  onClick={() =>
                    void run("Checking authorization", async () => {
                      const result = await bridge.authPoll(device.flow_id);
                      setPollAfter(
                        Date.now() +
                          (result.retry_after_seconds || device.interval) *
                            1000,
                      );
                      setAuth(result);
                      if (result.connected) {
                        setDevice(null);
                        await refresh();
                      } else
                        setError(
                          "Authorization is pending. Complete the GitHub browser step, then check again after the polling interval.",
                        );
                    })
                  }
                >
                  <RefreshCw size={14} />
                  {clock < pollAfter
                    ? `Check in ${Math.ceil((pollAfter - clock) / 1000)}s`
                    : "Check authorization"}
                </Button>
                <Button
                  variant="ghost"
                  disabled={!!busy}
                  onClick={() =>
                    void run("Cancelling pending authorization", async () => {
                      await bridge.authCancel(device.flow_id);
                      setDevice(null);
                      setPollAfter(0);
                    })
                  }
                >
                  Start again
                </Button>
              </div>
            )}
            <p className="small subtle">
              Inspector access is read-only. Connecting an account does not
              grant access to every repository.
            </p>
          </>
        )}
      </Dialog>
      <Dialog
        open={confirmPrepare}
        onOpenChange={setConfirmPrepare}
        title="Prepare this local candidate?"
        description="The engine will create a standalone backup, prove restoration, and prepare a separate candidate. Your source repository is not rewritten."
      >
        <div className="prepare-summary">
          <div>
            <span>Selected commits</span>
            <strong>{plan?.selected_oids.length}</strong>
          </div>
          <div>
            <span>Objects affected</span>
            <strong>{plan?.affected_oids.length}</strong>
          </div>
          <div>
            <span>Remote changes</span>
            <strong>None</strong>
          </div>
        </div>
        <p className="small subtle">
          The candidate must pass complete validation. Preparation can use
          additional disk space for independent object storage and recovery
          artifacts.
        </p>
        <div className="dialog-actions">
          <Button variant="secondary" onClick={() => setConfirmPrepare(false)}>
            Cancel
          </Button>
          <Button onClick={() => void prepare()}>
            <ShieldCheck size={16} />
            Prepare and validate
          </Button>
        </div>
      </Dialog>
      <Dialog
        open={reportPreview !== null}
        onOpenChange={(value) => {
          if (!value) setReportPreview(null);
        }}
        title="Review diagnostic report"
        description="Review the exact redacted contents before exporting. Nothing is uploaded."
      >
        <pre className="report-preview">
          {JSON.stringify(reportPreview, null, 2)}
        </pre>
        <div className="dialog-actions">
          <Button variant="secondary" onClick={() => setReportPreview(null)}>
            Cancel
          </Button>
          <Button
            onClick={() =>
              void run("Exporting reviewed report", async () => {
                setReport((await bridge.export(operation!.id)).path);
                setReportPreview(null);
              })
            }
          >
            Export report
          </Button>
        </div>
      </Dialog>
    </div>
  );
}

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import App from "../App";
import type { Commit, RepositoryList, Scan } from "../types";

const mocks = vi.hoisted(() => ({
  dashboard: vi.fn(),
  calendar: vi.fn(),
  day: vi.fn(),
  repositories: vi.fn(),
  importRepository: vi.fn(),
  identities: vi.fn(),
  plan: vi.fn(),
  demo: vi.fn(),
  scan: vi.fn(),
}));
vi.mock("../bridge", () => ({ isNative: () => true, bridge: mocks }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: () => Promise.resolve(() => {}),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
// JSDOM has no viewport for the virtualized table; keep the real flow around
// a semantic table substitute that exposes the commits and selection it receives.
vi.mock("../components/CommitTable", () => ({
  CommitTable: ({
    commits,
    selected,
    onToggle,
    disabled,
  }: {
    commits: Commit[];
    selected: Set<string>;
    onToggle: (oid: string) => void;
    disabled: boolean;
  }) => (
    <div aria-label="Inspected commits">
      {commits.map((commit) => (
        <label key={commit.oid}>
          <input
            type="checkbox"
            checked={selected.has(commit.oid)}
            disabled={disabled}
            onChange={() => onToggle(commit.oid)}
          />
          {commit.subject}
        </label>
      ))}
    </div>
  ),
}));

const year = new Date().getFullYear();
const day = `${year}-09-01`;
const nextDay = `${year}-09-02`;
const authorEmail = "demo@example.test";
function scannedRepository(id: string): Scan {
  const commit: Commit = {
    oid: `${id}-selected`,
    parents: [],
    tree_oid: "tree",
    author_name: "Fixture author",
    author_email: authorEmail,
    committer_name: "Fixture author",
    committer_email: authorEmail,
    author_date: `${day}T23:45:00-10:00`,
    subject: `${id} selected-day commit`,
    blockers: [],
  };
  return {
    id,
    repository_path: `/isolated-imports/${id}`,
    repository_name: `${id} imported history`,
    head: `${id}-next-day`,
    branch: "main",
    commits: [
      commit,
      {
        ...commit,
        oid: `${id}-next-day`,
        parents: [commit.oid],
        author_date: `${nextDay}T00:15:00+14:00`,
        subject: `${id} next-day commit`,
      },
    ],
    total_commits: 2,
    blockers: [],
    scanned_at: `${day}T12:00:00Z`,
  };
}
const repositoryAccess: RepositoryList = {
  partial: false,
  issues: [],
  installations: [
    {
      id: 1,
      account: { login: "fixture" },
      repository_selection: "selected",
      suspended: false,
    },
  ],
  repositories: [1, 2].map((id) => ({
    id,
    full_name: `fixture/repository-${id}`,
    private: true,
    fork: false,
    archived: false,
    default_branch: "main",
    contents_read: true,
    contents_write: false,
    permission_status: "verified",
  })),
};

beforeEach(() => {
  mocks.dashboard.mockResolvedValue({
    auth: { connected: true, login: "fixture", client_id_configured: true },
    operations: [],
    toolchain: [],
    writer_enabled: false,
  });
  mocks.calendar.mockResolvedValue({
    year,
    fetched_at: `${day}T12:00:00Z`,
    days: [{ date: day, count: 3, level: 2 }],
    partial: false,
    coverage_note: "Generated observed snapshot.",
  });
  mocks.day.mockResolvedValue({
    date: day,
    observed_total: 3,
    partial: false,
    issues: [],
    coverage: "Generated day aggregate.",
    totals: {
      commits: 2,
      issues: 0,
      pull_requests: 1,
      reviews: 0,
      repositories_created: 0,
      restricted: 0,
    },
    repositories: [
      {
        full_name: "fixture/repository-1",
        private: true,
        observed_commit_count: 2,
        partial: false,
      },
    ],
  });
  mocks.repositories.mockResolvedValue(repositoryAccess);
  mocks.identities.mockResolvedValue({
    emails: [
      { email: authorEmail, verified: true, primary: true, visibility: null },
    ],
    fetched_at: `${day}T12:00:00Z`,
    coverage_note: "Generated exact identity evidence.",
  });
  mocks.plan.mockResolvedValue({
    id: "reviewed-plan",
    scan_id: "first",
    plan_hash: "immutable-fixture-plan",
    selected_oids: ["first-selected"],
    affected_oids: ["first-selected", "first-next-day"],
    baseline_oid: "first-next-day",
    blockers: [],
    edits: [],
  });
});
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

async function openCalendarDayAndImport() {
  render(<App />);
  fireEvent.click(await screen.findByRole("button", { name: "Load calendar" }));
  await screen.findByRole("button", { name: "Refresh snapshot" });
  fireEvent.click(
    screen.getByRole("button", {
      name: new RegExp(`September 1, ${year}: 3 observed contributions`),
    }),
  );
  await screen.findByText("2 commit contributions");
  fireEvent.click(
    screen.getByRole("button", { name: "Inspect commits for this day" }),
  );
  expect(
    screen.getByText(`Inspect commits authored on ${day}`),
  ).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Refresh access" }));
  fireEvent.click(
    await screen.findByRole("button", {
      name: "Import and inspect fixture/repository-1",
    }),
  );
  await screen.findByRole("heading", { name: "first imported history" });
}

async function reviewFirstSelection() {
  fireEvent.click(
    screen.getByRole("checkbox", { name: "first selected-day commit" }),
  );
  fireEvent.change(
    screen.getByRole("combobox", { name: "Confirmed author email" }),
    {
      target: { value: authorEmail },
    },
  );
  fireEvent.click(
    screen.getByRole("checkbox", {
      name: /I confirm this exact email is my identity/,
    }),
  );
  fireEvent.click(screen.getByRole("button", { name: "Review local plan" }));
  await screen.findByRole("button", { name: "Prepare local candidate" });
  await waitFor(() =>
    expect(
      screen.getByRole("button", {
        name: "Import and inspect fixture/repository-2",
      }),
    ).toBeEnabled(),
  );
}

describe("calendar to imported history", () => {
  it("opens Instructions and starts a fresh generated demo without importing GitHub history", async () => {
    mocks.demo.mockResolvedValue({ repository_path: "/generated/demo" });
    mocks.scan.mockResolvedValue({
      ...scannedRepository("demo"),
      synthetic: true,
    });
    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "Instructions" }));
    expect(
      screen.getByRole("heading", { name: "Instructions", level: 1 }),
    ).toBeInTheDocument();
    fireEvent.click(
      screen.getByRole("button", { name: "Create demo and start" }),
    );
    await screen.findByRole("heading", { name: "demo imported history" });
    expect(mocks.demo).toHaveBeenCalledTimes(1);
    expect(mocks.scan).toHaveBeenCalledExactlyOnceWith("/generated/demo");
    expect(mocks.importRepository).not.toHaveBeenCalled();
    expect(screen.getByLabelText("Filter by author date")).toHaveValue("");
    expect(screen.getByText("0 selected")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Instructions" }));
    fireEvent.click(screen.getByRole("button", { name: "Open repositories" }));
    expect(
      screen.getByRole("heading", { name: "demo imported history" }),
    ).toBeInTheDocument();
  });
  it("carries the chosen calendar day into an imported scan without selecting commits or altering observations", async () => {
    mocks.importRepository.mockResolvedValue(scannedRepository("first"));
    await openCalendarDayAndImport();
    expect(mocks.importRepository).toHaveBeenCalledExactlyOnceWith(1);
    expect(screen.getByLabelText("Filter by author date")).toHaveValue(day);
    expect(screen.getByLabelText("Local repository path")).toHaveValue(
      "/isolated-imports/first",
    );
    expect(
      screen.getByRole("checkbox", { name: "first selected-day commit" }),
    ).not.toBeChecked();
    expect(
      screen.queryByRole("checkbox", { name: "first next-day commit" }),
    ).not.toBeInTheDocument();
    expect(screen.getByText("0 selected")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Calendar" }));
    expect(
      screen.getByRole("button", {
        name: new RegExp(`September 1, ${year}: 3 observed contributions`),
      }),
    ).toBeInTheDocument();
    expect(mocks.calendar).toHaveBeenCalledTimes(1);
  });

  it("keeps the date filter but clears the previous selection, identity approval, and plan when another repository imports", async () => {
    mocks.importRepository
      .mockResolvedValueOnce(scannedRepository("first"))
      .mockResolvedValueOnce(scannedRepository("second"));
    await openCalendarDayAndImport();
    await reviewFirstSelection();
    fireEvent.click(
      screen.getByRole("button", {
        name: "Import and inspect fixture/repository-2",
      }),
    );
    await screen.findByRole("heading", { name: "second imported history" });
    expect(mocks.importRepository).toHaveBeenNthCalledWith(2, 2);
    expect(screen.getByLabelText("Filter by author date")).toHaveValue(day);
    expect(screen.getByText("0 selected")).toBeInTheDocument();
    expect(
      screen.queryByRole("checkbox", { name: "first selected-day commit" }),
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole("checkbox", { name: "second selected-day commit" }),
    ).not.toBeChecked();
    expect(
      screen.queryByRole("button", { name: "Prepare local candidate" }),
    ).not.toBeInTheDocument();
    fireEvent.click(
      screen.getByRole("checkbox", { name: "second selected-day commit" }),
    );
    expect(
      screen.getByRole("combobox", { name: "Confirmed author email" }),
    ).toHaveValue("");
    expect(
      screen.getByRole("checkbox", {
        name: /I confirm this exact email is my identity/,
      }),
    ).not.toBeChecked();
    expect(
      screen.getByRole("button", { name: "Review local plan" }),
    ).toBeDisabled();
  });

  it("retains the inspected history and reviewed selection after an import fails, with a visible retryable error", async () => {
    mocks.importRepository
      .mockResolvedValueOnce(scannedRepository("first"))
      .mockRejectedValueOnce(new Error("Repository access was revoked"));
    await openCalendarDayAndImport();
    await reviewFirstSelection();
    fireEvent.click(
      screen.getByRole("button", {
        name: "Import and inspect fixture/repository-2",
      }),
    );
    await waitFor(() => {
      const alerts = screen.getAllByRole("alert");
      expect(
        alerts.some((alert) =>
          alert.textContent?.includes("Repository access was revoked"),
        ),
      ).toBe(true);
      expect(
        alerts.some((alert) =>
          alert.textContent?.includes(
            "Choose Import and inspect on that repository to retry",
          ),
        ),
      ).toBe(true);
    });
    expect(
      screen.getByRole("heading", { name: "first imported history" }),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("Local repository path")).toHaveValue(
      "/isolated-imports/first",
    );
    expect(screen.getByLabelText("Filter by author date")).toHaveValue(day);
    expect(
      screen.getByRole("checkbox", { name: "first selected-day commit" }),
    ).toBeChecked();
    expect(
      screen.getByRole("checkbox", {
        name: /I confirm this exact email is my identity/,
      }),
    ).toBeChecked();
    expect(
      screen.getByRole("button", { name: "Prepare local candidate" }),
    ).toBeEnabled();
    expect(
      screen.getByRole("button", {
        name: "Import and inspect fixture/repository-2",
      }),
    ).toBeEnabled();
    expect(mocks.plan).toHaveBeenCalledExactlyOnceWith(
      "first",
      ["first-selected"],
      authorEmail,
    );
  });
});

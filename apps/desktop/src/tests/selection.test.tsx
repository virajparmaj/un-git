import { afterEach, describe, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import type { Commit } from "../types";
import App from "../App";
const mocks = vi.hoisted(() => ({
  dashboard: vi.fn(),
  demo: vi.fn(),
  scan: vi.fn(),
  plan: vi.fn(),
}));
vi.mock("../bridge", () => ({ isNative: () => true, bridge: mocks }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: () => Promise.resolve(() => {}),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("../components/CommitTable", () => ({
  CommitTable: ({
    commits,
    onToggle,
    disabled,
  }: {
    commits: Commit[];
    onToggle: (oid: string) => void;
    disabled: boolean;
  }) => (
    <div>
      {commits.map((c) => (
        <button key={c.oid} onClick={() => onToggle(c.oid)} disabled={disabled}>
          Select {c.oid}
        </button>
      ))}
    </div>
  ),
}));
afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});
describe("explicit selection approval", () => {
  it("invalidates a reviewed plan and identity confirmation when selection changes", async () => {
    const commit = {
      parents: [],
      tree_oid: "tree",
      author_name: "Demo",
      author_email: "demo@example.test",
      committer_name: "Demo",
      committer_email: "demo@example.test",
      author_date: "2026-01-01",
      subject: "Test",
      blockers: [],
    };
    mocks.dashboard.mockResolvedValue({
      auth: { connected: false, client_id_configured: false },
      operations: [],
      toolchain: [],
      writer_enabled: false,
    });
    mocks.demo.mockResolvedValue({
      repository_path: "/test/demo",
      label: "Synthetic fixture",
    });
    mocks.scan.mockResolvedValue({
      id: "scan",
      repository_path: "/test/demo",
      repository_name: "demo",
      head: "b",
      branch: "main",
      commits: [
        { ...commit, oid: "a" },
        { ...commit, oid: "b", parents: ["a"] },
      ],
      total_commits: 2,
      blockers: [],
      scanned_at: "2026-01-01",
      synthetic: true,
    });
    mocks.plan.mockResolvedValue({
      id: "plan",
      scan_id: "scan",
      plan_hash: "hash",
      selected_oids: ["a"],
      affected_oids: ["a", "b"],
      baseline_oid: "b",
      blockers: [],
      edits: [],
    });
    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "Repositories" }));
    fireEvent.click(screen.getByRole("button", { name: /Create local demo/ }));
    await screen.findByRole("button", { name: "Select a" });
    fireEvent.click(screen.getByRole("button", { name: "Select a" }));
    fireEvent.change(
      screen.getByRole("combobox", { name: "Confirmed author email" }),
      { target: { value: "demo@example.test" } },
    );
    const confirmation = screen.getByRole("checkbox", {
      name: /I confirm this is the synthetic/,
    });
    fireEvent.click(confirmation);
    fireEvent.click(screen.getByRole("button", { name: "Review local plan" }));
    await screen.findByRole("button", { name: "Prepare local candidate" });
    expect(mocks.plan).toHaveBeenCalledWith("scan", ["a"], "demo@example.test");
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Select b" })).toBeEnabled(),
    );
    fireEvent.click(screen.getByRole("button", { name: "Select b" }));
    expect(
      screen.queryByRole("button", { name: "Prepare local candidate" }),
    ).not.toBeInTheDocument();
    expect(confirmation).not.toBeChecked();
    expect(
      screen.getByRole("button", { name: "Review local plan" }),
    ).toBeDisabled();
  });
});

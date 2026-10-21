import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { Calendar, dayLabel, yearDays } from "../components/Calendar";
import { OperationDetail } from "../components/OperationDetail";
import { HistoryGraph } from "../components/HistoryGraph";
import type { Operation } from "../types";

afterEach(cleanup);
vi.mock("../bridge", () => ({ bridge: { day: vi.fn() } }));

describe("observed contribution truth", () => {
  it("opens local inspection for the exact selected day without changing observations", () => {
    const inspect = vi.fn();
    const snapshot = {
      year: 2024,
      fetched_at: "2024-02-01T12:00:00Z",
      partial: false,
      coverage_note: "Observed",
      days: [{ date: "2024-01-01", count: 8, level: 4 as const }],
    };
    render(
      <Calendar
        year={2024}
        snapshot={snapshot}
        onYear={vi.fn()}
        onRefresh={vi.fn()}
        onConnect={vi.fn()}
        onInspectDay={inspect}
        connected={false}
        busy={false}
      />,
    );
    fireEvent.click(
      screen.getByRole("button", {
        name: /Monday, January 1, 2024: 8 observed/,
      }),
    );
    expect(inspect).not.toHaveBeenCalled();
    fireEvent.click(
      screen.getByRole("button", { name: "Inspect commits for this day" }),
    );
    expect(inspect).toHaveBeenCalledWith("2024-01-01");
    expect(snapshot.days[0].count).toBe(8);
  });
  it("retains leap days and never describes unavailable data as zero", () => {
    expect(yearDays(2024)).toHaveLength(366);
    expect(yearDays(2024)).toContain("2024-02-29");
    expect(dayLabel("2024-01-01")).toContain("unavailable");
    expect(
      dayLabel("2024-01-01", { date: "2024-01-01", count: 0, level: 0 }),
    ).toContain("0 observed");
  });
  it("renders known zero and unknown separately with keyboard traversal", () => {
    render(
      <Calendar
        year={2024}
        snapshot={{
          year: 2024,
          fetched_at: "2024-01-02T00:00:00Z",
          partial: true,
          coverage_note: "Partial",
          days: [
            { date: "2024-01-01", count: 0, level: 0 },
            { date: "2024-01-02", count: null, level: null },
          ],
        }}
        onYear={vi.fn()}
        onRefresh={vi.fn()}
        onConnect={vi.fn()}
        connected={false}
        busy={false}
      />,
    );
    const zero = screen.getByRole("button", {
      name: /Monday, January 1, 2024: 0 observed/,
    });
    const unknown = screen.getByRole("button", {
      name: /Tuesday, January 2, 2024: not loaded/,
    });
    expect(zero).toHaveClass("level-0");
    expect(unknown).toHaveClass("unknown");
    zero.focus();
    fireEvent.keyDown(zero, { key: "ArrowDown" });
    expect(unknown).toHaveFocus();
    expect(screen.getByText("Partial visibility")).toBeInTheDocument();
  });
});

describe("operation safety presentation", () => {
  const operation: Operation = {
    id: "op",
    plan_id: "plan",
    repository_name: "fixture",
    state: "PREPARING",
    created_at: "2026-01-01",
    selected_count: 1,
    rewritten_count: 5,
    baseline_oid: "a".repeat(40),
    validation_checks: [],
    blockers: [],
  };
  it("does not label predicted rewrites as completed and exposes safe cancellation", () => {
    const cancel = vi.fn();
    render(
      <OperationDetail
        operation={operation}
        onRefresh={vi.fn()}
        onVerify={vi.fn()}
        onCancel={cancel}
        onExport={vi.fn()}
        busy={true}
        exportPath=""
      />,
    );
    expect(screen.getByText("Predicted affected objects")).toBeInTheDocument();
    expect(screen.queryByText("All checks passed")).not.toBeInTheDocument();
    fireEvent.click(
      screen.getByRole("button", { name: "Cancel local preparation" }),
    );
    expect(cancel).toHaveBeenCalledOnce();
    expect(
      screen.queryByRole("button", { name: /publish/i }),
    ).not.toBeInTheDocument();
  });
  it("offers report review and artifact revalidation for a local candidate", () => {
    const preview = vi.fn();
    render(
      <OperationDetail
        operation={{
          ...operation,
          state: "VALIDATED",
          candidate_oid: "b".repeat(40),
          validation_checks: [
            { name: "tree_equality", passed: true, detail: "Trees preserved" },
          ],
        }}
        onRefresh={vi.fn()}
        onVerify={vi.fn()}
        onCancel={vi.fn()}
        onExport={preview}
        busy={false}
        exportPath=""
      />,
    );
    expect(screen.getByText("Not published")).toBeInTheDocument();
    fireEvent.click(
      screen.getByRole("button", { name: "Review diagnostic report" }),
    );
    expect(preview).toHaveBeenCalledOnce();
    expect(
      screen.getByRole("button", { name: "Revalidate artifacts" }),
    ).toBeEnabled();
  });
  it("retains all graph nodes and parent edges for a merge", () => {
    const base = {
      tree_oid: "t",
      author_name: "Demo",
      author_email: "demo@example.test",
      committer_name: "Other",
      committer_email: "other@example.test",
      author_date: "2026-01-01",
      subject: "Commit",
      blockers: [],
    };
    const commits = [
      { ...base, oid: "a", parents: [] },
      { ...base, oid: "b", parents: ["a"] },
      { ...base, oid: "c", parents: ["a"] },
      { ...base, oid: "d", parents: ["b", "c"] },
    ];
    const { container } = render(
      <HistoryGraph
        commits={commits}
        selected={new Set(["b"])}
        affected={new Set(["b", "d"])}
      />,
    );
    expect(container.querySelectorAll("path")).toHaveLength(4);
    expect(container.querySelectorAll("g")).toHaveLength(4);
    expect(container.querySelectorAll("circle")).toHaveLength(5);
  });
});

describe("stale validation and observation handling", () => {
  it("keeps the observed snapshot unchanged beneath a distinct partial plan layer", () => {
    const snapshot = {
      year: 2024,
      fetched_at: "2024-01-02",
      partial: false,
      coverage_note: "Observed",
      days: [{ date: "2024-01-01", count: 8, level: 4 as const }],
    };
    const original = JSON.stringify(snapshot);
    render(
      <Calendar
        year={2024}
        snapshot={snapshot}
        onYear={vi.fn()}
        onRefresh={vi.fn()}
        onConnect={vi.fn()}
        connected={false}
        busy={false}
        planSelection={{ "2024-01-01": { selected: 1, inspected: 3 } }}
      />,
    );
    const cell = screen.getByRole("button", {
      name: /Monday, January 1, 2024: 8 observed/,
    });
    expect(cell).not.toHaveClass("plan-selected");
    fireEvent.click(screen.getByRole("button", { name: "Plan selection" }));
    expect(cell).toHaveClass("plan-selected", "plan-partial", "level-4");
    expect(cell).toHaveAccessibleName(
      /1 of 3 inspected commits; partial local selection/,
    );
    fireEvent.click(cell);
    expect(
      screen.getByText(/Other commits, repositories, pull requests, reviews/),
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Observed snapshot" }));
    expect(cell).not.toHaveClass("plan-selected");
    expect(JSON.stringify(snapshot)).toBe(original);
  });
  it("removes success presentation after revalidation blocks a formerly valid operation", () => {
    const props = {
      onRefresh: vi.fn(),
      onVerify: vi.fn(),
      onCancel: vi.fn(),
      onExport: vi.fn(),
      busy: false,
      exportPath: "",
    };
    const operation: Operation = {
      id: "op",
      plan_id: "plan",
      repository_name: "fixture",
      state: "VALIDATED",
      created_at: "2026-01-01",
      selected_count: 1,
      rewritten_count: 2,
      baseline_oid: "a".repeat(40),
      candidate_oid: "b".repeat(40),
      backup_path: "/private/backup.bundle",
      blockers: [],
      validation_checks: [
        { name: "trees", passed: true, detail: "Trees preserved" },
      ],
    };
    const { container, rerender } = render(
      <OperationDetail {...props} operation={operation} />,
    );
    expect(screen.getByText("All checks passed")).toBeInTheDocument();
    expect(container.querySelector(".operation-symbol.good")).not.toBeNull();
    rerender(
      <OperationDetail
        {...props}
        operation={{
          ...operation,
          state: "BLOCKED",
          blockers: ["Candidate checksum changed"],
        }}
      />,
    );
    expect(
      screen.getByRole("heading", { name: "Operation blocked" }),
    ).toBeInTheDocument();
    expect(screen.queryByText("All checks passed")).not.toBeInTheDocument();
    expect(
      screen.queryByText("Verified recovery bundle"),
    ).not.toBeInTheDocument();
    expect(screen.getByText("PREVIOUS PASS")).toBeInTheDocument();
    expect(container.querySelector(".operation-symbol.good")).toBeNull();
    expect(container.querySelector(".validation-row .text-accent")).toBeNull();
  });
  it("does not style an inconsistent validated state with blockers as successful", () => {
    render(
      <OperationDetail
        operation={{
          id: "x",
          plan_id: "p",
          repository_name: "r",
          state: "VALIDATED",
          created_at: "",
          selected_count: 1,
          rewritten_count: 1,
          baseline_oid: "a",
          validation_checks: [
            { name: "trees", passed: true, detail: "old check" },
          ],
          blockers: ["Artifact missing"],
        }}
        onRefresh={vi.fn()}
        onVerify={vi.fn()}
        onCancel={vi.fn()}
        onExport={vi.fn()}
        busy={false}
        exportPath=""
      />,
    );
    expect(
      screen.getByRole("heading", { name: "Validation requires attention" }),
    ).toBeInTheDocument();
    expect(screen.queryByText("All checks passed")).not.toBeInTheDocument();
  });
  it("retains observed counts when a day is selected and labels a stale snapshot", () => {
    const snapshot = {
      year: 2024,
      fetched_at: "2024-02-01T12:00:00Z",
      partial: false,
      coverage_note: "Saved data",
      stale: true,
      fetch_error: "Offline",
      days: [{ date: "2024-01-01", count: 8, level: 4 as const }],
    };
    const original = JSON.stringify(snapshot);
    const retry = vi.fn();
    render(
      <Calendar
        year={2024}
        snapshot={snapshot}
        onYear={vi.fn()}
        onRefresh={retry}
        onConnect={vi.fn()}
        connected={false}
        busy={false}
      />,
    );
    const day = screen.getByRole("button", {
      name: /Monday, January 1, 2024: 8 observed/,
    });
    fireEvent.click(day);
    expect(day).toHaveClass("level-4");
    expect(day).not.toHaveClass("level-0");
    expect(
      screen.getByText("Saved snapshot · refresh failed"),
    ).toBeInTheDocument();
    expect(screen.getByText(/Offline/)).toBeInTheDocument();
    expect(JSON.stringify(snapshot)).toBe(original);
    expect(
      screen.getByText(/8 observed contributions\. Repository-level/),
    ).toBeInTheDocument();
  });
  it("does not display a partial-day sum as a total contribution count", () => {
    const { container } = render(
      <Calendar
        year={2024}
        snapshot={{
          year: 2024,
          fetched_at: "2024-01-01",
          partial: true,
          coverage_note: "Incomplete",
          days: [{ date: "2024-01-01", count: 3, level: 1 }],
        }}
        onYear={vi.fn()}
        onRefresh={vi.fn()}
        onConnect={vi.fn()}
        connected={false}
        busy={false}
      />,
    );
    expect(container.querySelector(".large-number")).toHaveTextContent("—");
  });
  it("does not show another year's late snapshot under the current year", () => {
    render(
      <Calendar
        year={2025}
        snapshot={{
          year: 2024,
          fetched_at: "2024-01-01",
          partial: false,
          coverage_note: "Complete",
          days: [{ date: "2024-01-01", count: 3, level: 1 }],
        }}
        onYear={vi.fn()}
        onRefresh={vi.fn()}
        onConnect={vi.fn()}
        connected={true}
        busy={false}
      />,
    );
    expect(screen.getByText("No snapshot loaded")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Load calendar" }),
    ).toBeInTheDocument();
  });
});

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { CommitDayFilter } from "../components/CommitDayFilter";
import type { Commit } from "../types";

afterEach(cleanup);

function commit(
  oid: string,
  author_date: string,
  author_email = "demo@example.test",
): Commit {
  return {
    oid,
    parents: [],
    tree_oid: "tree",
    author_name: "Demo",
    author_email,
    committer_name: "Demo",
    committer_email: "committer@example.test",
    author_date,
    subject: "Generated test commit",
    blockers: [],
  };
}

const commits = [
  commit("late", "2026-09-01T23:45:00-10:00"),
  commit("early", "2026-09-02T00:15:00+14:00"),
  commit("same", "2026-09-01T12:00:00+00:00"),
  commit("case", "2026-09-01T12:00:00+00:00", "Demo@example.test"),
];

describe("local commit day selection", () => {
  it("uses original author-date offsets across midnight and keeps exact email case separate", () => {
    const onSelect = vi.fn();
    render(
      <CommitDayFilter
        commits={commits}
        selected={new Set(["early"])}
        date="2026-09-01"
        onDate={vi.fn()}
        onSelect={onSelect}
      />,
    );
    expect(screen.getByRole("status")).toHaveTextContent(
      "3 local commits on 2026-09-01",
    );
    expect(screen.getByRole("status")).toHaveTextContent(
      "1 selected commit is outside this day",
    );
    fireEvent.click(
      screen.getByRole("button", {
        name: "Select 2 commits by demo@example.test on this day",
      }),
    );
    expect(onSelect).toHaveBeenLastCalledWith(["late", "same"]);
    fireEvent.click(
      screen.getByRole("button", {
        name: "Select 1 commit by Demo@example.test on this day",
      }),
    );
    expect(onSelect).toHaveBeenLastCalledWith(["case"]);
  });

  it("filters and clears dates without implicitly selecting or clearing commits", () => {
    const onSelect = vi.fn();
    const onDate = vi.fn();
    const props = { commits, selected: new Set(["late"]), onDate, onSelect };
    const { rerender } = render(<CommitDayFilter {...props} date="" />);
    expect(screen.getByRole("status")).toHaveTextContent(
      "Showing all 4 local commits",
    );
    expect(
      screen.queryByRole("button", { name: /Select .* on this day/ }),
    ).not.toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("Filter by author date"), {
      target: { value: "2026-09-02" },
    });
    expect(onDate).toHaveBeenCalledWith("2026-09-02");
    expect(onSelect).not.toHaveBeenCalled();
    rerender(<CommitDayFilter {...props} date="2026-09-02" />);
    expect(screen.getByRole("status")).toHaveTextContent(
      "1 local commit on 2026-09-02",
    );
    fireEvent.click(screen.getByRole("button", { name: "All dates" }));
    expect(onDate).toHaveBeenLastCalledWith("");
    expect(onSelect).not.toHaveBeenCalled();
    expect(props.selected).toEqual(new Set(["late"]));
  });

  it("explains empty local days without calling them zero GitHub contributions", () => {
    render(
      <CommitDayFilter
        commits={commits}
        selected={new Set(["late", "early"])}
        date="2026-09-03"
        onDate={vi.fn()}
        onSelect={vi.fn()}
      />,
    );
    expect(screen.getByRole("status")).toHaveTextContent(
      "0 local commits on 2026-09-03",
    );
    expect(screen.getByRole("status")).toHaveTextContent(
      "2 selected commits are outside this day",
    );
    expect(
      screen.getByText(/GitHub may show activity from other repositories/),
    ).toBeInTheDocument();
    expect(
      screen.getByText(
        /File contents, commit dates, and parent relationships are preserved/,
      ),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /Select .* on this day/ }),
    ).not.toBeInTheDocument();
  });

  it("disables all changes while an operation is busy", () => {
    const onSelect = vi.fn();
    render(
      <CommitDayFilter
        commits={commits}
        selected={new Set()}
        date="2026-09-01"
        onDate={vi.fn()}
        onSelect={onSelect}
        disabled
      />,
    );
    expect(screen.getByLabelText("Filter by author date")).toBeDisabled();
    for (const button of screen.getAllByRole("button")) {
      expect(button).toBeDisabled();
      fireEvent.click(button);
    }
    expect(onSelect).not.toHaveBeenCalled();
  });
});

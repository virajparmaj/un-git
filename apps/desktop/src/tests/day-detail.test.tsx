import { afterEach, describe, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { DayDetail } from "../components/DayDetail";
import type { DaySnapshot } from "../types";

const day = vi.hoisted(() => vi.fn());
vi.mock("../bridge", () => ({ bridge: { day } }));
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

const snapshot: DaySnapshot = {
  date: "2026-09-01",
  observed_total: 5,
  partial: false,
  coverage: "Observed GitHub aggregates.",
  issues: [],
  totals: {
    commits: 5,
    issues: 0,
    pull_requests: 0,
    reviews: 0,
    repositories_created: 0,
    restricted: 0,
  },
  repositories: [
    {
      full_name: "fixture/selected",
      private: false,
      observed_commit_count: 2,
      partial: false,
    },
  ],
};

describe("day contribution coverage", () => {
  it("exposes incomplete grouping even when the surviving repository is complete, and allows retry", async () => {
    day
      .mockResolvedValueOnce({
        ...snapshot,
        partial: true,
        issues: [
          {
            code: "REPOSITORY_COVERAGE_INCOMPLETE",
            message: "Some repositories are unavailable.",
          },
        ],
      })
      .mockResolvedValueOnce({
        ...snapshot,
        repositories: [
          { ...snapshot.repositories[0], observed_commit_count: 5 },
        ],
      });
    render(<DayDetail date={snapshot.date} />);
    await screen.findByText("Incomplete day details");
    expect(
      screen.getByText("Some repositories are unavailable."),
    ).toBeInTheDocument();
    expect(screen.getByText("2 commit contributions")).toBeInTheDocument();
    expect(screen.getByText("5")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Retry day details" }));
    await screen.findByText("5 commit contributions");
    expect(
      screen.queryByText("Incomplete day details"),
    ).not.toBeInTheDocument();
    expect(day).toHaveBeenCalledTimes(2);
  });

  it("keeps missing repository counts unknown and displays API issues", async () => {
    day.mockResolvedValue({
      ...snapshot,
      issues: [
        {
          code: "PARTIAL_GRAPHQL",
          message: "GitHub returned partial contribution data.",
        },
      ],
      repositories: [
        {
          ...snapshot.repositories[0],
          observed_commit_count: null,
          partial: true,
        },
      ],
    });
    render(<DayDetail date={snapshot.date} />);
    await waitFor(() =>
      expect(
        screen.getByText("Unknown commit contributions"),
      ).toBeInTheDocument(),
    );
    expect(screen.getByText("Incomplete day details")).toBeInTheDocument();
    expect(
      screen.getByText("GitHub returned partial contribution data."),
    ).toBeInTheDocument();
    expect(
      screen.queryByText("0 commit contributions"),
    ).not.toBeInTheDocument();
  });
});

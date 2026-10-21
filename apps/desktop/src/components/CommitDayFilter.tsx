import { useMemo } from "react";
import type { Commit } from "../types";
import { Button } from "./ui/button";

export function CommitDayFilter({
  commits,
  selected,
  date,
  onDate,
  onSelect,
  disabled = false,
}: {
  commits: Commit[];
  selected: Set<string>;
  date: string;
  onDate: (date: string) => void;
  onSelect: (oids: string[]) => void;
  disabled?: boolean;
}) {
  const dayCommits = useMemo(
    () => commits.filter((commit) => commit.author_date.slice(0, 10) === date),
    [commits, date],
  );
  const byEmail = useMemo(() => {
    const groups = new Map<string, string[]>();
    for (const commit of dayCommits) {
      const oids = groups.get(commit.author_email) ?? [];
      oids.push(commit.oid);
      groups.set(commit.author_email, oids);
    }
    return [...groups.entries()];
  }, [dayCommits]);
  const selectedOutsideDay = date
    ? commits.filter(
        (commit) =>
          selected.has(commit.oid) && commit.author_date.slice(0, 10) !== date,
      ).length
    : 0;
  return (
    <section className="day-evidence" aria-label="Local commit day selection">
      <h2>Choose commits by day</h2>
      <div className="identity-fields">
        <label>
          Filter by author date
          <input
            type="date"
            aria-label="Filter by author date"
            value={date}
            disabled={disabled}
            onChange={(event) => onDate(event.target.value)}
          />
        </label>
        <Button
          variant="secondary"
          disabled={disabled || !date}
          onClick={() => onDate("")}
        >
          All dates
        </Button>
      </div>
      <p className="subtle">
        Days use each commit’s original author date and time-zone offset.
        Filtering does not change your selection.
      </p>
      <p role="status">
        {date
          ? `${dayCommits.length} local commit${dayCommits.length === 1 ? "" : "s"} on ${date}.`
          : `Showing all ${commits.length} local commit${commits.length === 1 ? "" : "s"}.`}
        {selectedOutsideDay > 0 &&
          ` ${selectedOutsideDay} selected commit${selectedOutsideDay === 1 ? " is" : "s are"} outside this day.`}
      </p>
      {date && dayCommits.length === 0 && (
        <p className="subtle">
          No local commits have this author date in the inspected history.
          GitHub may show activity from other repositories, pull requests,
          reviews, or restricted contributions on this day.
        </p>
      )}
      {date && dayCommits.length > 0 && (
        <>
          <p className="subtle">
            Choose an exact author email to replace your current selection with
            its commits on this day. You will still need to confirm ownership.
          </p>
          <div className="flex flex-wrap gap-2 my-3">
            {byEmail.map(([email, oids]) => (
              <Button
                key={email}
                variant="secondary"
                disabled={disabled}
                onClick={() => onSelect([...oids])}
              >
                Select {oids.length} commit{oids.length === 1 ? "" : "s"} by{" "}
                {email || "(empty author email)"} on this day
              </Button>
            ))}
          </div>
        </>
      )}
      <p className="subtle">
        Selection proposes an attribution change only. File contents, commit
        dates, and parent relationships are preserved. GitHub’s observed totals
        and non-commit activity remain separate; selecting commits does not
        remove calendar events.
      </p>
    </section>
  );
}

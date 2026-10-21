import { useEffect, useState } from "react";
import { bridge } from "../bridge";
import type { DaySnapshot } from "../types";
import { Button } from "./ui/button";

export function DayDetail({ date }: { date: string }) {
  const [data, setData] = useState<DaySnapshot | null>(null);
  const [error, setError] = useState("");
  const [retry, setRetry] = useState(0);
  useEffect(() => {
    let active = true;
    setData(null);
    setError("");
    void bridge
      .day(date)
      .then((result) => {
        if (active) setData(result);
      })
      .catch((e) => {
        if (active) setError(e.message);
      });
    return () => {
      active = false;
    };
  }, [date, retry]);
  if (error)
    return (
      <div className="notice notice-warning">
        <span>{error}</span>
        <Button variant="secondary" onClick={() => setRetry(retry + 1)}>
          Retry day details
        </Button>
      </div>
    );
  if (!data)
    return (
      <p role="status" className="subtle">
        Loading repository and contribution evidence…
      </p>
    );
  const labels: Record<keyof DaySnapshot["totals"], string> = {
    commits: "Commits",
    issues: "Issues",
    pull_requests: "Pull requests",
    reviews: "Reviews",
    repositories_created: "Repositories created",
    restricted: "Restricted",
  };
  return (
    <section className="day-evidence">
      {data.stale && (
        <div className="notice notice-warning" role="status">
          <div>
            <strong>Saved day details · refresh failed</strong>
            <p>{data.fetch_error || "GitHub could not be refreshed."}</p>
            {data.fetched_at !== undefined && (
              <p>
                Observed{" "}
                {new Date(
                  typeof data.fetched_at === "number"
                    ? data.fetched_at * 1000
                    : data.fetched_at,
                ).toLocaleString()}
                .
              </p>
            )}
            <Button
              variant="secondary"
              size="sm"
              onClick={() => setRetry((value) => value + 1)}
            >
              Retry day refresh
            </Button>
          </div>
        </div>
      )}
      {(data.partial || Boolean(data.issues?.length)) && (
        <div className="notice notice-warning" role="status">
          <div>
            <strong>Incomplete day details</strong>
            <p>
              Some repository or contribution details are unavailable. Observed
              totals may include activity that is not listed below.
            </p>
            {data.issues?.map((issue, index) => (
              <p key={`${issue.code}-${index}`}>{issue.message}</p>
            ))}
            <Button
              variant="secondary"
              size="sm"
              onClick={() => setRetry((value) => value + 1)}
            >
              Retry day details
            </Button>
          </div>
        </div>
      )}
      <div className="day-category-grid">
        {Object.entries(data.totals).map(([key, value]) => (
          <div key={key}>
            <strong className="mono">{value ?? "Unknown"}</strong>
            <span>{labels[key as keyof typeof labels]}</span>
          </div>
        ))}
      </div>
      {data.repositories.map((repo) => (
        <div className="setting-row" key={repo.full_name}>
          <div>
            <strong>{repo.full_name}</strong>
            <p>
              {repo.private ? "Private repository" : "Public repository"}
              {repo.partial ? " · Partial coverage" : ""}
            </p>
          </div>
          <span className="mono">
            {repo.observed_commit_count ?? "Unknown"} commit contributions
          </span>
        </div>
      ))}
      <p className="small subtle">
        {data.coverage} These are observed aggregates; exact candidate commits
        come from a local history scan.
      </p>
    </section>
  );
}

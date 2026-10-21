import { useMemo, useRef, useState } from "react";
import {
  ArrowDown,
  ChevronLeft,
  ChevronRight,
  CalendarDays,
  Grid2X2,
  List,
  RefreshCw,
  Info,
} from "lucide-react";
import * as Tabs from "@radix-ui/react-tabs";
import { Button } from "./ui/button";
import { DayDetail } from "./DayDetail";
import type { ContributionDay, ContributionSnapshot } from "../types";
export function yearDays(year: number) {
  const days: string[] = [];
  for (
    let d = new Date(Date.UTC(year, 0, 1));
    d.getUTCFullYear() === year;
    d.setUTCDate(d.getUTCDate() + 1)
  )
    days.push(d.toISOString().slice(0, 10));
  return days;
}
export function dayLabel(date: string, observation?: ContributionDay) {
  return `${new Date(date + "T12:00:00Z").toLocaleDateString("en-US", { weekday: "long", month: "long", day: "numeric", year: "numeric", timeZone: "UTC" })}: ${observation?.count != null ? `${observation.count} observed contributions` : "not loaded or unavailable"}`;
}
export function Calendar({
  snapshot: receivedSnapshot,
  year,
  onYear,
  onRefresh,
  onConnect,
  onInspectDay,
  connected,
  busy,
  planSelection = {},
}: {
  snapshot: ContributionSnapshot | null;
  year: number;
  onYear: (year: number) => void;
  onRefresh: () => void;
  onConnect: () => void;
  onInspectDay?: (date: string) => void;
  connected: boolean;
  busy: boolean;
  planSelection?: Record<string, { selected: number; inspected: number }>;
}) {
  const days = useMemo(() => yearDays(year), [year]);
  const snapshot = receivedSnapshot?.year === year ? receivedSnapshot : null;
  const data = useMemo(
    () => new Map(snapshot?.days.map((day) => [day.date, day])),
    [snapshot],
  );
  const [selected, setSelected] = useState<string | null>(null);
  const [focus, setFocus] = useState(0);
  const [layer, setLayer] = useState<"observed" | "plan">("observed");
  const showPlan = layer === "plan" && Object.keys(planSelection).length > 0;
  const selectionLabel = (date: string) => {
    const selection = planSelection[date];
    return showPlan && selection
      ? `; selected for attribution change: ${selection.selected} of ${selection.inspected} inspected commits; ${selection.selected < selection.inspected ? "partial local selection" : "other activity may remain"}`
      : "";
  };
  const cells = useRef<(HTMLButtonElement | null)[]>([]);
  const offset = new Date(`${year}-01-01T12:00:00Z`).getUTCDay();
  const total =
    snapshot &&
    !snapshot.partial &&
    snapshot.days.length > 0 &&
    snapshot.days.every((day) => day.count !== null)
      ? snapshot.days.reduce((sum, day) => sum + (day.count ?? 0), 0)
      : undefined;
  const selectedDay = selected ? data.get(selected) : undefined;
  function move(index: number, key: string) {
    const delta = {
      ArrowRight: 7,
      ArrowLeft: -7,
      ArrowDown: 1,
      ArrowUp: -1,
      Home: -index,
      End: days.length - 1 - index,
    }[key];
    if (delta === undefined) return false;
    const next = Math.min(days.length - 1, Math.max(0, index + delta));
    setFocus(next);
    cells.current[next]?.focus();
    return true;
  }
  return (
    <div className="calendar-page">
      <div className="page-heading">
        <div>
          <div className="eyebrow">ATTRIBUTION OVERVIEW</div>
          <h1>Contribution calendar</h1>
          <p>
            Start with what GitHub sees. Understand what your history contains.
          </p>
        </div>
        <div className="year-control">
          <Button
            variant="ghost"
            size="icon"
            aria-label="Previous year"
            disabled={year <= 2008}
            onClick={() => {
              onYear(year - 1);
              setSelected(null);
            }}
          >
            <ChevronLeft size={16} />
          </Button>
          <span className="mono">{year}</span>
          <Button
            variant="ghost"
            size="icon"
            aria-label="Next year"
            disabled={year >= new Date().getFullYear()}
            onClick={() => {
              onYear(year + 1);
              setSelected(null);
            }}
          >
            <ChevronRight size={16} />
          </Button>
        </div>
      </div>
      {Object.keys(planSelection).length > 0 && (
        <div className="plan-layer-toolbar">
          <div
            className="view-switch"
            role="group"
            aria-label="Calendar data layer"
          >
            <button
              aria-pressed={!showPlan}
              onClick={() => setLayer("observed")}
            >
              Observed snapshot
            </button>
            <button aria-pressed={showPlan} onClick={() => setLayer("plan")}>
              Plan selection
            </button>
          </div>
          <span className="subtle">
            Author dates from the inspected Git history. Calendar changes are
            uncertain.
          </span>
        </div>
      )}
      <section
        className="calendar-surface"
        aria-label="Contribution observations"
      >
        <div className="section-toolbar">
          <div>
            <span className="large-number">
              {total === undefined ? "—" : total.toLocaleString()}
            </span>
            <span className="number-label"> observed contributions</span>
          </div>
          <div
            className={`status ${snapshot?.partial || snapshot?.stale ? "status-warning" : ""}`}
          >
            <span className="status-dot" />
            {snapshot
              ? snapshot.stale
                ? "Saved snapshot · refresh failed"
                : snapshot.partial
                  ? "Partial visibility"
                  : "Snapshot loaded"
              : connected
                ? "No snapshot loaded"
                : "Awaiting GitHub connection"}
          </div>
        </div>
        <Tabs.Root defaultValue="grid">
          <div className="calendar-subtoolbar">
            <span className="subtle">
              {snapshot
                ? `Observed ${new Date(snapshot.fetched_at).toLocaleString()}`
                : "No contribution data has been loaded"}
            </span>
            <Tabs.List className="view-switch" aria-label="Calendar view">
              <Tabs.Trigger value="grid" aria-label="Grid view">
                <Grid2X2 size={14} />
              </Tabs.Trigger>
              <Tabs.Trigger value="list" aria-label="Day list view">
                <List size={15} />
              </Tabs.Trigger>
            </Tabs.List>
          </div>
          <Tabs.Content value="grid">
            <div className="heatmap-scroll">
              <div className="heatmap-months">
                {"Jan Feb Mar Apr May Jun Jul Aug Sep Oct Nov Dec"
                  .split(" ")
                  .map((month) => (
                    <span key={month}>{month}</span>
                  ))}
              </div>
              <div className="heatmap-layout">
                <div className="day-axis">
                  <span>Mon</span>
                  <span>Wed</span>
                  <span>Fri</span>
                </div>
                <div
                  className="heatmap"
                  role="group"
                  aria-label={`${year} contribution days. Use arrow keys to navigate.`}
                >
                  {Array.from({ length: offset }, (_, i) => (
                    <span className="heatmap-spacer" key={"pad" + i} />
                  ))}
                  {days.map((date, index) => (
                    <button
                      ref={(el) => {
                        cells.current[index] = el;
                      }}
                      key={date}
                      tabIndex={index === focus ? 0 : -1}
                      onFocus={() => setFocus(index)}
                      onKeyDown={(event) => {
                        if (move(index, event.key)) event.preventDefault();
                      }}
                      onClick={() => setSelected(date)}
                      className={`heatmap-cell ${data.get(date)?.count != null && data.get(date)?.level != null ? `level-${data.get(date)!.level}` : "unknown"} ${selected === date ? "day-selected" : ""} ${showPlan && planSelection[date] ? "plan-selected" : ""} ${showPlan && planSelection[date]?.selected < planSelection[date]?.inspected ? "plan-partial" : ""}`}
                      aria-label={
                        dayLabel(date, data.get(date)) + selectionLabel(date)
                      }
                      aria-pressed={selected === date}
                      title={
                        dayLabel(date, data.get(date)) + selectionLabel(date)
                      }
                    />
                  ))}
                </div>
              </div>
            </div>
          </Tabs.Content>
          <Tabs.Content value="list">
            <div className="calendar-day-list">
              {snapshot?.days.length ? (
                snapshot.days.map((day) => (
                  <button
                    key={day.date}
                    className="day-list-row"
                    onClick={() => setSelected(day.date)}
                  >
                    <span className="mono">{day.date}</span>
                    <span>
                      {day.count === null
                        ? "Unavailable"
                        : `${day.count} observed contributions`}
                    </span>
                    {showPlan && planSelection[day.date] && (
                      <span className="text-warning">
                        {planSelection[day.date].selected} selected for
                        attribution change · {planSelection[day.date].inspected}{" "}
                        inspected
                      </span>
                    )}
                    <ArrowDown size={14} />
                  </button>
                ))
              ) : (
                <div className="inline-empty">
                  <CalendarDays size={22} />
                  <span>Connect GitHub to load the accessible day list.</span>
                </div>
              )}
            </div>
          </Tabs.Content>
        </Tabs.Root>
        <div className="calendar-legend">
          <span>
            <span className="legend-unknown" /> Not loaded / unavailable
          </span>
          {showPlan && (
            <span>
              <span className="legend-cell plan-selected" /> Selected for
              attribution change · dot = partial local selection
            </span>
          )}
          <div>
            <span>Less</span>
            {[0, 1, 2, 3, 4].map((level) => (
              <span key={level} className={`legend-cell level-${level}`} />
            ))}
            <span>More</span>
          </div>
        </div>
        <div className="calendar-footnote">
          <Info size={14} />
          <span>
            Total observed contributions include more than commits. Selecting a
            day does not change history.
          </span>
        </div>
      </section>
      {snapshot?.stale && (
        <div className="notice notice-warning" role="status">
          <Info size={16} />
          <div>
            <strong>Showing the last saved observation</strong>
            <p>
              {snapshot.fetch_error || "GitHub could not be refreshed."} The
              displayed counts and observation timestamp have not changed.
            </p>
            <Button
              variant="secondary"
              size="sm"
              disabled={busy || !connected}
              onClick={onRefresh}
            >
              Retry calendar refresh
            </Button>
          </div>
        </div>
      )}
      {selected && (
        <section className="day-detail">
          <div>
            <h2>
              {new Date(selected + "T12:00:00Z").toLocaleDateString("en-US", {
                month: "long",
                day: "numeric",
                timeZone: "UTC",
              })}
            </h2>
            <p>
              {selectedDay?.count != null
                ? `${selectedDay.count} observed contributions. Repository-level attribution must be inspected separately.`
                : "This day has not been loaded. Unknown is different from zero."}
            </p>
          </div>
          <span className="badge mono">{selected}</span>
        </section>
      )}
      {selected && showPlan && planSelection[selected] && (
        <div className="notice notice-warning">
          <Info size={16} />
          <span>
            <strong>Selected for attribution change</strong>
            <br />
            {planSelection[selected].selected} of{" "}
            {planSelection[selected].inspected} inspected commits selected.
            Other commits, repositories, pull requests, reviews, and restricted
            activity may remain. Observed counts are unchanged.
          </span>
        </div>
      )}
      {selected && snapshot && connected && <DayDetail date={selected} />}
      {selected && onInspectDay && (
        <div className="coverage-row">
          <Info size={16} />
          <p>
            Inspect a local repository for commits with this author date. A
            calendar count does not identify exact commits or prove ownership.
            Attribution changes preserve the files introduced by those commits.
          </p>
          <Button
            variant="secondary"
            disabled={busy}
            onClick={() => onInspectDay(selected)}
          >
            Inspect commits for this day
          </Button>
        </div>
      )}
      {!snapshot ? (
        <section className="connect-callout">
          <div className="callout-icon">
            <CalendarDays size={23} />
          </div>
          <div>
            <h2>A clear view of your contributions</h2>
            <p>
              Connect your GitHub account to load your calendar and understand
              its coverage. Your repositories and rewrite artifacts stay on this
              Mac.
            </p>
          </div>
          <Button onClick={connected ? onRefresh : onConnect} disabled={busy}>
            {connected ? "Load calendar" : "Connect GitHub"}{" "}
            <ChevronRight size={16} />
          </Button>
        </section>
      ) : (
        <div className="coverage-row">
          <Info size={16} />
          <p>{snapshot.coverage_note}</p>
          <Button
            variant="secondary"
            disabled={busy || !connected}
            onClick={onRefresh}
          >
            <RefreshCw size={14} className={busy ? "spin" : ""} />
            Refresh snapshot
          </Button>
        </div>
      )}
      <div className="principle-row">
        <span className="mono">01 / OBSERVE</span>
        <p>
          The calendar is a record of GitHub’s response.
          <br />
          <span>Local changes never overwrite observed counts.</span>
        </p>
      </div>
    </div>
  );
}

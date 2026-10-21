import type { Commit } from "../types";

export function HistoryGraph({
  commits,
  selected,
  affected,
}: {
  commits: Commit[];
  selected: Set<string>;
  affected: Set<string>;
}) {
  // Show a complete graph only when it fits; never imply omitted history was deleted.
  if (commits.length > 80)
    return (
      <p className="subtle">
        {commits.length} original commit nodes. Graph preview is limited to 80
        nodes; the complete commit mapping must be checked during candidate
        validation.
      </p>
    );
  const byId = new Map(commits.map((commit) => [commit.oid, commit]));
  if (
    commits.some((commit) => commit.parents.some((parent) => !byId.has(parent)))
  )
    return (
      <p className="notice notice-warning">
        The available history has parent connections outside this scan. A
        complete topology preview is unavailable; missing commits are not
        treated as removed.
      </p>
    );
  const sorted: Commit[] = [];
  const seen = new Set<string>();
  function visit(commit: Commit) {
    if (seen.has(commit.oid)) return;
    seen.add(commit.oid);
    for (const parent of commit.parents) {
      const node = byId.get(parent);
      if (node) visit(node);
    }
    sorted.push(commit);
  }
  commits.forEach(visit);
  const index = new Map(sorted.map((commit, i) => [commit.oid, i]));
  return (
    <div className="history-preview">
      <div className="section-title">
        <h3>Original topology to preserve</h3>
        <span className="subtle">
          {sorted.length} nodes ·{" "}
          {sorted.reduce((sum, c) => sum + c.parents.length, 0)} parent edges
        </span>
      </div>
      <svg
        viewBox={`0 0 700 ${sorted.length * 34 + 16}`}
        role="img"
        aria-label="Original commit nodes and ordered parent connections. Coral identity markers indicate selected attribution edits; all nodes and edges must be preserved during validation."
      >
        {sorted.flatMap((commit, i) =>
          commit.parents.map((parent, p) => {
            const row = index.get(parent);
            if (row === undefined) return null;
            const y1 = row * 34 + 20,
              y2 = i * 34 + 20;
            return (
              <path
                key={`${commit.oid}-${p}`}
                d={
                  p === 0
                    ? `M 20 ${y1} L 20 ${y2}`
                    : `M 20 ${y1} C ${70 + p * 15} ${y1}, ${70 + p * 15} ${y2}, 20 ${y2}`
                }
                fill="none"
                stroke="#A1A1AA"
                strokeWidth="1.2"
              />
            );
          }),
        )}
        {sorted.map((commit, i) => (
          <g key={commit.oid} transform={`translate(0 ${i * 34 + 20})`}>
            <circle
              cx="20"
              cy="0"
              r="4"
              fill="#141416"
              stroke={affected.has(commit.oid) ? "#B8E65C" : "#F4F4F5"}
              strokeWidth="1.5"
            />
            {selected.has(commit.oid) && (
              <circle cx="44" cy="8" r="3" fill="#E36D6D" />
            )}
            <text
              x="85"
              y="4"
              fill="#A1A1AA"
              fontSize="12"
              fontFamily="Geist Mono, monospace"
            >
              {commit.oid.slice(0, 10)}
            </text>
            <text x="184" y="4" fill="#F4F4F5" fontSize="12">
              {commit.subject.slice(0, 51)}
            </text>
            <title>
              {commit.oid}: {commit.subject}
              {selected.has(commit.oid) ? " · Selected author email edit" : ""}
            </title>
          </g>
        ))}
      </svg>
      <p className="small subtle">
        Coral marks the identity association to change. Candidate validation
        must preserve every commit and ordered parent edge shown here.
      </p>
    </div>
  );
}

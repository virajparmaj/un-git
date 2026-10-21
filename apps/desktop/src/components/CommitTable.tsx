import { useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { Copy, GitCommitHorizontal, Check } from "lucide-react";
import { Dialog } from "./ui/dialog";
import { Button } from "./ui/button";
import type { Commit } from "../types";
export function CommitTable({
  commits,
  selected,
  onToggle,
  disabled = false,
}: {
  commits: Commit[];
  selected: Set<string>;
  onToggle: (oid: string) => void;
  disabled?: boolean;
}) {
  const parent = useRef<HTMLDivElement>(null);
  const [detail, setDetail] = useState<Commit | null>(null);
  const [copied, setCopied] = useState(false);
  const [copyError, setCopyError] = useState("");
  const virtualizer = useVirtualizer({
    count: commits.length,
    getScrollElement: () => parent.current,
    estimateSize: () => 48,
    overscan: 12,
  });
  const rows = virtualizer.getVirtualItems();
  async function copy() {
    try {
      await navigator.clipboard.writeText(detail!.oid);
      setCopied(true);
    } catch {
      setCopyError(
        "Copy unavailable. Select the full SHA below to copy it manually.",
      );
    }
  }
  return (
    <>
      <div
        className="commit-table"
        role="table"
        aria-label="Local commit history"
        aria-rowcount={commits.length + 1}
      >
        <div className="commit-table-head commit-columns" role="row">
          <span role="columnheader">
            <span className="sr-only">Select</span>
          </span>
          <span role="columnheader">Commit</span>
          <span role="columnheader">Author</span>
          <span role="columnheader">Author date</span>
          <span role="columnheader">Object ID</span>
        </div>
        <div ref={parent} className="commit-scroll">
          <div
            style={{ height: virtualizer.getTotalSize(), position: "relative" }}
          >
            {rows.map((row) => {
              const commit = commits[row.index];
              return (
                <div
                  key={commit.oid}
                  className={`commit-row commit-columns ${selected.has(commit.oid) ? "commit-selected" : ""}`}
                  role="row"
                  aria-rowindex={row.index + 2}
                  style={{
                    position: "absolute",
                    top: 0,
                    left: 0,
                    width: "100%",
                    height: row.size,
                    transform: `translateY(${row.start}px)`,
                  }}
                >
                  <span role="cell">
                    <input
                      type="checkbox"
                      disabled={disabled}
                      checked={selected.has(commit.oid)}
                      onChange={() => onToggle(commit.oid)}
                      aria-label={`Select ${commit.oid.slice(0, 8)} ${commit.subject}`}
                    />
                  </span>
                  <button
                    role="cell"
                    className="commit-subject"
                    onClick={() => {
                      setDetail(commit);
                      setCopied(false);
                      setCopyError("");
                    }}
                  >
                    <GitCommitHorizontal size={15} />
                    <span>{commit.subject || "(empty message)"}</span>
                  </button>
                  <span
                    role="cell"
                    className="commit-author"
                    title={commit.author_email}
                  >
                    {commit.author_name}
                    <small>{commit.author_email}</small>
                  </span>
                  <span role="cell" className="mono date-cell">
                    {commit.author_date}
                  </span>
                  <button
                    role="cell"
                    className="hash-button mono"
                    onClick={() => {
                      setDetail(commit);
                      setCopied(false);
                      setCopyError("");
                    }}
                    aria-label={`Inspect full commit ${commit.oid}`}
                  >
                    {commit.oid.slice(0, 8)}
                  </button>
                </div>
              );
            })}
          </div>
        </div>
      </div>
      <Dialog
        open={!!detail}
        onOpenChange={(open) => {
          if (!open) setDetail(null);
        }}
        title="Commit details"
        description="Raw metadata from the local Git object. Identity ownership is not inferred from a matching name."
      >
        {detail && (
          <>
            <div className="detail-subject">
              {detail.subject || "(empty message)"}
            </div>
            <dl className="metadata-list">
              <dt>Object ID</dt>
              <dd className="mono full-value">
                {detail.oid}
                <Button
                  variant="ghost"
                  size="icon"
                  onClick={copy}
                  aria-label="Copy full commit ID"
                >
                  {copied ? <Check size={14} /> : <Copy size={14} />}
                </Button>
              </dd>
              <dt>Author</dt>
              <dd>
                {detail.author_name}
                <br />
                <span className="mono">{detail.author_email}</span>
              </dd>
              <dt>Committer</dt>
              <dd>
                {detail.committer_name}
                <br />
                <span className="mono">{detail.committer_email}</span>
              </dd>
              <dt>Author date</dt>
              <dd className="mono">{detail.author_date}</dd>
              <dt>Tree</dt>
              <dd className="mono full-value">{detail.tree_oid}</dd>
              <dt>Ordered parents</dt>
              <dd className="mono full-value">
                {detail.parents.length
                  ? detail.parents.map((p, i) => (
                      <div key={p}>
                        {i + 1}. {p}
                      </div>
                    ))
                  : "Root commit"}
              </dd>
            </dl>
            {copyError && (
              <p role="alert" className="notice">
                {copyError}
              </p>
            )}
            {detail.blockers.length > 0 && (
              <div className="notice notice-warning">
                {detail.blockers.join(" · ")}
              </div>
            )}
          </>
        )}
      </Dialog>
    </>
  );
}

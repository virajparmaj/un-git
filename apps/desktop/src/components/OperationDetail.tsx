import {
  Check,
  CheckCircle2,
  Download,
  GitBranch,
  ShieldCheck,
  Clock3,
  FolderLock,
  AlertTriangle,
  RefreshCw,
  ChevronRight,
} from "lucide-react";
import { Button } from "./ui/button";
import type { Operation } from "../types";
const titles: Record<string, string> = {
  SCANNED: "Scanned",
  PLANNED: "Planned",
  BACKUP_VERIFIED: "Backup verified",
  PREPARING: "Preparing candidate",
  VALIDATED: "Local validation passed",
  AWAITING_APPROVAL: "Local candidate ready",
  READY_TO_PUBLISH: "Ready for review",
  CANCELLED_BEFORE_PUSH: "Cancelled before publication",
  BLOCKED: "Operation blocked",
  FAILED_LOCAL: "Local preparation failed",
  REMOTE_OUTCOME_UNKNOWN: "Remote outcome unknown",
  REMOTE_DRIFT: "Remote drift detected",
  NEEDS_ATTENTION: "Needs attention",
  REFS_VERIFIED: "Repository updated",
  OBSERVING: "Calendar observation pending",
};
export const stateTitle = (state: string) =>
  titles[state] || state.toLowerCase().replaceAll("_", " ");
export function OperationDetail({
  operation,
  onExport,
  onRefresh,
  onVerify,
  onCancel,
  busy,
  exportPath,
}: {
  operation: Operation;
  onExport: () => void;
  onRefresh: () => void;
  onVerify: () => void;
  onCancel: () => void;
  busy: boolean;
  exportPath: string;
}) {
  const validationIsCurrent =
    ["VALIDATED", "AWAITING_APPROVAL", "READY_TO_PUBLISH"].includes(
      operation.state,
    ) && operation.blockers.length === 0;
  const passed =
    validationIsCurrent &&
    operation.validation_checks.length > 0 &&
    operation.validation_checks.every((check) => check.passed);
  const blocked =
    operation.blockers.length > 0 ||
    [
      "BLOCKED",
      "FAILED_LOCAL",
      "REMOTE_OUTCOME_UNKNOWN",
      "REMOTE_DRIFT",
      "NEEDS_ATTENTION",
    ].includes(operation.state);
  const priorEvidence =
    !validationIsCurrent && operation.validation_checks.length > 0;
  return (
    <div className="operation-detail">
      <div className="operation-top">
        <div className={`operation-symbol ${passed ? "good" : ""}`}>
          {passed ? (
            <ShieldCheck size={30} />
          ) : blocked ? (
            <AlertTriangle size={28} className="text-warning" />
          ) : (
            <Clock3 size={28} />
          )}
        </div>
        <div>
          <span className="eyebrow">LOCAL OPERATION</span>
          <h2>
            {!passed &&
            ["VALIDATED", "AWAITING_APPROVAL", "READY_TO_PUBLISH"].includes(
              operation.state,
            )
              ? "Validation requires attention"
              : stateTitle(operation.state)}
          </h2>
          <p className="mono operation-id">{operation.id}</p>
        </div>
        <Button
          variant="ghost"
          size="icon"
          disabled={busy}
          onClick={onRefresh}
          aria-label="Refresh operation"
        >
          <RefreshCw size={16} className={busy ? "spin" : ""} />
        </Button>
      </div>
      <div className="operation-metrics">
        <div>
          <strong>{operation.selected_count}</strong>
          <span>Selected commits</span>
        </div>
        <div>
          <strong>{operation.rewritten_count}</strong>
          <span>
            {operation.candidate_oid
              ? "Commit objects rewritten"
              : "Predicted affected objects"}
          </span>
        </div>
        <div>
          <strong>
            {operation.validation_checks.filter((check) => check.passed).length}
            <small> / {operation.validation_checks.length}</small>
          </strong>
          <span>
            {priorEvidence
              ? "Previously passed checks"
              : "Validation checks passed"}
          </span>
        </div>
      </div>
      <div className="status-split">
        <div>
          <GitBranch size={17} />
          <span>Repository</span>
          <strong>{operation.repository_status || "Not published"}</strong>
        </div>
        <div>
          <Clock3 size={17} />
          <span>Calendar</span>
          <strong>
            {operation.calendar_status || "Not observed · local operation"}
          </strong>
        </div>
      </div>
      {blocked && (
        <div className="notice notice-warning">
          <AlertTriangle size={17} />
          <div>
            <strong>Attention required</strong>
            {operation.blockers.length === 0 && (
              <p>
                The operation is not currently validated. Review its state and
                recheck the stored artifacts.
              </p>
            )}
            {operation.blockers.map((b, i) => (
              <p key={i}>{b}</p>
            ))}
          </div>
        </div>
      )}
      <section className="detail-section">
        <div className="section-title">
          <h3>Validation evidence</h3>
          <span className="badge">
            {passed
              ? "All checks passed"
              : priorEvidence
                ? "Previous results · not current approval"
                : "Actual engine results"}
          </span>
        </div>
        {operation.validation_checks.length ? (
          <div className="validation-list">
            {operation.validation_checks.map((check, i) => (
              <div key={i} className="validation-row">
                {check.passed && validationIsCurrent ? (
                  <CheckCircle2 size={16} className="text-accent" />
                ) : check.passed ? (
                  <Clock3 size={16} className="subtle" />
                ) : (
                  <AlertTriangle size={16} className="text-warning" />
                )}
                <div>
                  <strong>{check.name}</strong>
                  <p>{check.detail}</p>
                </div>
                <span
                  className={`mono ${check.passed && validationIsCurrent ? "text-accent" : check.passed ? "subtle" : "text-warning"}`}
                >
                  {check.passed
                    ? validationIsCurrent
                      ? "PASS"
                      : "PREVIOUS PASS"
                    : "BLOCKED"}
                </span>
              </div>
            ))}
          </div>
        ) : (
          <p className="subtle">
            Validation results appear after the engine completes its checks.
          </p>
        )}
      </section>
      <section className="detail-section">
        <h3>{passed ? "Exact local result" : "Recorded local artifacts"}</h3>
        <div className="oid-comparison">
          <div>
            <span>Baseline</span>
            <code>{operation.baseline_oid || "Not available"}</code>
          </div>
          <ChevronRight size={16} />
          <div>
            <span>Candidate</span>
            <code>{operation.candidate_oid || "Not created"}</code>
          </div>
        </div>
      </section>
      {operation.backup_path && (
        <section className="backup-row">
          <FolderLock size={19} />
          <div>
            <strong>
              {passed
                ? "Verified recovery bundle"
                : "Recovery bundle · recheck before use"}
            </strong>
            <p className="mono full-value">{operation.backup_path}</p>
            <p>
              Keep this private repository backup until the operation is
              resolved.
            </p>
          </div>
        </section>
      )}
      <div className="notice">
        <ShieldCheck size={17} />
        <span>
          Remote publication is unavailable in this build. A validated local
          candidate does not change GitHub or its contribution calendar.
        </span>
      </div>
      <div className="operation-actions">
        {operation.candidate_oid && (
          <Button variant="secondary" onClick={onVerify} disabled={busy}>
            <ShieldCheck size={15} />
            Revalidate artifacts
          </Button>
        )}
        {["PLANNED", "BACKUP_VERIFIED", "PREPARING"].includes(
          operation.state,
        ) && (
          <Button variant="secondary" onClick={onCancel}>
            Cancel local preparation
          </Button>
        )}
        <Button variant="secondary" onClick={onExport} disabled={busy}>
          <Download size={15} />
          Review diagnostic report
        </Button>
        <span className="subtle">Redacted locally. No automatic upload.</span>
      </div>
      {exportPath && (
        <div className="notice">
          <Check size={16} />
          <span>
            Report saved to{" "}
            <span className="mono full-value">{exportPath}</span>
          </span>
        </div>
      )}
    </div>
  );
}

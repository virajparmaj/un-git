import { ArrowRight, BookOpen, GitBranch, Info } from "lucide-react";
import { Button } from "./ui/button";

export function Instructions({
  onDemo,
  onRepositories,
  onCalendar,
  disabled,
  dataDirectory,
}: {
  onDemo: () => void;
  onRepositories: () => void;
  onCalendar: () => void;
  disabled: boolean;
  dataDirectory?: string;
}) {
  return (
    <article className="instructions">
      <div className="page-heading">
        <div>
          <div className="eyebrow">A GUIDED FIRST RUN</div>
          <h1>Instructions</h1>
          <p>
            Inspect a day, prepare an attribution change, and verify what stays
            intact.
          </p>
        </div>
        <BookOpen size={23} className="text-secondary" />
      </div>
      <div className="notice">
        <Info size={17} />
        <div>
          <strong>This build prepares changes locally.</strong>
          <p>
            It can read GitHub and validate an independent candidate. It cannot
            publish, restore remote history, or remove contributions from
            GitHub’s calendar.
          </p>
        </div>
      </div>

      <section className="instructions-section" aria-labelledby="demo-guide">
        <span className="eyebrow">01 / TRY THE LOCAL ENGINE</span>
        <h2 id="demo-guide">First test: the generated demo</h2>
        <p>
          Start here. The demo creates isolated history on this Mac and needs no
          GitHub authorization.
        </p>
        <Button onClick={onDemo} disabled={disabled}>
          <GitBranch size={16} /> Create demo and start
        </Button>
        <ol>
          <li>
            <strong>Create the demo.</strong> Use the button above, or
            Repositories → Create local demo.
            <p className="instruction-result">
              Expect: “Attribution demo,” a synthetic fixture badge, and 6
              commits on main.
            </p>
          </li>
          <li>
            <strong>Select the historical commit.</strong> Search for{" "}
            <code>Selected historical commit</code> and check that row. Choose{" "}
            <code>demo@un-git.test</code> in Confirmed author email.
            <p className="instruction-result">
              Expect: 1 selected commit. This generated identity will not match
              your GitHub account; that is expected for this demo.
            </p>
          </li>
          <li>
            <strong>Confirm the synthetic identity and review.</strong> Check
            the identity confirmation, then choose Review local plan. This
            confirmation is for the generated fixture only; never use it to
            claim someone else’s identity.
            <p className="instruction-result">
              Expect: 1 selected commit and 5 affected commit objects, with all
              6 commits and their parent relationships still in the graph.
            </p>
          </li>
          <li>
            <strong>Prepare the candidate.</strong> Choose Prepare local
            candidate, review the local-only summary, then Prepare and validate.
            <p className="instruction-result">
              Expect: Operations shows “Local validation passed,” 13 / 13
              checks, old and new tips, and a verified recovery bundle. GitHub
              is unchanged.
            </p>
          </li>
          <li>
            <strong>Check the saved result.</strong> Use Revalidate artifacts.
            Return to Operations later to reopen it. Review diagnostic report
            lets you inspect the redacted contents before exporting.
            <p className="instruction-result">
              Expect: validation still passes. Export saves a local file and
              uploads nothing.
            </p>
          </li>
        </ol>
      </section>

      <section className="instructions-section" aria-labelledby="import-guide">
        <span className="eyebrow">02 / READ GITHUB HISTORY</span>
        <h2 id="import-guide">Second test: import virajparmaj/test</h2>
        <p>
          This walkthrough uses the already authorized disposable repository.
          Importing only downloads history.
        </p>
        <ol>
          <li>
            <strong>Open Repositories → Refresh access.</strong>
            <p className="instruction-result">
              Expect: virajparmaj/test in the installed repository list with
              Contents read-only. If it is absent, follow “Add more
              repositories” below.
            </p>
          </li>
          <li>
            <strong>Choose Import and inspect beside virajparmaj/test.</strong>{" "}
            Keep the app open until it finishes. If macOS requests Keychain
            access, handle the prompt directly; never paste credentials into
            un-git or a chat.
            <p className="instruction-result">
              Expect: the repository name, branch, commit table, and local
              snapshot timestamp. An empty repository or unsupported history may
              show a specific blocker.
            </p>
          </li>
          <li>
            <strong>Inspect without preparing a change.</strong> Search a commit
            or set Filter by author date to a date shown in the table. Choose
            All dates to remove the filter.
            <p className="instruction-result">
              Expect: matching local commits. Import again to obtain newer
              remote history; existing snapshots and plans remain unchanged.
            </p>
          </li>
        </ol>
        <Button
          variant="secondary"
          onClick={onRepositories}
          disabled={disabled}
        >
          Open repositories <ArrowRight size={15} />
        </Button>
        <h3>Add more repositories</h3>
        <p>
          In Repositories, choose Manage repository access. On GitHub, choose
          Configure beside your Inspector App, keep Only select repositories,
          add the repositories, and save. Return here and choose Refresh access.
          Signing in and selecting installation repositories are separate steps.
        </p>
        <p>
          Keep the Inspector’s permissions read-only. If authorization expires,
          reconnect through Connect GitHub using the public Inspector client ID.
          No client secret or private key is needed.
        </p>
      </section>

      <section className="instructions-section" aria-labelledby="day-guide">
        <span className="eyebrow">03 / UNDERSTAND A DAY</span>
        <h2 id="day-guide">From calendar counts to exact commits</h2>
        <ol>
          <li>
            In Calendar, load or refresh a snapshot and choose a day. Read the
            category breakdown and coverage notes.
          </li>
          <li>
            Choose Inspect commits for this day, then import the relevant
            installed repository or Browse to an existing clone. The selected
            day carries into the inspector.
          </li>
          <li>
            Use an exact email’s day-selection button or individual checkboxes.
            An email-group button replaces the selection. Changing the date
            filter alone keeps it; selections outside that day remain disclosed.
          </li>
          <li>
            For your own commits, choose the exact author email, review account
            evidence, explicitly confirm ownership, and review the local plan.
            Preparation is available only if the engine’s restrictions pass.
          </li>
        </ol>
        <p>
          GitHub supplies observed totals and repository/day aggregates, not a
          list of exact commit IDs. Totals can include pull requests, reviews,
          repository creation, and restricted activity. Local commits use their
          original author dates and time-zone offsets. An email match is
          ownership evidence, not proof that every matching commit contributes
          to the calendar.
        </p>
        <Button variant="secondary" onClick={onCalendar} disabled={disabled}>
          Open calendar <ArrowRight size={15} />
        </Button>
      </section>

      <section
        className="instructions-section"
        aria-labelledby="preservation-guide"
      >
        <span className="eyebrow">04 / KNOW WHAT CHANGES</span>
        <h2 id="preservation-guide">Attribution changes keep the content</h2>
        <p>
          The current mode changes only the confirmed author email in selected
          commits. It preserves every historical file tree, names, dates,
          messages, coauthor trailers, committer metadata, and ordered parent
          relationships. A selected commit’s additions, edits, and deletions
          remain part of history.
        </p>
        <p>
          This is not a revert, a dropped commit, or a squash. Selected commits
          and their descendants receive new IDs because Git IDs include metadata
          and parent IDs. The source repository remains intact; preparation
          writes a separate candidate and a standalone backup.
        </p>
        <p>
          Validation proves preservation for the supported local rewrite. It
          does not prove a future GitHub calendar change. The replacement email
          policy still needs live attribution testing. Publication, remote
          restore, coauthor editing, and committer-only cleanup are unavailable.
        </p>
      </section>

      <section
        className="instructions-section"
        aria-labelledby="recovery-guide"
      >
        <span className="eyebrow">05 / VERIFY AND RECOVER</span>
        <h2 id="recovery-guide">Results, backups, and problems</h2>
        <p>
          In Operations, read the validation evidence and Exact local result.
          “Local validation passed” means the candidate and restored backup
          passed all checks. Repository status remains “remote unchanged”;
          Calendar is a separate observation.
        </p>
        <p>
          The exact backup path is shown in each operation. App storage is{" "}
          <code>{dataDirectory || "shown in Settings → Storage"}</code>. Backups
          are under{" "}
          <code>operations/&lt;operation-id&gt;/artifacts/backup.bundle</code>;
          imports are under{" "}
          <code>imports/&lt;import-id&gt;/repository.git</code>. Keep backups
          private and retain them while investigating a result.
        </p>
        <dl>
          <dt>Access denied or missing repository</dt>
          <dd>
            Check installation selection and read permissions, then Refresh
            access. Reconnect only when authorization is missing or revoked.
          </dd>
          <dt>Network failure, changed refs, or storage limit</dt>
          <dd>
            Read the error, restore connectivity or free space, and retry Import
            and inspect. Each attempt uses a fresh directory; a failed attempt
            does not replace your current inspection. Imports require at least
            1.25 GiB free and enforce preview limits.
          </dd>
          <dt>Preparation blocked or validation failed</dt>
          <dd>
            Read every blocker. Multi-branch histories, additional refs,
            signatures, workflows, LFS, and submodules can prevent preparation.
            Keep the artifacts; do not edit them to make a check pass. Review
            and export the diagnostic report if needed.
          </dd>
          <dt>Interrupted work</dt>
          <dd>
            Reopen the app and inspect Operations. Interrupted preparation is
            retained and marked failed; create a fresh scan and plan. Imports do
            not resume automatically. If the app is force-quit, a Git download
            can outlive it and its app-managed deadline; automatic orphan
            cleanup is not yet implemented. Before retrying an interrupted
            import, ensure its Git process has ended. Incomplete import folders
            are retained and must not be used as validated snapshots.
          </dd>
        </dl>
        <p>
          There is no remote rollback button in this preview. Restoring a backup
          locally does not update GitHub. No action here guarantees that a
          contribution square disappears.
        </p>
      </section>
    </article>
  );
}

import { useEffect, useState } from "react";
import { CheckCircle2, Info } from "lucide-react";
import { bridge } from "../bridge";
import type { IdentityEvidence as Evidence } from "../types";
import { Button } from "./ui/button";

export function IdentityEvidence({
  email,
  connected,
  accountKey,
}: {
  email: string;
  connected: boolean;
  accountKey?: string;
}) {
  const [evidence, setEvidence] = useState<Evidence | null>(null);
  const [error, setError] = useState("");
  const [retry, setRetry] = useState(0);
  useEffect(() => {
    let active = true;
    setEvidence(null);
    setError("");
    if (connected)
      void bridge
        .identities()
        .then((result) => {
          if (active) setEvidence(result);
        })
        .catch((cause: unknown) => {
          if (active)
            setError(cause instanceof Error ? cause.message : String(cause));
        });
    return () => {
      active = false;
    };
  }, [connected, accountKey, retry]);
  if (!email) return null;
  const match = evidence?.emails.find((item) => item.email === email);
  const verified = match?.verified === true;
  return (
    <div className="notice">
      {verified ? (
        <CheckCircle2 size={16} className="text-accent" />
      ) : (
        <Info size={16} />
      )}
      <div>
        <strong>
          {!connected
            ? "Local confirmation only"
            : error
              ? "Account evidence unavailable"
              : !evidence
                ? "Checking account email evidence…"
                : verified
                  ? "Exact verified GitHub email match"
                  : "No verified exact account email match"}
        </strong>
        <p>
          {error ||
            (verified
              ? "GitHub reports this exact, case-sensitive email as verified on the connected account. Confirm the selected edits below."
              : "Names, aliases, and noreply patterns do not establish ownership. Explicit confirmation is still required.")}
        </p>
        {evidence && (
          <p>
            {evidence.coverage_note} Retrieved{" "}
            {new Date(evidence.fetched_at).toLocaleString()}.
          </p>
        )}
        {error && (
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setRetry((value) => value + 1)}
          >
            Retry account evidence
          </Button>
        )}
      </div>
    </div>
  );
}

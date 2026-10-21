import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { IdentityEvidence } from "../components/IdentityEvidence";
const identities = vi.hoisted(() => vi.fn());
vi.mock("../bridge", () => ({ bridge: { identities } }));
afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});
describe("identity evidence", () => {
  it("only calls an exact case-sensitive verified email an account match", async () => {
    identities.mockResolvedValue({
      emails: [
        {
          email: "Owner@example.test",
          verified: true,
          primary: true,
          visibility: null,
        },
      ],
      fetched_at: "2026-01-01",
      coverage_note: "Account emails only.",
    });
    const { rerender } = render(
      <IdentityEvidence email="owner@example.test" connected />,
    );
    await screen.findByText("No verified exact account email match");
    rerender(<IdentityEvidence email="Owner@example.test" connected />);
    await screen.findByText("Exact verified GitHub email match");
    expect(
      screen.getByText(/Confirm the selected edits below/),
    ).toBeInTheDocument();
  });
  it("exposes unavailable evidence without guessing ownership", async () => {
    identities.mockRejectedValue(new Error("Email permission unavailable"));
    render(<IdentityEvidence email="someone@example.test" connected />);
    await screen.findByText("Account evidence unavailable");
    expect(
      screen.getByText("Email permission unavailable"),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Retry account evidence" }),
    ).toBeInTheDocument();
  });
  it("keeps local-only confirmation distinct from GitHub verification", async () => {
    render(<IdentityEvidence email="demo@example.test" connected={false} />);
    await waitFor(() =>
      expect(screen.getByText("Local confirmation only")).toBeInTheDocument(),
    );
    expect(identities).not.toHaveBeenCalled();
  });
});

import { afterEach, describe, expect, it, vi } from "vitest";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { RemoteRepositories } from "../components/RemoteRepositories";
import type { RepositoryList } from "../types";

const { repositories, openRepositoryAccess } = vi.hoisted(() => ({
  repositories: vi.fn(),
  openRepositoryAccess: vi.fn(),
}));
vi.mock("../bridge", () => ({
  bridge: { repositories, openRepositoryAccess },
}));
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

const access: RepositoryList = {
  partial: false,
  issues: [],
  installations: [
    {
      id: 1,
      account: { login: "fixture" },
      repository_selection: "selected",
      suspended: false,
    },
  ],
  repositories: [
    {
      id: 2,
      full_name: "fixture/selected",
      private: true,
      fork: false,
      archived: false,
      default_branch: "main",
      contents_read: true,
      contents_write: false,
      permission_status: "verified",
    },
  ],
};

describe("repository access management", () => {
  it("imports only the explicit repository ID and retains the list", async () => {
    repositories.mockResolvedValue(access);
    const onImport = vi.fn().mockResolvedValue(undefined);
    render(<RemoteRepositories busy={false} onImport={onImport} />);
    fireEvent.click(screen.getByRole("button", { name: "Refresh access" }));
    const button = await screen.findByRole("button", {
      name: "Import and inspect fixture/selected",
    });
    expect(onImport).not.toHaveBeenCalled();
    fireEvent.click(button);
    await waitFor(() => expect(onImport).toHaveBeenCalledExactlyOnceWith(2));
    await waitFor(() => expect(button).toBeEnabled());
    expect(screen.getByText("fixture/selected")).toBeInTheDocument();
    expect(
      screen.getByText(/isolated local snapshot using read-only/),
    ).toBeInTheDocument();
  });

  it("disables access controls and every import during an import and while the parent is busy", async () => {
    repositories.mockResolvedValue({
      ...access,
      repositories: [
        ...access.repositories,
        { ...access.repositories[0], id: 3, full_name: "fixture/other" },
      ],
    });
    let finishImport!: () => void;
    const onImport = vi.fn().mockImplementation(
      () =>
        new Promise<void>((resolve) => {
          finishImport = resolve;
        }),
    );
    const { rerender } = render(
      <RemoteRepositories busy={false} onImport={onImport} />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Refresh access" }));
    const importButton = await screen.findByRole("button", {
      name: "Import and inspect fixture/selected",
    });
    fireEvent.click(importButton);
    expect(importButton).toHaveTextContent("Importing…");
    for (const button of screen.getAllByRole("button")) {
      expect(button).toBeDisabled();
      fireEvent.click(button);
    }
    expect(onImport).toHaveBeenCalledTimes(1);
    expect(repositories).toHaveBeenCalledTimes(1);
    expect(openRepositoryAccess).not.toHaveBeenCalled();
    await act(async () => finishImport());
    expect(importButton).toBeEnabled();
    rerender(<RemoteRepositories busy onImport={onImport} />);
    for (const button of screen.getAllByRole("button"))
      expect(button).toBeDisabled();
  });

  it("disables imports without content access or a default branch", async () => {
    repositories.mockResolvedValue({
      ...access,
      repositories: [
        { ...access.repositories[0], contents_read: false },
        {
          ...access.repositories[0],
          id: 3,
          full_name: "fixture/empty",
          default_branch: null,
        },
      ],
    });
    const onImport = vi.fn();
    render(<RemoteRepositories busy={false} onImport={onImport} />);
    fireEvent.click(screen.getByRole("button", { name: "Refresh access" }));
    await screen.findByText("fixture/empty");
    for (const button of screen.getAllByRole("button", {
      name: /Import and inspect/,
    })) {
      expect(button).toBeDisabled();
      fireEvent.click(button);
    }
    expect(onImport).not.toHaveBeenCalled();
    expect(
      screen.getByRole("button", { name: "Refresh access" }),
    ).toBeEnabled();
  });

  it("keeps an import failure visible and allows a successful retry without losing the list", async () => {
    repositories.mockResolvedValue(access);
    const onImport = vi
      .fn()
      .mockRejectedValueOnce(new Error("Repository access was revoked"))
      .mockResolvedValueOnce(undefined);
    render(<RemoteRepositories busy={false} onImport={onImport} />);
    fireEvent.click(screen.getByRole("button", { name: "Refresh access" }));
    const button = await screen.findByRole("button", {
      name: "Import and inspect fixture/selected",
    });
    fireEvent.click(button);
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Repository access was revoked",
    );
    expect(screen.getByText("fixture/selected")).toBeInTheDocument();
    expect(button).toBeEnabled();
    fireEvent.click(button);
    await waitFor(() => expect(button).toBeEnabled());
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(onImport).toHaveBeenCalledTimes(2);
    expect(onImport).toHaveBeenNthCalledWith(2, 2);
    expect(screen.getByText("fixture/selected")).toBeInTheDocument();
  });

  it("reports excessive installed grants without suggesting the Inspector can publish", async () => {
    repositories.mockResolvedValue({
      ...access,
      installations: [
        {
          ...access.installations[0],
          permissions: { contents: "write", pull_requests: "write" },
        },
      ],
      repositories: [{ ...access.repositories[0], contents_write: true }],
    });
    render(<RemoteRepositories busy={false} onImport={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "Refresh access" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "This installation has write permissions",
    );
    expect(screen.getByText("Contents read and write")).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /publish/i }),
    ).not.toBeInTheDocument();
  });
  it("opens GitHub settings without fetching or claiming the repository selection changed", async () => {
    openRepositoryAccess.mockResolvedValue(undefined);
    render(<RemoteRepositories busy={false} onImport={vi.fn()} />);
    fireEvent.click(
      screen.getByRole("button", { name: "Manage repository access" }),
    );
    await waitFor(() => expect(openRepositoryAccess).toHaveBeenCalledTimes(1));
    expect(repositories).not.toHaveBeenCalled();
    expect(
      screen.getByText(/choose Configure beside your Inspector app/),
    ).toBeInTheDocument();
    expect(screen.queryByText(/visible repositories/)).not.toBeInTheDocument();
  });

  it("shows browser-opening failures with manual recovery and permits a retry", async () => {
    openRepositoryAccess
      .mockRejectedValueOnce(new Error("Browser unavailable"))
      .mockResolvedValueOnce(undefined);
    render(<RemoteRepositories busy={false} onImport={vi.fn()} />);
    fireEvent.click(
      screen.getByRole("button", { name: "Manage repository access" }),
    );
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("Browser unavailable");
    expect(alert).toHaveTextContent("github.com/settings/installations");
    fireEvent.click(
      screen.getByRole("button", { name: "Manage repository access" }),
    );
    await waitFor(() =>
      expect(screen.queryByRole("alert")).not.toBeInTheDocument(),
    );
    expect(openRepositoryAccess).toHaveBeenCalledTimes(2);
  });

  it("retains and labels the previous access list after a failed refresh, then recovers", async () => {
    repositories
      .mockResolvedValueOnce(access)
      .mockRejectedValueOnce(new Error("GitHub is unavailable"))
      .mockResolvedValueOnce({
        ...access,
        repositories: [
          ...access.repositories,
          { ...access.repositories[0], id: 3, full_name: "fixture/added" },
        ],
      });
    render(<RemoteRepositories busy={false} onImport={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "Refresh access" }));
    await screen.findByText("fixture/selected");
    fireEvent.click(screen.getByRole("button", { name: "Refresh access" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "last successful check",
    );
    expect(screen.getByText("fixture/selected")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Refresh access" }));
    await screen.findByText("fixture/added");
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(
      screen.getByText("1 installation(s) · 2 visible repositories"),
    ).toBeInTheDocument();
  });
});

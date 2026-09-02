import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import userEvent from "@testing-library/user-event";
import App from "./App";
import { api } from "./api";

vi.mock("./api", () => ({
  api: {
    profiles: vi.fn().mockResolvedValue([
      {
        name: "personal",
        profile: {
          github_user: "octocat",
          git_name: "Octo Cat",
          git_email: "octo@example.com",
          hostname: "github.com",
          allowed_owners: ["octocat"],
          signing_format: "openpgp",
          require_signing: false,
        },
      },
    ]),
    roots: vi.fn().mockResolvedValue([]),
    doctor: vi.fn(),
    updateProfile: vi.fn(),
    chooseKeyFile: vi.fn(),
  },
}));

describe("GitPersona app shell", () => {
  beforeEach(() => {
    window.history.replaceState({}, "", "/");
    vi.mocked(api.profiles).mockResolvedValue([
      {
        name: "personal",
        profile: {
          github_user: "octocat",
          git_name: "Octo Cat",
          git_email: "octo@example.com",
          hostname: "github.com",
          allowed_owners: ["octocat"],
          signing_format: "openpgp",
          require_signing: false,
        },
      },
    ]);
    vi.mocked(api.roots).mockResolvedValue([]);
  });
  it("shows all five primary destinations", async () => {
    render(<App />);
    expect(
      await screen.findByRole("heading", { name: "Repositories" }),
    ).toBeInTheDocument();
    for (const label of [
      "Profiles",
      "Repositories",
      "SSH & Signing",
      "Status",
      "Diagnostics",
    ])
      expect(screen.getByRole("button", { name: label })).toBeInTheDocument();
  });

  it("requires confirmation before force-rebinding drift", async () => {
    window.history.replaceState({}, "", "/?demo=1");
    render(<App />);
    await userEvent.click(
      screen.getByRole("button", { name: "Preview and rebind" }),
    );
    expect(
      screen.getByRole("button", { name: "Confirm force rebind" }),
    ).toBeInTheDocument();
    expect(
      screen.getByText("Confirm repository-local changes"),
    ).toBeInTheDocument();
  });

  it("shows offline diagnostics without hiding remediation", async () => {
    window.history.replaceState({}, "", "/?demo=1");
    render(<App />);
    await userEvent.click(screen.getByRole("button", { name: "Diagnostics" }));
    expect(
      screen.getByText("OpenSSH client was not found"),
    ).toBeInTheDocument();
    expect(screen.getByText(/Install the OpenSSH Client/)).toBeInTheDocument();
  });

  it("fails closed when configuration cannot be read", async () => {
    vi.mocked(api.profiles).mockRejectedValueOnce({
      message: "unsupported config schema version 99",
    });
    render(<App />);
    expect(
      await screen.findByRole("heading", {
        name: "Configuration could not be read",
      }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("heading", {
        name: "Start with a repository you know",
      }),
    ).not.toBeInTheDocument();
  });

  it("edits SSH authentication and signing settings", async () => {
    vi.mocked(api.updateProfile).mockResolvedValue({
      name: "personal",
      profile: {
        github_user: "octocat",
        git_name: "Octo Cat",
        git_email: "octo@example.com",
        hostname: "github.com",
        allowed_owners: ["octocat"],
        ssh_key: "~/.ssh/id_ed25519",
        signing_key: "~/.ssh/id_ed25519.pub",
        signing_format: "ssh",
        require_signing: true,
      },
    });
    render(<App />);
    await userEvent.click(
      await screen.findByRole("button", { name: "Profiles" }),
    );
    await userEvent.click(screen.getByRole("button", { name: "Edit" }));
    await userEvent.type(
      screen.getByLabelText("SSH key path"),
      "~/.ssh/id_ed25519",
    );
    await userEvent.selectOptions(
      screen.getByLabelText("Signing format"),
      "ssh",
    );
    await userEvent.type(
      screen.getByLabelText("Signing key"),
      "~/.ssh/id_ed25519.pub",
    );
    await userEvent.click(
      screen.getByLabelText("Require signed commits for bound repositories"),
    );
    await userEvent.click(screen.getByRole("button", { name: "Save profile" }));

    expect(api.updateProfile).toHaveBeenCalledWith(
      "personal",
      expect.objectContaining({
        ssh_key: "~/.ssh/id_ed25519",
        signing_key: "~/.ssh/id_ed25519.pub",
        signing_format: "ssh",
        require_signing: true,
      }),
    );
  });

  it("explains why SSH testing is unavailable without a key", async () => {
    render(<App />);
    await userEvent.click(
      await screen.findByRole("button", { name: "SSH & Signing" }),
    );
    expect(
      screen.getByRole("button", { name: "Test authentication" }),
    ).toBeDisabled();
    expect(
      screen.getByText("No SSH identity file is configured"),
    ).toBeInTheDocument();
  });

  it("renders repository status checks without crashing", async () => {
    window.history.replaceState({}, "", "/?demo=1");
    render(<App />);
    await userEvent.click(screen.getByRole("button", { name: "Status" }));
    expect(await screen.findByText("git author")).toBeInTheDocument();
    expect(
      screen.getByText("Git author matches the bound profile"),
    ).toBeInTheDocument();
  });

  it("shows guidance when status has no repository", async () => {
    render(<App />);
    await userEvent.click(
      await screen.findByRole("button", { name: "Status" }),
    );
    expect(screen.getByText("No repository selected")).toBeInTheDocument();
  });

  it("selects an SSH private key with the native picker", async () => {
    vi.mocked(api.chooseKeyFile).mockResolvedValue(
      "C:\\Users\\octo\\.ssh\\id_ed25519",
    );
    render(<App />);
    await userEvent.click(
      await screen.findByRole("button", { name: "Profiles" }),
    );
    await userEvent.click(screen.getByRole("button", { name: "Edit" }));
    await userEvent.click(screen.getByRole("button", { name: "Browse" }));
    expect(screen.getByLabelText("SSH key path")).toHaveValue(
      "C:\\Users\\octo\\.ssh\\id_ed25519",
    );
  });
});

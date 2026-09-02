import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import userEvent from "@testing-library/user-event";
import App from "./App";
import { api } from "./api";

vi.mock("./api", () => ({
  api: {
    profiles: vi
      .fn()
      .mockResolvedValue([
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
});

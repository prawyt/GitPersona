// Fixtures for the `?demo` walkthrough, split out of App.tsx to keep that file
// about the application. They are still bundled - the values flow into state
// initialisers and child props, so Rollup cannot prove the branches dead - but
// demo mode itself is unreachable in a production build; see the DEV gate in
// App.tsx. Dropping the bytes too would mean loading this module dynamically.
import type { NamedProfile, Profile, RepositorySummary } from "./types";

const emptyProfile: Profile = {
  github_user: "",
  git_name: "",
  git_email: "",
  hostname: "github.com",
  allowed_owners: [],
  signing_format: "openpgp",
  require_signing: false,
};

export const demoProfiles: NamedProfile[] = [
  {
    name: "personal",
    profile: {
      ...emptyProfile,
      github_user: "mira-dev",
      git_name: "Mira Chen",
      git_email: "mira@users.noreply.github.com",
      allowed_owners: ["mira-dev"],
      ssh_key: "~/.ssh/id_ed25519_personal",
      signing_key: "~/.ssh/id_ed25519_personal.pub",
      signing_format: "ssh",
      require_signing: true,
    },
  },
  {
    name: "opensource",
    profile: {
      ...emptyProfile,
      github_user: "mchen-oss",
      git_name: "Mira Chen",
      git_email: "oss@mira.dev",
      allowed_owners: ["rust-lang", "tauri-apps"],
    },
  },
];
export const demoRepos: RepositorySummary[] = [
  {
    name: "gitpersona",
    path: "C:\\dev\\gitpersona",
    bound_profile: "personal",
    git_name: "Mira Chen",
    git_email: "mira@users.noreply.github.com",
    status: "bound",
    remote: {
      url: "git@github.com:mira-dev/gitpersona.git",
      protocol: "ssh",
      hostname: "github.com",
      owner: "mira-dev",
      repository: "gitpersona",
    },
  },
  {
    name: "tauri-plugin-audit",
    path: "C:\\dev\\tauri-plugin-audit",
    bound_profile: "opensource",
    git_name: "Mira Chen",
    git_email: "oss@mira.dev",
    status: "drifted",
    detail: "Git email differs from the profile",
    remote: {
      url: "https://github.com/tauri-apps/tauri-plugin-audit.git",
      protocol: "https",
      hostname: "github.com",
      owner: "tauri-apps",
      repository: "tauri-plugin-audit",
    },
  },
  {
    name: "scratchpad",
    path: "C:\\dev\\scratchpad",
    git_name: "Mira Chen",
    git_email: "mira@users.noreply.github.com",
    status: "unbound",
    remote: {
      url: "git@github.com:mira-dev/scratchpad.git",
      protocol: "ssh",
      hostname: "github.com",
      owner: "mira-dev",
      repository: "scratchpad",
    },
  },
];

import { Channel, invoke } from "@tauri-apps/api/core";
import type {
  Account,
  DoctorReport,
  NamedProfile,
  Profile,
  ProfileDraft,
  RepositoryStatus,
  RepositorySummary,
  SshTestReport,
} from "./types";

export const api = {
  profiles: () => invoke<NamedProfile[]>("list_profiles"),
  createProfile: (name: string, profile: Profile) =>
    invoke<NamedProfile>("create_profile", { name, profile }),
  updateProfile: (name: string, profile: Profile) =>
    invoke<NamedProfile>("update_profile", { name, profile }),
  removeProfile: (name: string) => invoke<void>("remove_profile", { name }),
  chooseFolder: () => invoke<string | null>("choose_folder"),
  chooseKeyFile: () => invoke<string | null>("choose_key_file"),
  importPreview: (repository: string) =>
    invoke<ProfileDraft>("import_profile_preview", { repository }),
  roots: () => invoke<string[]>("list_repository_roots"),
  addRoot: (path: string) => invoke<string>("add_repository_root", { path }),
  removeRoot: (path: string) =>
    invoke<void>("remove_repository_root", { path }),
  scan: (onEvent: (event: unknown) => void) => {
    const events = new Channel<unknown>();
    events.onmessage = onEvent;
    return invoke<RepositorySummary[]>("scan_repositories", { events });
  },
  cancelScan: () => invoke<void>("cancel_repository_scan"),
  inspect: (repository: string, network = false) =>
    invoke<RepositoryStatus>("inspect_repository", { repository, network }),
  bind: (repository: string, profile: string, force = false) =>
    invoke<void>("bind_repository", { repository, profile, force }),
  unbind: (repository: string) =>
    invoke<void>("unbind_repository", { repository }),
  switchAccount: (profile: string) =>
    invoke<void>("switch_github_account", { profile }),
  accounts: (hostname: string) =>
    invoke<Account[]>("github_accounts", { hostname }),
  testSsh: (profile: string) => invoke<SshTestReport>("test_ssh", { profile }),
  doctor: () => invoke<DoctorReport>("doctor"),
};

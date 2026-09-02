export type SigningFormat = "openpgp" | "ssh";
export interface Profile { github_user: string; git_name: string; git_email: string; hostname: string; ssh_key?: string; allowed_owners: string[]; signing_key?: string; signing_format: SigningFormat; require_signing: boolean }
export interface NamedProfile { name: string; profile: Profile }
export interface RemoteInfo { protocol: "ssh" | "https"; hostname: string; owner: string; repository: string }
export interface RepositorySummary { path: string; name: string; bound_profile?: string; git_name?: string; git_email?: string; remote?: RemoteInfo; status: "bound" | "unbound" | "drifted" | "missing_profile" | "unavailable"; detail?: string }
export interface ProfileDraft { repository: string; github_user?: string; git_name?: string; git_email?: string; hostname: string; allowed_owners: string[]; signing_key?: string; signing_format: SigningFormat; require_signing: boolean; warnings: string[] }
export interface CheckItem { label: string; expected?: string; actual?: string; status: "pass" | "warning" | "fail" | "unavailable"; detail?: string }
export interface RepositoryStatus { report: { repository: string; profile?: string; overall: string; items: CheckItem[] }; network_checked: boolean }
export interface DependencyStatus { name: string; state: "ok" | "warning" | "unavailable"; detail: string; remediation?: string }
export interface DoctorReport { config_path: string; schema_version: number; profile_count: number; dependencies: DependencyStatus[]; profile_issues: string[]; healthy: boolean }
export interface SshTestReport { profile: string; expected_user: string; actual_user?: string; hostname: string; key?: string; status: "verified" | "rejected" | "unavailable"; message: string }
export interface ApiError { kind: string; message: string; exit_code: number; field?: string }
export interface Account { login: string; active: boolean; valid: boolean }

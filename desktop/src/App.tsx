import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  Activity,
  ArrowRightLeft,
  Check,
  ChevronRight,
  CircleAlert,
  CircleX,
  Copy,
  Folder,
  FolderPlus,
  GitBranch,
  KeyRound,
  LoaderCircle,
  MonitorCog,
  Plus,
  RefreshCw,
  Search,
  ShieldCheck,
  Square,
  Terminal,
  Trash2,
  UserRound,
  UsersRound,
  X,
} from "lucide-react";
import { api } from "./api";
import { demoProfiles, demoRepos } from "./demo";
import type {
  ApiError,
  DoctorReport,
  NamedProfile,
  Profile,
  RepositoryStatus,
  RepositorySummary,
  SshTestReport,
} from "./types";

type View = "profiles" | "repositories" | "ssh" | "status" | "diagnostics";
const views: { id: View; label: string; icon: typeof UserRound }[] = [
  { id: "profiles", label: "Profiles", icon: UsersRound },
  { id: "repositories", label: "Repositories", icon: GitBranch },
  { id: "ssh", label: "SSH & Signing", icon: KeyRound },
  { id: "status", label: "Status", icon: Activity },
  { id: "diagnostics", label: "Diagnostics", icon: MonitorCog },
];

const emptyProfile: Profile = {
  github_user: "",
  git_name: "",
  git_email: "",
  hostname: "github.com",
  allowed_owners: [],
  signing_format: "openpgp",
  require_signing: false,
};

function errorMessage(error: unknown) {
  return (
    (error as ApiError)?.message ||
    (error instanceof Error ? error.message : String(error))
  );
}

export default function App() {
  // Gated on DEV so demo mode cannot be reached in a shipped build, where a
  // stray `?demo` would otherwise replace real state with fixtures.
  const demo =
    import.meta.env.DEV && new URLSearchParams(location.search).has("demo");
  const [view, setView] = useState<View>("repositories");
  const [profiles, setProfiles] = useState<NamedProfile[]>(
    demo ? demoProfiles : [],
  );
  const [repositories, setRepositories] = useState<RepositorySummary[]>(
    demo ? demoRepos : [],
  );
  const [roots, setRoots] = useState<string[]>(demo ? ["C:\\dev"] : []);
  const [selectedRepo, setSelectedRepo] = useState<string>(
    demo ? demoRepos[1].path : "",
  );
  const [busy, setBusy] = useState(!demo);
  const [loadFailed, setLoadFailed] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");

  const load = useCallback(async () => {
    if (demo) return;
    setBusy(true);
    setError("");
    setLoadFailed(false);
    try {
      const [nextProfiles, nextRoots] = await Promise.all([
        api.profiles(),
        api.roots(),
      ]);
      setProfiles(nextProfiles);
      setRoots(nextRoots);
    } catch (reason) {
      setError(errorMessage(reason));
      setLoadFailed(true);
    } finally {
      setBusy(false);
    }
  }, [demo]);
  useEffect(() => {
    void load();
  }, [load]);
  const selected = repositories.find((repo) => repo.path === selectedRepo);
  // The dismissal timer is owned by an effect so that a notice raised just
  // before unmount does not fire setNotice on a torn-down component, and so a
  // second notice restarts the countdown rather than inheriting the first one's.
  const signal = useCallback((message: string) => setNotice(message), []);
  useEffect(() => {
    if (!notice) return;
    const timer = window.setTimeout(() => setNotice(""), 3500);
    return () => window.clearTimeout(timer);
  }, [notice]);

  return (
    <div className="app-shell">
      <header className="titlebar">
        <div className="wordmark">
          <span className="mark">
            <GitBranch size={17} />
          </span>
          <span>GitPersona</span>
          <span className="version">v{__APP_VERSION__}</span>
        </div>
        <div className="local-only">
          <ShieldCheck size={15} /> Local configuration only
        </div>
      </header>
      <aside className="sidebar" aria-label="Primary navigation">
        <nav>
          {views.map((item) => (
            <button
              aria-label={item.label}
              key={item.id}
              className={view === item.id ? "nav-item active" : "nav-item"}
              onClick={() => setView(item.id)}
            >
              <item.icon size={17} />
              <span>{item.label}</span>
              {item.id === "repositories" &&
                repositories.some((repo) => repo.status === "drifted") && (
                  <i className="nav-dot" aria-hidden="true" />
                )}
            </button>
          ))}
        </nav>
        <div className="sidebar-foot">
          <span className="status-dot ok" />
          Configuration protected
          <span className="muted">Schema 2 · no secrets stored</span>
        </div>
      </aside>
      <main className="workspace">
        {busy ? (
          <Loading />
        ) : loadFailed ? (
          view === "diagnostics" ? (
            <Diagnostics setError={setError} demo={demo} />
          ) : (
            <LoadFailure
              onRetry={load}
              onDiagnostics={() => setView("diagnostics")}
            />
          )
        ) : profiles.length === 0 ? (
          <Onboarding onDone={load} setError={setError} />
        ) : (
          <>
            {view === "profiles" && (
              <Profiles
                profiles={profiles}
                repositories={repositories}
                onChanged={load}
                signal={signal}
                setError={setError}
              />
            )}
            {view === "repositories" && (
              <Repositories
                profiles={profiles}
                repositories={repositories}
                setRepositories={setRepositories}
                roots={roots}
                setRoots={setRoots}
                selected={selected}
                setSelected={setSelectedRepo}
                signal={signal}
                setError={setError}
                demo={demo}
              />
            )}
            {view === "ssh" && (
              <Ssh
                profiles={profiles}
                signal={signal}
                setError={setError}
                demo={demo}
              />
            )}
            {view === "status" && (
              <Status
                repositories={repositories}
                selected={selected}
                setSelected={setSelectedRepo}
                setError={setError}
                demo={demo}
              />
            )}
            {view === "diagnostics" && (
              <Diagnostics setError={setError} demo={demo} />
            )}
          </>
        )}
      </main>
      {error && (
        <div className="toast error" role="alert">
          <CircleX size={17} />
          <span>{error}</span>
          <button aria-label="Dismiss error" onClick={() => setError("")}>
            <X size={16} />
          </button>
        </div>
      )}
      {notice && (
        <div className="toast success" role="status">
          <Check size={17} />
          <span>{notice}</span>
        </div>
      )}
    </div>
  );
}

function PageHeader({
  title,
  description,
  actions,
}: {
  title: string;
  description: string;
  actions?: React.ReactNode;
}) {
  return (
    <header className="page-header">
      <div>
        <h1>{title}</h1>
        <p>{description}</p>
      </div>
      {actions && <div className="page-actions">{actions}</div>}
    </header>
  );
}
function Loading() {
  return (
    <div className="loading">
      <LoaderCircle className="spin" />
      <span>Reading local configuration…</span>
    </div>
  );
}
function LoadFailure({
  onRetry,
  onDiagnostics,
}: {
  onRetry: () => Promise<void>;
  onDiagnostics: () => void;
}) {
  return (
    <section className="load-failure">
      <CircleX />
      <h1>Configuration could not be read</h1>
      <p>
        GitPersona has stopped before offering any write actions. Fix the
        reported error, then retry.
      </p>
      <div className="button-row">
        <button className="primary" onClick={() => void onRetry()}>
          <RefreshCw size={16} />
          Retry
        </button>
        <button className="secondary" onClick={onDiagnostics}>
          <MonitorCog size={16} />
          Open diagnostics
        </button>
      </div>
    </section>
  );
}
function StatusIcon({ status }: { status: string }) {
  return status === "bound" ||
    status === "pass" ||
    status === "ok" ||
    status === "verified" ? (
    <Check size={15} />
  ) : status === "unbound" || status === "warning" ? (
    <CircleAlert size={15} />
  ) : (
    <CircleX size={15} />
  );
}
function Badge({
  status,
  children,
}: {
  status: string;
  children?: React.ReactNode;
}) {
  return (
    <span className={`badge ${status}`}>
      <StatusIcon status={status} />
      {children ?? status.replace("_", " ")}
    </span>
  );
}

function Onboarding({
  onDone,
  setError,
}: {
  onDone: () => Promise<void>;
  setError: (value: string) => void;
}) {
  const [path, setPath] = useState("");
  const [draft, setDraft] = useState<Profile>(emptyProfile);
  const [name, setName] = useState("personal");
  const [step, setStep] = useState(1);
  const [working, setWorking] = useState(false);
  const choose = async () => {
    try {
      const selected = await api.chooseFolder();
      if (!selected) return;
      setPath(selected);
      const preview = await api.importPreview(selected);
      setDraft({
        github_user: preview.github_user ?? "",
        git_name: preview.git_name ?? "",
        git_email: preview.git_email ?? "",
        hostname: preview.hostname,
        allowed_owners: preview.allowed_owners,
        signing_key: preview.signing_key,
        signing_format: preview.signing_format,
        require_signing: preview.require_signing,
      });
      setStep(2);
    } catch (e) {
      setError(errorMessage(e));
    }
  };
  const finish = async () => {
    setWorking(true);
    try {
      await api.createProfile(name, draft);
      await api.bind(path, name);
      setStep(3);
      await onDone();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setWorking(false);
    }
  };
  return (
    <section className="onboarding">
      <div className="onboarding-copy">
        <div className="onboarding-icon">
          <ShieldCheck />
        </div>
        <h1>Start with a repository you know</h1>
        <p>
          GitPersona will preview its effective author, remote owner, GitHub
          account, and signing identity before writing anything.
        </p>
        <ol>
          <li className={step >= 1 ? "current" : ""}>Select repository</li>
          <li className={step >= 2 ? "current" : ""}>Confirm identity</li>
          <li className={step >= 3 ? "current" : ""}>Bind locally</li>
        </ol>
      </div>
      <div className="setup-panel">
        {step === 1 ? (
          <>
            <h2>Select an existing repository</h2>
            <p>
              Only this folder is inspected. Nothing else on disk is scanned.
            </p>
            <button className="drop-target" onClick={choose}>
              <FolderPlus />
              <strong>Choose repository folder</strong>
              <span>Git worktrees and gitfiles are supported</span>
            </button>
          </>
        ) : (
          <>
            <div className="selected-path">
              <Folder size={16} />
              {path}
            </div>
            <h2>Confirm the profile</h2>
            <ProfileFields
              name={name}
              setName={setName}
              profile={draft}
              setProfile={setDraft}
            />
            <div className="safe-note">
              <ShieldCheck size={16} />
              Binding changes repository-local Git settings only. GitHub CLI
              switching remains a separate action.
            </div>
            <button
              className="primary"
              disabled={
                working ||
                !name ||
                !draft.github_user ||
                !draft.git_name ||
                !draft.git_email
              }
              onClick={finish}
            >
              {working ? (
                <LoaderCircle className="spin" size={16} />
              ) : (
                <GitBranch size={16} />
              )}
              Create profile and bind
            </button>
          </>
        )}
      </div>
    </section>
  );
}

function ProfileFields({
  name,
  setName,
  profile,
  setProfile,
  nameLocked = false,
}: {
  name: string;
  setName: (v: string) => void;
  profile: Profile;
  setProfile: (v: Profile) => void;
  nameLocked?: boolean;
}) {
  const [pickerError, setPickerError] = useState("");
  const field = (key: keyof Profile, value: string | boolean) =>
    setProfile({ ...profile, [key]: value });
  const optionalField = (key: "ssh_key" | "signing_key", value: string) =>
    setProfile({ ...profile, [key]: value.trim() || undefined });
  return (
    <div className="form-grid">
      <label>
        Profile name
        <input
          disabled={nameLocked}
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="personal"
        />
      </label>
      <label>
        GitHub user
        <input
          value={profile.github_user}
          onChange={(e) => field("github_user", e.target.value)}
          placeholder="octocat"
        />
      </label>
      <label>
        Git author
        <input
          value={profile.git_name}
          onChange={(e) => field("git_name", e.target.value)}
          placeholder="Ada Lovelace"
        />
      </label>
      <label>
        Email
        <input
          type="email"
          value={profile.git_email}
          onChange={(e) => field("git_email", e.target.value)}
          placeholder="ada@example.com"
        />
      </label>
      <label>
        Hostname
        <input
          value={profile.hostname}
          onChange={(e) => field("hostname", e.target.value)}
        />
      </label>
      <label>
        Allowed owners
        <input
          value={profile.allowed_owners.join(", ")}
          onChange={(e) =>
            setProfile({
              ...profile,
              allowed_owners: e.target.value
                .split(",")
                .map((v) => v.trim())
                .filter(Boolean),
            })
          }
          placeholder="organization, username"
        />
      </label>
      <div className="form-field span-2">
        <label htmlFor="ssh-key-path">SSH key path</label>
        <span className="input-action">
          <input
            id="ssh-key-path"
            value={profile.ssh_key ?? ""}
            onChange={(e) => optionalField("ssh_key", e.target.value)}
            placeholder="Private key path, for example ~/.ssh/id_ed25519"
          />
          <button
            type="button"
            className="secondary"
            onClick={async () => {
              setPickerError("");
              try {
                const selected = await api.chooseKeyFile();
                if (selected) optionalField("ssh_key", selected);
              } catch (error) {
                setPickerError(errorMessage(error));
              }
            }}
          >
            <Folder size={15} />
            Browse
          </button>
        </span>
        <small className="field-hint">
          Choose the private key, not the matching <code>.pub</code> file.
        </small>
        {pickerError && (
          <small className="field-error" role="alert">
            {pickerError}
          </small>
        )}
      </div>
      <label>
        Signing format
        <select
          value={profile.signing_format}
          onChange={(e) => field("signing_format", e.target.value)}
        >
          <option value="openpgp">OpenPGP</option>
          <option value="ssh">SSH</option>
        </select>
      </label>
      <label>
        Signing key
        <input
          value={profile.signing_key ?? ""}
          onChange={(e) => optionalField("signing_key", e.target.value)}
          placeholder={
            profile.signing_format === "ssh"
              ? "Public key path or key:: value"
              : "OpenPGP key ID"
          }
        />
      </label>
      <label className="check-row span-2">
        <input
          type="checkbox"
          checked={profile.require_signing}
          onChange={(e) => field("require_signing", e.target.checked)}
        />
        <span>Require signed commits for bound repositories</span>
      </label>
    </div>
  );
}

function Profiles({
  profiles,
  repositories,
  onChanged,
  signal,
  setError,
}: {
  profiles: NamedProfile[];
  repositories: RepositorySummary[];
  onChanged: () => Promise<void>;
  signal: (v: string) => void;
  setError: (v: string) => void;
}) {
  const [selected, setSelected] = useState(profiles[0]?.name ?? "");
  const current = profiles.find((p) => p.name === selected) ?? profiles[0];
  const [mode, setMode] = useState<"view" | "edit" | "new">("view");
  const [draft, setDraft] = useState(current?.profile ?? emptyProfile);
  const [name, setName] = useState(current?.name ?? "");
  const [confirmRemove, setConfirmRemove] = useState(false);
  const [switchPreview, setSwitchPreview] = useState<{
    current: string;
    target: string;
  }>();
  useEffect(() => {
    if (current && mode !== "new") {
      setDraft(current.profile);
      setName(current.name);
      setConfirmRemove(false);
      setSwitchPreview(undefined);
    }
  }, [current, mode]);
  const save = async () => {
    try {
      const trimmedName = name.trim();
      if (!trimmedName) {
        setError("Profile name cannot be empty");
        return;
      }
      if (mode === "new") {
        await api.createProfile(trimmedName, draft);
        signal(`Profile '${trimmedName}' saved`);
        setSelected(trimmedName);
      } else {
        if (trimmedName !== current.name) {
          await api.renameProfile(current.name, trimmedName);
          await api.updateProfile(trimmedName, draft);
          signal(`Profile renamed to '${trimmedName}' and updated`);
          setSelected(trimmedName);
        } else {
          await api.updateProfile(current.name, draft);
          signal(`Profile '${trimmedName}' saved`);
        }
      }
      setMode("view");
      await onChanged();
    } catch (e) {
      setError(errorMessage(e));
    }
  };
  const duplicate = () => {
    let candidate = `${current.name}-copy`;
    let counter = 2;
    while (profiles.some((p) => p.name === candidate)) {
      candidate = `${current.name}-copy-${counter}`;
      counter += 1;
    }
    setName(candidate);
    setDraft({ ...current.profile });
    setMode("new");
  };
  const remove = async () => {
    try {
      await api.removeProfile(current.name);
      signal(`Profile '${current.name}' removed`);
      setConfirmRemove(false);
      await onChanged();
      setSelected(profiles.find((p) => p.name !== current.name)?.name ?? "");
    } catch (e) {
      setError(errorMessage(e));
    }
  };
  const previewSwitch = async () => {
    try {
      const accounts = await api.accounts(current.profile.hostname);
      setSwitchPreview({
        current: accounts.find((a) => a.active)?.login ?? "No active account",
        target: current.profile.github_user,
      });
    } catch (e) {
      setError(errorMessage(e));
    }
  };
  const switchAccount = async () => {
    try {
      await api.switchAccount(current.name);
      signal(`GitHub CLI switched to ${current.profile.github_user}`);
      setSwitchPreview(undefined);
    } catch (e) {
      setError(errorMessage(e));
    }
  };
  return (
    <section>
      <PageHeader
        title="Profiles"
        description="Identity records applied to repositories you explicitly bind."
        actions={
          <button
            className="secondary"
            onClick={() => {
              setMode("new");
              setName("");
              setDraft(emptyProfile);
            }}
          >
            <Plus size={16} />
            New profile
          </button>
        }
      />
      <div className="split-view">
        <div className="list-pane">
          <div className="pane-label">{profiles.length} configured</div>
          {profiles.map((item) => (
            <button
              className={`list-row ${item.name === current.name ? "selected" : ""}`}
              key={item.name}
              onClick={() => {
                setSelected(item.name);
                setMode("view");
              }}
            >
              <span className="avatar">
                {item.profile.github_user.slice(0, 2).toUpperCase()}
              </span>
              <span>
                <strong>{item.name}</strong>
                <small>
                  {item.profile.github_user} · {item.profile.hostname}
                </small>
              </span>
              <ChevronRight size={15} />
            </button>
          ))}
        </div>
        <div className="detail-pane">
          <div className="detail-head">
            <div>
              <h2>{mode === "new" ? "New profile" : current.name}</h2>
              <p>
                {mode === "new"
                  ? "Create a reusable local identity"
                  : `${current.profile.git_name} · ${current.profile.git_email}`}
              </p>
            </div>
            <div className="row-actions">
              {mode === "view" && (
                <>
                  <button
                    className="icon-button"
                    title="Duplicate profile"
                    onClick={duplicate}
                  >
                    <Copy size={16} />
                  </button>
                  <button className="secondary" onClick={() => setMode("edit")}>
                    Edit
                  </button>
                </>
              )}
            </div>
          </div>
          {mode !== "view" ? (
            <>
              <ProfileFields
                name={name}
                setName={setName}
                profile={draft}
                setProfile={setDraft}
              />
              <div className="button-row">
                <button className="primary" onClick={save}>
                  Save profile
                </button>
                <button className="secondary" onClick={() => setMode("view")}>
                  Cancel
                </button>
              </div>
            </>
          ) : (
            <>
              <dl className="identity-grid">
                <div>
                  <dt>GitHub account</dt>
                  <dd>{current.profile.github_user}</dd>
                </div>
                <div>
                  <dt>Hostname</dt>
                  <dd>{current.profile.hostname}</dd>
                </div>
                <div>
                  <dt>SSH identity</dt>
                  <dd>{current.profile.ssh_key ?? "Not configured"}</dd>
                </div>
                <div>
                  <dt>Commit signing</dt>
                  <dd>
                    {current.profile.require_signing
                      ? `${current.profile.signing_format.toUpperCase()} required`
                      : "Not required"}
                  </dd>
                </div>
                <div className="wide">
                  <dt>Allowed remote owners</dt>
                  <dd>
                    {current.profile.allowed_owners.join(", ") || "Any owner"}
                  </dd>
                </div>
              </dl>
              <div className="explicit-action">
                <div>
                  <strong>GitHub CLI account</strong>
                  <p>
                    Switching is explicit and separate from repository binding.
                  </p>
                </div>
                {switchPreview ? (
                  <div className="switch-preview">
                    <span>
                      <small>Current</small>
                      {switchPreview.current}
                    </span>
                    <ArrowRightLeft size={16} />
                    <span>
                      <small>Target</small>
                      {switchPreview.target}
                    </span>
                    <button className="primary" onClick={switchAccount}>
                      Confirm switch
                    </button>
                    <button
                      className="secondary"
                      onClick={() => setSwitchPreview(undefined)}
                    >
                      Cancel
                    </button>
                  </div>
                ) : (
                  <button className="secondary" onClick={previewSwitch}>
                    <ArrowRightLeft size={16} />
                    Review switch
                  </button>
                )}
              </div>
              <h3>Affected discovered repositories</h3>
              <div className="compact-list">
                {repositories
                  .filter((r) => r.bound_profile === current.name)
                  .map((repo) => (
                    <div key={repo.path}>
                      <GitBranch size={15} />
                      <span>
                        <strong>{repo.name}</strong>
                        <small>{repo.path}</small>
                      </span>
                      <Badge status={repo.status} />
                    </div>
                  ))}
                {!repositories.some(
                  (r) => r.bound_profile === current.name,
                ) && (
                  <p className="empty-copy">
                    No discovered repository uses this profile.
                  </p>
                )}
              </div>
              <div className="destructive-zone">
                {confirmRemove ? (
                  <>
                    <span>
                      Remove this profile? Bound repositories will report it
                      missing.
                    </span>
                    <button className="danger-text" onClick={remove}>
                      Confirm remove
                    </button>
                    <button
                      className="secondary"
                      onClick={() => setConfirmRemove(false)}
                    >
                      Cancel
                    </button>
                  </>
                ) : (
                  <button
                    className="danger-text"
                    onClick={() => setConfirmRemove(true)}
                  >
                    <Trash2 size={15} />
                    Remove profile
                  </button>
                )}
              </div>
            </>
          )}
        </div>
      </div>
    </section>
  );
}

function Repositories({
  profiles,
  repositories,
  setRepositories,
  roots,
  setRoots,
  selected,
  setSelected,
  signal,
  setError,
  demo,
}: {
  profiles: NamedProfile[];
  repositories: RepositorySummary[];
  setRepositories: (v: RepositorySummary[]) => void;
  roots: string[];
  setRoots: (v: string[]) => void;
  selected?: RepositorySummary;
  setSelected: (v: string) => void;
  signal: (v: string) => void;
  setError: (v: string) => void;
  demo: boolean;
}) {
  const [query, setQuery] = useState("");
  const [scanning, setScanning] = useState(false);
  const [scanProgress, setScanProgress] = useState("");
  const [profile, setProfile] = useState(
    selected?.bound_profile ?? profiles[0]?.name ?? "",
  );
  const [confirmRebind, setConfirmRebind] = useState(false);
  const [confirmUnbind, setConfirmUnbind] = useState(false);
  const [removeRoot, setRemoveRoot] = useState("");
  useEffect(() => {
    setProfile(selected?.bound_profile ?? profiles[0]?.name ?? "");
    setConfirmRebind(false);
    setConfirmUnbind(false);
  }, [selected?.path, selected?.bound_profile, profiles]);
  const filtered = useMemo(
    () =>
      repositories.filter((repo) =>
        `${repo.name} ${repo.path} ${repo.bound_profile}`
          .toLowerCase()
          .includes(query.toLowerCase()),
      ),
    [repositories, query],
  );
  const addRoot = async () => {
    if (demo) {
      setRoots([...roots, "C:\\work"]);
      return;
    }
    try {
      const path = await api.chooseFolder();
      if (path) setRoots([...roots, await api.addRoot(path)]);
    } catch (e) {
      setError(errorMessage(e));
    }
  };
  const revokeRoot = async (path: string) => {
    if (removeRoot !== path) {
      setRemoveRoot(path);
      return;
    }
    try {
      if (!demo) await api.removeRoot(path);
      setRoots(roots.filter((root) => root !== path));
      setRemoveRoot("");
      signal("Approved root removed");
    } catch (e) {
      setError(errorMessage(e));
    }
  };
  const scan = async () => {
    setScanning(true);
    setScanProgress("Starting approved-root scan…");
    try {
      const found = demo
        ? demoRepos
        : await api.scan((value) => {
            const event = value as {
              type?: string;
              root?: string;
              repositories?: number;
            };
            if (event.type === "root_started")
              setScanProgress(`Scanning ${event.root}`);
            if (event.type === "repository_found")
              setScanProgress("Repository found; continuing scan…");
            if (event.type === "finished")
              setScanProgress(`${event.repositories ?? 0} repositories found`);
          });
      setRepositories(found);
      if (!selected && found[0]) setSelected(found[0].path);
      signal(`Scan complete · ${found.length} repositories found`);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setScanning(false);
    }
  };
  const cancelScan = async () => {
    setScanProgress("Cancelling scan…");
    if (!demo) await api.cancelScan();
  };
  const bind = async () => {
    if (!selected) return;
    if (!confirmRebind) {
      setConfirmRebind(true);
      setConfirmUnbind(false);
      return;
    }
    try {
      if (!demo)
        await api.bind(selected.path, profile, selected.status === "drifted");
      signal(`${selected.name} bound to ${profile}`);
      setConfirmRebind(false);
      await scan();
    } catch (e) {
      setError(errorMessage(e));
    }
  };
  const unbind = async () => {
    if (!selected) return;
    if (!confirmUnbind) {
      setConfirmUnbind(true);
      setConfirmRebind(false);
      return;
    }
    try {
      if (!demo) await api.unbind(selected.path);
      signal(`${selected.name} restored to its original settings`);
      setConfirmUnbind(false);
      await scan();
    } catch (e) {
      setError(errorMessage(e));
    }
  };
  return (
    <section>
      <PageHeader
        title="Repositories"
        description="Scan approved folders and manage repository-local identity."
        actions={
          <>
            <button className="secondary" onClick={addRoot}>
              <FolderPlus size={16} />
              Add root
            </button>
            <button className="primary" onClick={scanning ? cancelScan : scan}>
              {scanning ? <Square size={14} /> : <RefreshCw size={16} />}
              {scanning ? "Cancel scan" : "Scan roots"}
            </button>
          </>
        }
      />
      <div className="root-strip">
        <span>Approved roots</span>
        {roots.map((root) => (
          <span className="root-chip" key={root}>
            <code>{root}</code>
            <button
              aria-label={`Remove approved root ${root}`}
              onClick={() => void revokeRoot(root)}
            >
              {removeRoot === root ? "Confirm" : <X size={13} />}
            </button>
          </span>
        ))}
        {roots.length === 0 && <em>No folders approved</em>}
        {scanProgress && (
          <span className="scan-progress" role="status">
            {scanProgress}
          </span>
        )}
      </div>
      <div className="split-view repo-split">
        <div className="list-pane">
          <div className="search">
            <Search size={15} />
            <input
              aria-label="Filter repositories"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Filter repositories"
            />
          </div>
          {filtered.map((repo) => (
            <button
              className={`repo-row ${selected?.path === repo.path ? "selected" : ""}`}
              key={repo.path}
              onClick={() => {
                setSelected(repo.path);
                setProfile(repo.bound_profile ?? profiles[0].name);
                setConfirmRebind(false);
              }}
            >
              <StatusIcon status={repo.status} />
              <span>
                <strong>{repo.name}</strong>
                <small>{repo.path}</small>
              </span>
              <span className={`status-word ${repo.status}`}>
                {repo.status.replace("_", " ")}
              </span>
            </button>
          ))}
          {filtered.length === 0 && (
            <p className="empty-copy pad">No matching repositories.</p>
          )}
        </div>
        <div className="detail-pane">
          {selected ? (
            <>
              <div className="detail-head">
                <div>
                  <div className="repo-title">
                    <GitBranch size={20} />
                    <h2>{selected.name}</h2>
                  </div>
                  <p className="path">{selected.path}</p>
                </div>
                <Badge status={selected.status} />
              </div>
              {selected.detail && (
                <div className="inline-alert">
                  <CircleAlert size={17} />
                  <div>
                    <strong>Identity drift detected</strong>
                    <p>{selected.detail}</p>
                  </div>
                </div>
              )}
              <dl className="identity-grid">
                <div>
                  <dt>Current author</dt>
                  <dd>{selected.git_name ?? "Not configured"}</dd>
                </div>
                <div>
                  <dt>Current email</dt>
                  <dd>{selected.git_email ?? "Not configured"}</dd>
                </div>
                <div>
                  <dt>Remote owner</dt>
                  <dd>{selected.remote?.owner ?? "No origin remote"}</dd>
                </div>
                <div>
                  <dt>Protocol</dt>
                  <dd>{selected.remote?.protocol.toUpperCase() ?? "—"}</dd>
                </div>
              </dl>
              <div className="bind-control">
                <label>
                  Apply profile
                  <select
                    value={profile}
                    onChange={(e) => {
                      setProfile(e.target.value);
                      setConfirmRebind(false);
                    }}
                  >
                    {profiles.map((item) => (
                      <option key={item.name}>{item.name}</option>
                    ))}
                  </select>
                </label>
                {confirmRebind && (
                  <div className="rebind-preview">
                    <strong>Confirm repository-local changes</strong>
                    <span>
                      Current author and email will be replaced by values from{" "}
                      <b>{profile}</b>.
                      {selected.status === "drifted" &&
                        " Drifted managed values will be overwritten."}{" "}
                      Original pre-bind values remain available for unbind.
                    </span>
                  </div>
                )}
                <div className="button-row">
                  <button className="primary" onClick={bind}>
                    <GitBranch size={16} />
                    {confirmRebind
                      ? selected.status === "drifted"
                        ? "Confirm force rebind"
                        : "Confirm bind"
                      : selected.status === "drifted"
                        ? "Preview and rebind"
                        : "Preview and bind"}
                  </button>
                  {confirmRebind && (
                    <button
                      className="secondary"
                      onClick={() => setConfirmRebind(false)}
                    >
                      Cancel
                    </button>
                  )}
                  {selected.bound_profile &&
                    !confirmRebind &&
                    !confirmUnbind && (
                      <button className="danger-text" onClick={unbind}>
                        Unbind and restore
                      </button>
                    )}
                  {confirmUnbind && (
                    <>
                      <span className="confirm-copy">
                        Restore the exact settings saved before the first bind?
                      </span>
                      <button className="danger-text" onClick={unbind}>
                        Confirm unbind
                      </button>
                      <button
                        className="secondary"
                        onClick={() => setConfirmUnbind(false)}
                      >
                        Cancel
                      </button>
                    </>
                  )}
                </div>
                <p>
                  <ShieldCheck size={15} />
                  This does not switch your active GitHub CLI account.
                </p>
              </div>
            </>
          ) : (
            <div className="empty-inspector">
              <GitBranch />
              <h2>Select a repository</h2>
              <p>
                Review its effective identity and preview changes before
                binding.
              </p>
            </div>
          )}
        </div>
      </div>
    </section>
  );
}

function Ssh({
  profiles,
  signal,
  setError,
  demo,
}: {
  profiles: NamedProfile[];
  signal: (v: string) => void;
  setError: (v: string) => void;
  demo: boolean;
}) {
  const [selected, setSelected] = useState(profiles[0].name);
  const [report, setReport] = useState<SshTestReport>();
  const profile = profiles.find((p) => p.name === selected)!;
  const test = async () => {
    try {
      const result = demo
        ? ({
            profile: selected,
            expected_user: profile.profile.github_user,
            actual_user: profile.profile.github_user,
            hostname: profile.profile.hostname,
            key: profile.profile.ssh_key,
            status: "verified",
            message: `SSH authenticates as ${profile.profile.github_user}`,
          } as SshTestReport)
        : await api.testSsh(selected);
      setReport(result);
      signal("SSH authentication test complete");
    } catch (e) {
      setError(errorMessage(e));
    }
  };
  return (
    <section>
      <PageHeader
        title="SSH & Signing"
        description="Inspect configured keys and run authentication tests only when requested."
      />
      <div className="settings-layout">
        <div className="profile-selector">
          <label>
            Profile
            <select
              value={selected}
              onChange={(e) => {
                setSelected(e.target.value);
                setReport(undefined);
              }}
            >
              {profiles.map((p) => (
                <option key={p.name}>{p.name}</option>
              ))}
            </select>
          </label>
        </div>
        <div className="settings-section">
          <div className="section-head">
            <div>
              <h2>SSH authentication</h2>
              <p>
                GitPersona never edits <code>~/.ssh/config</code>.
              </p>
            </div>
            <button
              className="primary"
              onClick={test}
              disabled={!profile.profile.ssh_key}
              title={
                profile.profile.ssh_key
                  ? undefined
                  : "Configure a private SSH key path in Profiles first"
              }
            >
              <Terminal size={16} />
              Test authentication
            </button>
          </div>
          <dl className="identity-grid">
            <div>
              <dt>Host</dt>
              <dd>{profile.profile.hostname}</dd>
            </div>
            <div>
              <dt>Expected account</dt>
              <dd>{profile.profile.github_user}</dd>
            </div>
            <div className="wide">
              <dt>Identity file</dt>
              <dd>{profile.profile.ssh_key ?? "No key configured"}</dd>
            </div>
          </dl>
          {!profile.profile.ssh_key && (
            <div className="test-result unavailable" role="status">
              <CircleAlert size={17} />
              <div>
                <strong>No SSH identity file is configured</strong>
                <p>
                  Edit this profile under Profiles and add the private key path
                  used for authentication.
                </p>
              </div>
            </div>
          )}
          {report && (
            <div className={`test-result ${report.status}`}>
              <StatusIcon status={report.status} />
              <div>
                <strong>{report.message}</strong>
                <p>
                  The test was initiated manually and made one SSH connection.
                </p>
              </div>
            </div>
          )}
        </div>
        <div className="settings-section">
          <div className="section-head">
            <div>
              <h2>Commit signing</h2>
              <p>Expected settings applied when this profile is bound.</p>
            </div>
            <Badge
              status={profile.profile.require_signing ? "bound" : "warning"}
            >
              {profile.profile.require_signing ? "Required" : "Optional"}
            </Badge>
          </div>
          <dl className="identity-grid">
            <div>
              <dt>Format</dt>
              <dd>{profile.profile.signing_format.toUpperCase()}</dd>
            </div>
            <div>
              <dt>Signing key</dt>
              <dd>{profile.profile.signing_key ?? "Not configured"}</dd>
            </div>
          </dl>
        </div>
      </div>
    </section>
  );
}

function Status({
  repositories,
  selected,
  setSelected,
  setError,
  demo,
}: {
  repositories: RepositorySummary[];
  selected?: RepositorySummary;
  setSelected: (v: string) => void;
  setError: (v: string) => void;
  demo: boolean;
}) {
  const [report, setReport] = useState<RepositoryStatus>();
  const [refreshing, setRefreshing] = useState(false);
  const inspect = useCallback(
    async (network = false) => {
      if (!selected) return;
      setRefreshing(true);
      try {
        setReport(
          demo
            ? {
                network_checked: network,
                report: {
                  repository: selected.path,
                  profile: selected.bound_profile,
                  overall: "warning",
                  checks: [
                    {
                      id: "git_author",
                      expected: "Mira Chen",
                      actual: selected.git_name,
                      status: "ok",
                      message: "Git author matches the bound profile",
                    },
                    {
                      id: "git_email",
                      expected: "oss@mira.dev",
                      actual: selected.git_email,
                      status: selected.status === "drifted" ? "failure" : "ok",
                      message:
                        selected.detail ??
                        "Git email matches the bound profile",
                    },
                    {
                      id: "remote_owner",
                      expected: "tauri-apps",
                      actual: selected.remote?.owner,
                      status: "ok",
                      message: "Remote owner is allowed by the profile",
                    },
                    {
                      id: "github_cli",
                      expected: selected.bound_profile,
                      actual: network ? "mira-dev" : undefined,
                      status: network ? "warning" : "unverified",
                      message: network
                        ? "Active account differs; no switch was performed."
                        : "Run network refresh to check.",
                    },
                  ],
                },
              }
            : await api.inspect(selected.path, network),
        );
      } catch (e) {
        setError(errorMessage(e));
      } finally {
        setRefreshing(false);
      }
    },
    [selected, demo, setError],
  );
  const inspectRef = useRef(inspect);
  useEffect(() => {
    inspectRef.current = inspect;
  }, [inspect]);
  // Re-inspect when the selection moves to a different repository. `inspect`
  // is intentionally not a dependency: it is rebuilt whenever the `selected`
  // object identity changes, which happens on every parent refresh, and
  // depending on it would re-run the network-free check on each of those.
  const selectedPath = selected?.path;
  useEffect(() => {
    if (!selectedPath) return;
    void inspectRef.current(false);
  }, [selectedPath]);
  return (
    <section>
      <PageHeader
        title="Status"
        description="Expected versus actual identity for one repository."
        actions={
          <button
            className="secondary"
            onClick={() => inspect(true)}
            disabled={!selected || refreshing}
          >
            {refreshing ? (
              <LoaderCircle className="spin" size={16} />
            ) : (
              <RefreshCw size={16} />
            )}
            Refresh network checks
          </button>
        }
      />
      <div className="status-toolbar">
        <label>
          Repository
          <select
            value={selected?.path ?? ""}
            onChange={(e) => setSelected(e.target.value)}
          >
            {repositories.map((repo) => (
              <option value={repo.path} key={repo.path}>
                {repo.name} — {repo.path}
              </option>
            ))}
          </select>
        </label>
        {report && (
          <span className="network-note">
            {report.network_checked
              ? "Network checks current"
              : "Local checks only"}
          </span>
        )}
      </div>
      {!selected ? (
        <div className="empty-inspector status-empty">
          <GitBranch size={28} />
          <h2>No repository selected</h2>
          <p>
            Add an approved root and scan it under Repositories, then return
            here to inspect its identity.
          </p>
        </div>
      ) : refreshing && !report ? (
        <Loading />
      ) : report ? (
        <table className="check-table">
          <thead>
            <tr>
              <th>Check</th>
              <th>Expected</th>
              <th>Actual</th>
              <th>Result</th>
            </tr>
          </thead>
          <tbody>
            {report.report.checks.map((item) => (
              <tr key={item.id}>
                <th scope="row">
                  {item.id.replaceAll("_", " ")}
                  <small>{item.message}</small>
                </th>
                <td>{item.expected ?? "—"}</td>
                <td>{item.actual ?? "Not checked"}</td>
                <td>
                  <Badge status={item.status} />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      ) : null}
    </section>
  );
}

function Diagnostics({
  setError,
  demo,
}: {
  setError: (v: string) => void;
  demo: boolean;
}) {
  const [report, setReport] = useState<DoctorReport>();
  const load = useCallback(async () => {
    try {
      setReport(
        demo
          ? {
              config_path:
                "C:\\Users\\mira\\AppData\\Roaming\\gitpersona\\config.toml",
              schema_version: 2,
              profile_count: 2,
              healthy: false,
              profile_issues: [],
              dependencies: [
                { name: "git", state: "ok", detail: "git version 2.49.0" },
                { name: "gh", state: "ok", detail: "gh version 2.76.1" },
                {
                  name: "ssh",
                  state: "unavailable",
                  detail: "OpenSSH client was not found",
                  remediation:
                    "Install the OpenSSH Client optional feature and restart GitPersona.",
                },
              ],
            }
          : await api.doctor(),
      );
    } catch (e) {
      setError(errorMessage(e));
    }
  }, [demo, setError]);
  useEffect(() => {
    void load();
  }, [load]);
  return (
    <section>
      <PageHeader
        title="Diagnostics"
        description="Structured checks for GitPersona, Git, GitHub CLI, and OpenSSH."
        actions={
          <button className="secondary" onClick={load}>
            <RefreshCw size={16} />
            Run again
          </button>
        }
      />
      {report && (
        <>
          <div
            className={`health-banner ${report.healthy ? "healthy" : "attention"}`}
          >
            <StatusIcon status={report.healthy ? "ok" : "warning"} />
            <div>
              <strong>
                {report.healthy
                  ? "Everything required is available"
                  : `${report.dependencies.filter((item) => item.state !== "ok").length + report.profile_issues.length} checks need attention`}
              </strong>
              <p>
                Configuration schema {report.schema_version} ·{" "}
                {report.profile_count} profiles · {report.config_path}
              </p>
            </div>
          </div>
          {report.profile_issues.length > 0 && (
            <div className="profile-issues">
              <strong>Profile configuration</strong>
              {report.profile_issues.map((issue) => (
                <p key={issue}>
                  <CircleAlert size={15} />
                  {issue}
                </p>
              ))}
            </div>
          )}
          <div className="diagnostic-list">
            {report.dependencies.map((item) => (
              <div className="diagnostic-row" key={item.name}>
                <span className={`dependency-icon ${item.state}`}>
                  <StatusIcon status={item.state} />
                </span>
                <div>
                  <h2>{item.name}</h2>
                  <p>{item.detail}</p>
                  {item.remediation && (
                    <div className="remediation">
                      <code>{item.remediation}</code>
                      <button
                        title="Copy remediation"
                        onClick={() =>
                          navigator.clipboard.writeText(item.remediation!)
                        }
                      >
                        <Copy size={15} />
                      </button>
                    </div>
                  )}
                </div>
                <Badge status={item.state} />
              </div>
            ))}
          </div>
        </>
      )}
    </section>
  );
}

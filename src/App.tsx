import { useEffect, useRef, useState, type MouseEvent } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  ArrowRight,
  Check,
  ChevronLeft,
  ChevronRight,
  CircleHelp,
  FileSearch,
  FolderLock,
  HardDrive,
  History,
  LoaderCircle,
  MessageSquare,
  Moon,
  Plus,
  ScanLine,
  Settings2,
  ShieldCheck,
  Sparkles,
  Sun,
  Trash2,
  X,
} from "lucide-react";
import {
  call,
  defaults,
  native,
  size,
  type Advice,
  type Drive,
  type Item,
  type Preview,
  type Progress,
  type ScanView,
  type Settings,
  type RecoveryRecord,
  type CleanupResult,
  type LocalStatus,
} from "./api";

const navigation = [
  { name: "Cleaner", icon: Trash2 },
  { name: "AI assistant", icon: Sparkles },
  { name: "Protected items", icon: FolderLock },
  { name: "Recovery", icon: History },
  { name: "Settings", icon: Settings2 },
] as const;
type Page = (typeof navigation)[number]["name"];
const descriptions: Record<string, string> = {
  Cleaner: "Analyze your drive, select files, then review cleanup.",
  "AI assistant": "Get help deciding what to keep.",
  "Protected items": "Choose files and folders to keep out of cleanup.",
  Recovery: "Restore files from local quarantine.",
  Settings: "Appearance and optional AI connections.",
};
const emptyProgress: Progress = {
  scan_id: "",
  files: 0,
  bytes: 0,
  candidates: 0,
  candidate_bytes: 0,
  errors: 0,
  skipped: 0,
  status: "idle",
};

export default function App() {
  const [page, setPage] = useState<Page>("Cleaner");
  const [settings, setSettings] = useState<Settings>(defaults);
  const [drives, setDrives] = useState<Drive[]>([]);
  const [root, setRoot] = useState("");
  const [progress, setProgress] = useState<Progress>(emptyProgress);
  const [scan, setScan] = useState("");
  const [scanRevision, setScanRevision] = useState(0);
  const active = useRef("");
  const [view, setView] = useState<ScanView | null>(null);
  const [history, setHistory] = useState<Progress[]>([]);
  const [reviews, setReviews] = useState<Preview[]>([]);
  const [filter, setFilter] = useState("");
  const [offset, setOffset] = useState(0);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [preview, setPreview] = useState<Preview | null>(null);
  const [recovery, setRecovery] = useState<RecoveryRecord[]>([]);
  const [restore, setRestore] = useState<RecoveryRecord | null>(null);
  const [approved, setApproved] = useState(false);
  const [quarantining, setQuarantining] = useState(false);
  const [cleanupProgress, setCleanupProgress] = useState<CleanupResult | null>(
    null,
  );
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [working, setWorking] = useState(false);
  const [duplicateCount, setDuplicateCount] = useState(0);
  const [deduplicating, setDeduplicating] = useState(false);
  const [goal, setGoal] = useState("");
  const [advice, setAdvice] = useState<Advice | null>(null);
  const [rule, setRule] = useState("");
  const [ruleKind, setRuleKind] = useState<
    "protected" | "patterns" | "exclusions"
  >("protected");
  const [proposed, setProposed] = useState<Settings | null>(null);
  const [key, setKey] = useState("");
  const [localStatus, setLocalStatus] = useState<LocalStatus | null>(null);
  const [localProgress, setLocalProgress] = useState("");
  const [settingUp, setSettingUp] = useState(false);
  const [apps, setApps] = useState<{ name: string; bytes: number }[]>([]);

  const scanning = progress.status === "scanning";
  const loaded = useRef(false);
  const report = (e: unknown) => setError(String(e));
  function openSetupLink(event: MouseEvent<HTMLAnchorElement>) {
    if (!native) return;
    event.preventDefault();
    void call("open_setup_link", {
      url: event.currentTarget.getAttribute("href"),
    }).catch(report);
  }
  async function refreshHistory() {
    const [h, r, recovery] = await Promise.all([
      call<Progress[]>("scan_history"),
      call<Preview[]>("review_history"),
      call<RecoveryRecord[]>("recovery_history"),
    ]);
    setHistory(h);
    setReviews(r);
    setRecovery(recovery);
  }

  useEffect(() => {
    if (!native) return;
    let disposed = false;
    const unlisteners: (() => void)[] = [];
    const attach = async () => {
      for (const [event, handler] of [
        [
          "scan-progress",
          (p: Progress) => {
            if (p.scan_id === active.current) {
              setProgress(p);
              if (p.status !== "scanning") {
                setScanRevision((revision) => revision + 1);
                void refreshHistory().catch(report);
              }
            }
          },
        ],
        ["operation-error", (e: string) => report(e)],
        ["duplicate-progress", (n: number) => setDuplicateCount(n)],
        ["cleanup-progress", (p: CleanupResult) => setCleanupProgress(p)],
        ["local-setup-progress", (p: string) => setLocalProgress(p)],
      ] as const) {
        const unlisten = await listen(event, (e) => {
          if (!disposed) (handler as (v: unknown) => void)(e.payload);
        });
        if (disposed) unlisten();
        else unlisteners.push(unlisten);
      }
    };
    void attach().catch(report);
    return () => {
      disposed = true;
      unlisteners.forEach((fn) => fn());
    };
  }, []);
  useEffect(() => {
    if (!native || loaded.current) return;
    loaded.current = true;
    void Promise.all([
      call<Settings>("get_settings"),
      call<Drive[]>("drives"),
      call<Progress[]>("scan_history"),
      call<Preview[]>("review_history"),
      call<RecoveryRecord[]>("recovery_history"),
    ])
      .then(([s, d, h, r, recovery]) => {
        setSettings(s);
        setDrives(d);
        setRoot(d[0]?.path ?? "");
        setHistory(h);
        setReviews(r);
        setRecovery(recovery);
        if (h[0]) {
          setScan(h[0].scan_id);
          setProgress(h[0]);
        }
      })
      .catch(report);
  }, []);
  useEffect(() => {
    document.documentElement.dataset.theme = settings.theme;
  }, [settings.theme]);
  useEffect(() => {
    if (!preview && !proposed && !restore) return;
    const previous = document.activeElement as HTMLElement | null;
    const modal = document.querySelector<HTMLElement>(".modal");
    const first = modal?.querySelector<HTMLElement>(
      "button:not(:disabled), input, select, textarea",
    );
    first?.focus();
    function trap(event: KeyboardEvent) {
      if (event.key === "Escape" && !working) {
        setPreview(null);
        setProposed(null);
        setRestore(null);
      }
      if (event.key !== "Tab" || !modal) return;
      const controls = [
        ...modal.querySelectorAll<HTMLElement>(
          'button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex="0"]',
        ),
      ];
      const first = controls[0],
        last = controls.at(-1);
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last?.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first?.focus();
      }
    }
    document.addEventListener("keydown", trap);
    return () => {
      document.removeEventListener("keydown", trap);
      previous?.focus();
    };
  }, [preview, proposed, restore, working]);
  useEffect(() => {
    setApproved(false);
  }, [preview?.id, restore?.id]);
  useEffect(() => {
    if (!scan || scanning || deduplicating) return;
    let stale = false;
    void call<ScanView>("scan_view", { scan, offset, filter })
      .then((v) => {
        if (!stale) setView(v);
      })
      .catch(report);
    return () => {
      stale = true;
    };
  }, [scan, offset, filter, scanning, deduplicating, scanRevision]);
  function navigate(name: Page) {
    setPage(name);
    setFilter("");
    setOffset(0);
  }
  async function task(fn: () => Promise<void>) {
    setError("");
    setNotice("");
    setWorking(true);
    try {
      await fn();
    } catch (e) {
      report(e);
    } finally {
      setWorking(false);
    }
  }
  async function start() {
    await task(async () => {
      const id = await call<string>("start_scan", { root });
      active.current = id;
      setScan(id);
      setView(null);
      setSelected(new Set());
      setAdvice(null);
      setOffset(0);
      setFilter("");
      setProgress({ ...emptyProgress, scan_id: id, status: "scanning" });
      setPage("Cleaner");
      // Capture a fast scan that completed before invoke returned.
      const v = await call<ScanView>("scan_view", {
        scan: id,
        offset: 0,
        filter: "",
      });
      setProgress((current) =>
        current.scan_id === id &&
        current.status !== "scanning" &&
        v.progress.status === "scanning"
          ? current
          : v.progress,
      );
      if (v.progress.status !== "scanning") {
        setScanRevision((revision) => revision + 1);
      }
    });
  }
  async function persist(s: Settings) {
    await call("save_settings", { settings: s });
    setSettings(s);
    setSelected(new Set());
    setAdvice(null);
    setProposed(null);
    setRule("");
    setNotice(
      "Preferences saved. Protection rules are rechecked when creating a preview.",
    );
  }
  const selectedBytes = (view?.items ?? [])
    .filter((i) => selected.has(i.id))
    .reduce((n, i) => n + i.bytes, 0);
  const fileTable = (
    <>
      <div className="table-tools">
        <label className="inline">
          Show{" "}
          <select
            aria-label="Category"
            value={filter}
            onChange={(e) => {
              setFilter(e.target.value);
              setOffset(0);
              setSelected(new Set());
            }}
          >
            <option value="">All files</option>
            <option value="eligible">Cleanup candidates</option>
            {view?.categories.map((c) => (
              <option key={c.category}>{c.category}</option>
            ))}
          </select>
        </label>
        <span>{view?.total.toLocaleString() ?? 0} files · 200 per page</span>
      </div>
      {view?.items.length ? (
        <div className="file-list">
          {view.items.map((item: Item) => (
            <label className="file-row" key={item.id}>
              <input
                type="checkbox"
                aria-label={`Select ${item.path}`}
                disabled={!item.eligible || scanning}
                checked={selected.has(item.id)}
                onChange={(e) =>
                  setSelected(
                    e.target.checked
                      ? new Set([...selected, item.id])
                      : new Set([...selected].filter((id) => id !== item.id)),
                  )
                }
              />
              <div className="file-info">
                <strong title={item.path}>
                  {item.path.split(/[\\/]/).pop()}
                </strong>
                <small title={item.path}>{item.path}</small>
                <small>{item.reason}</small>
              </div>
              <div className="file-meta">
                <strong>{size(item.bytes)}</strong>
                <span className={`tag ${item.eligible ? "mint" : ""}`}>
                  {item.eligible ? "Can review" : item.category}
                </span>
                {item.duplicate_group && (
                  <small>SHA-256 match · keep by default</small>
                )}
              </div>
            </label>
          ))}
        </div>
      ) : (
        <Empty
          icon={FileSearch}
          title={
            scan ? "No files in this view" : "Choose a drive. Click Analyze."
          }
          text={
            scan
              ? "Try another category or scan a different folder."
              : "Choose a local drive or folder. Scanning never changes your files."
          }
        />
      )}
      {!!view?.total && (
        <div className="pagination">
          <button
            disabled={offset === 0 || working}
            onClick={() => {
              setOffset(Math.max(0, offset - 200));
              setSelected(new Set());
            }}
          >
            <ChevronLeft size={16} />
            Previous
          </button>
          <span>
            {offset + 1}–{Math.min(offset + 200, view.total)} of{" "}
            {view.total.toLocaleString()}
          </span>
          <button
            disabled={offset + 200 >= view.total || working}
            onClick={() => {
              setOffset(offset + 200);
              setSelected(new Set());
            }}
          >
            Next
            <ChevronRight size={16} />
          </button>
        </div>
      )}
    </>
  );

  return (
    <div className="app-shell">
      <aside className="sidebar" inert={!!(preview || proposed || restore)}>
        <a
          className="brand"
          href="#"
          onClick={(e) => {
            e.preventDefault();
            navigate("Cleaner");
          }}
        >
          <div className="brand-mark">
            <ScanLine size={25} />
          </div>
          <span>
            ClutterSift<small>Windows drive cleaner</small>
          </span>
        </a>
        <div className="workspace-label">TOOLS</div>
        <nav aria-label="Main navigation">
          {navigation.map(({ name, icon: Icon }) => (
            <button
              key={name}
              className={page === name ? "nav-item active" : "nav-item"}
              aria-current={page === name ? "page" : undefined}
              onClick={() => navigate(name)}
            >
              <Icon size={19} />
              <span>{name}</span>
            </button>
          ))}
        </nav>
        <div className="sidebar-bottom">
          <div className="version">
            <span className="online-dot" />
            0.1.0 · Utility beta
          </div>
        </div>
      </aside>
      <main inert={!!(preview || proposed || restore)}>
        <header className="topbar">
          <span>
            <span className="breadcrumb">ClutterSift</span>
            <ChevronRight size={14} />
            {page}
          </span>
          <div className="top-actions">
            <span className="privacy">
              <ShieldCheck size={14} />
              Local storage
            </span>
            <button
              className="icon-button"
              aria-label="Toggle theme"
              onClick={() =>
                setSettings({
                  ...settings,
                  theme: settings.theme === "light" ? "dark" : "light",
                })
              }
            >
              {settings.theme === "light" ? (
                <Moon size={18} />
              ) : (
                <Sun size={18} />
              )}
            </button>
          </div>
        </header>
        <div className="content">
          <div className="page-heading">
            <div>
              <h1>{page}</h1>
              <p>{descriptions[page]}</p>
            </div>
            <span className="beta-label">Open source · v0.1</span>
          </div>
          {!native && (
            <div className="banner">
              <CircleHelp size={18} />
              <span>
                Browser preview · Launch the desktop app to access real storage
                data. No sample scan results are shown.
              </span>
            </div>
          )}
          {error && (
            <div className="banner error" role="alert">
              <X size={18} />
              <span>{error}</span>
              <button
                className="icon-button"
                aria-label="Dismiss error"
                onClick={() => setError("")}
              >
                <X size={16} />
              </button>
            </div>
          )}
          {notice && (
            <div className="banner" role="status">
              <Check size={18} />
              {notice}
            </div>
          )}

          {page === "Cleaner" && (
            <>
              <section className="scan-target panel">
                <div className="scan-controls">
                  <label>
                    Drive
                    <select
                      value={drives.some((d) => d.path === root) ? root : ""}
                      onChange={(e) => setRoot(e.target.value)}
                      disabled={scanning || working}
                    >
                      <option value="">Custom path</option>
                      {drives.map((d) => (
                        <option key={d.path} value={d.path}>
                          {d.name || "Local disk"} ({d.path})
                        </option>
                      ))}
                    </select>
                  </label>
                  <label className="grow">
                    Folder or drive path (or a single file)
                    <input
                      value={root}
                      onChange={(e) => setRoot(e.target.value)}
                      placeholder={"C:\\ or D:\\Downloads"}
                      disabled={scanning || working}
                    />
                  </label>
                </div>
                {drives.find((d) => d.path === root) && (
                  <small>
                    {size(drives.find((d) => d.path === root)!.available)} free
                    of {size(drives.find((d) => d.path === root)!.total)}
                  </small>
                )}
              </section>
              <div className="cleaner-workspace">
                <section
                  className="category-pane panel"
                  aria-label="Analysis categories"
                >
                  <h3>File categories</h3>
                  <button
                    className={filter === "" ? "category active" : "category"}
                    onClick={() => {
                      setFilter("");
                      setOffset(0);
                      setSelected(new Set());
                    }}
                  >
                    <HardDrive size={17} />
                    <span>All files</span>
                    <small>{progress.files.toLocaleString()}</small>
                  </button>
                  <button
                    className={
                      filter === "eligible" ? "category active" : "category"
                    }
                    onClick={() => {
                      setFilter("eligible");
                      setOffset(0);
                      setSelected(new Set());
                    }}
                  >
                    <Trash2 size={17} />
                    <span>Cleanup candidates</span>
                    <small>{progress.candidates.toLocaleString()}</small>
                  </button>
                  {view?.categories.map((c) => (
                    <button
                      key={c.category}
                      className={
                        filter === c.category ? "category active" : "category"
                      }
                      onClick={() => {
                        setFilter(c.category);
                        setOffset(0);
                        setSelected(new Set());
                      }}
                    >
                      <FileSearch size={17} />
                      <span>{c.category}</span>
                      <small>{size(c.bytes)}</small>
                    </button>
                  ))}
                  <div className="category-note">
                    <ShieldCheck size={18} />
                    <p>
                      System files, personal folders and projects are protected
                      by default.
                    </p>
                  </div>
                  <details className="scan-history">
                    <summary>Recent scans ({history.length})</summary>
                    {history.map((h) => (
                      <button
                        key={h.scan_id}
                        disabled={scanning || working}
                        onClick={() => {
                          active.current = "";
                          setScan(h.scan_id);
                          setProgress(h);
                          setSelected(new Set());
                          setAdvice(null);
                          navigate("Cleaner");
                        }}
                      >
                        {h.files.toLocaleString()} files · {h.status}
                        <small>{h.scan_id.slice(0, 8)}</small>
                      </button>
                    ))}
                  </details>
                </section>
                <section className="results-pane panel">
                  <div className="panel-heading">
                    <h3>{scan ? "Analysis results" : "Ready to analyze"}</h3>
                    <button
                      disabled={!scan || scanning || working}
                      onClick={() =>
                        void task(async () => {
                          setDuplicateCount(0);
                          setDeduplicating(true);
                          try {
                            const n = await call<number>("find_duplicates", {
                              scan,
                            });
                            setNotice(
                              `${n} matching files confirmed. Matches remain protected from cleanup; this pass is limited to 20,000 files.`,
                            );
                          } finally {
                            setDeduplicating(false);
                          }
                        })
                      }
                    >
                      <FileSearch size={15} />
                      Find exact duplicates
                    </button>
                  </div>
                  {scan && (
                    <div className="analysis-summary" aria-live="polite">
                      <Check size={18} />
                      <strong>
                        {scanning
                          ? "Analyzing…"
                          : progress.status === "complete"
                            ? "Scan complete"
                            : progress.status}
                      </strong>
                      <span>
                        {progress.files.toLocaleString()} files ·{" "}
                        {size(progress.bytes)}
                      </span>
                      <small>
                        {progress.errors} inaccessible · {progress.skipped}{" "}
                        excluded
                      </small>
                    </div>
                  )}
                  {scanning && <div className="indeterminate" />}
                  {deduplicating && (
                    <p role="status">
                      Compared {duplicateCount.toLocaleString()} files.{" "}
                      <button
                        onClick={() => void call("cancel_scan").catch(report)}
                      >
                        Cancel duplicate pass
                      </button>
                    </p>
                  )}
                  {fileTable}
                </section>
              </div>
              <footer className="cleaner-actions">
                <div>
                  <strong>
                    {selected.size} files selected · {size(selectedBytes)}
                  </strong>
                  <small>
                    Quarantine keeps a recovery copy. No disk space is freed.
                  </small>
                </div>
                {scanning ? (
                  <button
                    onClick={() => void call("cancel_scan").catch(report)}
                  >
                    Cancel scan
                  </button>
                ) : (
                  <button
                    className="secondary"
                    disabled={!native || !root || working}
                    onClick={() => void start()}
                  >
                    <ScanLine size={17} />
                    Analyze
                  </button>
                )}
                <button
                  className="primary"
                  disabled={!selected.size || scanning || working}
                  onClick={() =>
                    void task(async () => {
                      setPreview(
                        await call("cleanup_preview", {
                          scan,
                          ids: [...selected],
                        }),
                      );
                      await refreshHistory();
                    })
                  }
                >
                  <Trash2 size={17} />
                  Review cleanup
                </button>
                {working && !preview && !deduplicating && (
                  <button
                    onClick={() => void call("cancel_scan").catch(report)}
                  >
                    Cancel verification
                  </button>
                )}
              </footer>
            </>
          )}

          {page === "AI assistant" && (
            <section className="panel chat-panel">
              <div className="assistant-welcome">
                <span className="assistant-icon">
                  <Sparkles size={28} />
                </span>
                <h2>A little guidance goes a long way.</h2>
                <p>
                  Ask about a cleanup goal. Your saved protections always take
                  priority.
                </p>
                <div className="prompt-chips">
                  {[
                    "Keep my games and coding projects.",
                    "Explain the temporary-file candidates.",
                    "Help me understand what I should keep.",
                  ].map((text) => (
                    <button key={text} onClick={() => setGoal(text)}>
                      {text}
                    </button>
                  ))}
                </div>
              </div>
              <div className="privacy-note">
                <ShieldCheck size={18} />
                <span>
                  {settings.provider === "openrouter"
                    ? "Cloud AI sends your typed goal and up to 200 anonymous IDs, categories, and file sizes. No filenames, paths, or contents. Do not type private information into your goal."
                    : settings.provider === "ollama"
                      ? "Uses the loopback Ollama service. Choose a locally installed model and disable cloud features in Ollama."
                      : "Offline mode uses deterministic guidance. Optional AI is configured in Settings."}
                </span>
              </div>
              {advice && (
                <div className="advice" role="status">
                  <strong>
                    <Sparkles size={17} />
                    {settings.provider === "offline"
                      ? "Offline guidance"
                      : "Assistant recommendation"}
                  </strong>
                  <p>{advice.summary}</p>
                  {advice.candidates.map((c) => (
                    <p key={c.id}>
                      <span className="tag">{c.id.slice(0, 8)}</span>{" "}
                      {c.explanation}
                    </p>
                  ))}
                  {!!advice.candidates.length && (
                    <button
                      className="secondary"
                      onClick={() => {
                        navigate("Cleaner");
                        setFilter("eligible");
                        setSelected(
                          new Set(advice.candidates.map((c) => c.id)),
                        );
                      }}
                    >
                      Review suggested IDs
                      <ArrowRight size={15} />
                    </button>
                  )}
                </div>
              )}
              <form
                className="chat-input"
                onSubmit={(e) => {
                  e.preventDefault();
                  void task(async () =>
                    setAdvice(await call("ask_assistant", { scan, goal })),
                  );
                }}
              >
                <textarea
                  aria-label="Cleanup goal"
                  value={goal}
                  maxLength={4000}
                  onChange={(e) => setGoal(e.target.value)}
                  placeholder="What would you like to make room for?"
                  rows={3}
                />
                <button
                  className="primary"
                  disabled={!scan || !goal.trim() || working || scanning}
                  type="submit"
                >
                  {working ? (
                    <LoaderCircle className="spin" size={17} />
                  ) : (
                    <ArrowRight size={18} />
                  )}
                  Ask assistant
                </button>
              </form>
              <button
                className="secondary"
                disabled={!native || !goal.trim() || working}
                onClick={() =>
                  void task(async () =>
                    setProposed(
                      await call<Settings>("propose_protections", { goal }),
                    ),
                  )
                }
              >
                <FolderLock size={16} />
                Propose protections from this goal
              </button>
              <p className="fine">
                Protection proposals are interpreted locally and require
                confirmation. Known folder names and existing absolute paths are
                supported; quote paths with spaces. Custom application locations
                and relocated personal folders need explicit paths.
              </p>
              <small className="fine">
                Scan first. Advice covers up to 200 eligible candidates and
                never executes actions. To preserve specific paths, confirm
                rules in Protected items.
              </small>
            </section>
          )}

          {page === "Protected items" && (
            <>
              <section className="protection-intro">
                <ShieldCheck size={28} />
                <div>
                  <h3>Some boundaries aren’t optional.</h3>
                  <p>
                    Windows, application installations, known personal folders,
                    detected projects, and sensitive application data are
                    protected by built-in rules. Application protection cannot
                    identify every dependency.
                  </p>
                </div>
              </section>
              <section className="panel">
                <div className="panel-heading">
                  <h3>Your saved rules</h3>
                  <span className="tag mint">Backend enforced</span>
                </div>
                <form
                  className="rule-form"
                  onSubmit={(e) => {
                    e.preventDefault();
                    if (rule.trim())
                      setProposed({
                        ...settings,
                        [ruleKind]: [...settings[ruleKind], rule.trim()],
                      });
                  }}
                >
                  <select
                    aria-label="Rule type"
                    value={ruleKind}
                    onChange={(e) =>
                      setRuleKind(e.target.value as typeof ruleKind)
                    }
                  >
                    <option value="protected">Protect file or folder</option>
                    <option value="patterns">Protect filename pattern</option>
                    <option value="exclusions">Exclude from scan</option>
                  </select>
                  <input
                    aria-label="Protection rule"
                    value={rule}
                    onChange={(e) => setRule(e.target.value)}
                    placeholder={
                      ruleKind === "patterns"
                        ? "*.save"
                        : "Absolute path to an existing file or folder"
                    }
                  />
                  <button
                    type="submit"
                    className="secondary"
                    disabled={!rule.trim() || !native}
                  >
                    <Plus size={17} />
                    Review rule
                  </button>
                </form>
                {(["protected", "patterns", "exclusions"] as const).map(
                  (kind) => (
                    <div key={kind}>
                      <h4>
                        {kind === "protected"
                          ? "Files & folders"
                          : kind === "patterns"
                            ? "Filename patterns"
                            : "Scan exclusions"}
                      </h4>
                      {settings[kind].length ? (
                        settings[kind].map((p, i) => (
                          <div className="rule-row" key={`${p}-${i}`}>
                            <FolderLock size={18} />
                            <span>{p}</span>
                            <button
                              className="icon-button"
                              aria-label={`Remove ${p}`}
                              onClick={() =>
                                setProposed({
                                  ...settings,
                                  [kind]: settings[kind].filter(
                                    (_, n) => n !== i,
                                  ),
                                })
                              }
                            >
                              <Trash2 size={16} />
                            </button>
                          </div>
                        ))
                      ) : (
                        <p className="fine">No custom rules here yet.</p>
                      )}
                    </div>
                  ),
                )}
              </section>
            </>
          )}

          {page === "Recovery" && (
            <>
              <section className="panel">
                <div className="panel-heading">
                  <h3>Quarantine & restore</h3>
                  <button
                    className="secondary"
                    disabled={working || !native}
                    onClick={() => void task(refreshHistory)}
                  >
                    Refresh recovery
                  </button>
                </div>
                <p className="fine">
                  Quarantine preserves files on this drive; it does not reclaim
                  storage. Restore uses the original path and never replaces an
                  existing file. Keep the local journal and recovery folder
                  together. Latest 500 operations shown.
                </p>
                {recovery.length ? (
                  recovery.map((r) => (
                    <div className="history-row" key={r.id}>
                      <History size={19} />
                      <span>
                        <strong>{r.item.path}</strong>
                        <small>
                          {r.status} · {size(r.item.bytes)} ·{" "}
                          {new Date(r.created * 1000).toLocaleString()}
                        </small>
                        <small>Recovery file: {r.stored}</small>
                        {r.error && <small>{r.error}</small>}
                      </span>
                      <button
                        className="secondary"
                        disabled={working || r.status !== "quarantined"}
                        onClick={() => setRestore(r)}
                      >
                        Review restore
                      </button>
                    </div>
                  ))
                ) : (
                  <Empty
                    icon={History}
                    title="No quarantine operations yet"
                    text="Approved moves and their restore status appear here. Dry-run previews below do not change files."
                  />
                )}
              </section>
              <section className="panel">
                <div className="panel-heading">
                  <h3>Dry-run review history</h3>
                  <span className="tag">Preview records</span>
                </div>
                {reviews.length ? (
                  reviews.map((r) => (
                    <button
                      className="history-row"
                      key={r.id}
                      onClick={() =>
                        setPreview({
                          ...r,
                          executable: false,
                          notice:
                            "Historical review only. Create a fresh preview before quarantining any files.",
                        })
                      }
                    >
                      <FileSearch size={19} />
                      <span>
                        <strong>{r.items.length} files reviewed</strong>
                        <small>
                          Preview {r.id.slice(0, 8)} · see quarantine history
                          for operations
                        </small>
                      </span>
                      <strong>{size(r.bytes)}</strong>
                      <ChevronRight size={16} />
                    </button>
                  ))
                ) : (
                  <div className="quiet-empty">
                    Your reviewed plans will appear here.
                  </div>
                )}
              </section>
            </>
          )}

          {page === "Settings" && (
            <>
              <section className="panel settings-panel">
                <h3>Assistant & privacy</h3>
                <label>
                  AI provider
                  <select
                    value={settings.provider}
                    disabled={working}
                    onChange={(e) =>
                      setSettings({
                        ...settings,
                        provider: e.target.value,
                        model: "",
                        cloud_consent: false,
                      })
                    }
                  >
                    <option value="offline">
                      Offline · deterministic rules
                    </option>
                    <option value="ollama">Ollama · local service</option>
                    <option value="openrouter">
                      OpenRouter · optional cloud
                    </option>
                  </select>
                </label>
                {settings.provider !== "offline" && (
                  <label>
                    Model name
                    <input
                      value={settings.model}
                      disabled={working}
                      onChange={(e) =>
                        setSettings({ ...settings, model: e.target.value })
                      }
                      placeholder={
                        settings.provider === "ollama"
                          ? "Locally installed model name"
                          : "Provider/model identifier"
                      }
                    />
                  </label>
                )}
                {settings.provider === "ollama" && (
                  <section className="setup-guide" aria-label="Local AI setup">
                    <h3>Set up local AI</h3>
                    <p>
                      One click installs Ollama for your Windows account if
                      needed, starts the local service, and downloads
                      qwen2.5:1.5b. Internet is needed for setup. Allow at least
                      4 GB for Ollama plus about 1 GB for the model; model use
                      also needs free RAM.
                    </p>
                    <p className="fine">
                      Setup uses Windows Package Manager and accepts the{" "}
                      <a
                        href="https://github.com/ollama/ollama/blob/main/LICENSE"
                        onClick={openSetupLink}
                        target="_blank"
                        rel="noreferrer"
                      >
                        Ollama
                      </a>{" "}
                      and{" "}
                      <a
                        href="https://ollama.com/library/qwen2.5:1.5b"
                        onClick={openSetupLink}
                        target="_blank"
                        rel="noreferrer"
                      >
                        model license
                      </a>{" "}
                      terms. Existing models are kept. Setup selects the
                      downloaded model and saves these preferences after
                      verification.
                    </p>
                    <div className="button-row">
                      <button
                        className="primary"
                        disabled={!native || working}
                        onClick={() =>
                          void task(async () => {
                            setSettingUp(true);
                            setLocalStatus(null);
                            setLocalProgress("Starting local setup…");
                            try {
                              const ready = await call<LocalStatus>(
                                "setup_local_llm",
                                { approved: true },
                              );
                              if (
                                !ready.running ||
                                !ready.models.includes("qwen2.5:1.5b")
                              ) {
                                throw new Error(
                                  "Local model setup was not verified. Preferences were not saved.",
                                );
                              }
                              setLocalStatus(ready);
                              const next = {
                                ...settings,
                                provider: "ollama",
                                model: "qwen2.5:1.5b",
                                cloud_consent: false,
                              };
                              setSettings(next);
                              await persist(next);
                              setNotice(
                                "Local AI is configured. Scan a folder, then open AI assistant to ask a question. The first response may take time while the model loads.",
                              );
                            } catch (error) {
                              setLocalProgress(
                                "Setup stopped. Check the error above; installed files and model data are retained.",
                              );
                              throw error;
                            } finally {
                              setSettingUp(false);
                            }
                          })
                        }
                      >
                        {settingUp
                          ? "Setting up local AI…"
                          : "Install Ollama & download model"}
                      </button>
                      <button
                        disabled={!native || working}
                        onClick={() =>
                          void task(async () => {
                            setLocalStatus(null);
                            setLocalStatus(
                              await call<LocalStatus>("local_llm_status"),
                            );
                          })
                        }
                      >
                        Check local setup
                      </button>
                      {settingUp && (
                        <button
                          onClick={() => {
                            setLocalProgress(
                              "Cancellation requested. Waiting for the current installer or network read to stop…",
                            );
                            void call("cancel_scan").catch(report);
                          }}
                        >
                          Cancel setup
                        </button>
                      )}
                    </div>
                    {localProgress && <p role="status">{localProgress}</p>}
                    {localStatus && (
                      <p role="status">
                        {localStatus.running
                          ? `Ollama is running. Local models: ${localStatus.models.join(", ") || "none installed"}.`
                          : "Ollama is not running. Use automatic setup or open Ollama from the Start menu."}
                      </p>
                    )}
                    <details>
                      <summary>Manual setup and troubleshooting</summary>
                      <ol>
                        <li>
                          If automatic installation fails, install{" "}
                          <a
                            href="https://ollama.com/download/windows"
                            onClick={openSetupLink}
                            target="_blank"
                            rel="noreferrer"
                          >
                            Ollama for Windows
                          </a>{" "}
                          and open it from the Start menu. Alternatively,
                          install Microsoft App Installer to make Windows
                          Package Manager available, then retry.
                        </li>
                        <li>
                          In PowerShell, run{" "}
                          <code>ollama pull qwen2.5:1.5b</code>. Enter{" "}
                          <code>qwen2.5:1.5b</code> in Model name above, check
                          the local setup, and save preferences.
                        </li>
                        <li>
                          For local-only Ollama, set the user environment
                          variable <code>OLLAMA_NO_CLOUD=1</code>, then quit and
                          restart Ollama. A server started by ClutterSift
                          already receives this setting; an existing server
                          keeps its current configuration.
                        </li>
                        <li>
                          The connection stays at <code>127.0.0.1:11434</code>.
                          Cancellation leaves installed software and model data
                          in place. A separate installer may finish after
                          cancellation. Restart setup to resume a model
                          download.
                        </li>
                      </ol>
                      <a
                        href="https://docs.ollama.com/windows"
                        onClick={openSetupLink}
                        target="_blank"
                        rel="noreferrer"
                      >
                        Ollama Windows documentation
                      </a>
                    </details>
                  </section>
                )}
                {settings.provider === "openrouter" && (
                  <>
                    <section
                      className="setup-guide"
                      aria-label="Cloud AI setup"
                    >
                      <h3>Connect OpenRouter</h3>
                      <ol>
                        <li>
                          Create an account at{" "}
                          <a
                            href="https://openrouter.ai"
                            onClick={openSetupLink}
                            target="_blank"
                            rel="noreferrer"
                          >
                            OpenRouter
                          </a>
                          . Add credits if your chosen model requires them.
                          Requests may incur provider charges.
                        </li>
                        <li>
                          Open{" "}
                          <a
                            href="https://openrouter.ai/settings/keys"
                            onClick={openSetupLink}
                            target="_blank"
                            rel="noreferrer"
                          >
                            API keys
                          </a>
                          , create a key with a spending limit, and paste it
                          into the password field below. Click{" "}
                          <strong>Save API key securely</strong>.
                        </li>
                        <li>
                          Choose a model that supports{" "}
                          <a
                            href="https://openrouter.ai/docs/guides/features/structured-outputs"
                            onClick={openSetupLink}
                            target="_blank"
                            rel="noreferrer"
                          >
                            structured outputs / JSON schema
                          </a>{" "}
                          in the{" "}
                          <a
                            href="https://openrouter.ai/models?fmt=cards&supported_parameters=structured_outputs"
                            onClick={openSetupLink}
                            target="_blank"
                            rel="noreferrer"
                          >
                            model catalog
                          </a>
                          . Copy its full provider/model identifier into Model
                          name above.
                        </li>
                        <li>
                          Read the consent below, enable it if you agree, then
                          click <strong>Save preferences</strong>.
                        </li>
                        <li>
                          Analyze a folder, open <strong>AI assistant</strong>,
                          and ask a question. If the service rejects the
                          request, check your key, balance, model identifier and
                          structured-output support.
                        </li>
                      </ol>
                      <p className="fine">
                        Your typed goal is sent verbatim. Do not include private
                        information. Keys are stored in Windows Credential
                        Manager. Choosing a provider does not send data or
                        enable consent.
                      </p>
                    </section>
                    <label className="checkbox-label">
                      <input
                        type="checkbox"
                        disabled={working}
                        checked={settings.cloud_consent}
                        onChange={(e) =>
                          setSettings({
                            ...settings,
                            cloud_consent: e.target.checked,
                          })
                        }
                      />
                      <span>
                        I consent to sending my typed goals and anonymous
                        candidate IDs, categories, and file sizes to OpenRouter
                        and its model providers. Filenames, paths, and contents
                        are never sent.
                      </span>
                    </label>
                    <label>
                      OpenRouter API key
                      <input
                        type="password"
                        autoComplete="off"
                        value={key}
                        onChange={(e) => setKey(e.target.value)}
                        placeholder="Stored only in Windows Credential Manager"
                      />
                    </label>
                    <div className="button-row">
                      <button
                        disabled={!native || !key || working}
                        onClick={() =>
                          void task(async () => {
                            await call("save_api_key", { key });
                            setKey("");
                            setNotice(
                              "API key saved in Windows Credential Manager.",
                            );
                          })
                        }
                      >
                        Save API key securely
                      </button>
                      <button
                        disabled={!native || working}
                        onClick={() =>
                          void task(async () => {
                            await call("save_api_key", { key: "" });
                            setKey("");
                            setNotice("Credential removed.");
                          })
                        }
                      >
                        Remove stored key
                      </button>
                    </div>
                  </>
                )}
                <h3>Appearance</h3>
                <label>
                  Theme
                  <select
                    value={settings.theme}
                    disabled={working}
                    onChange={(e) =>
                      setSettings({ ...settings, theme: e.target.value })
                    }
                  >
                    <option value="dark">Dark</option>
                    <option value="light">Light</option>
                    <option value="system">System</option>
                  </select>
                </label>
                <div className="privacy-note">
                  <ShieldCheck size={20} />
                  <span>
                    No telemetry. No automatic cleanup. Preferences,
                    inventories, and review history stay in local SQLite
                    storage. Inventories contain local paths and are not
                    encrypted.
                  </span>
                </div>
                <button
                  className="primary"
                  disabled={!native || working}
                  onClick={() => void task(async () => persist(settings))}
                >
                  <Check size={17} />
                  Save preferences
                </button>
              </section>
              <section className="panel">
                <h3>Utility limitations</h3>
                <p className="fine">
                  Permanent deletion, Windows Recycle Bin integration, orphaned
                  application detection, empty-directory removal, protection
                  profiles, and largest-folder aggregation are not enabled.
                  Duplicate checks are opt-in and bounded. This beta is for
                  storage analysis and review and explicitly approved
                  quarantine. Quarantine does not free storage.
                </p>
              </section>
              <details className="panel">
                <summary>Application inventory</summary>
                <p className="fine">
                  Reported application sizes may be inaccurate. Application
                  files cannot be selected for cleanup.
                </p>
                <button
                  disabled={!native || working}
                  onClick={() =>
                    void task(async () => setApps(await call("installed_apps")))
                  }
                >
                  Read application inventory
                </button>
                {apps.slice(0, 200).map((a) => (
                  <div className="rule-row" key={a.name}>
                    <span>{a.name}</span>
                    <small>
                      {a.bytes ? size(a.bytes) : "Size not reported"}
                    </small>
                  </div>
                ))}
              </details>
            </>
          )}
          <footer>
            <ShieldCheck size={14} />
            Scans are read-only. Cleanup requires your approval.
            <span>Open source · No telemetry</span>
          </footer>
        </div>
      </main>
      {preview && (
        <div className="modal-backdrop">
          <section
            className="modal"
            role="dialog"
            aria-modal="true"
            aria-labelledby="preview-title"
          >
            <div className="panel-heading">
              <h2 id="preview-title">Your dry-run preview</h2>
              <button
                autoFocus
                className="icon-button"
                aria-label="Close preview"
                disabled={working}
                onClick={() => setPreview(null)}
              >
                <X size={20} />
              </button>
            </div>
            <div className="banner">
              <ShieldCheck size={20} />
              <span>{preview.notice}</span>
            </div>
            {error && (
              <div className="banner error" aria-live="polite">
                {error}
              </div>
            )}
            <h3>
              {preview.items.length} files · {size(preview.bytes)}
            </h3>
            <div className="preview-items">
              {preview.items.map((i) => (
                <div key={i.id}>
                  <strong>{i.path}</strong>
                  <small>
                    {size(i.bytes)} · {i.reason}
                  </small>
                </div>
              ))}
            </div>
            {preview.executable && (
              <>
                <label className="check-label">
                  <input
                    type="checkbox"
                    checked={approved}
                    disabled={working}
                    onChange={(e) => setApproved(e.target.checked)}
                  />
                  I reviewed these files and approve moving them to quarantine.
                  I understand this does not free disk space.
                </label>
                <button
                  className="primary"
                  disabled={!approved || working}
                  onClick={() =>
                    void task(async () => {
                      setQuarantining(true);
                      setCleanupProgress(null);
                      setPreview({
                        ...preview,
                        executable: false,
                        notice:
                          "Quarantine requested. If the operation stops, refresh Recovery to inspect every recorded outcome. Some files may already have moved.",
                      });
                      try {
                        const result = await call<CleanupResult>(
                          "execute_cleanup",
                          { preview: preview.id, approved: true },
                        );
                        setPreview(null);
                        setSelected(new Set());
                        setNotice(
                          `${result.quarantined} quarantined · ${result.failed} failed${result.cancelled ? " · cancelled" : ""}. No disk space reclaimed. See Recovery for each result.`,
                        );
                        await refreshHistory();
                        navigate("Recovery");
                      } finally {
                        setQuarantining(false);
                      }
                    })
                  }
                >
                  Approve & quarantine {preview.items.length} files
                </button>
              </>
            )}
            {quarantining && (
              <div className="banner">
                <span>
                  {cleanupProgress
                    ? `${cleanupProgress.quarantined} moved · ${cleanupProgress.failed} failed`
                    : "Revalidating files…"}
                </span>
                <button
                  className="secondary"
                  onClick={() => void call("cancel_scan").catch(report)}
                >
                  Cancel remaining files
                </button>
              </div>
            )}
            <button
              className="secondary"
              disabled={working}
              onClick={() => setPreview(null)}
            >
              Close review
            </button>
          </section>
        </div>
      )}
      {restore && (
        <div className="modal-backdrop">
          <section
            className="modal"
            role="dialog"
            aria-modal="true"
            aria-labelledby="restore-title"
          >
            <h2 id="restore-title">Confirm restore</h2>
            {error && (
              <div className="banner error" aria-live="polite">
                {error}
              </div>
            )}
            <p>Restore this quarantined file to its original location:</p>
            <p className="fine">{restore.item.path}</p>
            <p>
              Existing files will never be overwritten. If the original
              directory is missing, restore stops.
            </p>
            <label className="check-label">
              <input
                type="checkbox"
                disabled={working}
                checked={approved}
                onChange={(e) => setApproved(e.target.checked)}
              />
              I approve restoring this file.
            </label>
            <button
              className="primary"
              disabled={!approved || working}
              onClick={() =>
                void task(async () => {
                  await call("restore_item", {
                    id: restore.id,
                    approved: true,
                  });
                  setRestore(null);
                  setNotice("File restored to its original location.");
                  await refreshHistory();
                })
              }
            >
              Restore file
            </button>
            <button
              className="secondary"
              disabled={working}
              onClick={() => setRestore(null)}
            >
              Cancel restore
            </button>
          </section>
        </div>
      )}
      {proposed && (
        <div className="modal-backdrop">
          <section
            className="modal"
            role="dialog"
            aria-modal="true"
            aria-labelledby="rule-title"
          >
            <h2 id="rule-title">Confirm protection changes</h2>
            <p>
              Saving replaces your custom rules with the list below. Built-in
              protections always remain active.
            </p>
            {(["protected", "patterns", "exclusions"] as const).map((k) => (
              <div key={k}>
                <h4>{k}</h4>
                {proposed[k].length ? (
                  proposed[k].map((p, i) => (
                    <p className="path" key={`${p}-${i}`}>
                      {p}
                    </p>
                  ))
                ) : (
                  <small>No custom rules</small>
                )}
              </div>
            ))}
            <div className="button-row">
              <button autoFocus onClick={() => setProposed(null)}>
                Cancel
              </button>
              <button
                className="primary"
                disabled={working}
                onClick={() => void task(async () => persist(proposed))}
              >
                <ShieldCheck size={17} />
                Confirm and save rules
              </button>
            </div>
          </section>
        </div>
      )}
    </div>
  );
}
function Empty({
  icon: Icon,
  title,
  text,
}: {
  icon: typeof MessageSquare;
  title: string;
  text: string;
}) {
  return (
    <div className="empty">
      <Icon size={29} />
      <h3>{title}</h3>
      <p>{text}</p>
    </div>
  );
}

import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState, type KeyboardEvent } from "react";
import { CheckIcon, DownloadIcon, ErrorIcon, SpinnerIcon } from "../components/icons";
import { DEFAULT_LAUNCHER_PATH, loadAppSettings, saveAppSettings } from "../lib/settings";
import { checkForUpdate, downloadAndInstallUpdate, type Update } from "../lib/updater";

type UpdateStatus = "idle" | "checking" | "latest" | "available" | "downloading" | "installing" | "check-failed" | "update-failed";

interface SettingsScreenProps {
  version: string;
  /** 保存済みの launch.exe パスにファイルが存在するか */
  isLauncherFound: boolean;
  onError: (message: string) => void;
}

export function SettingsScreen({ version, isLauncherFound, onError }: SettingsScreenProps) {
  const [status, setStatus] = useState<UpdateStatus>("idle");
  const [update, setUpdate] = useState<Update | null>(null);
  /** download 進捗 (0〜1)。サイズ不明の場合は null */
  const [downloadProgress, setDownloadProgress] = useState<number | null>(null);
  const isBusy = status === "checking" || status === "downloading" || status === "installing";
  const isUpdateAvailable = update !== null && (status === "available" || status === "update-failed");
  const latestVersion = update?.version ?? "";

  const runCheck = async () => {
    setStatus("checking");
    try {
      const found = await checkForUpdate();
      setUpdate(found);
      setStatus(found ? "available" : "latest");
    } catch {
      setUpdate(null);
      setStatus("check-failed");
    }
  };

  const runUpdate = async (target: Update) => {
    setDownloadProgress(null);
    setStatus("downloading");
    try {
      await downloadAndInstallUpdate(target, (ratio) => {
        setDownloadProgress(ratio);
        if (ratio === 1) setStatus("installing");
      });
      setStatus("installing");
    } catch {
      setStatus("update-failed");
    }
  };

  const onUpdateButtonClick = () => {
    if (isBusy) return;
    if (isUpdateAvailable && update) void runUpdate(update);
    else void runCheck();
  };

  const progressPercent = downloadProgress === null ? null : Math.round(downloadProgress * 100);

  const statusText: Record<UpdateStatus, string> = {
    idle: "Not checked yet",
    checking: "Checking for updates…",
    latest: "You're on the latest version",
    available: `Version ${latestVersion} is available`,
    downloading: `Downloading version ${latestVersion}…`,
    installing: "Installing… Polaris will restart automatically",
    "check-failed": "Failed to check for updates",
    "update-failed": "Failed to install the update",
  };

  const buttonLabel: Record<UpdateStatus, string> = {
    idle: "Check",
    checking: "Checking",
    latest: "Check",
    available: `Update to ${latestVersion}`,
    downloading: progressPercent === null ? "Downloading" : `${progressPercent}%`,
    installing: "Installing",
    "check-failed": "Retry",
    "update-failed": "Retry update",
  };

  const statusIcon =
    status === "checking" || status === "downloading" || status === "installing" ? (
      <SpinnerIcon size={20} className="icon-spin" />
    ) : status === "latest" ? (
      <CheckIcon size={20} strokeWidth={2.4} className="icon-pop" />
    ) : status === "available" ? (
      <DownloadIcon size={20} className="icon-pop" />
    ) : status === "check-failed" || status === "update-failed" ? (
      <ErrorIcon size={20} className="icon-pop" />
    ) : null;

  const isError = status === "check-failed" || status === "update-failed";
  const releaseNotes = update?.body?.trim();

  return (
    <div className="settings-list">
      <LauncherSettingRow isLauncherFound={isLauncherFound} onError={onError} />
      <div className="settings-row">
        <div className="settings-row-text">
          <span className="settings-row-title">Updates</span>
          <span className="settings-row-description">
            Current version <span className="settings-version">{version}</span>
          </span>
          <span
            className={`settings-update-status ${isUpdateAvailable || status === "downloading" || status === "installing" ? "is-available" : ""} ${isError ? "is-error" : ""}`}
          >
            {statusText[status]}
          </span>
        </div>
        <button
          className={`update-button ${isUpdateAvailable ? "is-available" : ""} ${isBusy ? "is-busy" : ""}`}
          onClick={onUpdateButtonClick}
          disabled={isBusy}
        >
          {status === "downloading" && progressPercent !== null && (
            <span className="update-button-progress" style={{ width: `${progressPercent}%` }} aria-hidden="true" />
          )}
          <span className="update-button-icon">{statusIcon}</span>
          <span className="update-button-label">{buttonLabel[status]}</span>
        </button>
        {update && releaseNotes && (
          <div className="update-release-notes">
            <span className="update-release-notes-title">What's new in {latestVersion}</span>
            <pre className="update-release-notes-body">{releaseNotes}</pre>
          </div>
        )}
      </div>
    </div>
  );
}

/** インスタンスを開くのに使う launch.exe のパス。確定 (blur / Enter / 参照 / リセット) 時に保存する */
function LauncherSettingRow({ isLauncherFound, onError }: Pick<SettingsScreenProps, "isLauncherFound" | "onError">) {
  const [savedPath, setSavedPath] = useState<string | null>(null);
  const [draftPath, setDraftPath] = useState("");

  useEffect(() => {
    let isCancelled = false;
    loadAppSettings()
      .then((settings) => settings.launcherPath)
      .catch(() => DEFAULT_LAUNCHER_PATH)
      .then((path) => {
        if (isCancelled) return;
        setSavedPath(path);
        setDraftPath(path);
      });
    return () => {
      isCancelled = true;
    };
  }, []);

  const isLoaded = savedPath !== null;

  const commit = async (path: string) => {
    const next = path.trim();
    setDraftPath(next);
    if (next === savedPath) return;
    try {
      await saveAppSettings({ launcherPath: next });
      setSavedPath(next);
    } catch {
      onError("Failed to save the launch.exe path");
    }
  };

  const browse = async () => {
    const selected = await open({
      title: "Select VRChat launch.exe",
      defaultPath: draftPath || DEFAULT_LAUNCHER_PATH,
      filters: [{ name: "launch.exe", extensions: ["exe"] }],
    }).catch(() => null);
    if (selected) await commit(selected);
  };

  const onInputKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") e.currentTarget.blur();
  };

  return (
    <div className="settings-row launcher-setting-row">
      <div className="settings-row-text">
        <span className="settings-row-title">VRChat launcher</span>
        <span className="settings-row-description">Path to launch.exe, used to open instances in VRChat</span>
      </div>
      <div className="launcher-path-field">
        <input
          className="text-input launcher-path-input"
          value={draftPath}
          onChange={(e) => setDraftPath(e.target.value)}
          onBlur={() => void commit(draftPath)}
          onKeyDown={onInputKeyDown}
          disabled={!isLoaded}
          spellCheck={false}
          aria-label="launch.exe path"
        />
        <button className="btn-secondary launcher-browse-button" onClick={() => void browse()} disabled={!isLoaded}>
          Browse
        </button>
        <button
          className="btn-secondary launcher-reset-button"
          onClick={() => void commit(DEFAULT_LAUNCHER_PATH)}
          disabled={!isLoaded || draftPath === DEFAULT_LAUNCHER_PATH}
        >
          Reset
        </button>
      </div>
      {isLoaded && (
        <span className={`launcher-path-status ${isLauncherFound ? "is-found" : "is-missing"}`}>
          {isLauncherFound ? <CheckIcon size={18} strokeWidth={2.4} /> : <ErrorIcon size={18} />}
          {isLauncherFound ? "launch.exe found" : "launch.exe not found"}
        </span>
      )}
    </div>
  );
}

import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState, type KeyboardEvent } from "react";
import { CheckIcon, DownloadIcon, ErrorIcon, SpinnerIcon } from "../components/icons";
import type { UpdaterState, UpdateStatus } from "../hooks/useUpdater";
import { openAuthLogFolder } from "../lib/auth";
import { formatDateTime } from "../lib/format";
import { DEFAULT_LAUNCHER_PATH, type AppSettings } from "../lib/settings";

interface SettingsScreenProps {
  version: string;
  /** 読み込み完了までは null */
  appSettings: AppSettings | null;
  onUpdateAppSettings: (patch: Partial<AppSettings>) => Promise<void>;
  updater: UpdaterState;
  /** Developer mode の設定項目を表示するか (隠しコマンドで表示する) */
  isDeveloperModeVisible: boolean;
  /** 保存済みの launch.exe パスにファイルが存在するか */
  isLauncherFound: boolean;
  onError: (message: string) => void;
}

export function SettingsScreen({ version, appSettings, onUpdateAppSettings, updater, isDeveloperModeVisible, isLauncherFound, onError }: SettingsScreenProps) {
  const saveAppSettings = async (patch: Partial<AppSettings>, errorMessage: string) => {
    try {
      await onUpdateAppSettings(patch);
    } catch {
      onError(errorMessage);
    }
  };

  return (
    <div className="settings-list">
      <LauncherSettingRow
        savedPath={appSettings?.launcherPath ?? null}
        onSave={(launcherPath) => saveAppSettings({ launcherPath }, "Failed to save the launch.exe path")}
        isLauncherFound={isLauncherFound}
      />
      <ToggleSettingRow
        title="Run in background"
        description="Polaris が完全に終了せずに最小化され、グループインスタンスの監視を続けます。"
        isOn={appSettings?.runInBackground ?? false}
        isDisabled={!appSettings}
        onToggle={(runInBackground) => void saveAppSettings({ runInBackground }, "Failed to save Run in background")}
      />
      <ToggleSettingRow
        title="Launch at startup"
        description="PC起動時に Polaris を自動で開きます。"
        isOn={appSettings?.launchAtStartup ?? false}
        isDisabled={!appSettings}
        onToggle={(launchAtStartup) => void saveAppSettings({ launchAtStartup }, "Failed to save Launch at startup")}
      />
      <UpdateSettingRow version={version} updater={updater} />
      {isDeveloperModeVisible && (
        <ToggleSettingRow
          title="Developer mode"
          description="開発・テスト用の設定項目を表示します。"
          isOn={appSettings?.developerMode ?? false}
          isDisabled={!appSettings}
          onToggle={(developerMode) => void saveAppSettings({ developerMode }, "Failed to save Developer mode")}
        />
      )}
      {appSettings?.developerMode && (
        <div className="developer-settings">
          <span className="developer-settings-title">Developer</span>
          <ToggleSettingRow
            title="Simulate available update"
            description="アップデートサーバーに問い合わせず、ダミーのアップデートを表示します。実際のダウンロードやインストールは行いません。"
            isOn={appSettings.simulateUpdateAvailable}
            onToggle={(simulateUpdateAvailable) =>
              void saveAppSettings({ simulateUpdateAvailable }, "Failed to save Simulate available update")
            }
          />
          <ToggleSettingRow
            title="Thumbnail mode"
            description="サムネイル撮影用に、フレンドをダミーデータに置き換えます。実際のフレンドは非表示になり、ピン留めしたフレンドはそのまま残ります。"
            isOn={appSettings.thumbnailMode}
            onToggle={(thumbnailMode) => void saveAppSettings({ thumbnailMode }, "Failed to save Thumbnail mode")}
          />
          <ToggleSettingRow
            title="Simulate last joined"
            description="グループインスタンスの「最後に入ってからの経過時間」を、実際の記録の代わりに見本の値（0m / 12m / 3h05m / 23h59m）で表示します。実際の記録は変更されません。"
            isOn={appSettings.simulateLastJoined}
            onToggle={(simulateLastJoined) => void saveAppSettings({ simulateLastJoined }, "Failed to save Simulate last joined")}
          />
          <ToggleSettingRow
            title="Auth logging"
            description="VRChat API へのリクエストと、認証・バックグラウンド取得・自動更新・ウィンドウ状態などの出来事をログファイルに詳しく記録します。cookie やパスワードの値は記録しません。"
            isOn={appSettings.authLogging}
            onToggle={(authLogging) => void saveAppSettings({ authLogging }, "Failed to save Auth logging")}
          />
          <div className="settings-row auth-log-folder-row">
            <div className="settings-row-text">
              <span className="settings-row-title">Auth log folder</span>
              <span className="settings-row-description">auth.log の保存先フォルダを開きます。</span>
            </div>
            <button
              className="btn-secondary auth-log-open-folder-button"
              onClick={() => openAuthLogFolder().catch(() => onError("Failed to open the log folder"))}
            >
              Open folder
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

function UpdateSettingRow({ version, updater }: { version: string; updater: UpdaterState }) {
  const { status, update, downloadProgress, lastCheckedAt, isBusy, isUpdateAvailable, check, install } = updater;
  const latestVersion = update?.version ?? "";

  const onUpdateButtonClick = () => {
    if (isBusy) return;
    if (isUpdateAvailable && update) void install(update);
    else void check();
  };

  const progressPercent = downloadProgress === null ? null : Math.round(downloadProgress * 100);

  const statusText: Record<UpdateStatus, string> = {
    idle: "Not checked yet",
    checking: "Checking for updates…",
    latest: "",
    available: `Version ${latestVersion} is available${update?.isSimulated ? " (simulated)" : ""}`,
    downloading: `Downloading version ${latestVersion}…`,
    installing: update?.isSimulated ? "Installing… (simulated)" : "Installing… Polaris will restart automatically",
    "check-failed": "Failed to check for updates",
    "update-failed": "Failed to install the update",
  };

  const buttonLabel: Record<UpdateStatus, string> = {
    idle: "Check",
    checking: "Checking",
    latest: "Check",
    available: "Update",
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
    <div className="settings-row update-setting-row">
      <div className="settings-row-text">
        <span className="settings-row-title">Updates</span>
        <span className="settings-row-description">
          現在のバージョン <span className="settings-version">{version}</span>
        </span>
        {statusText[status] && (
          <span
            className={`settings-update-status ${isUpdateAvailable || status === "downloading" || status === "installing" ? "is-available" : ""} ${isError ? "is-error" : ""}`}
          >
            {statusText[status]}
          </span>
        )}
        {lastCheckedAt !== null && (
          <span className="settings-update-last-checked">Last checked: {formatDateTime(lastCheckedAt)}</span>
        )}
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
  );
}

interface ToggleSettingRowProps {
  title: string;
  description: string;
  isOn: boolean;
  isDisabled?: boolean;
  onToggle: (isOn: boolean) => void;
}

/** on/off を切り替える設定項目。行全体ではなく toggle switch を押して切り替える */
function ToggleSettingRow({ title, description, isOn, isDisabled = false, onToggle }: ToggleSettingRowProps) {
  return (
    <div className="settings-row">
      <div className="settings-row-text">
        <span className="settings-row-title">{title}</span>
        <span className="settings-row-description">{description}</span>
      </div>
      <button
        className={`toggle-switch ${isOn ? "is-on" : ""}`}
        role="switch"
        aria-checked={isOn}
        aria-label={title}
        disabled={isDisabled}
        onClick={() => onToggle(!isOn)}
      >
        <span className="toggle-switch-thumb" />
      </button>
    </div>
  );
}

/** インスタンスを開くのに使う launch.exe のパス。確定 (blur / Enter / 参照 / リセット) 時に保存する */
function LauncherSettingRow({
  savedPath,
  onSave,
  isLauncherFound,
}: {
  /** 読み込み完了までは null */
  savedPath: string | null;
  onSave: (path: string) => Promise<void>;
  isLauncherFound: boolean;
}) {
  const [draftPath, setDraftPath] = useState(savedPath ?? "");

  useEffect(() => {
    if (savedPath !== null) setDraftPath(savedPath);
  }, [savedPath]);

  const isLoaded = savedPath !== null;

  const commit = async (path: string) => {
    const next = path.trim();
    setDraftPath(next);
    if (next === savedPath) return;
    await onSave(next);
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
        <span className="settings-row-description">launch.exe のパス</span>
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

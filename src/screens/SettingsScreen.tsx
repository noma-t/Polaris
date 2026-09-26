import { useState } from "react";
import { CheckIcon, DownloadIcon, ErrorIcon, SpinnerIcon } from "../components/icons";
import { checkForUpdate, downloadAndInstallUpdate, type Update } from "../lib/updater";

type UpdateStatus = "idle" | "checking" | "latest" | "available" | "downloading" | "installing" | "check-failed" | "update-failed";

export function SettingsScreen({ version }: { version: string }) {
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

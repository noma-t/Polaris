import { useState } from "react";
import { CheckIcon, DownloadIcon, SpinnerIcon } from "../components/icons";
import { MOCK_LATEST_VERSION, MOCK_UPDATE_AVAILABLE } from "../data/mock";

type UpdateStatus = "idle" | "checking" | "latest" | "available" | "updating";

const STATUS_TEXT: Record<UpdateStatus, string> = {
  idle: "Not checked yet",
  checking: "Checking for updates…",
  latest: "You're on the latest version",
  available: `Version ${MOCK_LATEST_VERSION} is available`,
  updating: `Version ${MOCK_LATEST_VERSION} is available`,
};

const BUTTON_LABEL: Record<UpdateStatus, string> = {
  idle: "Check",
  checking: "Checking",
  latest: "Check",
  available: `Update to ${MOCK_LATEST_VERSION}`,
  updating: `Update to ${MOCK_LATEST_VERSION}`,
};

export function SettingsScreen({ version }: { version: string }) {
  const [status, setStatus] = useState<UpdateStatus>("idle");
  const isBusy = status === "checking" || status === "updating";
  const isUpdateAvailable = status === "available";

  // モック: 実際の更新処理は未実装 (updating は 3 秒後に available へ戻す)
  const onUpdateButtonClick = () => {
    if (isBusy) return;
    if (isUpdateAvailable) {
      setStatus("updating");
      setTimeout(() => setStatus("available"), 3000);
    } else {
      setStatus("checking");
      setTimeout(
        () => setStatus(MOCK_UPDATE_AVAILABLE && version !== MOCK_LATEST_VERSION ? "available" : "latest"),
        900 + Math.random() * 2600,
      );
    }
  };

  const statusIcon =
    status === "checking" ? (
      <SpinnerIcon size={20} className="icon-spin" />
    ) : status === "latest" ? (
      <CheckIcon size={20} strokeWidth={2.4} className="icon-pop" />
    ) : status === "available" || status === "updating" ? (
      <DownloadIcon size={20} className="icon-pop" />
    ) : null;

  return (
    <div className="settings-list">
      <div className="settings-row">
        <div className="settings-row-text">
          <span className="settings-row-title">Updates</span>
          <span className="settings-row-description">
            Current version <span className="settings-version">{version}</span>
          </span>
          <span className={`settings-update-status ${isUpdateAvailable ? "is-available" : ""}`}>{STATUS_TEXT[status]}</span>
        </div>
        <button
          className={`update-button ${isUpdateAvailable ? "is-available" : ""} ${isBusy ? "is-busy" : ""}`}
          onClick={onUpdateButtonClick}
          disabled={isBusy}
        >
          <span className="update-button-icon">{statusIcon}</span>
          <span>{BUTTON_LABEL[status]}</span>
        </button>
      </div>
    </div>
  );
}

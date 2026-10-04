import type { CSSProperties, ReactNode } from "react";

interface IconProps {
  size?: number;
  className?: string;
  style?: CSSProperties;
}

function StrokeIcon({ size = 24, className, style, strokeWidth = 1.8, viewBox = "0 0 24 24", children }: IconProps & { strokeWidth?: number; viewBox?: string; children: ReactNode }) {
  return (
    <svg
      className={className}
      style={{ flex: "none", ...style }}
      width={size}
      height={size}
      viewBox={viewBox}
      fill="none"
      stroke="currentColor"
      strokeWidth={strokeWidth}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      {children}
    </svg>
  );
}

export const EyeIcon = ({ crossed, ...p }: IconProps & { crossed?: boolean }) => (
  <StrokeIcon {...p}>
    <path d="M2 12s3.6-6.5 10-6.5S22 12 22 12s-3.6 6.5-10 6.5S2 12 2 12z" />
    <circle cx="12" cy="12" r="3" />
    {crossed && <line x1="4" y1="4" x2="20" y2="20" />}
  </StrokeIcon>
);

export const SidebarToggleIcon = (p: IconProps) => (
  <StrokeIcon {...p}>
    <rect x="3.5" y="4.5" width="17" height="15" rx="2" />
    <line x1="9.5" y1="4.5" x2="9.5" y2="19.5" />
  </StrokeIcon>
);

export const InstancesIcon = (p: IconProps) => (
  <StrokeIcon {...p}>
    <line x1="8" y1="6" x2="20" y2="6" />
    <line x1="8" y1="12" x2="20" y2="12" />
    <line x1="8" y1="18" x2="20" y2="18" />
    <circle cx="4" cy="6" r="0.6" />
    <circle cx="4" cy="12" r="0.6" />
    <circle cx="4" cy="18" r="0.6" />
  </StrokeIcon>
);

export const MoonIcon = (p: IconProps) => (
  <StrokeIcon {...p}>
    <path d="M20 14.5A8 8 0 1 1 9.5 4a6.5 6.5 0 0 0 10.5 10.5z" />
  </StrokeIcon>
);

export const SettingsIcon = (p: IconProps) => (
  <StrokeIcon {...p}>
    <line x1="4" y1="7" x2="20" y2="7" />
    <line x1="4" y1="17" x2="20" y2="17" />
    <circle cx="9" cy="7" r="2.4" fill="var(--color-panel)" />
    <circle cx="15" cy="17" r="2.4" fill="var(--color-panel)" />
  </StrokeIcon>
);

export const InfoIcon = (p: IconProps) => (
  <StrokeIcon {...p}>
    <circle cx="12" cy="12" r="9" />
    <line x1="12" y1="11" x2="12" y2="16.5" />
    <circle cx="12" cy="7.8" r="0.6" />
  </StrokeIcon>
);

export const SignOutIcon = (p: IconProps) => (
  <StrokeIcon {...p}>
    <path d="M14 4h4a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2h-4" />
    <polyline points="9 8 5 12 9 16" />
    <line x1="5" y1="12" x2="15" y2="12" />
  </StrokeIcon>
);

export const GroupsIcon = (p: IconProps) => (
  <StrokeIcon {...p}>
    <circle cx="9" cy="8" r="3.2" />
    <path d="M3 19c0-3.3 2.7-5.5 6-5.5s6 2.2 6 5.5" />
    <circle cx="17" cy="9" r="2.4" />
    <path d="M16.5 13.6c2.6.2 4.5 2.1 4.5 4.9" />
  </StrokeIcon>
);

export const FriendIcon = (p: IconProps) => (
  <StrokeIcon {...p}>
    <circle cx="12" cy="8" r="3.6" />
    <path d="M5 20c0-3.9 3.1-6.5 7-6.5s7 2.6 7 6.5" />
  </StrokeIcon>
);

/** 表示 / 非表示の管理 (リスト + 目) */
export const VisibilityListIcon = (p: IconProps) => (
  <StrokeIcon {...p}>
    <line x1="3" y1="5" x2="20" y2="5" />
    <line x1="3" y1="10.5" x2="14" y2="10.5" />
    <line x1="3" y1="16" x2="8" y2="16" />
    <path d="M11 17s2.2-3.8 5.5-3.8S22 17 22 17s-2.2 3.8-5.5 3.8S11 17 11 17z" />
    <circle cx="16.5" cy="17" r="1.3" />
  </StrokeIcon>
);

export const CloseIcon = (p: IconProps) => (
  <StrokeIcon {...p}>
    <line x1="6" y1="6" x2="18" y2="18" />
    <line x1="18" y1="6" x2="6" y2="18" />
  </StrokeIcon>
);

export const ChevronDownIcon =(p: IconProps) => (
  <StrokeIcon {...p} viewBox="0 0 20 20" strokeWidth={2.4}>
    <polyline points="4,7 10,13 16,7" />
  </StrokeIcon>
);

export const ErrorIcon = (p: IconProps) => (
  <StrokeIcon {...p} strokeWidth={2.2}>
    <circle cx="12" cy="12" r="9" />
    <line x1="12" y1="7.5" x2="12" y2="13" />
    <circle cx="12" cy="16.4" r="0.6" />
  </StrokeIcon>
);

export const CheckIcon = ({ strokeWidth = 2.6, ...p }: IconProps & { strokeWidth?: number }) => (
  <StrokeIcon {...p} viewBox="0 0 20 20" strokeWidth={strokeWidth}>
    <polyline points="4,10.5 8.5,15 16,5.5" />
  </StrokeIcon>
);

export const SpinnerIcon = (p: IconProps) => (
  <StrokeIcon {...p} viewBox="0 0 20 20" strokeWidth={2.4}>
    <circle cx="10" cy="10" r="7" opacity="0.25" />
    <path d="M10 3a7 7 0 0 1 7 7" />
  </StrokeIcon>
);

export const DownloadIcon = (p: IconProps) => (
  <StrokeIcon {...p} viewBox="0 0 20 20" strokeWidth={2.4}>
    <line x1="10" y1="3.5" x2="10" y2="13" />
    <polyline points="5.5,9 10,13.5 14.5,9" />
    <line x1="4" y1="17" x2="16" y2="17" />
  </StrokeIcon>
);

/** 外部リンク (ブラウザで開く) */
export const ExternalLinkIcon = (p: IconProps) => (
  <StrokeIcon {...p} strokeWidth={2.2}>
    <path d="M18 13.5V18a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h4.5" />
    <polyline points="14 4 20 4 20 10" />
    <line x1="11" y1="13" x2="20" y2="4" />
  </StrokeIcon>
);

export const RefreshIcon = (p: IconProps) => (
  <StrokeIcon {...p} strokeWidth={2}>
    <path d="M4 11a8 8 0 0 1 14.3-4.3" />
    <polyline points="19 3 19 7.5 14.5 7.5" />
    <path d="M20 13a8 8 0 0 1-14.3 4.3" />
    <polyline points="5 21 5 16.5 9.5 16.5" />
  </StrokeIcon>
);

export const MapPinIcon = (p: IconProps) => (
  <StrokeIcon {...p}>
    <path d="M12 21.5s-7-6.2-7-11.5a7 7 0 0 1 14 0c0 5.3-7 11.5-7 11.5z" />
    <circle cx="12" cy="10" r="2.5" />
  </StrokeIcon>
);

export const UsersIcon = (p: IconProps) => (
  <StrokeIcon {...p}>
    <circle cx="9" cy="8" r="3.2" />
    <path d="M3 19c0-3.3 2.7-5.5 6-5.5s6 2.2 6 5.5" />
    <circle cx="17" cy="9" r="2.4" />
    <path d="M16.5 13.6c2.6.2 4.5 2.1 4.5 4.9" />
  </StrokeIcon>
);

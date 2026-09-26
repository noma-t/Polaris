import { useState, type MouseEvent } from "react";
import { toVrchatImageSrc } from "../lib/auth";
import { PolarisLogo } from "./PolarisLogo";
import { SidebarToggleIcon, SignOutIcon } from "./icons";
import { NAV_ITEMS, type NavScreen } from "./navigation";

interface SidebarProps {
  activeScreen: NavScreen | null;
  username: string;
  /** VRChat 側のアイコン URL。null ならプレースホルダーを表示する */
  userIconUrl: string | null;
  /** 展開表示したいか。canExpand が false の間も保持する */
  isOpenPreferred: boolean;
  onOpenPreferredChange: (isOpen: boolean) => void;
  /** false の間は幅不足のため rail (折りたたみ) 表示に固定し、展開操作も無効にする */
  canExpand: boolean;
  /** 右上に通知 badge (ドット) を付ける nav item */
  badgedScreens: NavScreen[];
  onNavigate: (screen: NavScreen) => void;
  onSignOut: () => void;
}

export function Sidebar({
  activeScreen,
  username,
  userIconUrl,
  isOpenPreferred,
  onOpenPreferredChange,
  canExpand,
  badgedScreens,
  onNavigate,
  onSignOut,
}: SidebarProps) {
  const isOpen = isOpenPreferred && canExpand;
  const isExpandable = canExpand && !isOpen;
  const [userMenuPos, setUserMenuPos] = useState<{ left: number; bottom: number } | null>(null);
  /** 読み込みに失敗したアイコン URL。一致する間はプレースホルダーに戻す */
  const [failedIconUrl, setFailedIconUrl] = useState<string | null>(null);
  const isUserIconVisible = userIconUrl !== null && userIconUrl !== failedIconUrl;

  const toggleUserMenu = (e: MouseEvent<HTMLButtonElement>) => {
    const rect = e.currentTarget.getBoundingClientRect();
    setUserMenuPos((pos) => (pos ? null : { left: rect.left, bottom: window.innerHeight - rect.top + 8 }));
  };

  return (
    <>
      <aside className={`sidebar ${isOpen ? "is-open" : "is-collapsed"} ${isExpandable ? "is-expandable" : ""}`}>
        <div className="sidebar-header">
          <button
            className="sidebar-logo-button"
            onClick={() => isExpandable && onOpenPreferredChange(true)}
            aria-label={isExpandable ? "Open sidebar" : "Polaris"}
            title={isExpandable ? "Open sidebar" : "Polaris"}
          >
            <PolarisLogo className="sidebar-logo-mark" />
            <SidebarToggleIcon className="sidebar-logo-toggle-icon" size={26} />
          </button>
          <span className="sidebar-title">Polaris</span>
          <button
            className="sidebar-close-button"
            onClick={() => onOpenPreferredChange(false)}
            aria-label="Close sidebar"
            title="Close sidebar"
            tabIndex={isOpen ? 0 : -1}
          >
            <SidebarToggleIcon size={24} />
          </button>
        </div>

        <nav className="sidebar-nav">
          {NAV_ITEMS.map(({ screen, label, Icon }) => {
            const isActive = screen === activeScreen;
            const hasBadge = badgedScreens.includes(screen);
            return (
              <button
                key={screen}
                className={`sidebar-nav-item ${isActive ? "is-active" : ""}`}
                onClick={() => onNavigate(screen)}
                aria-label={label}
                aria-current={isActive ? "page" : undefined}
                title={label}
              >
                {isActive && <span className="sidebar-nav-indicator" />}
                <span className="sidebar-nav-icon">
                  <Icon size={24} />
                  {hasBadge && <span className="sidebar-nav-badge" aria-hidden="true" />}
                </span>
                <span className="sidebar-label">{label}</span>
              </button>
            );
          })}
        </nav>

        <div className="sidebar-footer">
          <div className="sidebar-divider" />
          <button
            className={`sidebar-user-button ${userMenuPos ? "is-menu-open" : ""}`}
            onClick={toggleUserMenu}
            aria-label={username}
            title={username}
            aria-expanded={!!userMenuPos}
          >
            <span className="sidebar-user-avatar">
              {isUserIconVisible && (
                <img
                  className="sidebar-user-avatar-image"
                  src={toVrchatImageSrc(userIconUrl)}
                  alt=""
                  draggable={false}
                  onError={() => setFailedIconUrl(userIconUrl)}
                />
              )}
            </span>
            <span className="sidebar-label sidebar-username">{username}</span>
          </button>
        </div>
      </aside>

      {userMenuPos && (
        <>
          <div className="user-menu-backdrop" onClick={() => setUserMenuPos(null)} />
          <div className="user-menu" role="menu" style={{ left: userMenuPos.left, bottom: userMenuPos.bottom }}>
            <button
              className="user-menu-item"
              role="menuitem"
              onClick={() => {
                setUserMenuPos(null);
                onSignOut();
              }}
            >
              <SignOutIcon size={22} />
              Sign Out
            </button>
          </div>
        </>
      )}
    </>
  );
}

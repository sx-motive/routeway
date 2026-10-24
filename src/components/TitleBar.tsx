import { FolderOpen, Minus, MoonStar, Sun, X } from "lucide-react";
import { Button } from "./ui/button";
import { useTheme } from "../theme/ThemeProvider";
import { hideWindow, minimizeWindow, revealConfig } from "../lib/ipc";
import { useI18n } from "../i18n";
import type { Key } from "../i18n/en";
import appIcon from "../assets/brand/64x64.png";

export type Tab = "home" | "settings";

const TABS: { id: Tab; label: Key }[] = [
  { id: "home", label: "tab.home" },
  { id: "settings", label: "tab.settings" },
];

export function TitleBar({ tab, onTabChange }: { tab: Tab; onTabChange: (tab: Tab) => void }) {
  const { theme, setTheme } = useTheme();
  const { t } = useI18n();

  return (
    <div data-tauri-drag-region className="flex h-9 shrink-0 items-center px-2 select-none">
      <span data-tauri-drag-region className="flex shrink-0 items-center pr-3 pl-1">
        <img src={appIcon} alt="Routeway" width={20} height={20} draggable={false}
          data-tauri-drag-region className="brand-logo size-5 shrink-0" />
      </span>
      <nav className="flex h-full items-stretch">
        {TABS.map(({ id, label }) => (
          <button
            key={id}
            type="button"
            onClick={() => onTabChange(id)}
            className={[
              "border-b px-2.5 text-xs transition-colors outline-none focus-visible:bg-bg-elevated",
              tab === id
                ? "border-accent text-fg"
                : "border-transparent text-fg-muted/70 hover:text-fg",
            ].join(" ")}
          >
            {t(label)}
          </button>
        ))}
      </nav>
      <div className="flex-1" data-tauri-drag-region />
      <Button
        variant="ghost"
        size="icon"
        className="h-7 w-7 text-fg-muted/50"
        aria-label={t("titlebar.openConfig")}
        title={t("titlebar.openConfig")}
        onClick={() => void revealConfig()}
      >
        <FolderOpen className="h-3.5 w-3.5" />
      </Button>
      <Button
        variant="ghost"
        size="icon"
        className="h-7 w-7 text-fg-muted/50"
        aria-label={t("titlebar.theme")}
        title={t("titlebar.theme")}
        onClick={() => setTheme(theme === "light" ? "dark" : "light")}
      >
        {theme === "light" ? (
          <MoonStar className="h-3.5 w-3.5" />
        ) : (
          <Sun className="h-3.5 w-3.5" />
        )}
      </Button>
      <Button
        variant="ghost"
        size="icon"
        className="h-7 w-7 text-fg-muted/50"
        aria-label={t("titlebar.minimize")}
        title={t("titlebar.minimize")}
        onClick={() => void minimizeWindow()}
      >
        <Minus className="h-3.5 w-3.5" />
      </Button>
      <Button
        variant="ghost"
        size="icon"
        className="h-7 w-7 text-fg-muted/50"
        aria-label={t("titlebar.close")}
        title={t("titlebar.closeHint")}
        onClick={() => void hideWindow()}
      >
        <X className="h-3.5 w-3.5" />
      </Button>
    </div>
  );
}

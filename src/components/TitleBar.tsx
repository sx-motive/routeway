import { FolderOpen, Minus, MoonStar, Sun } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Button } from "./ui/button";
import { useTheme } from "../theme/ThemeProvider";
import { revealConfig } from "../lib/ipc";

export function TitleBar() {
  const { theme, setTheme } = useTheme();

  return (
    <div data-tauri-drag-region className="flex h-9 shrink-0 items-center px-2 select-none">
      <span className="pl-1 text-xs text-fg-muted/50">Routeway</span>
      <div className="flex-1" data-tauri-drag-region />
      <Button
        variant="ghost"
        size="icon"
        className="h-7 w-7 text-fg-muted/50"
        aria-label="Открыть каталог конфига"
        title="Открыть каталог конфига"
        onClick={() => void revealConfig()}
      >
        <FolderOpen className="h-3.5 w-3.5" />
      </Button>
      <Button
        variant="ghost"
        size="icon"
        className="h-7 w-7 text-fg-muted/50"
        aria-label="Переключить тему"
        title="Переключить тему"
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
        aria-label="Свернуть в трей"
        title="Свернуть в трей — маршрутизация продолжит работать"
        onClick={() => void getCurrentWindow().hide()}
      >
        <Minus className="h-3.5 w-3.5" />
      </Button>
    </div>
  );
}

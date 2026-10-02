import { useEffect, useRef, useState } from "react";
import { ArrowsClockwise, CheckCircle, DownloadSimple, WarningCircle } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  inspectBalatroModSetup,
  installLatestBalatroModSetup,
  type BalatroModSetupInstallResult,
  type BalatroModSetupStatus,
} from "@/lib/balatro/balatro-mod-setup";

export interface BalatroModSetupProps {
  appdataPath?: string;
  gamePath?: string;
  warningOnly?: boolean;
  onPathsResolved?: (paths: { appdataPath: string | null; gamePath: string | null }) => void;
  onStatusChange?: (status: BalatroModSetupStatus | null) => void;
  onInstallingChange?: (isInstalling: boolean) => void;
  onOpenSettings?: () => void;
}

const errorMessage = (error: unknown, fallback = "Could not check Balatro mod support. Try again.") => error instanceof Error
  ? error.message
  : typeof error === "string" ? error : fallback;

export default function BalatroModSetup({
  appdataPath = "",
  gamePath = "",
  warningOnly = false,
  onPathsResolved,
  onStatusChange,
  onInstallingChange,
  onOpenSettings,
}: BalatroModSetupProps) {
  const [status, setStatus] = useState<BalatroModSetupStatus | null>(null);
  const [isChecking, setIsChecking] = useState(true);
  const [isInstalling, setIsInstalling] = useState(false);
  const [error, setError] = useState("");
  const [installed, setInstalled] = useState<BalatroModSetupInstallResult | null>(null);
  const [refresh, setRefresh] = useState(0);
  const mountedRef = useRef(true);
  const installingRef = useRef(false);
  const inspectionRef = useRef(0);
  const installedRef = useRef<BalatroModSetupInstallResult | null>(null);
  const installationErrorRef = useRef(false);
  const lastPathsKeyRef = useRef("");
  const pathsKey = JSON.stringify([appdataPath.trim(), gamePath.trim()]);
  const pathsKeyRef = useRef(pathsKey);
  pathsKeyRef.current = pathsKey;
  const callbacksRef = useRef({ onPathsResolved, onStatusChange, onInstallingChange });
  callbacksRef.current = { onPathsResolved, onStatusChange, onInstallingChange };

  useEffect(() => {
    mountedRef.current = true;
    return () => { mountedRef.current = false; inspectionRef.current += 1; };
  }, []);

  useEffect(() => {
    const inspection = ++inspectionRef.current;
    const changedPaths = lastPathsKeyRef.current !== pathsKey;
    lastPathsKeyRef.current = pathsKey;
    if (changedPaths) {
      const previousInstall = installedRef.current;
      const usesInstalledPaths = previousInstall
        && (!appdataPath.trim() || appdataPath.trim() === previousInstall.status.appdataPath)
        && (!gamePath.trim() || gamePath.trim() === previousInstall.status.gamePath);
      if (!usesInstalledPaths) {
        installedRef.current = null;
        setInstalled(null);
        setStatus(null);
        callbacksRef.current.onStatusChange?.(null);
      }
      setError("");
      installationErrorRef.current = false;
    }
    if (isInstalling) return;
    setIsChecking(true);
    const timer = setTimeout(async () => {
      try {
        const next = await inspectBalatroModSetup({ appdataPath, gamePath });
        if (!mountedRef.current || inspection !== inspectionRef.current) return;
        const previousInstall = installedRef.current;
        if (previousInstall && (previousInstall.status.appdataPath !== next.appdataPath
          || previousInstall.status.gamePath !== next.gamePath)) {
          installedRef.current = null;
          setInstalled(null);
        }
        setStatus(next);
        if (!installationErrorRef.current) setError("");
        callbacksRef.current.onStatusChange?.(next);
        callbacksRef.current.onPathsResolved?.({ appdataPath: next.appdataPath, gamePath: next.gamePath });
      } catch (failure) {
        if (!mountedRef.current || inspection !== inspectionRef.current) return;
        setStatus(null);
        setError(errorMessage(failure));
        callbacksRef.current.onStatusChange?.(null);
      } finally {
        if (mountedRef.current && inspection === inspectionRef.current) setIsChecking(false);
      }
    }, 300);
    return () => { clearTimeout(timer); inspectionRef.current += 1; };
  }, [appdataPath, gamePath, pathsKey, isInstalling, refresh]);

  const handleInstall = async () => {
    if (installingRef.current || isChecking || !status?.canInstall) return;
    installingRef.current = true;
    setIsInstalling(true);
    setError("");
    installationErrorRef.current = false;
    callbacksRef.current.onInstallingChange?.(true);
    const requestedPaths = pathsKeyRef.current;
    try {
      const result = await installLatestBalatroModSetup({ appdataPath, gamePath });
      if (!mountedRef.current || requestedPaths !== pathsKeyRef.current) return;
      installedRef.current = result;
      setInstalled(result);
      setStatus(result.status);
      callbacksRef.current.onStatusChange?.(result.status);
      callbacksRef.current.onPathsResolved?.({ appdataPath: result.status.appdataPath, gamePath: result.status.gamePath });
    } catch (failure) {
      if (mountedRef.current && requestedPaths === pathsKeyRef.current) {
        installationErrorRef.current = true;
        setError(errorMessage(failure, "Could not install mod support. Try again."));
      }
    } finally {
      installingRef.current = false;
      callbacksRef.current.onInstallingChange?.(false);
      if (mountedRef.current) setIsInstalling(false);
    }
  };

  const ready = !!status?.steamoddedInstalled && !!status?.lovelyInstalled;
  const steamoddedGuide = status?.platform === "macos"
    ? "https://docs.smods.dev/Installation/Installing%20Steamodded%20mac/"
    : status?.platform === "linux"
      ? "https://docs.smods.dev/Installation/Installing%20Steamodded%20linux/"
      : "https://github.com/Steamodded/smods/wiki";
  const openGuide = async (url: string) => {
    try {
      await openUrl(url);
    } catch {
      if (mountedRef.current) setError("Could not open the installation guide. Try opening its link in your browser.");
    }
  };
  if (warningOnly && ready && !installed && !error && !isInstalling) return null;
  const icon = isChecking || isInstalling ? <ArrowsClockwise className="h-4 w-4 shrink-0 animate-spin text-muted-foreground" />
    : installed || ready ? <CheckCircle className="h-4 w-4 shrink-0 text-emerald-500" />
      : <WarningCircle className="h-4 w-4 shrink-0 text-amber-500" />;

  return (
    <div aria-busy={isChecking || isInstalling} className={`space-y-3 rounded-md border p-3 ${warningOnly && !ready && !installed ? "border-amber-500/25 bg-amber-500/5" : "border-border"}`}>
      <div className="flex items-center gap-2 text-sm font-medium">
        {icon}
        {installed ? "Mod support installed" : "Balatro mod support"}
      </div>
      {isInstalling ? (
        <p role="status" className="text-xs text-muted-foreground">Downloading and installing the latest Steamodded and Lovely…</p>
      ) : installed ? (
        <div role="status" className="space-y-1 text-xs text-muted-foreground">
          <p>Steamodded {installed.steamoddedVersion} and Lovely {installed.lovelyVersion} installed.</p>
          {installed.backups.length > 0 && <details>
            <summary className="cursor-pointer">Previous files were backed up</summary>
            <ul className="mt-1 space-y-1 text-[11px]">
              {installed.backups.map((path) => <li key={path} className="break-all">{path}</li>)}
            </ul>
          </details>}
        </div>
      ) : isChecking ? (
        <p role="status" className="text-xs text-muted-foreground">Checking Steamodded and Lovely…</p>
      ) : status ? (
        <div className="space-y-2 text-xs">
          {!ready && <p className="text-muted-foreground">Steamodded and Lovely are needed to play your mod in Balatro.</p>}
          <div className="flex flex-wrap gap-x-4 gap-y-1">
            <span>Steamodded: <span className="text-muted-foreground">{status.steamoddedInstalled ? `Installed${status.steamoddedVersion ? ` (${status.steamoddedVersion})` : ""}` : "Missing"}</span></span>
            <span>Lovely: <span className="text-muted-foreground">{status.lovelyInstalled ? `Installed${status.lovelyVersion ? ` (${status.lovelyVersion})` : ""}` : "Missing"}</span></span>
          </div>
          {status.issues.map((issue, index) => <p key={index} className="text-muted-foreground">{issue}</p>)}
        </div>
      ) : null}
      {error && <p role="alert" className="break-words text-xs text-destructive">{error}</p>}
      {!isChecking && !isInstalling && !status?.canInstall && !installed && (
        <p className="flex flex-wrap gap-x-3 gap-y-1 text-xs text-muted-foreground">
          <a href={steamoddedGuide} target="_blank" rel="noopener noreferrer"
            className="text-primary hover:underline" onClick={(event) => { event.preventDefault(); void openGuide(steamoddedGuide); }}>Steamodded installation guide</a>
          <a href="https://github.com/ethangreen-dev/lovely-injector#manual-installation" target="_blank" rel="noopener noreferrer"
            className="text-primary hover:underline" onClick={(event) => { event.preventDefault(); void openGuide("https://github.com/ethangreen-dev/lovely-injector#manual-installation"); }}>Lovely installation guide</a>
        </p>
      )}
      {(!installed || !warningOnly) && (
        <>
          {status?.canInstall && !isInstalling && <p className="text-[11px] text-muted-foreground">Close Balatro first. Setup downloads the latest official Steamodded and Lovely and backs up files it replaces.</p>}
          <div className="flex flex-wrap items-center gap-2">
            <Button type="button" size="sm" variant={ready ? "outline" : "default"}
              disabled={isChecking || isInstalling || !status?.canInstall} onClick={handleInstall}
              icon={<DownloadSimple className="h-4 w-4" />}>
              {isInstalling ? "Installing…" : ready ? "Update mod support" : "Set up mod support"}
            </Button>
            {!isInstalling && <Button type="button" size="sm" variant="ghost" disabled={isChecking}
              onClick={() => {
                installationErrorRef.current = false;
                installedRef.current = null;
                setInstalled(null);
                setError("");
                setRefresh((value) => value + 1);
              }}
              icon={<ArrowsClockwise className="h-4 w-4" />}>Check again</Button>}
            {warningOnly && onOpenSettings && !isChecking && !isInstalling && (!status?.canInstall || !!error) && (
              <Button type="button" size="sm" variant="link" onClick={onOpenSettings}>Open Settings</Button>
            )}
          </div>
        </>
      )}
    </div>
  );
}

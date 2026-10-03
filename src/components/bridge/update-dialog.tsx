import { useEffect, useState, useMemo } from "react";
import { ArrowRight, DownloadSimple } from "@phosphor-icons/react";
import { motion } from "framer-motion";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import {
  onUpdateAvailable,
  performUpdate,
  type UpdateInfo,
} from "@/lib/services/release-updater";

export function UpdateDialog() {
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
  const [isUpdating, setIsUpdating] = useState(false);
  const [updateError, setUpdateError] = useState<string | null>(null);

  const confettiPieces = useMemo(
    () =>
      Array.from({ length: 40 }, (_, i) => ({
        id: i,
        left: Math.random() * 100,
        delay: Math.random() * 2,
        duration: 1.5 + Math.random() * 2,
        color: [
          "bg-primary",
          "bg-accent",
          "bg-blue-500",
          "bg-purple-500",
          "bg-pink-500",
        ][Math.floor(Math.random() * 5)],
      })),
    [],
  );

  useEffect(() => {
    return onUpdateAvailable((info) => {
      setUpdateError(null);
      setUpdateInfo(info);
    });
  }, []);

  useEffect(() => {
    const handleShowDialog = (event: Event) => {
      const customEvent = event as CustomEvent<UpdateInfo | undefined>;
      const detail = customEvent.detail;
      setUpdateError(null);
      setUpdateInfo(
        detail ?? {
          currentVersion: "2.0.0-beta",
          latestVersion: "2.1.0",
          channel: "stable",
          asset: null,
          releaseUrl: "",
          installation: "automatic",
        },
      );
    };

    window.addEventListener("show-update-dialog", handleShowDialog);
    return () => {
      window.removeEventListener("show-update-dialog", handleShowDialog);
    };
  }, []);

  if (!updateInfo) return null;

  const handleUpdate = async (manual = false) => {
    if (!updateInfo.releaseUrl) {
      setUpdateError("This is a preview of the update dialog. No download is attached.");
      return;
    }

    setIsUpdating(true);
    setUpdateError(null);
    const update = manual
      ? { ...updateInfo, installation: "manual" as const, asset: null }
      : updateInfo;
    try {
      await performUpdate(update);
      if (update.installation === "manual") {
        setIsUpdating(false);
        setUpdateInfo(null);
      }
    } catch (error) {
      console.error("Update failed", error);
      setIsUpdating(false);
      setUpdateError(typeof error === "string" ? error :
        error instanceof Error ? error.message : "The update could not be started. Please try again.");
    }
  };

  return (
    <Dialog
      open={!!updateInfo}
      onOpenChange={(open) => {
        if (!open && !isUpdating) setUpdateInfo(null);
      }}
    >
      <DialogContent
        overlayContent={
          <div className="absolute inset-0 overflow-hidden pointer-events-none">
            {confettiPieces.map((piece) => (
              <motion.div
                key={`confetti-${piece.id}`}
                className={`absolute top-0 h-3 w-1.5 rounded-full ${piece.color}`}
                style={{ left: `${piece.left}%` }}
                initial={{ opacity: 0, y: -20, rotate: 0 }}
                animate={{
                  opacity: [0, 1, 1, 0],
                  y: ["-5vh", "20vh", "60vh", "105vh"],
                  rotate: [0, 180, 360, 720],
                }}
                transition={{
                  duration: piece.duration,
                  repeat: Number.POSITIVE_INFINITY,
                  repeatDelay: Math.random() * 2,
                  delay: piece.delay,
                  ease: "linear",
                }}
              />
            ))}
          </div>
        }
        className="overflow-hidden border border-border/60 bg-card p-0 sm:max-w-md outline-none"
        onPointerDownOutside={(e) => {
          if (isUpdating) e.preventDefault();
        }}
        onEscapeKeyDown={(e) => {
          if (isUpdating) e.preventDefault();
        }}
      >
        <div className="relative p-6 pb-2">
          <DialogHeader className="text-left space-y-2">
            <div className="flex items-center gap-2">
              <DownloadSimple className="h-5 w-5 shrink-0 text-muted-foreground" weight="regular" />
              <DialogTitle className="text-xl font-semibold tracking-tight">
                Update Available
              </DialogTitle>
            </div>
            <DialogDescription className="text-sm text-muted-foreground">
              A new <strong className="text-foreground capitalize">{updateInfo.channel}</strong> version of Joker Forge is available.
            </DialogDescription>
          </DialogHeader>
        </div>

        <div className="px-6 py-4">
          <div className="flex flex-col gap-1.5 rounded-lg p-3">
            <div className="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
              Version Details
            </div>
            <div className="flex flex-wrap items-start gap-2 font-mono text-sm">
              <span
                className="break-all text-muted-foreground"
                title={updateInfo.currentVersion}
              >
                {updateInfo.currentVersion}
              </span>
              <ArrowRight
                className="h-3.5 w-3.5 shrink-0 text-muted-foreground/60"
                weight="bold"
              />
              <span
                className="break-all font-medium text-foreground"
                title={updateInfo.latestVersion}
              >
                {updateInfo.latestVersion}
              </span>
            </div>
          </div>
          <p className="mt-3 text-sm text-muted-foreground">
            {updateInfo.installation === "manual"
              ? "Download the package for your computer from the release page, then close Joker Forge and follow the package’s installation steps."
              : "Your project changes will be saved before Joker Forge closes. The app will reopen after the update finishes."}
          </p>
          {updateError && (
            <div className="mt-3">
              <p role="alert" className="text-sm text-destructive">{updateError}</p>
              {updateInfo.installation === "automatic" && updateInfo.releaseUrl && (
                <Button variant="link" className="px-0" disabled={isUpdating}
                  onClick={() => void handleUpdate(true)}>
                  Download manually
                </Button>
              )}
            </div>
          )}
        </div>

        <DialogFooter className="flex w-full flex-row gap-2 border-t border-border/40 bg-muted/10 p-4">
          <Button
            variant="outline"
            className="w-1/2 cursor-pointer"
            type="button"
            onClick={() => setUpdateInfo(null)}
            disabled={isUpdating}
          >
            Remind Me Later
          </Button>
          <Button
            className="w-1/2 cursor-pointer"
            type="button"
            onClick={() => void handleUpdate()}
            disabled={isUpdating}
          >
            {isUpdating
              ? updateInfo.installation === "manual" ? "Opening..." : "Updating..."
              : updateInfo.installation === "manual" ? "Download Update"
              : updateError ? "Retry Update" : "Install Now"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

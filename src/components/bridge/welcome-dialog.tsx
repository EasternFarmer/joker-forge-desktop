import { useId, useRef, useState } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ArrowUpRight, MessageSquare, Sparkles } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";

const WELCOME_DISMISSED_KEY = "jokerforge-welcome-dismissed";
const ISSUES_URL = "https://github.com/Jaydchw/joker-forge-desktop/issues/new/choose";
let dismissedInMemory = false;

export function shouldShowWelcome(): boolean {
  if (dismissedInMemory) return false;
  try {
    return window.localStorage.getItem(WELCOME_DISMISSED_KEY) !== "true";
  } catch {
    return true;
  }
}

interface WelcomeDialogProps {
  open: boolean;
  onDismiss: () => void;
}

export function WelcomeDialog({ open, onDismiss }: WelcomeDialogProps) {
  const descriptionId = useId();
  const feedbackId = useId();
  const confirmButton = useRef<HTMLButtonElement>(null);
  const [linkError, setLinkError] = useState(false);

  const dismiss = () => {
    dismissedInMemory = true;
    try {
      window.localStorage.setItem(WELCOME_DISMISSED_KEY, "true");
    } catch {
    }
    onDismiss();
  };

  const openIssues = async () => {
    setLinkError(false);
    try {
      if (isTauri()) {
        await openUrl(ISSUES_URL);
      } else {
        window.open(ISSUES_URL, "_blank", "noopener,noreferrer");
      }
      dismiss();
    } catch {
      setLinkError(true);
    }
  };

  return (
    <Dialog open={open} onOpenChange={(nextOpen) => { if (!nextOpen) dismiss(); }}>
      <DialogContent
        className="sm:max-w-md"
        aria-describedby={`${descriptionId} ${feedbackId}`}
        onOpenAutoFocus={(event) => {
          event.preventDefault();
          confirmButton.current?.focus();
        }}
      >
        <DialogHeader className="text-left">
          <div className="mb-2 flex size-11 items-center justify-center rounded-xl bg-primary/10 text-primary">
            <Sparkles className="size-6" aria-hidden="true" />
          </div>
          <DialogTitle className="pr-4 text-xl leading-snug">
            Welcome to Joker Forge Desktop!
          </DialogTitle>
          <DialogDescription id={descriptionId} className="leading-relaxed">
            This is the desktop version of the web app Joker Forge.
            Please note that it is still in development. 
          </DialogDescription>
        </DialogHeader>

        <div className="flex gap-3 rounded-lg border border-border bg-muted/30 p-4">
          <MessageSquare className="mt-0.5 size-4 shrink-0 text-primary" aria-hidden="true" />
          <div className="space-y-1">
            <p className="text-sm font-medium">Got an idea or found a bug?</p>
            <p id={feedbackId} className="text-sm leading-relaxed text-muted-foreground">
              Please add suggestions and report issues on the GitHub repository. It will greatly help improve Joker Forge.
            </p>
          </div>
        </div>

        {linkError && (
          <p role="alert" className="text-sm text-destructive">
            Could not open GitHub. You can find us at github.com/Jaydchw/joker-forge-desktop.
          </p>
        )}

        <DialogFooter>
          <Button variant="outline" onClick={() => { void openIssues(); }}>
            Suggestions &amp; issues
            <ArrowUpRight aria-hidden="true" />
          </Button>
          <Button ref={confirmButton} onClick={dismiss}>Got it</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

import { useInstallPrompt } from "../../state/installPrompt";
import { isIOS, isStandaloneDisplay } from "../../utils/platform";
import { Button } from "../ui/Button";

/**
 * Install nudge shown on Home and Settings (`04-frontend-architecture.md`
 * §14.4): a real "Install" button where `beforeinstallprompt` fired, else a
 * dismissible static banner on iOS (which never fires that event). Renders
 * nothing once installed (standalone) or when neither applies.
 */
export function InstallPwaPrompt() {
  const canInstall = useInstallPrompt((s) => s.event !== null);
  const promptInstall = useInstallPrompt((s) => s.promptInstall);
  const iosBannerDismissed = useInstallPrompt((s) => s.iosBannerDismissed);
  const dismissIosBanner = useInstallPrompt((s) => s.dismissIosBanner);

  if (isStandaloneDisplay()) return null;

  if (canInstall) {
    return (
      <div className="flex items-center justify-between gap-3 rounded-2xl border border-line bg-white p-3 shadow-soft">
        <span className="text-sm font-semibold">
          Install OpenNewsletter for the best experience.
        </span>
        <Button size="sm" onClick={() => void promptInstall()}>
          Install
        </Button>
      </div>
    );
  }

  if (isIOS() && !iosBannerDismissed) {
    return (
      <div className="flex items-center justify-between gap-3 rounded-2xl border border-line bg-white p-3 shadow-soft">
        <span className="text-sm">
          Add OpenNewsletter to your Home Screen: tap <span aria-hidden="true">&#x2191;</span>{" "}
          Share, then &quot;Add to Home Screen&quot;.
        </span>
        <button
          onClick={dismissIosBanner}
          aria-label="Dismiss install banner"
          className="shrink-0 text-inkmuted"
        >
          ✕
        </button>
      </div>
    );
  }

  return null;
}

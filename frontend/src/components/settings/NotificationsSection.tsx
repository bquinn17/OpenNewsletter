import { useEffect, useState } from "react";
import {
  disablePush,
  ensurePushSubscription,
  getLocalPushState,
  isPushSupported,
  type LocalPushState,
} from "../../pwa/pushSetup";
import { isIOS, isStandaloneDisplay } from "../../utils/platform";
import { summarizeUserAgent } from "../../utils/userAgent";
import { usePushPreferences, usePushSubscriptions } from "../../api/queries";
import {
  useRemovePushSubscription,
  useSendTestPush,
  usePutPushPreference,
} from "../../api/mutations";
import { useToasts } from "../../state/toast";
import { formatRelative } from "../../utils/dates";
import { Button } from "../ui/Button";
import { Pill } from "../ui/Pill";
import type { components } from "../../types/api";

type S = components["schemas"];

/**
 * Settings "Notifications" section (`07-notifications.md` §11.1/§14, M11
 * D11). Three top-level states — iOS-not-installed, unsupported, and the
 * full control surface (master toggle, test push, devices, per-group prefs)
 * — plus a denied-permission sub-state of the last one.
 */
export function NotificationsSection({ config }: { config: S["ConfigResponse"] }) {
  const pushToast = useToasts((s) => s.push);

  const [localState, setLocalState] = useState<LocalPushState | null>(null);
  const [refreshKey, setRefreshKey] = useState(0);
  const [toggleBusy, setToggleBusy] = useState(false);
  const [testResults, setTestResults] = useState<S["PushTestResult"][] | null>(null);

  const supported = isPushSupported();
  const iosNotInstalled = isIOS() && !isStandaloneDisplay();

  useEffect(() => {
    if (!supported || iosNotInstalled) return;
    let cancelled = false;
    void getLocalPushState().then((state) => {
      if (!cancelled) setLocalState(state);
    });
    return () => {
      cancelled = true;
    };
  }, [supported, iosNotInstalled, refreshKey]);

  const subscriptionsQuery = usePushSubscriptions();
  const preferencesQuery = usePushPreferences();
  const removeSub = useRemovePushSubscription();
  const sendTest = useSendTestPush();

  if (iosNotInstalled) {
    return (
      <SectionShell>
        <p className="p-4 text-sm text-inkmuted">
          Add OpenNewsletter to your Home Screen to get notifications.
        </p>
      </SectionShell>
    );
  }

  if (!supported) {
    return (
      <SectionShell>
        <p className="p-4 text-sm text-inkmuted">
          Push isn&apos;t supported in this browser. The PWA still works — install it for the best
          experience.
        </p>
      </SectionShell>
    );
  }

  const denied = localState?.permission === "denied";
  const enabled = localState?.permission === "granted" && !!localState.endpoint;
  const thisDeviceEndpoint = localState?.endpoint ?? null;

  async function handleToggle(next: boolean) {
    setToggleBusy(true);
    try {
      if (next) {
        const result = await ensurePushSubscription(config.vapidPublicKey, { prompt: true });
        if (result.status === "denied") {
          pushToast("Notifications were blocked — enable them in your browser settings.", "error");
        }
      } else {
        await disablePush();
      }
    } catch {
      pushToast("Couldn't update push notifications — try again.", "error");
    } finally {
      setToggleBusy(false);
      setRefreshKey((k) => k + 1);
      void subscriptionsQuery.refetch();
    }
  }

  async function handleSendTest() {
    setTestResults(null);
    try {
      const { results } = await sendTest.mutateAsync();
      setTestResults(results);
    } catch {
      pushToast("Couldn't send a test push.", "error");
    }
  }

  async function handleRemoveDevice(endpoint: string) {
    await removeSub.mutateAsync(endpoint);
    if (endpoint === thisDeviceEndpoint) {
      try {
        const registration = await navigator.serviceWorker.ready;
        const subscription = await registration.pushManager.getSubscription();
        if (subscription && subscription.endpoint === endpoint) await subscription.unsubscribe();
      } catch {
        // Best effort — the server-side row is already gone either way.
      }
      setRefreshKey((k) => k + 1);
    }
  }

  const devices = subscriptionsQuery.data?.items ?? [];

  return (
    <SectionShell>
      <div className="flex items-center gap-3 p-4">
        <div className="flex-1">
          <div className="text-sm font-semibold">Push notifications</div>
          {denied && (
            <p className="mt-0.5 text-xs text-coral">
              Blocked for this browser — enable them in your browser settings, then reload.
            </p>
          )}
        </div>
        <button
          onClick={() => void handleToggle(!enabled)}
          disabled={toggleBusy || denied}
          className={`toggle ${enabled ? "on" : ""}`}
          aria-label="Toggle push notifications"
        />
      </div>

      <div className="p-4">
        <Button
          type="button"
          size="sm"
          variant="soft"
          disabled={devices.length === 0 || sendTest.isPending}
          onClick={() => void handleSendTest()}
        >
          {sendTest.isPending ? "Sending…" : "Send test push"}
        </Button>
        {testResults && (
          <ul className="mt-2 space-y-1 text-xs text-inkmuted">
            {testResults.length === 0 && <li>No devices to send to yet.</li>}
            {testResults.map((r) => (
              <li key={r.subscriptionId}>
                {summarizeUserAgent(r.userAgent)}: {r.outcome}
                {r.statusCode ? ` (${r.statusCode})` : ""}
              </li>
            ))}
          </ul>
        )}
      </div>

      <div className="p-4">
        <div className="mb-2 text-xs font-bold uppercase tracking-widest text-inkmuted">
          My devices
        </div>
        {devices.length === 0 && (
          <p className="text-sm text-inkmuted">No devices subscribed yet.</p>
        )}
        <div className="space-y-2">
          {devices.map((d) => (
            <div key={d.subscriptionId} className="flex items-center gap-2 text-sm">
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <span className="truncate">{summarizeUserAgent(d.userAgent)}</span>
                  {d.endpoint === thisDeviceEndpoint && <Pill tone="grape">This device</Pill>}
                </div>
                <div className="text-xs text-inkmuted">
                  {d.lastSuccessAt ? `Last success ${formatRelative(d.lastSuccessAt)}` : "Never"}
                </div>
              </div>
              <button
                type="button"
                onClick={() => void handleRemoveDevice(d.endpoint)}
                className="text-xs font-semibold text-coral"
              >
                Remove
              </button>
            </div>
          ))}
        </div>
      </div>

      <div className="p-4">
        <div className="mb-2 text-xs font-bold uppercase tracking-widest text-inkmuted">
          Per-group notifications
        </div>
        <div className="space-y-3">
          {config.memberships.map((m) => (
            <GroupPrefRow
              key={m.groupId}
              groupId={m.groupId}
              groupName={m.groupName}
              pref={preferencesQuery.data?.items.find((p) => p.groupId === m.groupId)}
            />
          ))}
        </div>
        <p className="mt-3 text-xs text-inkmuted">New-edition notifications are always on.</p>
      </div>
    </SectionShell>
  );
}

function SectionShell({ children }: { children: React.ReactNode }) {
  return (
    <div className="mt-5 px-5">
      <div className="mb-2 ml-1 text-xs font-bold uppercase tracking-widest text-inkmuted">
        Notifications
      </div>
      <div className="divide-y divide-line rounded-3xl border border-line bg-white shadow-soft">
        {children}
      </div>
    </div>
  );
}

function GroupPrefRow({
  groupId,
  groupName,
  pref,
}: {
  groupId: string;
  groupName: string;
  pref: S["PushPreferenceResponse"] | undefined;
}) {
  const putPref = usePutPushPreference(groupId);
  const cycleOpen = pref?.cycleOpen ?? true;
  const deadlineReminders = pref?.deadlineReminders ?? true;

  return (
    <div>
      <div className="text-sm font-semibold">{groupName}</div>
      <div className="mt-1 flex flex-wrap gap-4">
        <label className="flex items-center gap-2 text-xs text-inkmuted">
          <input
            type="checkbox"
            checked={cycleOpen}
            disabled={putPref.isPending}
            onChange={(e) => putPref.mutate({ cycleOpen: e.target.checked, deadlineReminders })}
          />
          Cycle open
        </label>
        <label className="flex items-center gap-2 text-xs text-inkmuted">
          <input
            type="checkbox"
            checked={deadlineReminders}
            disabled={putPref.isPending}
            onChange={(e) => putPref.mutate({ cycleOpen, deadlineReminders: e.target.checked })}
          />
          Deadline reminders
        </label>
      </div>
    </div>
  );
}

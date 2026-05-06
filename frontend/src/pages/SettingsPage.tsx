import { Link } from "react-router-dom";
import {
  useConfig,
  useNotificationPrefs,
  usePushDevices,
  useRemovePushDevice,
  useSendTestPush,
  useUpdateNotificationPref,
} from "../api/queries";
import { useAuth } from "../auth/AuthProvider";
import { useToasts } from "../state/toast";
import { PageHeader } from "../components/layout/PageHeader";
import { Avatar } from "../components/ui/Avatar";
import { formatRelative } from "../utils/dates";
import { useState } from "react";

export function SettingsPage() {
  const { data: config } = useConfig();
  const { signOut } = useAuth();
  const { data: devices = [] } = usePushDevices();
  const removeDevice = useRemovePushDevice();
  const sendTest = useSendTestPush();
  const { data: prefs = [] } = useNotificationPrefs();
  const updatePref = useUpdateNotificationPref();
  const pushToast = useToasts((s) => s.push);
  const [pushOn, setPushOn] = useState(true);

  const handleTest = async () => {
    const result = await sendTest.mutateAsync();
    pushToast(`Test push sent to ${result.devices} device${result.devices === 1 ? "" : "s"}.`, "success");
  };

  return (
    <div className="bg-cream pb-12">
      <PageHeader title="You" back="/" />

      <div className="px-5 pt-5">
        <div className="bg-white rounded-3xl border border-line shadow-soft p-5 flex items-center gap-4">
          <Avatar name={config?.user.displayName ?? "?"} color={config?.user.avatarColor} size="xl" />
          <div className="flex-1 min-w-0">
            <div className="font-display text-xl font-bold">{config?.user.displayName}</div>
            <div className="text-sm text-inkmuted truncate">{config?.user.email}</div>
            <button className="text-grape font-semibold text-sm mt-1">Edit profile</button>
          </div>
        </div>
      </div>

      <div className="px-5 mt-5">
        <div className="text-xs uppercase tracking-widest text-inkmuted font-bold mb-2 ml-1">Notifications</div>
        <div className="bg-white rounded-3xl border border-line shadow-soft divide-y divide-line">
          <div className="p-4 flex items-center gap-3">
            <div className="w-10 h-10 rounded-2xl bg-coral/10 grid place-items-center text-lg">🔔</div>
            <div className="flex-1">
              <div className="font-semibold">Push notifications</div>
              <div className="text-xs text-inkmuted">96h, 48h, and 24h before each deadline</div>
            </div>
            <button
              onClick={() => setPushOn((s) => !s)}
              className={`toggle ${pushOn ? "on" : ""}`}
              aria-label="Toggle push notifications"
            />
          </div>
          <div className="p-4">
            <div className="text-xs uppercase tracking-widest text-inkmuted font-bold mb-3">Per group</div>
            <div className="space-y-3">
              {config?.memberships.map((m) => {
                const pref = prefs.find((p) => p.groupId === m.groupId);
                const enabled = pref?.cycleOpen ?? true;
                return (
                  <div key={m.groupId} className="flex items-center gap-3">
                    <div className={`w-8 h-8 rounded-2xl ${m.gradient.className}`} />
                    <div className="flex-1 text-sm font-semibold">{m.groupName}</div>
                    <button
                      onClick={() =>
                        updatePref.mutate({
                          groupId: m.groupId,
                          patch: { cycleOpen: !enabled, deadlineReminders: !enabled },
                        })
                      }
                      className={`toggle ${enabled ? "on" : ""}`}
                      aria-label={`Toggle for ${m.groupName}`}
                    />
                  </div>
                );
              })}
            </div>
          </div>
        </div>
      </div>

      <div className="px-5 mt-5">
        <div className="text-xs uppercase tracking-widest text-inkmuted font-bold mb-2 ml-1">Your devices</div>
        <div className="bg-white rounded-3xl border border-line shadow-soft divide-y divide-line">
          {devices.map((d) => (
            <div key={d.subscriptionId} className="p-4 flex items-center gap-3">
              <div className="text-2xl">{d.icon}</div>
              <div className="flex-1">
                <div className="font-semibold text-sm">{d.userAgent}</div>
                <div className="text-xs text-inkmuted">
                  {d.lastSuccessAt ? `last push ${formatRelative(d.lastSuccessAt)}` : "no pushes yet"}
                </div>
              </div>
              <button
                onClick={() => removeDevice.mutate(d.subscriptionId)}
                className="text-xs text-coral font-semibold"
              >
                Remove
              </button>
            </div>
          ))}
          {devices.length === 0 && (
            <div className="p-4 text-sm text-inkmuted">No devices subscribed yet.</div>
          )}
        </div>
        <button
          disabled={sendTest.isPending}
          onClick={handleTest}
          className="mt-3 text-sm text-grape font-semibold"
        >
          {sendTest.isPending ? "Sending…" : "Send test push to all devices"}
        </button>
      </div>

      <div className="px-5 mt-5">
        <div className="text-xs uppercase tracking-widest text-inkmuted font-bold mb-2 ml-1">Your groups</div>
        <div className="bg-white rounded-3xl border border-line shadow-soft divide-y divide-line">
          {config?.memberships.map((m) => (
            <div key={m.groupId} className="p-4 flex items-center gap-3">
              <div className={`w-10 h-10 rounded-2xl ${m.gradient.className}`} />
              <div className="flex-1">
                <div className="font-semibold text-sm">{m.groupName}</div>
                <div className="text-xs text-inkmuted capitalize">
                  {m.role}
                </div>
              </div>
              {m.role === "admin" ? (
                <Link to={`/g/${m.groupId}/admin`} className="text-xs text-grape font-semibold">
                  Manage
                </Link>
              ) : (
                <Link to={`/g/${m.groupId}`} className="text-xs text-grape font-semibold">
                  View
                </Link>
              )}
            </div>
          ))}
          <Link to="/join" className="w-full p-4 text-left flex items-center gap-3 hover:bg-cream block">
            <div className="w-10 h-10 rounded-2xl bg-cream border-2 border-dashed border-line grid place-items-center text-inkmuted">
              ＋
            </div>
            <div className="flex-1 text-sm font-semibold">Join with an invite code</div>
            <span className="text-grape">→</span>
          </Link>
        </div>
      </div>

      <div className="px-5 mt-6">
        <button
          onClick={signOut}
          className="w-full bg-white text-coral border border-coral/20 rounded-2xl p-3 text-sm font-semibold"
        >
          Sign out
        </button>
        <p className="text-center text-xs text-inkmuted mt-4">v0.1 · made with care</p>
      </div>
    </div>
  );
}

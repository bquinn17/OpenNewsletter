import { Link, useNavigate } from "react-router-dom";
import {
  useConfig,
  useLeaveGroup,
  useNotificationPrefs,
  usePatchUser,
  usePushDevices,
  usePushEnabled,
  useRemoveAvatar,
  useRemovePushDevice,
  useSendTestPush,
  useSetPushEnabled,
  useUpdateNotificationPref,
  useUploadAvatar,
} from "../api/queries";
import { useAuth } from "../auth/AuthProvider";
import { useToasts } from "../state/toast";
import { PageHeader } from "../components/layout/PageHeader";
import { Avatar } from "../components/ui/Avatar";
import { Button } from "../components/ui/Button";
import { formatRelative } from "../utils/dates";
import { useEffect, useRef, useState } from "react";
import type { NotificationPref } from "../api/types";

const AVATAR_COLORS: { slug: string; cls: string }[] = [
  { slug: "grape", cls: "bg-grape" },
  { slug: "coral", cls: "bg-coral" },
  { slug: "mint", cls: "bg-mint" },
  { slug: "sky", cls: "bg-sky" },
  { slug: "sun", cls: "bg-sun" },
  { slug: "peach", cls: "bg-peach" },
];

export function SettingsPage() {
  const navigate = useNavigate();
  const { data: config } = useConfig();
  const { signOut } = useAuth();
  const { data: devices = [] } = usePushDevices();
  const removeDevice = useRemovePushDevice();
  const sendTest = useSendTestPush();
  const { data: prefs = [] } = useNotificationPrefs();
  const updatePref = useUpdateNotificationPref();
  const { data: pushEnabled } = usePushEnabled();
  const setPushEnabled = useSetPushEnabled();
  const patchUser = usePatchUser();
  const uploadAvatar = useUploadAvatar();
  const removeAvatar = useRemoveAvatar();
  const leaveGroup = useLeaveGroup();
  const pushToast = useToasts((s) => s.push);

  const [editing, setEditing] = useState(false);
  const [draftName, setDraftName] = useState("");
  const [draftColor, setDraftColor] = useState("grape");
  const fileInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (config?.user && !editing) {
      setDraftName(config.user.displayName);
      setDraftColor(config.user.avatarColor);
    }
  }, [config?.user, editing]);

  const pushOn = pushEnabled?.enabled ?? true;

  const handleTest = async () => {
    const result = await sendTest.mutateAsync();
    pushToast(`Test push sent to ${result.devices} device${result.devices === 1 ? "" : "s"}.`, "success");
  };

  const saveProfile = async () => {
    const trimmed = draftName.trim();
    if (!trimmed) {
      pushToast("Name can't be empty.", "error");
      return;
    }
    await patchUser.mutateAsync({ displayName: trimmed, avatarColor: draftColor });
    pushToast("Profile updated.", "success");
    setEditing(false);
  };

  const handleAvatarFile = async (file: File | null | undefined) => {
    if (!file) return;
    if (!file.type.startsWith("image/")) {
      pushToast("Pick an image file.", "error");
      return;
    }
    if (file.size > 5 * 1024 * 1024) {
      pushToast("Avatar must be under 5 MB.", "error");
      return;
    }
    const dataUrl = await new Promise<string>((resolve, reject) => {
      const r = new FileReader();
      r.onload = () => resolve(r.result as string);
      r.onerror = () => reject(r.error ?? new Error("read failed"));
      r.readAsDataURL(file);
    });
    await uploadAvatar.mutateAsync(dataUrl);
    pushToast("Avatar updated.", "success");
  };

  const handleLeave = async (groupId: string, groupName: string) => {
    if (!confirm(`Leave ${groupName}? You'll lose access to its newsletters and need a new invite to rejoin.`)) {
      return;
    }
    try {
      await leaveGroup.mutateAsync(groupId);
      pushToast(`Left ${groupName}.`, "default");
    } catch {
      // global mutation handler will surface the error toast
    }
  };

  return (
    <div className="bg-cream pb-12">
      <PageHeader title="You" back="/" />

      <div className="px-5 pt-5">
        <div className="bg-white rounded-3xl border border-line shadow-soft p-5">
          {!editing ? (
            <div className="flex items-center gap-4">
              <Avatar
                name={config?.user.displayName ?? "?"}
                color={config?.user.avatarColor}
                url={config?.user.avatarUrl}
                size="xl"
              />
              <div className="flex-1 min-w-0">
                <div className="font-display text-xl font-bold">{config?.user.displayName}</div>
                <div className="text-sm text-inkmuted truncate">{config?.user.email}</div>
                <button
                  onClick={() => setEditing(true)}
                  className="text-grape font-semibold text-sm mt-1"
                >
                  Edit profile
                </button>
              </div>
            </div>
          ) : (
            <div>
              <div className="flex items-center gap-4">
                <Avatar
                  name={draftName || "?"}
                  color={draftColor}
                  url={config?.user.avatarUrl}
                  size="xl"
                />
                <div className="flex-1">
                  <label className="block text-xs uppercase tracking-widest text-inkmuted font-bold mb-1">
                    Display name
                  </label>
                  <input
                    value={draftName}
                    onChange={(e) => setDraftName(e.target.value)}
                    maxLength={40}
                    className="w-full bg-cream border border-line rounded-2xl px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-coral/30"
                  />
                </div>
              </div>

              <div className="mt-4">
                <div className="text-xs uppercase tracking-widest text-inkmuted font-bold mb-2">
                  Photo
                </div>
                <div className="flex items-center gap-2">
                  <input
                    ref={fileInputRef}
                    type="file"
                    accept="image/*"
                    className="hidden"
                    onChange={(e) => {
                      void handleAvatarFile(e.target.files?.[0]);
                      if (fileInputRef.current) fileInputRef.current.value = "";
                    }}
                  />
                  <Button
                    variant="ghost"
                    onClick={() => fileInputRef.current?.click()}
                    disabled={uploadAvatar.isPending}
                  >
                    {uploadAvatar.isPending ? "Uploading…" : config?.user.avatarUrl ? "Replace photo" : "Upload photo"}
                  </Button>
                  {config?.user.avatarUrl && (
                    <Button
                      variant="soft"
                      onClick={async () => {
                        await removeAvatar.mutateAsync();
                        pushToast("Photo removed.", "default");
                      }}
                      disabled={removeAvatar.isPending}
                    >
                      Remove photo
                    </Button>
                  )}
                </div>
                <p className="text-xs text-inkmuted mt-1.5">
                  When no photo is set, your friends see your initial on the color below.
                </p>
              </div>

              <div className="mt-4">
                <div className="text-xs uppercase tracking-widest text-inkmuted font-bold mb-2">
                  Avatar color
                </div>
                <div className="flex flex-wrap gap-2">
                  {AVATAR_COLORS.map((c) => (
                    <button
                      key={c.slug}
                      onClick={() => setDraftColor(c.slug)}
                      aria-label={c.slug}
                      className={`w-9 h-9 rounded-full ${c.cls} ring-offset-2 ring-offset-white transition ${
                        draftColor === c.slug ? "ring-2 ring-ink" : ""
                      }`}
                    />
                  ))}
                </div>
              </div>

              <div className="flex justify-end gap-2 mt-4">
                <Button variant="ghost" onClick={() => setEditing(false)} disabled={patchUser.isPending}>
                  Cancel
                </Button>
                <Button onClick={saveProfile} disabled={patchUser.isPending}>
                  {patchUser.isPending ? "Saving…" : "Save"}
                </Button>
              </div>
            </div>
          )}
        </div>
      </div>

      <div className="px-5 mt-5">
        <div className="text-xs uppercase tracking-widest text-inkmuted font-bold mb-2 ml-1">Notifications</div>
        <div className="bg-white rounded-3xl border border-line shadow-soft divide-y divide-line">
          <div className="p-4 flex items-center gap-3">
            <div className="w-10 h-10 rounded-2xl bg-coral/10 grid place-items-center text-lg">🔔</div>
            <div className="flex-1">
              <div className="font-semibold">Push notifications</div>
              <div className="text-xs text-inkmuted">Master switch — turn off to silence everything</div>
            </div>
            <button
              onClick={() => setPushEnabled.mutate(!pushOn)}
              disabled={setPushEnabled.isPending}
              className={`toggle ${pushOn ? "on" : ""}`}
              aria-label="Toggle push notifications"
            />
          </div>
          {config?.memberships.map((m) => {
            const pref = prefs.find((p) => p.groupId === m.groupId);
            return (
              <GroupNotificationPanel
                key={m.groupId}
                groupId={m.groupId}
                groupName={m.groupName}
                gradientClass={m.gradient.className}
                pref={pref}
                disabled={!pushOn || updatePref.isPending}
                onUpdate={(patch) => updatePref.mutate({ groupId: m.groupId, patch })}
              />
            );
          })}
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
              <div className="flex-1 min-w-0">
                <div className="font-semibold text-sm truncate">{m.groupName}</div>
                <div className="text-xs text-inkmuted capitalize">{m.role}</div>
              </div>
              {m.role === "admin" && (
                <Link to={`/g/${m.groupId}/admin`} className="text-xs text-grape font-semibold">
                  Manage
                </Link>
              )}
              <button
                onClick={() => navigate(`/g/${m.groupId}`)}
                className="text-xs text-grape font-semibold"
              >
                View
              </button>
              <button
                onClick={() => handleLeave(m.groupId, m.groupName)}
                className="text-xs text-coral font-semibold"
                disabled={leaveGroup.isPending}
              >
                Leave
              </button>
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

function GroupNotificationPanel({
  groupId,
  groupName,
  gradientClass,
  pref,
  disabled,
  onUpdate,
}: {
  groupId: string;
  groupName: string;
  gradientClass: string;
  pref: NotificationPref | undefined;
  disabled: boolean;
  onUpdate: (patch: Partial<NotificationPref>) => void;
}) {
  const cycleOpen = pref?.cycleOpen ?? true;
  const deadlineReminders = pref?.deadlineReminders ?? true;
  const publication = pref?.publication ?? true;

  return (
    <div className="p-4">
      <div className="flex items-center gap-3 mb-3">
        <div className={`w-8 h-8 rounded-2xl ${gradientClass}`} />
        <div className="flex-1 text-sm font-semibold">{groupName}</div>
      </div>
      <div className="space-y-2 pl-11" id={`prefs-${groupId}`}>
        <PrefRow
          label="When the cycle opens"
          hint="Heads-up that questions are live"
          value={cycleOpen}
          disabled={disabled}
          onChange={(v) => onUpdate({ cycleOpen: v })}
        />
        <PrefRow
          label="Deadline reminders"
          hint="96h, 48h, and 24h before close"
          value={deadlineReminders}
          disabled={disabled}
          onChange={(v) => onUpdate({ deadlineReminders: v })}
        />
        <PrefRow
          label="When the edition publishes"
          hint="A nudge when everyone's answers are live"
          value={publication}
          disabled={disabled}
          onChange={(v) => onUpdate({ publication: v })}
        />
      </div>
    </div>
  );
}

function PrefRow({
  label,
  hint,
  value,
  disabled,
  onChange,
}: {
  label: string;
  hint: string;
  value: boolean;
  disabled: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <div className="flex items-center gap-3">
      <div className="flex-1">
        <div className="text-sm">{label}</div>
        <div className="text-xs text-inkmuted">{hint}</div>
      </div>
      <button
        onClick={() => onChange(!value)}
        disabled={disabled}
        className={`toggle ${value ? "on" : ""} ${disabled ? "opacity-50 pointer-events-none" : ""}`}
        aria-label={`Toggle ${label}`}
      />
    </div>
  );
}

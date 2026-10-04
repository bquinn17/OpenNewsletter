import { useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { zodResolver } from "@hookform/resolvers/zod";
import { useForm } from "react-hook-form";
import clsx from "clsx";
import { z } from "zod";
import { useAuth } from "../auth/useAuth";
import { ApiError } from "../api/client";
import { useConfig, useMe } from "../api/queries";
import { useLeaveGroup, useUpdateProfile } from "../api/mutations";
import { useToasts } from "../state/toast";
import { PageHeader } from "../components/layout/PageHeader";
import { Avatar } from "../components/ui/Avatar";
import { Button } from "../components/ui/Button";
import { ConfirmDialog } from "../components/ui/ConfirmDialog";
import { Spinner } from "../components/ui/Spinner";
import { AVATAR_COLOR_SLUGS, avatarColorClass } from "../utils/avatarColor";

// Bounds per `03-api-contract.md` §5.2 / `shared/openapi.yaml` (`PatchMeRequest.displayName`).
const profileSchema = z.object({
  displayName: z.string().trim().min(1, "Name can't be empty.").max(40, "40 characters max."),
  avatarColor: z.enum(AVATAR_COLOR_SLUGS),
});

type ProfileForm = z.infer<typeof profileSchema>;

export function SettingsPage() {
  const navigate = useNavigate();
  const { logout } = useAuth();
  const { data: me } = useMe();
  const { data: config } = useConfig();
  const updateProfile = useUpdateProfile();
  const leaveGroup = useLeaveGroup();
  const pushToast = useToasts((s) => s.push);

  const [editing, setEditing] = useState(false);
  const [leaveTarget, setLeaveTarget] = useState<{ groupId: string; groupName: string } | null>(
    null,
  );

  const {
    register,
    handleSubmit,
    reset,
    watch,
    setValue,
    formState: { errors },
  } = useForm<ProfileForm>({
    resolver: zodResolver(profileSchema),
    values: me ? { displayName: me.displayName, avatarColor: me.avatarColor } : undefined,
  });

  const draftName = watch("displayName");
  const draftColor = watch("avatarColor");

  const onSubmit = handleSubmit(async (values) => {
    try {
      await updateProfile.mutateAsync(values);
      pushToast("Profile updated.", "success");
      setEditing(false);
    } catch {
      // Global mutation handler surfaces the error toast.
    }
  });

  const handleLeave = async () => {
    if (!leaveTarget) return;
    try {
      await leaveGroup.mutateAsync(leaveTarget.groupId);
      pushToast(`Left ${leaveTarget.groupName}.`, "default");
    } catch (e) {
      const message =
        e instanceof ApiError && e.code === "LAST_ADMIN"
          ? "You're the only admin — promote someone else first."
          : e instanceof ApiError
            ? (e.problem?.detail ?? e.message)
            : "Couldn't leave that group.";
      pushToast(message, "error");
    } finally {
      setLeaveTarget(null);
    }
  };

  if (!me || !config) {
    return (
      <div className="flex justify-center py-20">
        <Spinner size="lg" />
      </div>
    );
  }

  return (
    <div className="bg-cream pb-12">
      <PageHeader title="You" back="/" />

      <div className="px-5 pt-5">
        <div className="rounded-3xl border border-line bg-white p-5 shadow-soft">
          {!editing ? (
            <div className="flex items-center gap-4">
              <Avatar
                name={me.displayName}
                url={me.avatarUrl}
                colorClassName={avatarColorClass(me.avatarColor)}
                size="xl"
              />
              <div className="min-w-0 flex-1">
                <div className="font-display text-xl font-bold">{me.displayName}</div>
                <div className="truncate text-sm text-inkmuted">{me.email}</div>
                <button
                  onClick={() => {
                    reset({ displayName: me.displayName, avatarColor: me.avatarColor });
                    setEditing(true);
                  }}
                  className="mt-1 text-sm font-semibold text-grape"
                >
                  Edit profile
                </button>
              </div>
            </div>
          ) : (
            <form onSubmit={onSubmit}>
              <div className="flex items-center gap-4">
                <Avatar
                  name={draftName || "?"}
                  colorClassName={avatarColorClass(draftColor)}
                  size="xl"
                />
                <div className="flex-1">
                  <label
                    htmlFor="displayName"
                    className="mb-1 block text-xs font-bold uppercase tracking-widest text-inkmuted"
                  >
                    Display name
                  </label>
                  <input
                    id="displayName"
                    {...register("displayName")}
                    maxLength={40}
                    aria-invalid={!!errors.displayName}
                    aria-describedby="displayName-error"
                    className="w-full rounded-2xl border border-line bg-cream px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-coral/30"
                  />
                  <p
                    id="displayName-error"
                    role="alert"
                    aria-live="polite"
                    className="mt-1 min-h-[1em] text-xs font-semibold text-coral"
                  >
                    {errors.displayName?.message ?? ""}
                  </p>
                </div>
              </div>

              <fieldset className="mt-4">
                <legend className="mb-2 text-xs font-bold uppercase tracking-widest text-inkmuted">
                  Avatar color
                </legend>
                <div role="radiogroup" aria-label="Avatar color" className="flex flex-wrap gap-2">
                  {AVATAR_COLOR_SLUGS.map((slug) => (
                    <label key={slug} className="cursor-pointer">
                      <input
                        type="radio"
                        value={slug}
                        checked={draftColor === slug}
                        onChange={() => setValue("avatarColor", slug, { shouldValidate: true })}
                        className="peer sr-only"
                        aria-label={slug}
                      />
                      <span
                        className={clsx(
                          "block h-9 w-9 rounded-full ring-offset-2 ring-offset-white transition",
                          avatarColorClass(slug),
                          draftColor === slug && "ring-2 ring-ink",
                        )}
                      />
                    </label>
                  ))}
                </div>
              </fieldset>

              <div className="mt-4 flex justify-end gap-2">
                <Button
                  type="button"
                  variant="ghost"
                  onClick={() => setEditing(false)}
                  disabled={updateProfile.isPending}
                >
                  Cancel
                </Button>
                <Button type="submit" disabled={updateProfile.isPending}>
                  {updateProfile.isPending ? "Saving…" : "Save"}
                </Button>
              </div>
            </form>
          )}
        </div>
      </div>

      <div className="mt-5 px-5">
        <div className="mb-2 ml-1 text-xs font-bold uppercase tracking-widest text-inkmuted">
          Your groups
        </div>
        <div className="divide-y divide-line rounded-3xl border border-line bg-white shadow-soft">
          {config.memberships.map((m) => (
            <div key={m.groupId} className="flex items-center gap-3 p-4">
              <div className="min-w-0 flex-1">
                <div className="truncate text-sm font-semibold">{m.groupName}</div>
                <div className="text-xs capitalize text-inkmuted">{m.role}</div>
              </div>
              <button
                onClick={() => navigate(`/g/${m.groupId}`)}
                className="text-xs font-semibold text-grape"
              >
                View
              </button>
              {m.role === "admin" && (
                <Link to={`/g/${m.groupId}/admin`} className="text-xs font-semibold text-grape">
                  Manage
                </Link>
              )}
              <button
                onClick={() => setLeaveTarget({ groupId: m.groupId, groupName: m.groupName })}
                className="text-xs font-semibold text-coral"
                disabled={leaveGroup.isPending}
              >
                Leave
              </button>
            </div>
          ))}
          {config.memberships.length === 0 && (
            <div className="p-4 text-sm text-inkmuted">You haven&apos;t joined a group yet.</div>
          )}
          <Link
            to="/join"
            className="block flex w-full items-center gap-3 p-4 text-left hover:bg-cream"
          >
            <div className="grid h-10 w-10 place-items-center rounded-2xl border-2 border-dashed border-line bg-cream text-inkmuted">
              ＋
            </div>
            <div className="flex-1 text-sm font-semibold">Join with an invite code</div>
            <span className="text-grape">→</span>
          </Link>
        </div>
      </div>

      <div className="mt-6 px-5">
        <button
          onClick={() => void logout()}
          className="w-full rounded-2xl border border-coral/20 bg-white p-3 text-sm font-semibold text-coral"
        >
          Sign out
        </button>
      </div>

      <ConfirmDialog
        open={!!leaveTarget}
        title={leaveTarget ? `Leave ${leaveTarget.groupName}?` : ""}
        message="You'll lose access to its newsletters and need a new invite to rejoin."
        confirmLabel="Leave"
        tone="danger"
        busy={leaveGroup.isPending}
        onConfirm={handleLeave}
        onCancel={() => setLeaveTarget(null)}
      />
    </div>
  );
}

import { useEffect, useState } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import { api } from "../api/client";
import { useRedeemInvite } from "../api/queries";
import { useToasts } from "../state/toast";
import type { Invite } from "../api/types";

export function JoinPage() {
  const [search] = useSearchParams();
  const initialCode = search.get("code") ?? "";
  const [code, setCode] = useState(initialCode);
  const [preview, setPreview] = useState<Invite | null>(null);
  const [validating, setValidating] = useState(false);
  const redeem = useRedeemInvite();
  const navigate = useNavigate();
  const pushToast = useToasts((s) => s.push);

  useEffect(() => {
    let cancel = false;
    if (!code) return;
    setValidating(true);
    api.previewInvite(code).then((res) => {
      if (cancel) return;
      setPreview(res);
      setValidating(false);
    });
    return () => {
      cancel = true;
    };
  }, [code]);

  const handleContinue = async () => {
    try {
      const result = await redeem.mutateAsync(code);
      pushToast(`Joined ${result.groupName} 🎉`, "success");
      navigate("/");
    } catch (e) {
      pushToast((e as Error).message ?? "Couldn't redeem", "error");
    }
  };

  return (
    <div className="hero-may min-h-screen text-white px-6 py-10 relative overflow-hidden">
      <div className="absolute -right-20 -top-20 w-72 h-72 rounded-full bg-white/10" />
      <div className="absolute -left-12 bottom-20 w-52 h-52 rounded-full bg-white/10" />

      <div className="relative z-10">
        <div className="flex items-center gap-2">
          <div className="w-9 h-9 rounded-2xl bg-white/15 grid place-items-center font-bold">O</div>
          <div className="font-display font-bold text-lg">OpenNewsletter</div>
        </div>

        <h1 className="font-display text-5xl font-bold leading-tight mt-12">
          Welcome to the
          <br />
          group chat
          <br />
          that <em className="text-sun not-italic">remembers.</em>
        </h1>
        <p className="opacity-90 mt-4 max-w-sm">
          A monthly newsletter you and your friends write together. Drop your invite code below to join.
        </p>
      </div>

      <div className="relative z-10 mt-8 bg-white text-ink rounded-4xl p-5 shadow-card">
        <label className="block text-xs uppercase tracking-widest text-inkmuted font-bold mb-2">Invite code</label>
        <div className="flex items-center gap-2 bg-cream rounded-2xl px-4 py-3 ring-2 ring-coral/30">
          <input
            value={code}
            onChange={(e) => setCode(e.target.value.toUpperCase())}
            className="flex-1 bg-transparent font-mono text-sm focus:outline-none"
            placeholder="VHQM-2K8R-PX3F-T9JN"
          />
          {validating ? (
            <span className="text-inkmuted text-xs">…</span>
          ) : preview ? (
            <span className="text-mint">✓</span>
          ) : code ? (
            <span className="text-coral text-xs">invalid</span>
          ) : null}
        </div>

        {preview && (
          <div className="mt-4 bg-gradient-to-br from-coral/10 to-peach/10 rounded-2xl p-4 flex items-center gap-3 animate-pop">
            <div className="w-12 h-12 rounded-2xl bg-gradient-to-br from-coral to-peach" />
            <div className="flex-1 min-w-0">
              <div className="text-xs text-inkmuted uppercase tracking-widest font-bold">You're invited to</div>
              <div className="font-display text-lg font-bold leading-tight">{preview.groupName}</div>
              <div className="text-xs text-inkmuted">role: {preview.roleOnRedeem}</div>
            </div>
          </div>
        )}

        <button
          disabled={!preview || redeem.isPending}
          onClick={handleContinue}
          className="w-full mt-4 bg-ink text-cream rounded-2xl py-3 font-bold flex items-center justify-center gap-2 disabled:opacity-50"
        >
          {redeem.isPending ? "Joining…" : "Continue with Google"} <span>→</span>
        </button>
        <div className="grid grid-cols-2 gap-2 mt-2">
          <button className="bg-white border border-line rounded-2xl py-3 text-sm font-semibold"> Apple</button>
          <button className="bg-white border border-line rounded-2xl py-3 text-sm font-semibold">f Facebook</button>
        </div>

        <p className="text-xs text-inkmuted mt-4 text-center">By continuing you agree to be excellent to your friends.</p>
      </div>

      <p className="relative z-10 text-center text-xs opacity-80 mt-6">
        Don't have a code? Ask the friend who told you about this.
      </p>
    </div>
  );
}

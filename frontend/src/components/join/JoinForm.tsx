import { zodResolver } from "@hookform/resolvers/zod";
import { useForm } from "react-hook-form";
import { useNavigate } from "react-router-dom";
import { z } from "zod";
import { Button } from "../ui/Button";

const schema = z.object({
  code: z
    .string()
    .trim()
    .min(1, "Enter an invite code")
    .transform((value) => value.toUpperCase()),
});

type FormValues = z.infer<typeof schema>;

type Props = {
  className?: string;
};

export function JoinForm({ className }: Props) {
  const navigate = useNavigate();
  const {
    register,
    handleSubmit,
    formState: { errors },
  } = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: { code: "" },
  });

  const onSubmit = handleSubmit((values) => {
    navigate(`/join?code=${encodeURIComponent(values.code)}`);
  });

  return (
    <form onSubmit={onSubmit} className={className} noValidate>
      <label
        htmlFor="invite-code"
        className="mb-2 block text-xs font-bold uppercase tracking-widest text-inkmuted"
      >
        Invite code
      </label>
      <div className="flex items-center gap-2">
        <input
          id="invite-code"
          {...register("code")}
          aria-invalid={!!errors.code}
          aria-describedby="invite-code-error"
          placeholder="VHQM-2K8R-PX3F-T9JN"
          className="flex-1 rounded-2xl border border-line bg-cream px-4 py-3 font-mono text-sm focus:outline-none focus:ring-2 focus:ring-coral/30"
        />
        <Button type="submit">Join</Button>
      </div>
      <p
        id="invite-code-error"
        role="alert"
        aria-live="polite"
        className="mt-1.5 min-h-[1em] text-xs font-semibold text-coral"
      >
        {errors.code?.message ?? ""}
      </p>
    </form>
  );
}

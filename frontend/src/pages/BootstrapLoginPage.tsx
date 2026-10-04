import { zodResolver } from "@hookform/resolvers/zod";
import { useState } from "react";
import { useForm } from "react-hook-form";
import { useNavigate } from "react-router-dom";
import { z } from "zod";
import { bootstrapAuth, CognitoAuthError } from "../auth/cognitoPasswordAuth";
import { userManager } from "../auth/userManager";
import { Button } from "../components/ui/Button";

const schema = z.object({
  username: z.string().min(1, "Username is required"),
  password: z.string().min(1, "Password is required"),
});

type FormValues = z.infer<typeof schema>;

export function BootstrapLoginPage(): JSX.Element {
  const navigate = useNavigate();
  const [authError, setAuthError] = useState<string | null>(null);
  const {
    register,
    handleSubmit,
    formState: { errors, isSubmitting },
  } = useForm<FormValues>({ resolver: zodResolver(schema) });

  const onSubmit = handleSubmit(async ({ username, password }) => {
    setAuthError(null);

    if (!userManager) {
      setAuthError("Bootstrap login isn't available in mock mode.");
      return;
    }

    let user;
    try {
      user = await bootstrapAuth({ flow: "USER_PASSWORD_AUTH", username, password });
    } catch (error) {
      setAuthError(error instanceof CognitoAuthError ? error.message : "Sign-in failed.");
      return;
    }

    await userManager.storeUser(user);
    await userManager.events.load(user);
    navigate("/", { replace: true });
  });

  return (
    <div className="mx-auto flex min-h-[70vh] max-w-sm flex-col justify-center px-4">
      <h1 className="mb-4 font-display text-2xl">Bootstrap admin sign-in</h1>
      <form onSubmit={onSubmit} noValidate className="flex flex-col gap-4">
        <div>
          <label htmlFor="bootstrap-username" className="mb-1 block text-sm font-medium">
            Username
          </label>
          <input
            id="bootstrap-username"
            type="text"
            autoComplete="username"
            aria-describedby={errors.username ? "bootstrap-username-error" : undefined}
            className="w-full rounded-md border border-line px-3 py-2"
            {...register("username")}
          />
          {errors.username && (
            <p id="bootstrap-username-error" role="alert" className="mt-1 text-sm text-coral">
              {errors.username.message}
            </p>
          )}
        </div>
        <div>
          <label htmlFor="bootstrap-password" className="mb-1 block text-sm font-medium">
            Password
          </label>
          <input
            id="bootstrap-password"
            type="password"
            autoComplete="current-password"
            aria-describedby={errors.password ? "bootstrap-password-error" : undefined}
            className="w-full rounded-md border border-line px-3 py-2"
            {...register("password")}
          />
          {errors.password && (
            <p id="bootstrap-password-error" role="alert" className="mt-1 text-sm text-coral">
              {errors.password.message}
            </p>
          )}
        </div>
        <div aria-live="polite">
          {authError && <p className="text-sm text-coral">{authError}</p>}
        </div>
        <Button type="submit" disabled={isSubmitting}>
          {isSubmitting ? "Signing in…" : "Sign in"}
        </Button>
      </form>
    </div>
  );
}

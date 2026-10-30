import { act, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { User } from "oidc-client-ts";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { env } from "../env";
import { AuthProvider } from "./AuthProvider";
import { useAuth } from "./useAuth";
import { userManager } from "./userManager";

const assign = vi.fn();
vi.mock("../utils/navigation", () => ({ assignLocation: (url: string) => assign(url) }));
vi.mock("../demo", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../demo")>()),
  isDemo: true,
  isMockMode: true,
}));

const storageKey = (): string =>
  `oidc.user:https://cognito-idp.us-east-1.amazonaws.com/${env.cognitoUserPoolId}:${env.cognitoClientId}`;

function storedUser(): User {
  return new User({
    id_token: "h.e30.s",
    access_token: "access-1",
    refresh_token: "refresh-1",
    token_type: "Bearer",
    expires_at: Math.floor(Date.now() / 1000) + 3600,
    profile: { sub: "real-sub", aud: env.cognitoClientId, iss: "i", exp: 0, iat: 0 },
  });
}

function Probe(): JSX.Element {
  const { status, user, logout } = useAuth();
  return (
    <div>
      <span>
        status:{status} name:{user?.name}
      </span>
      <button type="button" onClick={() => void logout()}>
        Sign out
      </button>
    </div>
  );
}

function renderProvider() {
  return render(
    <QueryClientProvider client={new QueryClient()}>
      <AuthProvider>
        <Probe />
      </AuthProvider>
    </QueryClientProvider>,
  );
}

function storageEvent(oldValue: string | null, newValue: string | null): StorageEvent {
  return new StorageEvent("storage", { key: storageKey(), oldValue, newValue });
}

beforeEach(() => assign.mockReset());
afterEach(() => vi.restoreAllMocks());

describe("AuthProvider in the demo (docs/design/frontend.md §17)", () => {
  it("is signed in as the demo user even with a real session stored", async () => {
    await userManager!.storeUser(storedUser());

    renderProvider();

    expect(screen.getByText("status:authenticated name:Quinn")).toBeInTheDocument();
  });

  it("signs out by leaving the demo, without touching the stored session", async () => {
    await userManager!.storeUser(storedUser());
    const fetchSpy = vi.spyOn(globalThis, "fetch");
    renderProvider();

    screen.getByRole("button", { name: "Sign out" }).click();

    await waitFor(() => expect(assign).toHaveBeenCalledWith("/"));
    expect(localStorage.getItem(storageKey())).not.toBeNull();
    expect(fetchSpy).not.toHaveBeenCalled();
  });

  it("leaves the demo when another tab signs in", () => {
    renderProvider();

    act(() => void window.dispatchEvent(storageEvent(null, "{}")));

    expect(assign).toHaveBeenCalledWith("/");
  });

  it("stays in the demo when another tab only renews a session", () => {
    renderProvider();

    act(() => void window.dispatchEvent(storageEvent("{}", '{"renewed":true}')));

    expect(assign).not.toHaveBeenCalled();
  });
});

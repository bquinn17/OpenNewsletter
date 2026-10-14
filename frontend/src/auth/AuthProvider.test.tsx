import { act, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { User } from "oidc-client-ts";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { env } from "../env";
import { useCurrentGroup } from "../state/currentGroup";
import { AuthProvider } from "./AuthProvider";
import { useAuth } from "./useAuth";
import { userManager } from "./userManager";

const assign = vi.fn();
vi.mock("../utils/navigation", () => ({ assignLocation: (url: string) => assign(url) }));

function jwt(claims: Record<string, unknown>): string {
  const body = btoa(JSON.stringify(claims))
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/, "");
  return `h.${body}.s`;
}

function makeUser(aud: string): User {
  return new User({
    id_token: jwt({ sub: "s1", aud }),
    access_token: "access-1",
    refresh_token: "refresh-1",
    token_type: "Bearer",
    expires_at: Math.floor(Date.now() / 1000) + 3600,
    profile: { sub: "s1", aud, iss: "i", exp: 0, iat: 0, name: "Quinn" },
  });
}

const storageKey = (): string =>
  `oidc.user:https://cognito-idp.us-east-1.amazonaws.com/${env.cognitoUserPoolId}:${env.cognitoClientId}`;

function LogoutProbe(): JSX.Element {
  const { status, logout } = useAuth();
  return (
    <div>
      <span>status:{status}</span>
      <button type="button" onClick={() => void logout()}>
        Sign out
      </button>
    </div>
  );
}

function renderProvider(queryClient = new QueryClient()) {
  return render(
    <QueryClientProvider client={queryClient}>
      <AuthProvider>
        <LogoutProbe />
      </AuthProvider>
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  assign.mockReset();
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("token storage (D1)", () => {
  it("persists the stored user in localStorage, not sessionStorage", async () => {
    await userManager!.storeUser(makeUser(env.cognitoClientId));

    expect(localStorage.getItem(storageKey())).not.toBeNull();
    expect(sessionStorage.getItem(storageKey())).toBeNull();
    await userManager!.removeUser();
  });

  it("restores a session from localStorage on load", async () => {
    await userManager!.storeUser(makeUser(env.cognitoClientId));
    renderProvider();

    expect(await screen.findByText("status:authenticated")).toBeInTheDocument();
    await act(() => userManager!.removeUser());
  });
});

describe("logout (docs/design/auth.md §8)", () => {
  it("clears storage, caches and stores, revokes the refresh token, then redirects", async () => {
    await userManager!.storeUser(makeUser(env.cognitoClientId));
    const queryClient = new QueryClient();
    queryClient.setQueryData(["config"], { some: "data" });
    useCurrentGroup.getState().setCurrentGroup("g_trail");
    const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async () => {
      // By the time revoke runs, steps 1–3 have already happened.
      expect(localStorage.getItem(storageKey())).toBeNull();
      expect(queryClient.getQueryData(["config"])).toBeUndefined();
      expect(useCurrentGroup.getState().currentGroupId).toBeNull();
      expect(assign).not.toHaveBeenCalled();
      return new Response(null, { status: 200 });
    });

    renderProvider(queryClient);
    await screen.findByText("status:authenticated");
    screen.getByRole("button", { name: "Sign out" }).click();

    await waitFor(() => expect(assign).toHaveBeenCalledTimes(1));
    expect(fetchMock).toHaveBeenCalledTimes(1);
    const [url, init] = fetchMock.mock.calls[0]!;
    expect(url).toBe(`https://${env.cognitoHostedDomain}/oauth2/revoke`);
    expect(init?.method).toBe("POST");
    expect(new Headers(init?.headers).get("Content-Type")).toBe(
      "application/x-www-form-urlencoded",
    );
    const body = new URLSearchParams(String(init?.body));
    expect(body.get("token")).toBe("refresh-1");
    expect(body.get("client_id")).toBe(env.cognitoClientId);
    expect(assign.mock.calls[0]![0]).toContain(
      `https://${env.cognitoHostedDomain}/logout?client_id=${env.cognitoClientId}`,
    );
  });

  it("revokes a bootstrap-login session against the admin-bootstrap client", async () => {
    await userManager!.storeUser(makeUser(env.cognitoBootstrapClientId!));
    const fetchMock = vi
      .spyOn(globalThis, "fetch")
      .mockResolvedValue(new Response(null, { status: 200 }));

    renderProvider();
    await screen.findByText("status:authenticated");
    screen.getByRole("button", { name: "Sign out" }).click();

    await waitFor(() => expect(assign).toHaveBeenCalledTimes(1));
    const body = new URLSearchParams(String(fetchMock.mock.calls[0]![1]?.body));
    expect(body.get("client_id")).toBe(env.cognitoBootstrapClientId!);
  });

  it("never uses a bootstrap client in a build without one (prod), revoking against the frontend client", async () => {
    const original = env.cognitoBootstrapClientId;
    env.cognitoBootstrapClientId = null;
    try {
      // Even a token whose `aud` names some other client goes to the frontend client.
      await userManager!.storeUser(makeUser("some-other-client"));
      const fetchMock = vi
        .spyOn(globalThis, "fetch")
        .mockResolvedValue(new Response(null, { status: 200 }));

      renderProvider();
      await screen.findByText("status:authenticated");
      screen.getByRole("button", { name: "Sign out" }).click();

      await waitFor(() => expect(assign).toHaveBeenCalledTimes(1));
      const body = new URLSearchParams(String(fetchMock.mock.calls[0]![1]?.body));
      expect(body.get("client_id")).toBe(env.cognitoClientId);
    } finally {
      env.cognitoBootstrapClientId = original;
    }
  });

  it("still logs out when the revoke call fails", async () => {
    await userManager!.storeUser(makeUser(env.cognitoClientId));
    vi.spyOn(globalThis, "fetch").mockRejectedValue(new TypeError("Failed to fetch"));

    renderProvider();
    await screen.findByText("status:authenticated");
    screen.getByRole("button", { name: "Sign out" }).click();

    await waitFor(() => expect(assign).toHaveBeenCalledTimes(1));
    expect(localStorage.getItem(storageKey())).toBeNull();
  });
});

import { User } from "oidc-client-ts";
import { afterEach, describe, expect, it, vi } from "vitest";
import { env } from "../env";
import { decodeIdTokenClaims } from "./cognitoPasswordAuth";
import { renewSession } from "./renewSession";
import { userManager } from "./userManager";

vi.mock("./userManager", () => ({
  userManager: {
    getUser: vi.fn(),
    signinSilent: vi.fn(),
    storeUser: vi.fn(),
    events: { load: vi.fn() },
  },
}));

// Non-null in these tests: the mock above always provides it.
const manager = vi.mocked(userManager!, { deep: true });

function jwt(claims: Record<string, unknown>): string {
  const bytes = new TextEncoder().encode(JSON.stringify(claims));
  const body = btoa(String.fromCharCode(...bytes))
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/, "");
  return `h.${body}.s`;
}

function storedUser(aud: string): User {
  return new User({
    id_token: jwt({ sub: "s1", aud }),
    access_token: "a",
    refresh_token: "refresh-1",
    token_type: "Bearer",
    profile: { sub: "s1", aud, iss: "i", exp: 0, iat: 0 },
  });
}

afterEach(() => {
  vi.restoreAllMocks();
  vi.clearAllMocks();
});

describe("renewSession", () => {
  it("renews a federated session with the refresh-token grant", async () => {
    const renewed = storedUser(env.cognitoClientId);
    manager.getUser.mockResolvedValue(storedUser(env.cognitoClientId));
    manager.signinSilent.mockResolvedValue(renewed);

    expect(await renewSession()).toBe(renewed);
    expect(manager.signinSilent).toHaveBeenCalledTimes(1);
  });

  it("renews a bootstrap session through Cognito REFRESH_TOKEN_AUTH, keeping the refresh token", async () => {
    manager.getUser.mockResolvedValue(storedUser(env.cognitoBootstrapClientId));
    const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(
        JSON.stringify({
          AuthenticationResult: {
            IdToken: jwt({ sub: "s1", aud: env.cognitoBootstrapClientId, name: "José" }),
            AccessToken: "a2",
            TokenType: "Bearer",
            ExpiresIn: 3600,
          },
        }),
        { status: 200 },
      ),
    );

    const renewed = await renewSession();

    expect(manager.signinSilent).not.toHaveBeenCalled();
    const body: unknown = JSON.parse(String(fetchMock.mock.calls[0]?.[1]?.body));
    expect(body).toMatchObject({
      AuthFlow: "REFRESH_TOKEN_AUTH",
      ClientId: env.cognitoBootstrapClientId,
      AuthParameters: { REFRESH_TOKEN: "refresh-1" },
    });
    expect(renewed?.refresh_token).toBe("refresh-1");
    expect(renewed?.profile.name).toBe("José");
    expect(manager.storeUser).toHaveBeenCalledWith(renewed);
  });

  it("returns null when renewal fails", async () => {
    manager.getUser.mockResolvedValue(storedUser(env.cognitoClientId));
    manager.signinSilent.mockRejectedValue(new Error("invalid_grant"));

    expect(await renewSession()).toBeNull();
  });
});

describe("decodeIdTokenClaims", () => {
  it("decodes UTF-8 claims", () => {
    expect(decodeIdTokenClaims(jwt({ sub: "s", name: "Zoë Ångström" })).name).toBe("Zoë Ångström");
  });
});

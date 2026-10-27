import { describe, expect, it } from "vitest";
import { buildEnv, type RawEnv } from "./env";

const devVars: RawEnv = {
  VITE_USE_MOCKS: "false",
  VITE_API_BASE_URL: "https://api.example.com",
  VITE_COGNITO_USER_POOL_ID: "us-east-1_X",
  VITE_COGNITO_CLIENT_ID: "frontend-client",
  VITE_COGNITO_HOSTED_DOMAIN: "pool.auth.us-east-1.amazoncognito.com",
};

describe("buildEnv", () => {
  it("builds a prod env with no bootstrap client variable, instead of throwing (D5)", () => {
    const env = buildEnv({ ...devVars, VITE_ENV: "prod" });

    expect(env.appEnv).toBe("prod");
    expect(env.cognitoBootstrapClientId).toBeNull();
    expect(env.bootstrapLoginEnabled).toBe(false);
  });

  it("ignores a bootstrap client ID in a prod build, keeping it out of prod code paths", () => {
    const env = buildEnv({
      ...devVars,
      VITE_ENV: "prod",
      VITE_COGNITO_BOOTSTRAP_CLIENT_ID: "bootstrap-client",
    });

    expect(env.cognitoBootstrapClientId).toBeNull();
    expect(env.bootstrapLoginEnabled).toBe(false);
  });

  it("enables bootstrap login in dev when the bootstrap client is set", () => {
    const env = buildEnv({ ...devVars, VITE_COGNITO_BOOTSTRAP_CLIENT_ID: "bootstrap-client" });

    expect(env.appEnv).toBe("dev");
    expect(env.cognitoBootstrapClientId).toBe("bootstrap-client");
    expect(env.bootstrapLoginEnabled).toBe(true);
  });

  it("leaves bootstrap login off in dev when the bootstrap client is unset or empty", () => {
    for (const value of [undefined, ""]) {
      const env = buildEnv({ ...devVars, VITE_COGNITO_BOOTSTRAP_CLIENT_ID: value });
      expect(env.cognitoBootstrapClientId).toBeNull();
      expect(env.bootstrapLoginEnabled).toBe(false);
    }
  });

  it("still throws on a missing required variable outside mock mode", () => {
    expect(() => buildEnv({ ...devVars, VITE_COGNITO_CLIENT_ID: "" })).toThrow(
      /VITE_COGNITO_CLIENT_ID/,
    );
    expect(() => buildEnv({ VITE_USE_MOCKS: "true" })).not.toThrow();
  });
});

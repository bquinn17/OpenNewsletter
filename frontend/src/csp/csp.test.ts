import { describe, expect, it } from "vitest";
import { buildCspPolicy } from "./csp";

describe("buildCspPolicy", () => {
  it("includes every origin, reduced from a full URL to just its origin", () => {
    const policy = buildCspPolicy({
      cdnBaseUrl: "https://cdn.example.com/some/path?x=1",
      apiBaseUrl: "https://api.example.com/v1",
      cognitoHostedDomain: "my-pool.auth.us-east-1.amazoncognito.com",
    });

    expect(policy).toContain("default-src 'self'");
    expect(policy).toContain("script-src 'self'");
    expect(policy).toContain("style-src 'self' 'unsafe-inline'");
    expect(policy).toContain("img-src 'self' data: blob: https://cdn.example.com");
    expect(policy).toContain(
      "connect-src 'self' https://api.example.com https://cdn.example.com " +
        "https://cognito-idp.us-east-1.amazonaws.com https://my-pool.auth.us-east-1.amazoncognito.com",
    );
    expect(policy).toContain("font-src 'self'");
    expect(policy).toContain("base-uri 'self'");
    expect(policy).toContain("form-action 'self' https://my-pool.auth.us-east-1.amazoncognito.com");
    expect(policy).toContain("worker-src 'self'");
    expect(policy).toContain("manifest-src 'self'");

    // No path/query leaked through into the policy — only the origin.
    expect(policy).not.toContain("/some/path");
    expect(policy).not.toContain("/v1");
  });

  it("omits empty/unset origins instead of leaving a blank or literal %VITE_*% token", () => {
    const policy = buildCspPolicy({});

    expect(policy).toContain("img-src 'self' data: blob:");
    expect(policy).not.toMatch(/img-src[^;]*blob: ;/);
    expect(policy).toContain("connect-src 'self' https://cognito-idp.us-east-1.amazonaws.com");
    expect(policy).toContain("form-action 'self'");
    expect(policy).not.toContain("form-action 'self' https://;");
    expect(policy).not.toContain("%VITE");
    expect(policy).not.toContain("undefined");
    expect(policy).not.toContain("null");
  });

  it("treats an empty string the same as unset (the real shape of an unset Vite env var)", () => {
    const policy = buildCspPolicy({ cdnBaseUrl: "", apiBaseUrl: "", cognitoHostedDomain: "" });

    expect(policy).toContain("img-src 'self' data: blob:");
    expect(
      policy
        .split(";")
        .find((d) => d.trim().startsWith("img-src"))
        ?.trim(),
    ).toBe("img-src 'self' data: blob:");
  });

  it("joins directives with '; ' and never leaves a %VITE token anywhere", () => {
    const policy = buildCspPolicy({
      cdnBaseUrl: "https://cdn.example.com",
      apiBaseUrl: "https://api.example.com",
      cognitoHostedDomain: "pool.auth.us-east-1.amazoncognito.com",
    });

    expect(policy.split("; ").length).toBeGreaterThanOrEqual(10);
    expect(policy).not.toMatch(/%VITE/);
  });
});

import { describe, expect, it } from "vitest";
import { buildCspPolicy, missingProdCspVars } from "./csp";

describe("buildCspPolicy", () => {
  it("includes every origin, reduced from a full URL to just its origin", () => {
    const policy = buildCspPolicy({
      mediaBaseUrl: "https://media.example.com/some/path?x=1",
      apiBaseUrl: "https://api.example.com/v1",
      cognitoHostedDomain: "my-pool.auth.us-east-1.amazoncognito.com",
      uploadOrigins: "https://uploads-a.example.com https://uploads-b.example.com/put",
    });

    expect(policy).toContain("default-src 'self'");
    expect(policy).toContain("script-src 'self'");
    expect(policy).toContain("style-src 'self' 'unsafe-inline'");
    expect(policy).toContain("img-src 'self' data: blob: https://media.example.com");
    expect(policy).toContain(
      "connect-src 'self' https://api.example.com https://uploads-a.example.com " +
        "https://uploads-b.example.com https://cognito-idp.us-east-1.amazonaws.com " +
        "https://my-pool.auth.us-east-1.amazoncognito.com",
    );
    expect(policy).toContain("font-src 'self'");
    expect(policy).toContain("base-uri 'self'");
    expect(policy).toContain("form-action 'self' https://my-pool.auth.us-east-1.amazoncognito.com");
    expect(policy).toContain("worker-src 'self'");
    expect(policy).toContain("manifest-src 'self'");

    // No path/query leaked through into the policy — only the origin.
    expect(policy).not.toContain("/some/path");
    expect(policy).not.toContain("/v1");
    expect(policy).not.toContain("/put");
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
    const policy = buildCspPolicy({
      mediaBaseUrl: "",
      apiBaseUrl: "",
      cognitoHostedDomain: "",
      uploadOrigins: "",
    });

    expect(policy).toContain("img-src 'self' data: blob:");
    expect(
      policy
        .split(";")
        .find((d) => d.trim().startsWith("img-src"))
        ?.trim(),
    ).toBe("img-src 'self' data: blob:");
  });

  it("drops an invalid or empty entry from uploadOrigins without dropping the valid ones", () => {
    const policy = buildCspPolicy({ uploadOrigins: "not-a-url   https://uploads.example.com  " });

    expect(policy).toContain("connect-src 'self' https://uploads.example.com");
  });

  it("joins directives with '; ' and never leaves a %VITE token anywhere", () => {
    const policy = buildCspPolicy({
      mediaBaseUrl: "https://media.example.com",
      apiBaseUrl: "https://api.example.com",
      cognitoHostedDomain: "pool.auth.us-east-1.amazoncognito.com",
    });

    expect(policy.split("; ").length).toBeGreaterThanOrEqual(10);
    expect(policy).not.toMatch(/%VITE/);
  });
});

describe("missingProdCspVars", () => {
  it("names nothing when the media origin and an upload origin are valid URLs", () => {
    expect(
      missingProdCspVars({
        mediaBaseUrl: "https://media.example.com",
        uploadOrigins: "https://uploads-a.example.com https://uploads-b.example.com",
      }),
    ).toEqual([]);
  });

  it("names each variable that's unset, empty or unparseable", () => {
    expect(missingProdCspVars({})).toEqual(["VITE_MEDIA_BASE_URL", "VITE_UPLOAD_ORIGINS"]);
    expect(missingProdCspVars({ mediaBaseUrl: "", uploadOrigins: " " })).toEqual([
      "VITE_MEDIA_BASE_URL",
      "VITE_UPLOAD_ORIGINS",
    ]);
    expect(
      missingProdCspVars({
        mediaBaseUrl: "not a url",
        uploadOrigins: "https://uploads.example.com",
      }),
    ).toEqual(["VITE_MEDIA_BASE_URL"]);
  });
});

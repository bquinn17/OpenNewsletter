import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { User } from "oidc-client-ts";
import { afterEach, describe, expect, it, vi } from "vitest";
import { AuthContext, type AuthValue } from "../../auth/useAuth";
import { userManager } from "../../auth/userManager";
import { env } from "../../env";
import { DemoBanner } from "./DemoBanner";

const assign = vi.fn();
vi.mock("../../utils/navigation", () => ({ assignLocation: (url: string) => assign(url) }));

const demoAuth: AuthValue = {
  status: "authenticated",
  user: { sub: "mock-sub", email: "quinn@example.com", name: "Quinn" },
  login: vi.fn(),
  logout: vi.fn(),
};

function renderBanner() {
  render(
    <AuthContext.Provider value={demoAuth}>
      <DemoBanner />
    </AuthContext.Provider>,
  );
}

afterEach(async () => {
  vi.clearAllMocks();
  await userManager!.removeUser();
});

describe("DemoBanner", () => {
  it("offers a signed-out visitor a real sign-in, which leaves the demo", async () => {
    renderBanner();

    await userEvent.click(screen.getByRole("button", { name: "Sign in" }));

    expect(screen.getByText(/nothing you do here is saved/)).toBeInTheDocument();
    expect(demoAuth.login).toHaveBeenCalledWith({ returnTo: "/" });
  });

  it("tells a signed-in visitor these aren't their groups, and offers no second sign-in", async () => {
    await userManager!.storeUser(
      new User({
        id_token: "h.e30.s",
        access_token: "a",
        token_type: "Bearer",
        expires_at: Math.floor(Date.now() / 1000) + 3600,
        profile: { sub: "real-sub", aud: env.cognitoClientId, iss: "i", exp: 0, iat: 0 },
      }),
    );

    renderBanner();

    expect(await screen.findByText(/these aren't your groups/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Sign in" })).not.toBeInTheDocument();
  });

  it("exits the demo with a full page load of the real home", async () => {
    renderBanner();

    await userEvent.click(screen.getByRole("button", { name: "Exit demo" }));

    expect(assign).toHaveBeenCalledWith("/");
  });
});

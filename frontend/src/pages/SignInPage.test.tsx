import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import { rememberProvider } from "../auth/lastProvider";
import { AuthContext, type AuthValue } from "../auth/useAuth";
import { SignInPage } from "./SignInPage";

const signedOut: AuthValue = {
  status: "unauthenticated",
  user: null,
  login: vi.fn().mockResolvedValue(undefined),
  logout: vi.fn(),
};

function renderSignIn(auth: AuthValue, url = "/signin?returnTo=%2Fsettings") {
  return render(
    <AuthContext.Provider value={auth}>
      <MemoryRouter initialEntries={[url]}>
        <Routes>
          <Route path="/signin" element={<SignInPage />} />
          <Route path="/settings" element={<div>Settings page</div>} />
          <Route path="/" element={<div>Home page</div>} />
        </Routes>
      </MemoryRouter>
    </AuthContext.Provider>,
  );
}

afterEach(() => vi.clearAllMocks());

describe("SignInPage", () => {
  it("signs in straight with the picked provider, keeping returnTo", async () => {
    renderSignIn(signedOut);

    await userEvent.click(screen.getByRole("button", { name: /Continue with Facebook/ }));

    expect(signedOut.login).toHaveBeenCalledWith({
      provider: "Facebook",
      returnTo: "/settings",
      invite: undefined,
    });
    expect(screen.getByRole("button", { name: /Opening Facebook/ })).toBeDisabled();
  });

  it("drops an off-site returnTo", async () => {
    renderSignIn(signedOut, "/signin?returnTo=%2F%2Fevil.example");

    await userEvent.click(screen.getByRole("button", { name: /Continue with Google/ }));

    expect(signedOut.login).toHaveBeenCalledWith(expect.objectContaining({ returnTo: "/" }));
  });

  it("marks the provider this browser used last", () => {
    rememberProvider("Google");

    renderSignIn(signedOut);

    expect(screen.getByRole("button", { name: /Continue with Google.*Last used/ })).toBeVisible();
    expect(screen.queryByRole("button", { name: /Facebook.*Last used/ })).not.toBeInTheDocument();
  });

  it("re-enables the buttons and says so when sign-in can't start", async () => {
    let reject!: (error: Error) => void;
    const login = vi.fn(() => new Promise<void>((_, rej) => (reject = rej)));
    renderSignIn({ ...signedOut, login });

    await userEvent.click(screen.getByRole("button", { name: /Continue with Google/ }));
    await act(async () => reject(new Error("offline")));

    expect(screen.getByRole("alert")).toHaveTextContent("Couldn't reach sign-in");
    expect(screen.getByRole("button", { name: /Continue with Google/ })).toBeEnabled();
  });

  it("sends someone already signed in on to returnTo", () => {
    renderSignIn({
      ...signedOut,
      status: "authenticated",
      user: { sub: "s", email: null, name: null },
    });

    expect(screen.getByText("Settings page")).toBeInTheDocument();
  });

  it("points newcomers at signing up with an invite code", () => {
    renderSignIn(signedOut);

    expect(screen.getByRole("link", { name: /Sign up with your invite code/ })).toHaveAttribute(
      "href",
      "/join",
    );
  });
});

import { render, screen } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";
import { AuthContext, type AuthValue } from "./useAuth";
import { RequireAuth } from "./RequireAuth";

function renderWithAuth(auth: AuthValue) {
  return render(
    <MemoryRouter initialEntries={["/g/g_trail/upcoming?tab=vote"]}>
      <AuthContext.Provider value={auth}>
        <Routes>
          <Route element={<RequireAuth />}>
            <Route path="/g/:groupId/upcoming" element={<div>Protected content</div>} />
          </Route>
        </Routes>
      </AuthContext.Provider>
    </MemoryRouter>,
  );
}

describe("RequireAuth", () => {
  it("calls login with the current location when unauthenticated", () => {
    const login = vi.fn().mockResolvedValue(undefined);
    renderWithAuth({ status: "unauthenticated", user: null, login, logout: vi.fn() });

    expect(login).toHaveBeenCalledWith({ returnTo: "/g/g_trail/upcoming?tab=vote" });
    expect(screen.getByRole("status")).toBeInTheDocument();
  });

  it("renders the protected route when authenticated", () => {
    const user = { sub: "u_quinn", email: "quinn@example.com", name: "Quinn" };
    renderWithAuth({ status: "authenticated", user, login: vi.fn(), logout: vi.fn() });

    expect(screen.getByText("Protected content")).toBeInTheDocument();
  });

  it("shows a loading indicator while status is loading", () => {
    renderWithAuth({ status: "loading", user: null, login: vi.fn(), logout: vi.fn() });

    expect(screen.getByRole("status")).toBeInTheDocument();
    expect(screen.queryByText("Protected content")).not.toBeInTheDocument();
  });
});

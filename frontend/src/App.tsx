import { createBrowserRouter, Navigate, RouterProvider } from "react-router-dom";
import { env } from "./env";
import { AppShell } from "./components/layout/AppShell";
import { RequireAuth } from "./auth/RequireAuth";
import { AuthCallbackPage } from "./pages/AuthCallbackPage";
import { BootstrapLoginPage } from "./pages/BootstrapLoginPage";
import { HomePage } from "./pages/HomePage";
import { JoinPage } from "./pages/JoinPage";
import { SettingsPage } from "./pages/SettingsPage";
import { GroupRedirect } from "./pages/GroupRedirect";
import { PendingMilestonePage } from "./pages/PendingMilestonePage";
import { NotFoundPage } from "./pages/NotFoundPage";
import { ErrorPage } from "./pages/ErrorPage";
import { RequireMembership } from "./routes/RequireMembership";
import { RequireGroupAdmin } from "./routes/RequireGroupAdmin";
import { CandidatesPage } from "./pages/CandidatesPage";
import { SuggestPage } from "./pages/SuggestPage";
import { NewsletterPage } from "./pages/NewsletterPage";
import { RespondPage } from "./pages/RespondPage";
import { GroupAdminPage } from "./pages/GroupAdminPage";

const router = createBrowserRouter([
  { path: "/auth/callback", element: <AuthCallbackPage /> },
  {
    path: "/",
    element: <AppShell />,
    errorElement: <ErrorPage />,
    children: [
      { index: true, element: <HomePage /> },
      { path: "join", element: <JoinPage /> },
      ...(env.appEnv === "dev"
        ? [{ path: "admin/bootstrap-login", element: <BootstrapLoginPage /> }]
        : []),
      {
        element: <RequireAuth />,
        children: [
          { path: "settings", element: <SettingsPage /> },
          {
            path: "g/:groupId",
            element: <RequireMembership />,
            children: [
              { index: true, element: <GroupRedirect /> },
              { path: "upcoming", element: <CandidatesPage /> },
              { path: "upcoming/suggest", element: <SuggestPage /> },
              { path: "n/:cycleId", element: <NewsletterPage /> },
              { path: "n/:cycleId/respond/:questionId", element: <RespondPage /> },
              { path: "n/:cycleId/respond", element: <Navigate to=".." replace /> },
              {
                element: <RequireGroupAdmin />,
                children: [
                  {
                    path: "admin",
                    element: env.useMocks ? (
                      <GroupAdminPage />
                    ) : (
                      <PendingMilestonePage message="Group administration arrives in a later release." />
                    ),
                  },
                ],
              },
            ],
          },
        ],
      },
      { path: "*", element: <NotFoundPage /> },
    ],
  },
]);

export function App() {
  return <RouterProvider router={router} />;
}

import { useEffect } from "react";
import { createBrowserRouter, Navigate, RouterProvider } from "react-router-dom";
import { env } from "./env";
import { useAuth } from "./auth/useAuth";
import { useConfig } from "./api/queries";
import { ensurePushSubscription } from "./pwa/pushSetup";
import { useInstallPrompt, type BeforeInstallPromptEvent } from "./state/installPrompt";
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
  const { status } = useAuth();
  const { data: config } = useConfig();
  const captureInstallPrompt = useInstallPrompt((s) => s.capture);

  // Re-subscribe-on-load (`07-notifications.md` §11.2): refreshes an
  // already-granted subscription so `lastSuccessAt` keeps ticking and a
  // dropped browser subscription reattaches. Never prompts — `prompt: false`.
  useEffect(() => {
    if (status !== "authenticated" || !config?.vapidPublicKey) return;
    ensurePushSubscription(config.vapidPublicKey, { prompt: false }).catch(() => undefined);
  }, [status, config?.vapidPublicKey]);

  // Install prompt capture (`04-frontend-architecture.md` §14.4).
  useEffect(() => {
    function handler(event: Event) {
      event.preventDefault();
      captureInstallPrompt(event as BeforeInstallPromptEvent);
    }
    window.addEventListener("beforeinstallprompt", handler);
    return () => window.removeEventListener("beforeinstallprompt", handler);
  }, [captureInstallPrompt]);

  return <RouterProvider router={router} />;
}

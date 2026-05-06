import { createBrowserRouter, Navigate, RouterProvider } from "react-router-dom";
import { AppShell } from "./components/layout/AppShell";
import { HomePage } from "./pages/HomePage";
import { NewsletterPage } from "./pages/NewsletterPage";
import { RespondPage } from "./pages/RespondPage";
import { CandidatesPage } from "./pages/CandidatesPage";
import { SuggestPage } from "./pages/SuggestPage";
import { SettingsPage } from "./pages/SettingsPage";
import { GroupAdminPage } from "./pages/GroupAdminPage";
import { JoinPage } from "./pages/JoinPage";
import { NotFoundPage } from "./pages/NotFoundPage";
import { GroupRedirect } from "./pages/GroupRedirect";

const router = createBrowserRouter([
  {
    path: "/",
    element: <AppShell />,
    children: [
      { index: true, element: <HomePage /> },
      { path: "g/:groupId", element: <GroupRedirect /> },
      { path: "g/:groupId/upcoming", element: <CandidatesPage /> },
      { path: "g/:groupId/upcoming/suggest", element: <SuggestPage /> },
      { path: "g/:groupId/n/:cycleId", element: <NewsletterPage /> },
      { path: "g/:groupId/n/:cycleId/respond/:questionId", element: <RespondPage /> },
      { path: "g/:groupId/n/:cycleId/respond", element: <Navigate to=".." replace /> },
      { path: "g/:groupId/admin", element: <GroupAdminPage /> },
      { path: "settings", element: <SettingsPage /> },
      { path: "join", element: <JoinPage /> },
      { path: "*", element: <NotFoundPage /> },
    ],
  },
]);

export function App() {
  return <RouterProvider router={router} />;
}

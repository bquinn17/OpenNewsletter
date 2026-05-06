import { Outlet } from "react-router-dom";
import { ToastStack } from "../ui/Toast";

export function AppShell() {
  return (
    <div className="canvas-bg min-h-screen">
      <div className="app-container">
        <Outlet />
      </div>
      <ToastStack />
    </div>
  );
}

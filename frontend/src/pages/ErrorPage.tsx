import { Link, isRouteErrorResponse, useRouteError } from "react-router-dom";
import { ApiError } from "../api/client";

export function ErrorPage() {
  const error = useRouteError();
  const routeError = isRouteErrorResponse(error) ? error : null;
  const apiError = error instanceof ApiError ? error : null;
  const status = routeError?.status ?? apiError?.status;
  const is404 = status === 404;
  const title = is404
    ? "Lost in the fog"
    : status
      ? `${status} — ${routeError?.statusText ?? apiError?.code ?? "Error"}`
      : "Something broke";
  const detail = is404
    ? "That page doesn't exist (or hasn't been built yet)."
    : apiError
      ? (apiError.problem?.detail ?? apiError.message)
      : routeError?.data && typeof routeError.data === "string"
        ? routeError.data
        : error instanceof Error
          ? error.message
          : "An unexpected error happened. Try again, or head home.";
  const correlationId = apiError?.problem?.correlationId;

  return (
    <div className="flex min-h-screen flex-col items-center justify-center bg-cream px-6 text-center">
      <div className="mb-3 text-6xl">{is404 ? "🌫️" : "🛠️"}</div>
      <h1 className="font-display text-3xl font-bold">{title}</h1>
      <p className="mt-2 max-w-sm text-inkmuted">{detail}</p>
      {correlationId && <p className="mt-1 text-xs text-inkmuted">Reference: {correlationId}</p>}
      <div className="mt-6 flex gap-3">
        <Link to="/" className="rounded-full bg-ink px-5 py-3 font-semibold text-cream">
          Take me home
        </Link>
        <button
          onClick={() => window.location.reload()}
          className="rounded-full border border-line bg-white px-5 py-3 font-semibold"
        >
          Reload
        </button>
      </div>
    </div>
  );
}

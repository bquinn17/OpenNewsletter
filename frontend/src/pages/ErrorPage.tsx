import { Link, isRouteErrorResponse, useRouteError } from "react-router-dom";

export function ErrorPage() {
  const error = useRouteError();
  const routeError = isRouteErrorResponse(error) ? error : null;
  const status = routeError?.status;
  const is404 = status === 404;
  const title = is404 ? "Lost in the fog" : status ? `${status} — ${routeError?.statusText ?? "Error"}` : "Something broke";
  const detail = is404
    ? "That page doesn't exist (or hasn't been built yet)."
    : routeError?.data && typeof routeError.data === "string"
      ? routeError.data
      : error instanceof Error
        ? error.message
        : "An unexpected error happened. Try again, or head home.";

  return (
    <div className="bg-cream min-h-screen flex flex-col items-center justify-center text-center px-6">
      <div className="text-6xl mb-3">{is404 ? "🌫️" : "🛠️"}</div>
      <h1 className="font-display text-3xl font-bold">{title}</h1>
      <p className="text-inkmuted mt-2 max-w-sm">{detail}</p>
      <div className="mt-6 flex gap-3">
        <Link to="/" className="px-5 py-3 rounded-full bg-ink text-cream font-semibold">
          Take me home
        </Link>
        <button
          onClick={() => window.location.reload()}
          className="px-5 py-3 rounded-full bg-white border border-line font-semibold"
        >
          Reload
        </button>
      </div>
    </div>
  );
}

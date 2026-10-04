import { Link } from "react-router-dom";

export function NotFoundPage() {
  return (
    <div className="flex min-h-screen flex-col items-center justify-center bg-cream px-6 text-center">
      <div className="mb-3 text-6xl">🌫️</div>
      <h1 className="font-display text-3xl font-bold">Lost in the fog</h1>
      <p className="mt-2 text-inkmuted">
        That page doesn&apos;t exist (or hasn&apos;t been built yet).
      </p>
      <Link to="/" className="mt-6 rounded-full bg-ink px-5 py-3 font-semibold text-cream">
        Take me home
      </Link>
    </div>
  );
}

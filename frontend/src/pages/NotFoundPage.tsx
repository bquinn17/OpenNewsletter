import { Link } from "react-router-dom";

export function NotFoundPage() {
  return (
    <div className="bg-cream min-h-screen flex flex-col items-center justify-center text-center px-6">
      <div className="text-6xl mb-3">🌫️</div>
      <h1 className="font-display text-3xl font-bold">Lost in the fog</h1>
      <p className="text-inkmuted mt-2">That page doesn't exist (or hasn't been built yet).</p>
      <Link to="/" className="mt-6 px-5 py-3 rounded-full bg-ink text-cream font-semibold">
        Take me home
      </Link>
    </div>
  );
}

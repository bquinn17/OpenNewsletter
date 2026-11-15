import { useEffect, useState, type CSSProperties } from "react";
import { useCelebration } from "../../state/celebration";

// The accent palette (`tailwind.config.ts`).
const COLORS = ["#FF5A6B", "#FFB178", "#7B5BFF", "#5AC8FF", "#3DD9A4", "#FFD23F"];
const PIECE_COUNT = 70;
const BURST_MS = 3600;

type Piece = { id: number; style: CSSProperties };

function makePieces(): Piece[] {
  return Array.from({ length: PIECE_COUNT }, (_, id) => {
    const size = 6 + Math.random() * 6;
    return {
      id,
      style: {
        left: `${Math.random() * 100}%`,
        width: size,
        height: size * (Math.random() < 0.5 ? 0.45 : 1),
        borderRadius: Math.random() < 0.3 ? "999px" : "2px",
        background: COLORS[id % COLORS.length],
        animationDelay: `${Math.random() * 0.5}s`,
        animationDuration: `${2.2 + Math.random() * 1.2}s`,
        "--confetti-drift": `${(Math.random() - 0.5) * 220}px`,
        "--confetti-spin": `${360 + Math.random() * 720}deg`,
      } as CSSProperties,
    };
  });
}

/**
 * Decorative confetti for `useCelebration`. Mounted once in `AppShell`, so a
 * burst keeps falling across the navigation that usually follows it. Hidden
 * under `prefers-reduced-motion`.
 */
export function ConfettiBurst() {
  const burst = useCelebration((s) => s.burst);
  const [pieces, setPieces] = useState<Piece[]>([]);

  useEffect(() => {
    if (burst === 0) return;
    setPieces(makePieces());
    const timer = setTimeout(() => setPieces([]), BURST_MS);
    return () => clearTimeout(timer);
  }, [burst]);

  if (pieces.length === 0) return null;

  return (
    <div
      aria-hidden="true"
      className="pointer-events-none fixed inset-0 z-50 overflow-hidden motion-reduce:hidden"
    >
      {pieces.map((piece) => (
        <span
          key={piece.id}
          className="absolute -top-4 block animate-confetti"
          style={piece.style}
        />
      ))}
    </div>
  );
}

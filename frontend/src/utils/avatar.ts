// Map mock avatar color slugs to Tailwind classes.

const palette: Record<string, { bg: string; text: string }> = {
  coral: { bg: "bg-coral", text: "text-white" },
  grape: { bg: "bg-grape", text: "text-white" },
  mint: { bg: "bg-mint", text: "text-white" },
  sky: { bg: "bg-sky", text: "text-white" },
  sun: { bg: "bg-sun", text: "text-ink" },
  peach: { bg: "bg-peach", text: "text-ink" },
};

export function avatarClasses(color?: string): string {
  const c = color && palette[color] ? palette[color] : palette.grape!;
  return `${c.bg} ${c.text}`;
}

export function initial(name: string): string {
  return name.trim().charAt(0).toUpperCase() || "?";
}

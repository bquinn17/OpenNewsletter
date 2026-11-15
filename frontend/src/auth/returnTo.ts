/**
 * A `returnTo` read from a URL, kept to an in-app path. `/signin?returnTo=` is
 * a link anyone can craft, so a value that isn't a same-origin path (`//host`,
 * `/\host`, `https://…`) goes Home instead of off-site.
 */
export function safeReturnTo(value: string | null | undefined): string {
  if (!value || !value.startsWith("/") || value.startsWith("//") || value.startsWith("/\\")) {
    return "/";
  }
  return value;
}

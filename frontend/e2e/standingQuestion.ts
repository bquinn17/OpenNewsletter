import { readFileSync } from "node:fs";

/** The standing question's prompt, read from the file the app itself uses (`docs/design/lifecycle.md` §14). */
export const STANDING_QUESTION_PROMPT = readFileSync(
  new URL("../../shared/standing-question.txt", import.meta.url),
  "utf8",
).trim();

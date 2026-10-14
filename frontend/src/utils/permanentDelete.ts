/**
 * Copy for every permanent image delete: a response-editor image, a comment
 * image removed or replaced, and an avatar removed or replaced
 * (`docs/design/api.md` §9.4/§9.7, D8). Media deletes are real, with no
 * bucket versioning to undo them.
 */
export const PERMANENT_IMAGE_DELETE_MESSAGE =
  "This permanently deletes the image. It can't be recovered.";

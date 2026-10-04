import { beforeEach, describe, expect, it } from "vitest";
import { stashInvite, takeStashedInvite } from "./inviteStash";

describe("inviteStash", () => {
  beforeEach(() => {
    window.sessionStorage.clear();
  });

  it("returns null when nothing has been stashed", () => {
    expect(takeStashedInvite()).toBeNull();
  });

  it("returns the stashed code and clears it", () => {
    stashInvite("DEMO-JOIN-CODE");

    expect(takeStashedInvite()).toBe("DEMO-JOIN-CODE");
    expect(takeStashedInvite()).toBeNull();
  });

  it("overwrites a previous stash with the latest code", () => {
    stashInvite("FIRST-CODE");
    stashInvite("SECOND-CODE");

    expect(takeStashedInvite()).toBe("SECOND-CODE");
  });
});

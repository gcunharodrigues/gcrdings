import { describe, expect, test } from "bun:test";
import { validateSessionSeek } from "../../src/lib/session-audio";

describe("validateSessionSeek", () => {
  test("accepts the exact stored timestamp when audio is ready", () => {
    expect(validateSessionSeek(125.25, 300, true)).toEqual({
      ok: true,
      seconds: 125.25,
    });
  });

  test.each([Number.NaN, Number.POSITIVE_INFINITY, -0.01])(
    "rejects invalid timestamp %p",
    (seconds) => {
      expect(validateSessionSeek(seconds, 300, true)).toEqual({
        ok: false,
        reason: "invalid-time",
      });
    },
  );

  test("rejects seek before audio is ready", () => {
    expect(validateSessionSeek(1, 300, false)).toEqual({
      ok: false,
      reason: "not-ready",
    });
  });

  test("rejects timestamp beyond the mixed track", () => {
    expect(validateSessionSeek(301, 300, true)).toEqual({
      ok: false,
      reason: "past-end",
    });
  });
});

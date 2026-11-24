// @ts-nocheck
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import {
  READINESS_UNAVAILABLE_LABEL,
  READINESS_UNAVAILABLE_MESSAGE,
  createRuntimeReadinessRefresher,
} from "./runtime-readiness.ts";

function deferred<T>() {
  let resolve: (value: T) => void;
  let reject: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

test("keeps prior readiness while pending, clears to unavailable on its own failure, and recovers", async () => {
  const commits = [];
  const pending = [];
  const refresher = createRuntimeReadinessRefresher(
    () => {
      const next = deferred();
      pending.push(next);
      return next.promise;
    },
    (value) => commits.push(value),
  );

  const first = refresher();
  assert.deepEqual(commits, []);
  pending[0].resolve({ color: "green", label: "Safe", operator_message: "ok" });
  await first;
  assert.deepEqual(commits, [{ color: "green", label: "Safe", operator_message: "ok" }]);

  const second = refresher();
  assert.deepEqual(commits, [{ color: "green", label: "Safe", operator_message: "ok" }]);
  pending[1].reject(new Error("runtime endpoint down"));
  await second;
  assert.deepEqual(commits, [
    { color: "green", label: "Safe", operator_message: "ok" },
    null,
  ]);
  assert.equal(READINESS_UNAVAILABLE_LABEL, "Readiness unavailable");
  assert.match(READINESS_UNAVAILABLE_MESSAGE, /unknown|could not|unavailable/i);

  const third = refresher();
  pending[2].resolve({ color: "yellow", label: "Degraded", operator_message: "check" });
  await third;
  assert.deepEqual(commits.at(-1), { color: "yellow", label: "Degraded", operator_message: "check" });
});

const newestOverlapCases = [
  {
    name: "newest success wins over an older overlapping success",
    settle: (pending) => {
      pending[1].resolve({ color: "yellow", label: "Newest" });
      pending[0].resolve({ color: "green", label: "Stale" });
    },
    expected: [{ color: "yellow", label: "Newest" }],
  },
  {
    name: "newest success wins over an older overlapping failure",
    settle: (pending) => {
      pending[1].resolve({ color: "yellow", label: "Newest" });
      pending[0].reject(new Error("stale failure"));
    },
    expected: [{ color: "yellow", label: "Newest" }],
  },
  {
    name: "newest failure clears even when an older success settles later",
    settle: (pending) => {
      pending[1].reject(new Error("newest failed"));
      pending[0].resolve({ color: "green", label: "Stale safe" });
    },
    expected: [null],
  },
];

for (const { name, settle, expected } of newestOverlapCases) {
  test(name, async () => {
    const commits = [];
    const pending = [];
    const refresher = createRuntimeReadinessRefresher(
      () => {
        const next = deferred();
        pending.push(next);
        return next.promise;
      },
      (value) => commits.push(value),
    );

    const first = refresher();
    const second = refresher();
    settle(pending);
    await second;
    await first;

    assert.deepEqual(commits, expected);
  });
}

test("App refresh wires readiness before ancillary and awaits it in finally", async () => {
  const source = await readFile(new URL("./main.tsx", import.meta.url), "utf8");
  const readinessStart = source.indexOf("const readiness = refreshReadiness();");
  assert.ok(readinessStart >= 0, "refreshReadiness call not found");
  const refreshEnd = source.indexOf("}, [refreshReadiness]);", readinessStart);
  assert.ok(refreshEnd > readinessStart, "refresh function end not found");
  const body = source.slice(readinessStart, refreshEnd);
  const ancillaryStart = body.indexOf("await Promise.all([");
  assert.ok(ancillaryStart > 0, "ancillary Promise.all not found in refresh body");
  assert.equal(body.slice(0, ancillaryStart).includes("await Promise.all(["), false);
  const ancillaryEnd = body.indexOf("]);", ancillaryStart);
  assert.ok(ancillaryEnd > ancillaryStart);
  const ancillaryBlock = body.slice(ancillaryStart, ancillaryEnd);
  assert.equal(ancillaryBlock.includes("runtime-safe-to-air"), false);
  assert.match(body, /finally \{ await readiness; \}/);
  assert.ok((source.match(/READINESS_UNAVAILABLE_LABEL/g) || []).length >= 3);
  assert.ok((source.match(/READINESS_UNAVAILABLE_MESSAGE/g) || []).length >= 2);
  assert.equal(source.includes("Service ready"), false);
});

test("readiness success commits even when ancillary refresh rejects", async () => {
  const commits = [];
  const refresher = createRuntimeReadinessRefresher(
    async () => ({ color: "green", label: "Safe", operator_message: "ok" }),
    (v) => commits.push(v),
  );
  const readiness = refresher();
  try {
    await Promise.reject(new Error("ancillary endpoint down"));
  } catch {
    // Simulates App catch block; readiness must still be awaited in finally.
  } finally {
    await readiness;
  }
  assert.deepEqual(commits, [{ color: "green", label: "Safe", operator_message: "ok" }]);
});

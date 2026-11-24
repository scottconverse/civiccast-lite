export const READINESS_UNAVAILABLE_LABEL = "Readiness unavailable";
export const READINESS_UNAVAILABLE_MESSAGE = "Runtime safe-to-air status could not be refreshed. Treat readiness as unknown until a successful update.";

export function createRuntimeReadinessRefresher<T>(
  load: () => Promise<T>,
  commit: (value: T | null) => void,
): () => Promise<void> {
  let newestRun = 0;
  return async () => {
    const run = ++newestRun;
    let value: T | null = null;
    try {
      value = await load();
    } catch {
      value = null;
    }
    if (run !== newestRun) return;
    commit(value);
  };
}

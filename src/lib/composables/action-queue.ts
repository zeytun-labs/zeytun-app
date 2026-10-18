/** Serialize related writes without changing the table's visual/loading state. */
export function createActionQueue(getScope: () => unknown = () => null) {
  let tail: Promise<unknown> = Promise.resolve();
  return {
    run<T>(action: () => Promise<T>): Promise<T> {
      const scope = getScope();
      const result = tail.then(() => {
        if (getScope() !== scope) throw new Error("The active profile changed. Try the action again.");
        return action();
      });
      tail = result.catch(() => {});
      return result;
    },
  };
}

export function useRowSelection<Id extends string | number>(
  getIds: () => Id[],
  getScope: () => unknown = () => null,
) {
  let stored = $state<Id[]>([]);
  let anchor: Id | null = null;
  const ids = $derived(stored.filter((id) => getIds().includes(id)));
  const scope = $derived(getScope());
  $effect(() => {
    scope;
    stored = [];
    anchor = null;
  });

  function replace(next: Id[]) { stored = [...new Set(next)]; }
  function toggle(id: Id, checked: boolean) {
    anchor = id;
    replace(checked ? [...ids, id] : ids.filter((item) => item !== id));
  }
  function select(id: Id, event?: MouseEvent, visible = getIds()) {
    if (event?.shiftKey && anchor !== null) {
      const start = visible.indexOf(anchor);
      const end = visible.indexOf(id);
      if (start >= 0 && end >= 0) {
        const range = visible.slice(Math.min(start, end), Math.max(start, end) + 1);
        replace(event.metaKey || event.ctrlKey ? [...ids, ...range] : range);
        return;
      }
    }
    if (event?.metaKey || event?.ctrlKey) toggle(id, !ids.includes(id));
    else { anchor = id; replace([id]); }
  }
  function selectAll(checked: boolean, visible = getIds()) {
    replace(checked ? [...ids, ...visible] : ids.filter((id) => !visible.includes(id)));
  }
  return { get ids() { return ids; }, replace, toggle, select, selectAll };
}

export type RowSelection<Id extends string | number> = ReturnType<typeof useRowSelection<Id>>;

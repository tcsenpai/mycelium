// Web build has no Tauri runtime. The ported App.tsx imports `listen` from
// '@tauri-apps/api/event' for two desktop-only events (database-changed,
// quick-add). On the web, react-query polling handles refresh and quick-add
// is reachable via the UI, so listen() is a no-op returning an unlisten fn.
// A vite alias maps '@tauri-apps/api/event' to this module.

type EventCallback = (event: { payload: unknown }) => void;

export async function listen(_event: string, _cb: EventCallback): Promise<() => void> {
  return () => {};
}

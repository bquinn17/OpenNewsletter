# OpenNewsletter — Frontend

React 18 + Vite + TypeScript + Tailwind app. The API client (`src/api/client.ts`) returns
mocked data with simulated latency, so the app runs end-to-end with no backend.

## Run

```bash
cd frontend
nvm use            # pick up .nvmrc → Node 20
npm install
npm run dev        # http://localhost:5173
```

Other scripts:

```bash
npm run build      # production bundle
npm run typecheck  # tsc --noEmit
```

## Stack notes

- **Routing**: React Router v6 data routers — see `src/App.tsx`.
- **Server state**: TanStack Query — hooks live in `src/api/queries.ts`.
- **Client state**: Zustand stores in `src/state/`.
- **Forms**: react-hook-form + zod (see `SuggestPage`).
- **Markdown**: react-markdown + remark-gfm + rehype-sanitize for published answers.
- **Mocks**: `src/api/mockData.ts` + `src/api/client.ts`. Mutations write back to the
  in-memory store, so the UI feels live across navigations within a session.
  Reload the tab to reset.

## Where to swap mocks for the real API

Replace the body of each method in `src/api/client.ts` with `fetch` against
`api.opennewsletter.example.com`. Callsites in `queries.ts` / `mutations.ts` and the
page components do not need to change.

# Brillion frontend

React + TypeScript (Vite) client for [Brillion](../SPEC.md) — talks to
the backend's GraphQL API (`SPEC.md` §6) and OAuth 2.0 provider (§7).

## Development

```sh
npm install
npm run dev
```

## Scripts

- `npm run dev` — Vite dev server
- `npm run build` — typecheck (`tsc -b`) + production build
- `npm run lint` — oxlint
- `npm run preview` — preview the production build locally

This is currently the stock Vite `react-ts` scaffold (phase 0). Routing,
the GraphQL client, and the OAuth PKCE login flow land in phase 1.

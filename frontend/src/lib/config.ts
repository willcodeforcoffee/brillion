// Backend origin — same one the OAuth redirect_uri and GraphQL client
// both target. Override with VITE_API_BASE for non-default setups; the
// default matches `cargo run -- serve`'s default port (SPEC.md §10).
export const API_BASE = import.meta.env.VITE_API_BASE ?? 'http://localhost:3000'

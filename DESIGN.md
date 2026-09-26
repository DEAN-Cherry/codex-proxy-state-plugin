# State observation page

The page follows `codex-proxy-rs/docs/theme.md` and the official workbench plugin entry pattern. Public `@codex-proxy/ui` is pinned to the v0.2.0 commit. Host-injected `--cp-*` tokens remain the theme authority; the page neither reads cookies nor fetches host endpoints. The IIFE entry and relative stylesheet are compatible with the host resource rewriting pipeline. `dist/` contains `index.html`, `app.js`, and `app.css`, which packaging places under `web/`.

## Composition

Use shared BaseButton, BaseInput, BaseCard and BaseTag primitives, their font stack, semantic colors, focus treatment and density. Tailwind's standard 4px spacing scale and the public theme's typography/radius/shadow tokens provide the layout scale. A compact toolbar leads into the model table and adjacent settings panel at large widths; smaller widths stack them. Tables scroll within their own region, never the page. Model actions are real keyboard-focusable buttons.

Account labels contain only IDs. Paging explicitly loads the next account page. Observation details contain fingerprints, length distributions and retained recent history, never raw State. Rule results are not account health. Local expiry is not upstream validity. Counts represent bounded retained observations, not lifetime totals or complete auditing.

## Data and editing

All API replies are parsed as unknown and validated at the bridge boundary. Queries use the bridge's separate query field. Refresh retains dirty settings and their original configuration version. A 409 retains the draft and blocks saving until the user explicitly loads the current configuration, preventing silent overwrites. A null expectedVersion is sent only when the fetched account has no configuration. Account changes are blocked while a draft is dirty; discard is explicit. Request sequence IDs prevent stale responses from replacing a newer account.

Production includes no fixture bridge or generated observations. `frontend/preview.html` is an explicit development-only Vite entry with isolated fixtures and public theme initialization, never referenced from `main.ts` or included in `dist/`. Add `?theme=dark` or `?conflict` for those preview states. Real host integration and package validation remain owned by the parent task.

## Verification

Pinned dependencies install through pnpm, including the allowlisted public UI preparation script. `pnpm typecheck`, `pnpm lint`, `pnpm test`, and `pnpm build` validate the frontend. Boundary tests cover rule parsing, model normalization, bridge pagination query encoding, malformed replies and typed 409 handling. No tests rely on sleeps or prose matching. The local language-server diagnostic tool lacks its executable; vue-tsc supplies compiler diagnostics instead. The parent owns production binary/bridge roundtrip QA and final packaging.

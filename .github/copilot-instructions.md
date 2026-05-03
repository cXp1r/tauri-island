# Project Guidelines

## Code Style
- Frontend uses Vanilla TypeScript with direct DOM manipulation patterns from [dynamic-island/src/main.ts](dynamic-island/src/main.ts) and [dynamic-island/src/settings.ts](dynamic-island/src/settings.ts).
- Backend uses Rust modules organized by capability; follow existing module split in [dynamic-island/src-tauri/src/lib.rs](dynamic-island/src-tauri/src/lib.rs).
- Keep changes focused and avoid touching build outputs under [dynamic-island/src-tauri/target](dynamic-island/src-tauri/target).

## Architecture
- App is a Tauri 2 desktop app with clear boundary:
  - Webview UI: TypeScript files under [dynamic-island/src](dynamic-island/src)
  - Native/system integration and AI proxy: Rust files under [dynamic-island/src-tauri/src](dynamic-island/src-tauri/src)
- Frontend should call Tauri commands/events for system features and AI requests instead of implementing those in browser code.
- Multi-page frontend entry is configured in [dynamic-island/vite.config.ts](dynamic-island/vite.config.ts) (`index.html` and `settings.html`).

## Build and Test
- Install deps: `cd dynamic-island && npm install`
- Frontend dev: `cd dynamic-island && npm run dev`
- Frontend build: `cd dynamic-island && npm run build`
- Tauri dev: `cd dynamic-island && npx tauri dev`
- Tauri build: `cd dynamic-island && npx tauri build`
- Windows debug helper: [dynamic-island/debug-run.bat](dynamic-island/debug-run.bat)
- If you change both frontend and Rust, prefer running `npx tauri dev` for end-to-end verification.

## Conventions
- Platform target is Windows-first; avoid introducing non-Windows-only assumptions unless guarded.
- Keep Vite dev port assumptions aligned with [dynamic-island/vite.config.ts](dynamic-island/vite.config.ts) (1420/1421).
- When adding Tauri capabilities, update [dynamic-island/src-tauri/capabilities/default.json](dynamic-island/src-tauri/capabilities/default.json).
- Settings persistence and config semantics are defined in [dynamic-island/src-tauri/src/settings.rs](dynamic-island/src-tauri/src/settings.rs); keep frontend fields compatible.

## References
- Product and architecture details: [dynamic-island/README.md](dynamic-island/README.md)
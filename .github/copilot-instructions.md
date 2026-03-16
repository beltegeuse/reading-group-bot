# Project Guidelines

## Architecture

- This is a Rocket 0.5 server with Diesel 2 + SQLite and Tera templates.
- Keep HTTP routes, form structs, template context structs, and request-flow logic in `src/main.rs`, following the existing project layout.
- Keep database access in `src/model.rs` and schema declarations in `src/schema.rs`.
- Tera templates live in `templates/`; static assets, uploaded PDFs, and generated thumbnails live under `static/`.
- Database migrations live in `migrations/` and are embedded and run automatically on startup from `src/main.rs`.

## Build And Verify

- After Rust changes, run `cargo fmt && cargo check`.
- Use `cargo run` for local development; local config is defined in `Rocket.toml` and defaults to `127.0.0.1:3001` with `db.sqlite`.
- Use `docker-compose up --build` when verifying container behavior or volume-backed storage.
- There is no maintained Rust test suite in this repo today, so do not claim test coverage unless you add and run tests yourself.

## Conventions

- Preserve the current project structure instead of introducing new modules unless the task clearly justifies it.
- Use Diesel queries inside `conn.run(|c| ...)` blocks; avoid raw SQL outside migration files.
- This codebase stores boolean-like state as `i32` flags (`is_admin`, `is_approved`, `is_disabled`, `readed`). Keep that convention unless a schema migration intentionally changes it.
- Authentication uses Rocket private cookies (`user_id`, `name`, `is_admin`). Admin-only flows should follow the existing `is_admin == 1 && is_disabled == 0` checks.
- Validate forms in route handlers and use `Flash<Redirect>` for user-facing success and error flows.
- Canonical user roles are `master_student`, `phd_student`, `prof`, and `other`. Reuse the existing normalization logic instead of adding ad hoc role variants.
- Venue handling is centralized around `ALLOWED_VENUES` in `src/main.rs`; keep the whitelist and `Other` flow consistent with the existing forms.
- Uploaded PDFs and thumbnails use UUID-based filenames and are served from `static/pdfs/` and `static/thumbnails/`.

## Pitfalls

- When changing the schema, update the migration, `src/schema.rs`, and the matching structs/methods in `src/model.rs` together.
- Startup applies embedded migrations automatically, so schema mistakes surface at application launch.
- Thumbnail generation in `src/pdf_utils.rs` depends on ImageMagick (`magick` or `convert`) and Ghostscript; keep failures non-fatal unless the task explicitly changes that behavior.
- New user registrations are pending approval by default; paper upload flows must continue to respect `is_approved` and `is_disabled`.
- Paper-title duplicate checks are currently handler-level, not enforced by the database. Do not assume uniqueness is guaranteed unless you add a DB constraint.
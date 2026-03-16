# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Reading Group Bot is a Rust web application for managing academic paper reading groups. Members can propose papers, vote on them, schedule discussions, and assign session roles.

**Stack:** Rocket 0.5 (web), Diesel 2 + SQLite (database), Tera (templates), Docker (deployment).

## Commands

```bash
cargo fmt && cargo check   # Format and type-check (run after every change)
cargo run                  # Run locally on 127.0.0.1:3001
docker-compose up --build  # Build and run in container
```

No test suite is maintained.

## Architecture

### Code Organization

| File | Purpose |
|------|---------|
| `src/main.rs` | All HTTP routes, form handling, session logic, context builders |
| `src/model.rs` | All database access — Diesel ORM models and queries |
| `src/schema.rs` | Auto-generated Diesel schema (do not edit manually) |
| `src/user.rs` | Default user seeding from `local/default_user.json` |
| `src/pdf_utils.rs` | Thumbnail generation via ImageMagick + Ghostscript |
| `templates/` | Tera templates (Jinja2-like) |
| `migrations/` | Embedded migrations, auto-run at startup |

**Rule:** HTTP/request logic stays in `main.rs`; database logic stays in `model.rs`.

### Data Model

- **logins** — users with `is_admin`, `is_approved`, `is_disabled` (all stored as `i32` 0/1), `role` (`master_student`, `phd_student`, `prof`, `other`)
- **papers** — submissions with `vote_count`, `readed`, `is_selected`, `presenter_id`, `discussed_at`
- **votes** — per-user vote records with a `value` field
- **paper_roles** — session role assignments per paper: `reviewer_friendly`, `reviewer_adversarial`, `archaeologist`, `futurist`

### Authentication & Authorization

- Sessions use Rocket private cookies storing `user_id`, `name`, `is_admin`
- Admin check: `is_admin == 1 && is_disabled == 0`
- New users are unapproved by default; paper uploads require approval

### Schema Changes

Adding or changing database columns requires three coordinated steps:
1. Create a new migration in `migrations/`
2. Update `src/schema.rs` to match
3. Update the corresponding structs in `src/model.rs`

Migration errors surface immediately at startup.

### Key Conventions

- Boolean flags are `i32` (0/1), not Rust `bool` — matches SQLite storage
- PDFs and thumbnails use UUID filenames, served from `static/pdfs/` and `static/thumbnails/`
- Allowed venues come from the `ALLOWED_VENUES` whitelist in `main.rs`
- Paper title duplicate checks are handler-level, not DB-enforced
- Thumbnail generation failures are non-fatal

## Configuration

- **`Rocket.toml`** — local: port 3001, debug log, 200 MiB file upload limit
- **`.env`** — `DATABASE_URL=db.sqlite`
- **Docker** — `ROCKET_ADDRESS=0.0.0.0`, `ROCKET_PROFILE=release`; three volumes: `db_data`, `pdfs`, `thumbnails`
- **`local/default_user.json`** — optional default user seeding (gitignored)

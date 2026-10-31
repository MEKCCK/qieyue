# Desktop parity checklist

What the Flutter original ships, where each feature gets its data, and how far the pure-Rust
desktop port has come. Endpoints are the ones the original calls (all served by `fqapi_core`).

Status key: **done** = implemented and exercised against real data; **partial** = works but the
original does more; **todo** = not started.

| Feature | Original | Endpoint(s) | Status |
| --- | --- | --- | --- |
| Search | `search_page` | `/api/v1/search`, `/search/suggest`, `/search/hot` | done (basic search); suggest/hot todo |
| Book detail + directory | `detail_page` | `/books/{id}/detail`, `/books/{id}/directory` | done |
| Reader | `reader/` | `/chapters/{item}` | done (scroll, prev/next); themes, fonts, comments anchor todo |
| Shelf add / remove | `detail_page`, reader toolbar | local `ShelfStore` | done |
| Shelf page | `library_page` | local | partial (sidebar); dedicated page with covers todo |
| Reading progress | `library_shelf_entry` | local | done |
| Categories | `category_page` | `/category?category_tab=` | done (channels, dimension groups); tag → book list todo |
| Short dramas | `drama_page`, `series_detail` | `/videos/{vid}`, `/series/{id}` | done (episode list → mpv) |
| Quality switching | `player_page` | `variants[]` in the stream response | todo |
| Home recommendation feed | `home_page`, `home_provider` | `/recommend/channels`, `/recommend/homepage`, `/recommend/series-feed` | todo |
| Rank boards | `rank_page` | rank catalogue from the novel homepage, `/rank/{id}` | todo |
| Comics | `comic_reader_page` | `/manga/{id}` | todo |
| Audiobooks | `audio`, `player`, `audio_listening` | `/audio/books/{id}`, chapter timeline | todo |
| Comments and danmaku | `detail_page`, `series_detail` | `/books/{id}/comments`, `/series/{id}/comments`, `/videos/{vid}/danmaku` | todo |
| Authors | `author_page` | `/authors/{id}` | todo |
| Cached downloads | `cached_books`, `cached_dramas` | `/videos/{vid}` download + `/src/*` | partial (drama download is implicit) |
| History | `library` | `/account/fanqie/history` | todo |
| Settings: theme, TTS, text replacement | `settings*` | local | todo |
| Mine / about | `mine_page`, `about_page` | local | todo |

## Order of work

1. Home feed (channels → tab → cells → feed) — also unlocks the rank catalogue.
2. Rank boards.
3. Comic reader (image pages, lazy loading, tap paging).
4. Audiobook playback (mpv audio + chapter list).
5. Comments and danmaku views.
6. Settings (theme, font size, text replacement) and history.

## Verification habit

Every feature is proven the same way: a headless call in `src/bin/smoke.rs` printing counts and a
sample, then a screenshot of the running window read back for the layout. No feature is reported
as done on "it compiled".

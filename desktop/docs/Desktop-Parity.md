# Desktop parity checklist

What the Flutter original ships, where each feature gets its data, and how far the pure-Rust
desktop port has come. Endpoints are the ones the original calls (all served by `fqapi_core`).

Status key: **done** = implemented and exercised against real data; **partial** = works but the
original does more; **todo** = not started.

| Feature | Original | Endpoint(s) | Status |
| --- | --- | --- | --- |
| Search | `search_page` | `/api/v1/search`, `/search/suggest`, `/search/hot` | done, with paging (offset/count, auto-load at the end); suggest/hot todo |
| Book detail + directory | `detail_page` | `/books/{id}/detail`, `/books/{id}/directory` | done |
| Reader | `reader/` | `/chapters/{item}` | done (scroll, prev/next); themes, fonts, comments anchor todo |
| Shelf add / remove | `detail_page`, reader toolbar | local `ShelfStore` | done |
| Shelf page | `library_page` | local | partial (sidebar); dedicated page with covers todo |
| Reading progress | `library_shelf_entry` | local | done |
| Categories | `category_page` | `/category?category_tab=` | done (channels, dimension groups); tag → book list todo |
| Short dramas | `drama_page`, `series_detail` | `/videos/{vid}`, `/series/{id}` | done (episode list → mpv) |
| Quality switching | `player_page` | `variants[]` in the stream response | todo |
| Home recommendation feed | `home_page`, `home_provider` | `/recommend/channels`, `/recommend/homepage`, `/recommend/series-feed` | partial: 11 channels are switchable, blocks resolve through `series-feed`; comic/manga cards still unparsed |
| Rank boards | `rank_page` | catalogue from the homepage payload, `/rank/{id}?algo_type&rank_sub_info_id&offset` | done: 6 boards, 16 categories, 30 per page, auto-load |
| Comics | `comic_reader_page` | `/manga/{item_id}` | done: pages decrypt to disk and render, 上一话/下一话 |
| Audiobooks | `audio`, `player`, `audio_listening` | `/audio/books/{id}/chapters/{item}`, `/books/{id}/tones` | partial: the chapter track plays in mpv with its CENC key; player UI/timeline todo |
| Comments and danmaku | `detail_page`, `series_detail` | `/books/{id}/comments`, `/series/{id}/comments`, `/videos/{vid}/danmaku` | todo |
| Authors | `author_page` | `/authors/{id}` | todo |
| 红果短剧 (Hongguo) | — | — | not supported by the original either |
| Cached downloads | `cached_books`, `cached_dramas` | `/videos/{vid}` download + `/src/*` | partial (drama download is implicit) |
| Manga channel | `home` 漫画 | `/recommend/homepage` (tab 9) | done: cell `book_data` grid parses |
| 番茄 account login (import shelf/history) | `desktop_login`, `fanqie_library_import_page` | `/account/fanqie/library`, `/account/fanqie/history` | todo — the original logs in through an embedded webview and imports read-only |
| Settings: theme, TTS, text replacement | `settings*` | local | todo |
| Mine / about | `mine_page`, `about_page` | local | todo |

## Order of work

1. Home feed: remaining tabs and the `series-feed` blocks (猜你喜欢) — also unlocks the rank
   catalogue.
2. Rank boards.
3. Comic reader (image pages, lazy loading, tap paging).
4. Audiobook playback (mpv audio + chapter list).
5. Comments and danmaku views.
6. Settings (theme, font size, text replacement) and history.

## Verification habit

Every feature is proven the same way: a headless call in `src/bin/smoke.rs` printing counts and a
sample, then a screenshot of the running window read back for the layout. No feature is reported
as done on "it compiled".

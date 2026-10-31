//! Concurrent request dispatcher.
//!
//! The core is async, so the GUI must not serialise calls: a slow channel
//! load used to block search and book opening because one loop handled every
//! command. Here each command runs in its own task behind a bounded permit
//! pool, and every reply carries the request id it belongs to. The UI keeps
//! only the newest id per domain, which makes a stale (slow) reply harmless.

use std::sync::mpsc::Sender;
use std::sync::Arc;

use tokio::sync::{mpsc as tokio_mpsc, Semaphore};

use crate::platform;

/// What a request is allowed to replace. Replies are dropped when a newer
/// request of the same kind has been issued in the meantime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum RequestKind {
    Search,
    Book,
    Chapter,
    Series,
    Manga,
    Category,
    RankCatalog,
    RankPage,
    Home,
    HomeMore,
    Comments,
    Play,
}

pub(crate) enum Cmd {
    Search {
        query: String,
        offset: usize,
    },
    OpenBook(String),
    OpenSeries {
        id: String,
        cover: String,
    },
    OpenChapter(String),
    PlayEpisode {
        vid: String,
        title: String,
    },
    PlayAudio {
        book_id: String,
        item_id: String,
        title: String,
    },
    OpenManga(String),
    Category(u32),
    RankCatalog,
    Comments {
        book_id: String,
        offset: usize,
    },
    RankPage {
        rank_id: String,
        algo: String,
        category: String,
        offset: usize,
    },
    Home,
    HomeChannel {
        tab_type: u32,
        tab_index: u32,
        client_template: String,
    },
    HomeMore {
        section: usize,
        cell_id: String,
        tab_type: u32,
        client_template: String,
        offset: usize,
    },
}

impl Cmd {
    pub(crate) fn kind(&self) -> RequestKind {
        match self {
            Cmd::Search { .. } => RequestKind::Search,
            Cmd::OpenBook(_) => RequestKind::Book,
            Cmd::OpenChapter(_) => RequestKind::Chapter,
            Cmd::OpenSeries { .. } => RequestKind::Series,
            Cmd::PlayEpisode { .. } | Cmd::PlayAudio { .. } => RequestKind::Play,
            Cmd::OpenManga(_) => RequestKind::Manga,
            Cmd::Category(_) => RequestKind::Category,
            Cmd::RankCatalog => RequestKind::RankCatalog,
            Cmd::Comments { .. } => RequestKind::Comments,
            Cmd::RankPage { .. } => RequestKind::RankPage,
            Cmd::Home | Cmd::HomeChannel { .. } => RequestKind::Home,
            Cmd::HomeMore { .. } => RequestKind::HomeMore,
        }
    }
}

pub(crate) struct Command {
    pub(crate) id: u64,
    pub(crate) job: Cmd,
}

pub(crate) struct Reply {
    pub(crate) id: u64,
    pub(crate) kind: RequestKind,
    pub(crate) msg: Msg,
}

pub(crate) enum Msg {
    Hits {
        hits: Vec<qieyue::Hit>,
        next: Option<usize>,
        append: bool,
    },
    Detail {
        id: String,
        title: String,
        author: String,
        summary: String,
        cover: String,
        is_comic: bool,
        chapters: Vec<qieyue::Chapter>,
    },
    Series(qieyue::Series),
    Chapter {
        title: String,
        paragraphs: Vec<String>,
    },
    Played(String),
    Category(qieyue::CategoryFront),
    Home(qieyue::Home),
    RankCatalog(qieyue::RankCatalog),
    RankPage {
        hits: Vec<qieyue::Hit>,
        append: bool,
    },
    HomeMoreFailed {
        section: usize,
    },
    Comments {
        rows: Vec<qieyue::Comment>,
        next: Option<usize>,
        append: bool,
    },
    Manga {
        pages: Vec<String>,
    },
    HomeAppend {
        section: usize,
        hits: Vec<qieyue::Hit>,
        next: Option<usize>,
    },
    Error(String),
}

/// How many backend calls may be in flight at once. The core is async and the
/// upstream tolerates parallel requests; this cap only keeps a burst of clicks
/// from turning into an unbounded fan-out.
const MAX_INFLIGHT: usize = 8;

/// Starts the dispatcher. Every command becomes a task; replies are forwarded
/// as they complete, so the GUI thread never waits for a request.
pub(crate) async fn run(mut cmds: tokio_mpsc::UnboundedReceiver<Command>, msgs: Sender<Reply>) {
    if let Err(err) = qieyue::init_backend().await {
        let _ = msgs.send(Reply {
            id: 0,
            kind: RequestKind::Play,
            msg: Msg::Error(format!("backend init failed: {err}")),
        });
        return;
    }

    let limit = Arc::new(Semaphore::new(MAX_INFLIGHT));

    while let Some(command) = cmds.recv().await {
        let limit = Arc::clone(&limit);
        let msgs = msgs.clone();
        tokio::spawn(async move {
            let Ok(_permit) = limit.acquire_owned().await else {
                return;
            };
            let kind = command.job.kind();
            let id = command.id;
            let msg = dispatch(command.job).await;
            let _ = msgs.send(Reply { id, kind, msg });
        });
    }
}

async fn dispatch(cmd: Cmd) -> Msg {
    match cmd {
        Cmd::Search { query, offset } => match qieyue::search_page(&query, offset, None).await {
            Ok(page) => Msg::Hits {
                hits: page.hits,
                next: page.next_offset,
                append: offset > 0,
            },
            Err(err) => Msg::Error(err),
        },
        Cmd::OpenBook(id) => match qieyue::open_book(&id).await {
            Ok(detail) => Msg::Detail {
                id,
                title: detail.title,
                author: detail.author,
                summary: detail.summary,
                cover: detail.cover,
                is_comic: detail.is_comic,
                chapters: detail.chapters,
            },
            Err(err) => Msg::Error(err),
        },
        Cmd::OpenSeries { id, cover } => match qieyue::open_series(&id).await {
            Ok(mut series) => {
                // The series endpoint often omits the artwork the card already had.
                if series.cover.is_empty() {
                    series.cover = cover;
                }
                Msg::Series(series)
            }
            Err(err) => Msg::Error(err),
        },
        Cmd::OpenChapter(item_id) => match qieyue::read_chapter(&item_id).await {
            Ok((title, paragraphs)) => Msg::Chapter { title, paragraphs },
            Err(err) => Msg::Error(err),
        },
        Cmd::PlayAudio {
            book_id,
            item_id,
            title,
        } => match qieyue::audio_for_chapter(&book_id, &item_id).await {
            Ok((url, key, tone)) => match platform::spawn_mpv_audio(&url, &key, &title) {
                Ok(()) => Msg::Played(format!("mpv 正在朗读: {title}（音色 {tone}）")),
                Err(err) => Msg::Error(err),
            },
            Err(err) => Msg::Error(err),
        },
        Cmd::OpenManga(item_id) => match qieyue::manga_chapter(&item_id).await {
            Ok(pages) => Msg::Manga { pages },
            Err(err) => Msg::Error(err),
        },
        Cmd::Comments { book_id, offset } => match qieyue::book_comments(&book_id, offset).await {
            Ok((rows, next)) => Msg::Comments {
                rows,
                next,
                append: offset > 0,
            },
            Err(err) => Msg::Error(err),
        },
        Cmd::RankCatalog => match qieyue::rank_catalog().await {
            Ok(catalog) => Msg::RankCatalog(catalog),
            Err(err) => Msg::Error(err),
        },
        Cmd::RankPage {
            rank_id,
            algo,
            category,
            offset,
        } => match qieyue::rank_page(&rank_id, &algo, &category, offset).await {
            Ok(hits) => Msg::RankPage {
                hits,
                append: offset > 0,
            },
            Err(err) => Msg::Error(err),
        },
        Cmd::Home => match qieyue::home().await {
            Ok(home) => Msg::Home(home),
            Err(err) => Msg::Error(err),
        },
        Cmd::HomeChannel {
            tab_type,
            tab_index,
            client_template,
        } => match qieyue::home_channel(tab_type, tab_index, &client_template).await {
            Ok(home) => Msg::Home(home),
            Err(err) => Msg::Error(err),
        },
        Cmd::HomeMore {
            section,
            cell_id,
            tab_type,
            client_template,
            offset,
        } => match qieyue::home_more(&cell_id, tab_type, &client_template, offset).await {
            Ok((hits, next)) => Msg::HomeAppend {
                section,
                hits,
                next,
            },
            Err(err) => {
                eprintln!("home_more: {err}");
                Msg::HomeMoreFailed { section }
            }
        },
        Cmd::Category(tab) => match qieyue::category_front(tab).await {
            Ok(front) => Msg::Category(front),
            Err(err) => Msg::Error(err),
        },
        Cmd::PlayEpisode { vid, title } => match qieyue::resolve_episode(&vid).await {
            Ok((path, key)) => match platform::spawn_mpv(&path, &key, &title) {
                Ok(()) => Msg::Played(format!("mpv 已启动: {title}")),
                Err(err) => Msg::Error(err),
            },
            Err(err) => Msg::Error(err),
        },
    }
}

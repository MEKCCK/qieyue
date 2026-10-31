//! Headless smoke test for the data path the GUI uses: init the backend, search, open the first
//! hit, read its first chapter. Run with `cargo run --release --bin smoke -- 斗破`.

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    fqapp_desktop::init_backend().await?;
    println!("backend ready");

    let query = std::env::args().nth(1).unwrap_or_else(|| "斗破".to_string());
    let books = fqapp_desktop::search(&query).await?;
    println!("search {query:?}: {} hits", books.len());
    for book in books.iter().take(3) {
        println!("  {} | {} | id={}", book.title, book.author, book.id);
    }

    let Some(first) = books.first().cloned() else {
        println!("no hits, stopping");
        return Ok(());
    };

    let detail = fqapp_desktop::open_book(&first.id).await?;
    let (title, author, summary) = (detail.title, detail.author, detail.summary);
    let chapters = detail.chapters;
    println!("cover: {}", !detail.cover.is_empty());
    println!(
        "detail: {title} / {author} / {} chapters / {:?}",
        chapters.len(),
        summary.chars().take(40).collect::<String>()
    );

    let Some(chapter) = chapters.first().cloned() else {
        println!("no chapters, stopping");
        return Ok(());
    };
    let (chapter_title, paragraphs) = fqapp_desktop::read_chapter(&chapter.item_id).await?;
    let chars: usize = paragraphs.iter().map(|p| p.chars().count()).sum();
    println!("chapter: {chapter_title} / {} paragraphs / {chars} chars", paragraphs.len());
    // Same call the GUI makes when a chapter is opened, so the shelf file is exercised too.
    fqapp_desktop::remember(fqapp_desktop::ShelfEntry {
        id: first.id.clone(),
        title: title.clone(),
        author: author.clone(),
        cover: detail.cover.clone(),
        item_id: chapter.item_id.clone(),
        chapter_title: chapter_title.clone(),
        index: 0,
    })?;
    println!("shelf: {} entries", fqapp_desktop::load_shelf().len());

    for paragraph in paragraphs.iter().take(2) {
        println!("  {:?}", paragraph.chars().take(60).collect::<String>());
    }

    // Short dramas: search hits carry the series id, episodes come from the series endpoint and
    // each one resolves to a downloaded file plus the CENC key mpv needs.
    let hits = fqapp_desktop::search_hits("总裁").await?;
    let series_id = hits.iter().find_map(|hit| hit.series.clone());
    match series_id {
        Some(id) => {
            let series = fqapp_desktop::open_series(&id).await?;
            println!(
                "series: {} / {} episodes / cover {}",
                series.title,
                series.episodes.len(),
                !series.cover.is_empty()
            );
            for episode in series.episodes.iter().take(3) {
                println!("  ep{} {} vid={}", episode.index, episode.title, episode.vid);
            }
            if let Some(episode) = series.episodes.first() {
                let (path, key) = fqapp_desktop::resolve_episode(&episode.vid).await?;
                let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                println!("resolved: {path} ({size} bytes) key={} chars", key.len());
            }
        }
        None => println!("no short drama in search hits"),
    }

    // Search paging: the second page must add rows, not repeat the first.
    let page_one = fqapp_desktop::search_page("斗破", 0, None).await?;
    let page_two = fqapp_desktop::search_page("斗破", 10, None).await?;
    let overlap = page_two
        .hits
        .iter()
        .filter(|hit| page_one.hits.iter().any(|a| a.id == hit.id))
        .count();
    println!(
        "paging: page1={} page2={} overlap={overlap} next={:?}→{:?}",
        page_one.hits.len(),
        page_two.hits.len(),
        page_one.next_offset,
        page_two.next_offset
    );

    // Home feed: tabs plus the recommendation blocks of the first tab.
    let front_home = fqapp_desktop::home().await?;
    let total: usize = front_home.sections.iter().map(|s| s.hits.len()).sum();
    println!(
        "home: {} tabs / {} sections / {} cards",
        front_home.tabs.len(),
        front_home.sections.len(),
        total
    );
    for section in front_home.sections.iter().take(2) {
        println!("  block {}: {} cards", section.title, section.hits.len());
        for hit in section.hits.iter().take(3) {
            println!("    {} | {}", hit.title, hit.author);
        }
    }

    // Channel switching: the same page with a different tab_type/tab_index/client_template.
    for (label, tab_type, index, template) in [
        ("小说", 25u32, 1u32, "12"),
        ("听书", 5, 4, "12"),
        ("漫画", 9, 7, "4"),
    ] {
        match fqapp_desktop::home_channel(tab_type, index, template).await {
            Ok(channel) => {
                let cards: usize = channel.sections.iter().map(|s| s.hits.len()).sum();
                println!(
                    "channel {label}: {} sections / {cards} cards / first={:?}",
                    channel.sections.len(),
                    channel.sections.first().map(|s| s.title.clone())
                );
            }
            Err(err) => println!("channel {label}: {err}"),
        }
    }

    // Audiobook: the play-info envelope must yield a stream URL mpv can open.
    if let Ok(tones) = fqapp_desktop::api_get("/api/v1/books/7542602438201576472/tones", "").await {
        let data = tones.get("data").cloned().unwrap_or_default();
        let rows = data.get("audio_tones").and_then(|v| v.as_array()).map(|v| v.len()).unwrap_or(0);
        println!("tones probe: code={:?} data_keys={:?} rows={rows}",
            tones.get("code"), data.as_object().map(|o| o.keys().take(4).cloned().collect::<Vec<_>>()));
    }
    match fqapp_desktop::audio_for_chapter(&first.id, &chapter.item_id).await {
        Ok((url, key, tone)) => println!(
            "audio: tone={tone} key={} url={}",
            if key.is_empty() { "none" } else { "16-byte" },
            &url[..url.len().min(60)]
        ),
        Err(err) => println!("audio failed: {err}"),
    }

    // Home paging: pull one more page of the first block that advertises a next offset.
    if let Some(section) = front_home
        .sections
        .iter()
        .find(|section| section.next_offset.is_some())
    {
        let before = section.hits.len();
        let next = section.next_offset.unwrap();
        let (more, after) =
            fqapp_desktop::home_more(&section.cell_id, section.tab_type, next).await?;
        println!(
            "home paging: {} {} -> +{} cards, next {:?} -> {:?}",
            section.title, before, more.len(), Some(next), after
        );
    } else {
        println!("home paging: no block offers a next offset");
    }

    // Categories: channels, dimensions and the tile counts per dimension.
    let front = fqapp_desktop::category_front(1).await?;
    println!(
        "category: {} tabs ({}) / {} dims / {} tiles",
        front.tabs.len(),
        front
            .tabs
            .iter()
            .map(|tab| tab.name.clone())
            .collect::<Vec<_>>()
            .join("、"),
        front.dims.len(),
        front.tiles.len()
    );
    for dim in front.dims.iter().skip(1) {
        let count = front.tiles.iter().filter(|t| t.dim == dim.value).count();
        println!("  {}: {} 个标签", dim.name, count);
    }

    // Kind detection: a comic must be routed to the image reader, a novel to the text one.
    println!(
        "kinds: novel is_comic={} comic is_comic={}",
        detail.is_comic,
        fqapp_desktop::open_book("7668179381876968510").await?.is_comic
    );

    // Manga: a comic book's first chapter decrypts to a list of served images.
    let comic = fqapp_desktop::open_book("7668179381876968510").await?;
    if let Some(first_page) = comic.chapters.first() {
        match fqapp_desktop::manga_chapter(&first_page.item_id).await {
            Ok(pages) => {
                let exists = pages
                    .iter()
                    .filter(|uri| std::path::Path::new(uri.trim_start_matches("file://")).exists())
                    .count();
                println!("manga: {}/{} pages on disk ({})", exists, pages.len(), comic.title);
            }
            Err(err) => println!("manga failed: {err}"),
        }
    }

    // Shelf toggle round trip.
    let entry = fqapp_desktop::ShelfEntry {
        id: "test-book".to_string(),
        title: "测试".to_string(),
        author: String::new(),
        cover: String::new(),
        item_id: String::new(),
        chapter_title: String::new(),
        index: 0,
    };
    let added = fqapp_desktop::add_shelf(entry)?;
    let present = fqapp_desktop::shelf_contains("test-book");
    fqapp_desktop::remove_shelf("test-book")?;
    let gone = !fqapp_desktop::shelf_contains("test-book");
    println!("shelf toggle: added={added} present={present} removed={gone}");

    Ok(())
}

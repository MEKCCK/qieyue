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

    let (title, author, summary, chapters) = fqapp_desktop::open_book(&first.id).await?;
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

    // Shelf toggle round trip.
    let entry = fqapp_desktop::ShelfEntry {
        id: "test-book".to_string(),
        title: "测试".to_string(),
        author: String::new(),
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

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
    for paragraph in paragraphs.iter().take(2) {
        println!("  {:?}", paragraph.chars().take(60).collect::<String>());
    }
    Ok(())
}

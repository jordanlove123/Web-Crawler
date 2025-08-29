use clap::Parser;

mod roboreader;
mod crawler;
use crawler::Crawler;

#[derive(Parser, Debug)]
#[command(name = "Rust Crawler", version = "0.1", about = "Web crawler using Rust")]
struct Args {
    /// Starting URL
    #[arg()]
    url: String,

    /// Depth
    #[arg(short, long, default_value_t = 2)]
    depth: usize,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a = Args::parse();
    let crawler = Crawler::new(&a.url, a.depth, 10)?;

    let _ = match crawler.crawl() {
        Ok(total_map) => {
            println!("");
            println!("Success!");
            println!("Top words found on {0:?} with a depth of {1:?}", a.url, a.depth);
            for (word, freq) in total_map.into_iter().take(10) {
                println!("{0:?}: {1:?} occurrences", word, freq);
            }
        },
        Err(e) => println!("Function exited with error: {e:?}")
    };

    Ok(())
}

extern crate clap;

use clap::Clap;
use regex::Regex;
use std::path::Path;

use crabler::*;

const INDEX_URL: &'static str = "https://apod.nasa.gov/apod/archivepix.html";
const ENTRY_PREFIX: &'static str = "https://apod.nasa.gov/apod/";

#[derive(Clap)]
#[clap(version = "0.1", author = "Potato")]
struct CliOpts {
    #[clap(long = "directory", short = "d", default_value = ".")]
    directory: String,
    #[clap(long = "threads", short = "t", default_value = "50")]
    threads: usize,
}

#[derive(WebScraper)]
#[on_response(on_response)]
#[on_html("b a[href]", index_handler)]
#[on_html("a[href]", entry_handler)]
struct Scraper {
    index_href_re: Regex,
    image_href_re: Regex,
    directory: String,
}

impl Scraper {
    async fn on_response(&self, response: Response) -> Result<()> {
        if response.url.ends_with(".jpg") && response.status == 200 {
            println!("Finished downloading {}", response.url);
        }

        Ok(())
    }

    async fn index_handler(&self, mut response: Response, a: Element) -> Result<()> {
        if let Some(href) = a.attr("href") {
            if self.index_href_re.is_match(&href[..]) {
                let href = format!("{}{}", ENTRY_PREFIX, href);
                // println!("Navigating to {}", href);
                response.navigate(href).await?;
            };
        }

        Ok(())
    }

    async fn entry_handler(&self, mut response: Response, a: Element) -> Result<()> {
        if let Some(href) = a.attr("href") {
            if self.image_href_re.is_match(&href[..]) {
                let slash_re = Regex::new(r"/").unwrap();
                let fname = slash_re.replace_all(&href, "_");
                let href = format!("{}{}", ENTRY_PREFIX, href);
                let p = Path::new(&self.directory).join(Path::new(&fname.to_string()));
                let destination = p.to_string_lossy();

                if !p.exists() {
                    println!("Downloading {}", destination);
                    response.download_file(href, destination.to_string()).await?;
                }
            };
        }

        Ok(())
    }
}

#[tokio::main(threaded_scheduler)]
async fn main() -> Result<()> {
    let opts: CliOpts = CliOpts::parse();

    let index_href_re = Regex::new(r"^ap\d{6}\.html$").unwrap();
    let image_href_re = Regex::new(r"^image/.*\.jpe?g$").unwrap();
    let directory = opts.directory.clone();
    let scraper = Scraper { index_href_re, image_href_re, directory };

    scraper.run(Opts::new().with_urls(vec![INDEX_URL]).with_threads(opts.threads)).await
}

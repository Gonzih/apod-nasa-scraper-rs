#[macro_use]
extern crate clap;

use futures::stream;
use futures::stream::StreamExt;
use regex::Regex;
use scraper::{Html, Selector};
use std::path::Path;
use tokio::fs::File;
use tokio::prelude::*;

const PARALLEL_LEVEL: usize = 5;

const INDEX_URL: &'static str = "https://apod.nasa.gov/apod/archivepix.html";
const ENTRY_PREFIX: &'static str = "https://apod.nasa.gov/apod/";

#[derive(Clap)]
#[clap(version = "0.1", author = "Potato")]
struct Opts {
    #[clap(long = "directory", short = "d", default_value = ".")]
    directory: String,
}

#[derive(Debug, Clone)]
struct Entry {
    url: String,
    title: String,
}

impl Entry {
    fn new(url: String, title: String) -> Self {
        Entry { url, title }
    }

    fn gen_url(&self) -> String {
        format!("{}{}", ENTRY_PREFIX, self.url)
    }

    async fn get_img_url(&self) -> Option<String> {
        let image_href_re = Regex::new(r"^image/.*\.jpe?g$").unwrap();
        let url = self.gen_url();

        let index = reqwest::get(&url)
            .await
            .expect(&format!("Colud not get data from url {}", url))
            .text()
            .await
            .expect(&format!("Colud not get response text from {}", url));
        let document = Html::parse_document(&index);
        let a_sel = Selector::parse("a").unwrap();

        for a in document.select(&a_sel) {
            if let Some(href) = a.value().attr("href") {
                if image_href_re.is_match(href) {
                    return Some(format!("{}{}", ENTRY_PREFIX, href));
                }
            }
        }

        None
    }

    async fn download_file(
        &self,
        directory: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let extension_re = Regex::new(r"\.html$").unwrap();
        let title_cleanup_re = Regex::new(r"/").unwrap();

        let fname = format!(
            "{} - {}.jpg",
            extension_re.replace_all(&self.url, ""),
            title_cleanup_re.replace_all(&self.title, ""),
        );

        let p = Path::new(&directory).join(Path::new(&fname));
        let path = &*p.to_string_lossy();

        if !p.exists() {
            if let Some(url) = self.get_img_url().await {
                println!("Downloading {} from {}", path, url);
                let response = reqwest::get(&*url).await?.bytes().await?;
                let mut dest = File::create(path).await?;
                dest.write_all(&response).await?;
            } else {
                println!(
                    "Skipping file {} - {}, could not load url",
                    self.url, self.title
                );
            }
        } else {
            println!("Skipping file {}, file exists", path);
        }

        Ok(())
    }
}

#[tokio::main(threaded_scheduler)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let opts: Opts = Opts::parse();

    let index_href_re = Regex::new(r"^ap\d{6}\.html$").unwrap();

    let index = reqwest::get(INDEX_URL).await?.text().await?;
    let document = Html::parse_document(&index);
    let b_sel = Selector::parse("b").unwrap();
    let a_sel = Selector::parse("a").unwrap();

    let mut entries = vec![];

    for b in document.select(&b_sel) {
        for a in b.select(&a_sel) {
            if let Some(href) = a.value().attr("href") {
                if index_href_re.is_match(href) {
                    let text = a.text().collect::<String>().clone();
                    let href = href.to_string().clone();

                    let entry = Entry::new(href, text);
                    entries.push(entry);
                }
            }
        }
    }

    println!("Found {} entries", entries.len());

    let handles = stream::iter(
        entries.into_iter().map(|entry| {
            let directory = opts.directory.clone();
            async move {
                entry.download_file(directory).await.expect("Could not download entry");
            }
        })
    ).buffer_unordered(PARALLEL_LEVEL).collect::<Vec<_>>();

    handles.await;

    Ok(())
}

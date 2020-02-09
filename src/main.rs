#[macro_use]
extern crate clap;

use regex::Regex;
use scraper::{Html, Selector};
use std::path::Path;
use tokio::fs::File;
use tokio::prelude::*;

const INDEX_URL: &'static str = "https://apod.nasa.gov/apod/archivepix.html";
const ENTRY_PREFIX: &'static str = "https://apod.nasa.gov/apod/";

#[derive(Clap)]
#[clap(version = "0.1", author = "Potato")]
struct Opts {
    #[clap(long = "directory", short = "d", default_value = ".")]
    directory: String,
}

#[derive(Debug)]
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

    async fn download(&self, directory: &String) -> Result<(), Box<dyn std::error::Error>> {
        let image_href_re = Regex::new(r"^image/.*\.jpg$").unwrap();
        let url = self.gen_url();

        let index = reqwest::get(&url).await?.text().await?;
        let document = Html::parse_document(&index);
        let a_sel = Selector::parse("a").unwrap();
        for a in document.select(&a_sel) {
            if let Some(href) = a.value().attr("href") {
                if image_href_re.is_match(href) {
                    let url = format!("{}{}", ENTRY_PREFIX, href);
                    self.download_file(url, directory).await?;
                }
            }
        }

        Ok(())
    }

    async fn download_file(
        &self,
        url: String,
        directory: &String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let extension_re = Regex::new(r"\.html$").unwrap();
        let fname = format!(
            "{} - {}.jpg",
            extension_re.replace_all(&self.url, ""),
            self.title
        );
        let p = Path::new(&directory).join(Path::new(&fname));
        let path = &*p.to_string_lossy();

        if !p.exists() {
            println!("Downloading file {} to {}", url, path);

            let response = reqwest::get(&*url).await?.bytes().await?;
            let mut dest = File::create(path).await?;
            dest.write_all(&response).await?;
        } else {
            println!("Skipping file {}, file exists", path);
        }

        Ok(())
    }
}

#[tokio::main]
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
                    let text = a.text().collect::<String>();
                    let entry = Entry::new(href.to_string(), text);
                    entries.push(entry);
                }
            }
        }
    }

    let futures = entries
        .iter()
        .map(|e| e.download(&opts.directory))
        .collect::<Vec<_>>();

    for f in futures {
        f.await?;
    }

    Ok(())
}

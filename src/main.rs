#[macro_use]
extern crate clap;

// use futures::future::join_all;
use regex::Regex;
use scraper::{Html, Selector};
use std::path::Path;
use std::sync::Arc;
use tokio::fs::File;
use tokio::prelude::*;
use tokio::sync::Semaphore;

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

    async fn get_img_url(&self) -> Option<String> {
        let image_href_re = Regex::new(r"^image/.*\.jpg$").unwrap();
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
        sem: Arc<Semaphore>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let _ = sem.acquire().await;
        if let Some(url) = self.get_img_url().await {
            let extension_re = Regex::new(r"\.html$").unwrap();
            let fname = format!(
                "{} - {}.jpg",
                extension_re.replace_all(&self.url, ""),
                self.title
            );
            let p = Path::new(&directory).join(Path::new(&fname));
            let path = &*p.to_string_lossy();

            if !p.exists() {
                let _ = sem.acquire().await;
                let response = reqwest::get(&*url).await?.bytes().await?;

                let _ = sem.acquire().await;
                let mut dest = File::create(path).await?;

                let _ = sem.acquire().await;
                dest.write_all(&response).await?;
            } else {
                println!("Skipping file {}, file exists", path);
            }
        } else {
            println!(
                "Skipping file {} - {}, could not load url",
                self.url, self.title
            );
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

    let semaphore = Arc::new(Semaphore::new(10));
    // let mut handles = vec![];

    for entry in entries {
        let directory = opts.directory.clone();
        let sem = semaphore.clone();
        entry
            .download_file(directory, sem)
            .await
            .expect("Could not download entry");
    }

    // join_all(handles).await;

    Ok(())
}

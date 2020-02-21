#[macro_use]
extern crate clap;

use futures::stream;
use futures::stream::StreamExt;
use regex::Regex;
use crabquery::{Document};
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
    #[clap(long = "threads", short = "t", default_value = "5")]
    threads: usize,
    #[clap(long = "verbose", short = "v")]
    verbose: bool,
}

#[derive(Debug, Clone)]
struct Entry {
    verbose: bool,
    url: String,
    title: String,
}

impl Entry {
    fn new(url: String, title: String, verbose: bool) -> Self {
        Entry { url, title, verbose }
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
        let document = Document::from(index);

        for a in document.select("a") {
            if let Some(href) = a.attr("href") {
                if image_href_re.is_match(&href[..]) {
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
                if self.verbose {
                    println!("Downloading {} from {}", path, url);
                }

                let response = reqwest::get(&*url).await?.bytes().await?;
                let mut dest = File::create(path).await?;
                dest.write_all(&response).await?;
            } else {
                if self.verbose {
                    println!(
                        "Skipping file {} - {}, could not load url",
                        self.url, self.title
                    );
                }
            }
        } else {
            if self.verbose {
                println!("Skipping file {}, file exists", path);
            }
        }

        Ok(())
    }
}

#[tokio::main(threaded_scheduler)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let opts: Opts = Opts::parse();

    let index_href_re = Regex::new(r"^ap\d{6}\.html$").unwrap();

    let index = reqwest::get(INDEX_URL).await?.text().await?;
    let document = Document::from(index);

    let mut entries = vec![];

    for a in document.select("b a") {
        if let Some(href) = a.attr("href") {
            if index_href_re.is_match(&href[..]) {
                if let Some(text) = a.text() {
                    let href = href.to_string();
                    let entry = Entry::new(href, text, opts.verbose);
                    entries.push(entry);
                }
            }
        }
    }

    if opts.verbose {
        println!("Found {} entries", entries.len());
    }

    let handles = stream::iter(
        entries.into_iter().map(|entry| {
            let directory = opts.directory.clone();
            async move {
                entry.download_file(directory).await.expect("Could not download entry");
            }
        })
    ).buffer_unordered(opts.threads).collect::<Vec<_>>();

    handles.await;

    Ok(())
}
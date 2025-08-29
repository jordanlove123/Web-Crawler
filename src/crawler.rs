use reqwest::blocking::get;
use scraper::{Html, Selector};
use std::sync::{
    Arc, 
    Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::collections::{HashSet, HashMap, VecDeque};
use std::thread;
use url::{Url, ParseError};

use crate::roboreader::RoboReader;

// Ensure that counter gets decremented if process_url exits early for any reason
struct WorkGuard<'a> {
    counter: &'a AtomicUsize,
}
impl<'a> Drop for WorkGuard<'a> {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::SeqCst);
    }
}

#[derive(Clone)]
pub struct Crawler {
    url_queue: Arc<Mutex<VecDeque<(String, Url, usize)>>>,
    visited_set: Arc<Mutex<HashSet<String>>>,
    work_counter: Arc<AtomicUsize>,
    robo_reader: RoboReader,
    max_threads: i32,
    max_depth: usize,
    base_url: String,
}

impl Crawler {
    pub fn new(url: &str, max_depth: usize, threads: i32) -> Result<Self, Box<dyn std::error::Error>> {
        let robo = match RoboReader::new(url, "MyCrawler") {
            Ok(robo) => robo,
            Err(e) => return Err(e),
        };

        Ok(Self {
            url_queue: Arc::new(Mutex::new(VecDeque::new())),
            visited_set: Arc::new(Mutex::new(HashSet::new())),
            work_counter: Arc::new(AtomicUsize::new(0)),
            robo_reader: robo,
            max_threads: threads,
            max_depth: max_depth,
            base_url: url.to_string(),
        })
    }

    fn add_url(&self, url: &str, base_url: &Url, depth: usize) {
        self.url_queue.lock().unwrap().push_back((url.to_string(), base_url.clone(), depth));
        self.visited_set.lock().unwrap().insert(url.to_string());
        self.work_counter.fetch_add(1, Ordering::SeqCst);
    }

    fn get_url(&self) -> Option<(String, Url, usize)> {
        self.url_queue.lock().unwrap().pop_front()
    }

    fn process_url(&self, url_data: &(String, Url, usize)) -> Result<HashMap<String, usize>, Box<dyn std::error::Error>> {
        let _guard = WorkGuard { counter: &self.work_counter };
        let url = url_data.0.as_str();
        let base_url = &url_data.1;
        let depth = url_data.2;
        let mut word_freqs: HashMap<String, usize> = HashMap::new();

        // Process url
        let parsed_url = match Url::parse(&url) {
            Ok(url) => url,
            Err(ParseError::RelativeUrlWithoutBase) => base_url.join(url)?,
            Err(e) => {
                return Err(Box::new(e));
            },
        };
        let total_url = parsed_url.as_str();
        println!("Processing url: {total_url:?}, depth: {depth:?}");

        // Get page data
        let request = get(total_url)?;
        let body = request.text()?;

        let document = Html::parse_document(&body);

        // Add word frequencies to a local hashmap
        let text_selector = Selector::parse("body :not(script):not(style):not(noscript)").unwrap();
        for element in document.select(&text_selector) {
            let text = element.text().collect::<Vec<_>>().join(" ");
            for word in text.split_whitespace() {
                let word = word.to_lowercase();
                let chars_to_remove = &['.', ',', '?', '!', ':', ';', '(', ')', '[', ']', '\'', '"', '<', '>'];
                let mut filtered_word = String::new();
                for c in word.chars() {
                    if !chars_to_remove.contains(&c) {
                        filtered_word.push(c);
                    }
                }
                *word_freqs.entry(word).or_insert(0) += 1;
            }
        }

        // Iterate through links on page
        let link_selector = Selector::parse("a").unwrap();
        for element in document.select(&link_selector) {
            if let Some(href) = element.value().attr("href") {
                if self.robo_reader.rules.contains_key(href) && !self.robo_reader.rules.get(href).unwrap() {
                    println!("Skipping {href:?} (disallowed by robots.txt)");
                    continue;
                }

                if self.visited_set.lock().unwrap().contains(href) {
                    continue;
                }

                if depth+1 <= self.max_depth {
                    self.add_url(&href, &parsed_url, depth+1);
                }
            }
        }

        Ok(word_freqs)
    }

    fn add_hashmap_data(&self, hash_og: &mut HashMap<String, usize>, hash_new: &mut HashMap<String, usize>) {
        for word in hash_new.keys() {
            if hash_og.contains_key(word) {
                hash_og.insert(word.to_string(), *hash_og.get(word).unwrap() + *hash_new.get(word).unwrap());
            }
            else {
                hash_og.insert(word.to_string(), *hash_new.get(word).unwrap());
            }
        }
    }

    pub fn crawl(&self) -> Result<Vec<(String, usize)>, Box<dyn std::error::Error>> {
        let mut handles = vec![];
        let arc_self = Arc::new(self.clone());
        let parsed_url = Url::parse(&self.base_url)?;
        let counter: Arc<AtomicUsize> = Arc::new(AtomicUsize::new(0));
        self.add_url(&self.base_url, &parsed_url, 0);

        for _ in 0..self.max_threads {
            let crawler = Arc::clone(&arc_self);
            let counter_clone = Arc::clone(&counter);
            let mut total_freqs: HashMap<String, usize> = HashMap::new();

            let handle = thread::spawn(move || {
                while crawler.work_counter.load(Ordering::SeqCst) > 0 {
                    match crawler.get_url() {
                        Some(url_data) => {
                            match crawler.process_url(&url_data) {
                                Ok(mut word_freqs) => {
                                    counter_clone.fetch_add(1, Ordering::SeqCst);
                                    crawler.add_hashmap_data(&mut total_freqs, &mut word_freqs);
                                },
                                Err(e) => println!("Error when processing {0:?}: {1:?}", url_data.1, e),
                            }
                        },
                        None => {
                            std::thread::yield_now();
                        },
                    }
                }
                total_freqs
            });
            handles.push(handle);
        }

        let mut hash_vec: Vec<HashMap<String, usize>> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        println!("Compiling frequency data");
        let mut total_map: HashMap<String, usize> = HashMap::new();
        for hashmap in hash_vec.iter_mut() {
            self.add_hashmap_data(&mut total_map, hashmap);
        }

        println!("Sorting frequency data");
        let mut freqs: Vec<(String, usize)> = total_map.iter().map(|(word, count)| (word.clone(), *count)).collect();
        freqs.sort_by(|a, b| b.1.cmp(&a.1));

        Ok(freqs)
    }
}
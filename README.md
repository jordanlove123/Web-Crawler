# Web-Crawler
A Rust web crawler that scrapes the most frequently used words on a website starting from a base URL, respecting robots.txt for that website.

## Usage
Run this from the command line using the following command:
```
rust-crawler.exe [OPTIONS] <URL>
```

There is one available option, depth (-d, --depth), which can be used to specify the depth of the crawl. Be careful using a depth of more than 3 on large websites, this could potentially take a while as the number of requests grows exponentially with depth.
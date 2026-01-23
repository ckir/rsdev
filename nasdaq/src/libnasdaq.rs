extern crate reqwest;
use reqwest::Error as ReqwestError;
use reqwest::header;

// use thiserror::Error;

// #[derive(Error, Debug)]
// pub enum FetchError {
//     #[error("Request failed: {0}")]
//     RequestError(#[from] ReqwestError),
//     #[error("Failed to fetch text")]
//     TextError,
// }

pub async fn fetch_text(url: &str) -> Result<String, ReqwestError> {
    let mut headers: header::HeaderMap = header::HeaderMap::new();
    headers.insert(
        "accept",
        "application/json, text/plain, */*".parse().unwrap(),
    );
    headers.insert(
        "accept-language",
        "en-US,en;q=0.9,el-GR;q=0.8,el;q=0.7".parse().unwrap(),
    );
    headers.insert("cache-control", "no-cache".parse().unwrap());
    headers.insert("dnt", "1".parse().unwrap());
    headers.insert("origin", "https://www.nasdaq.com".parse().unwrap());
    headers.insert("pragma", "no-cache".parse().unwrap());
    headers.insert("priority", "u=1, i".parse().unwrap());
    headers.insert("referer", "https://www.nasdaq.com/".parse().unwrap());
    headers.insert(
        "sec-ch-ua",
        "\"Google Chrome\";v=\"135\", \"Not-A.Brand\";v=\"8\", \"Chromium\";v=\"135\""
            .parse()
            .unwrap(),
    );
    headers.insert("sec-ch-ua-mobile", "?0".parse().unwrap());
    headers.insert("sec-ch-ua-platform", "\"Windows\"".parse().unwrap());
    headers.insert("sec-fetch-dest", "empty".parse().unwrap());
    headers.insert("sec-fetch-mode", "cors".parse().unwrap());
    headers.insert("sec-fetch-site", "same-site".parse().unwrap());
    headers.insert("user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/135.0.0.0 Safari/537.36".parse().unwrap());

    let client: reqwest::Client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();

    // let client = reqwest::blocking::Client::new();

    let response: reqwest::Response = client.get(url).headers(headers).send().await?;

    let text: String = response.text().await?;

    Ok(text)
}

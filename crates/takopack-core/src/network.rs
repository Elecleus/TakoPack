use std::{fs::File, path::Path};

use crate::error::Result;

pub fn fetch_binary(name: &str, dir: &Path, url: &str) -> Result<()> {
    let resp = ureq::get(url).call()?;
    let mut reader = resp.into_reader();

    let mut file = File::create(dir.join(name))?;
    std::io::copy(&mut reader, &mut file)?;

    Ok(())
}

pub fn fetch_json<T>(url: &str) -> Result<T>
where
    T: serde::de::DeserializeOwned,
{
    let reader = ureq::get(&url).call()?.into_reader();
    let result = serde_json::from_reader(reader)?;

    Ok(result)
}

pub fn fetch_jsonl<T>(url: &str) -> Result<Vec<T>>
where
    T: serde::de::DeserializeOwned,
{
    let raw = ureq::get(&url).call()?.into_string()?;
    let mut result: Vec<T> = Vec::new();

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        result.push(serde_json::from_str(trimmed)?);
    }

    Ok(result)
}

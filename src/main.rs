use chrono::naive::NaiveDate;
use rusoto_s3::{GetObjectRequest, S3Client, S3};
use std::cmp::Ordering;
use std::fmt::{self, Write};
use std::io::Read;
use tokio::io::AsyncReadExt;

#[derive(Debug, serde::Deserialize)]
struct Manifest {
    files: Vec<ManifestFile>,
}

#[derive(Debug, serde::Deserialize)]
struct ManifestFile {
    key: String,
}

const BUCKET_NAME: &str = "rust-inventories";

#[tokio::main]
async fn main() {
    let s3 = S3Client::new(Default::default());

    let mut date = NaiveDate::from_ymd(2019, 09, 15);
    let week = chrono::Duration::weeks(1);
    // We upload inventories every week, but we want to have the date of the
    // previous week, not the current week. So advance until we hit the
    //
    // Note the less than here, this means that even if today is the day we'll
    // generate based on last week's inventory; that ensures that we're working
    // with a fully prepared inventory (even if it's a bit stale).
    while date + week < chrono::Utc::today().naive_utc() {
        date += week;
    }

    let list = s3
        .list_objects_v2(rusoto_s3::ListObjectsV2Request {
            bucket: BUCKET_NAME.to_owned(),
            prefix: Some(format!(
                "static-rust-lang-org/static-rust-lang-org/all-objects-csv/{}",
                date,
            )),
            ..Default::default()
        })
        .await
        .unwrap();

    let key = list
        .contents
        .expect("at least one key")
        .iter()
        .find_map(|obj| obj.key.clone().filter(|k| k.ends_with("manifest.json")))
        .unwrap();

    let obj = s3
        .get_object(GetObjectRequest {
            bucket: BUCKET_NAME.to_owned(),
            key,
            ..Default::default()
        })
        .await
        .unwrap();

    let mut manifest = Vec::new();
    obj.body
        .unwrap()
        .into_async_read()
        .read_to_end(&mut manifest)
        .await
        .unwrap();
    let manifest: Manifest = serde_json::from_slice(&manifest).unwrap();
    let mut manifests = Vec::new();
    for ManifestFile { key } in manifest.files {
        let obj = s3
            .get_object(GetObjectRequest {
                bucket: BUCKET_NAME.to_owned(),
                key: key.clone(),
                ..Default::default()
            })
            .await
            .unwrap();
        // csv file with Bucket, Key, Size, ETag, ReplicationStatus
        let mut contents = Vec::new();
        obj.body
            .unwrap()
            .into_async_read()
            .read_to_end(&mut contents)
            .await
            .unwrap();
        let mut file = flate2::read::GzDecoder::new(&contents[..]);
        let mut contents = Vec::new();
        file.read_to_end(&mut contents).unwrap();
        let mut builder = csv::ReaderBuilder::new();
        builder.has_headers(false);
        let mut rdr = builder.from_reader(&contents[..]);
        for (idx, res) in rdr.deserialize().enumerate() {
            let record: InventoryRecord = res.unwrap_or_else(|e| {
                eprintln!("in file: {}", String::from_utf8_lossy(&contents));
                panic!(
                    "failed to deserialize record from file {} at idx={}: {:?}",
                    key, idx, e
                );
            });
            let key = record.key;

            if !key.starts_with("dist") {
                continue;
            }
            if let Some((prefix, filename)) = key.rsplit_once('/') {
                if let Some(filename) = filename.strip_suffix(".toml") {
                    if let Some(channel) = filename.strip_prefix("channel-rust-") {
                        // skip top-level manifest
                        if prefix == "dist" {
                            continue;
                        }
                        let date = key.split('/').nth(1).unwrap();
                        if date == "staging" {
                            continue;
                        }
                        let date = Date::parse(date).unwrap_or_else(|| {
                            panic!("failed to parse {} from key={}", date, key);
                        });

                        manifests.push((date, channel.to_owned()));
                    }
                }
            }
        }
    }
    manifests.sort_unstable_by(|(a_date, a_channel), (b_date, b_channel)| {
        a_date
            .cmp(&b_date)
            .then_with(|| match (a_channel.as_str(), b_channel.as_str()) {
                ("stable", "stable") => Ordering::Equal,
                ("stable", "beta") => Ordering::Less,
                ("stable", "nightly") => Ordering::Less,
                ("stable", _) => Ordering::Less,
                ("beta", "beta") => Ordering::Equal,
                ("beta", "stable") => Ordering::Greater,
                ("beta", "nightly") => Ordering::Less,
                ("beta", _) => Ordering::Less,
                ("nightly", "nightly") => Ordering::Equal,
                ("nightly", "beta") => Ordering::Greater,
                ("nightly", "stable") => Ordering::Greater,
                ("nightly", _) => Ordering::Less,
                (a, b) => a.cmp(&b),
            })
    });
    let mut out = String::new();
    for (date, channel) in manifests {
        writeln!(
            &mut out,
            "static.rust-lang.org/dist/{}/channel-rust-{}.toml",
            date, channel,
        )
        .unwrap();
    }
    std::fs::write("manifests.txt", out).unwrap();
}

#[derive(Debug, serde::Deserialize)]
struct InventoryRecord {
    bucket: serde::de::IgnoredAny,
    key: String,
    size: u64,
}

#[derive(PartialOrd, Ord, PartialEq, Eq)]
struct Date {
    year: u16,
    month: u8,
    day: u8,
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

impl fmt::Debug for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

impl Date {
    fn parse(s: &str) -> Option<Date> {
        let mut it = s.split('-');
        Some(Date {
            year: it.next()?.parse().ok()?,
            month: it.next()?.parse().ok()?,
            day: it.next()?.parse().ok()?,
        })
    }
}

use crate::{
    error::{ApiError, ApiResult},
    models::{Counters, Entry, LanguageCode, Topic},
};
use chrono::{DateTime, Utc};
use csv::StringRecord;
use serde::{Deserialize, Deserializer};
use serde_json::Value;
use std::collections::HashMap;
use strum::IntoEnumIterator;
use tokio::sync::RwLock;

pub type DictionaryHandle = RwLock<Dictionary>;

#[derive(Debug)]
pub struct Dictionary {
    url: String,
    pub entries: HashMap<u32, Entry>,
    pub counters: Counters,
    pub last_update: DateTime<Utc>,
}

impl Dictionary {
    pub async fn from_url(url: &str) -> ApiResult<DictionaryHandle> {
        let mut dict = Dictionary {
            url: url.to_string(),
            entries: HashMap::new(),
            counters: Default::default(),
            last_update: Utc::now(),
        };
        dict.update().await?;
        Ok(RwLock::new(dict))
    }

    pub async fn update(&mut self) -> ApiResult<()> {
        let (entries, counters) = fetch_dict(&self.url).await?;
        self.entries = entries;
        self.counters = counters;
        self.last_update = Utc::now();
        Ok(())
    }

    pub fn get_entry(&self, id: u32) -> ApiResult<Entry> {
        self.entries
            .get(&id)
            .ok_or(ApiError::EntryNotFound(id))
            .cloned()
    }

    pub fn search(
        &self,
        text: Option<String>,
        text_langs: Vec<LanguageCode>,
        sem_id: Option<u32>,
        topics: Vec<Topic>,
    ) -> ApiResult<Vec<Entry>> {
        let langs = if text_langs.is_empty() {
            LanguageCode::iter().collect()
        } else {
            text_langs
        };

        let filter = |e: &&Entry| -> bool {
            let sem_filter = match (sem_id, e.sem_id) {
                (Some(a), Some(b)) => a == b,
                (Some(_), None) => false,
                _ => true,
            };

            let topic_filter = if topics.is_empty() {
                true
            } else {
                e.topic.map_or(false, |t| topics.contains(&t))
            };

            let text_filter = match &text {
                Some(t) => e.matches(t, &langs),
                None => true,
            };

            sem_filter && topic_filter && text_filter
        };

        Ok(self
            .entries
            .values()
            .filter(filter)
            .cloned()
            .collect::<Vec<_>>())
    }
}

async fn fetch_dict(url: &str) -> ApiResult<(HashMap<u32, Entry>, Counters)> {
    let response = reqwest::get(url).await?.text().await?;
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(response.as_bytes());

    let mut entries = HashMap::new();

    let mut records = reader.records();
    let _header = records.next();

    println!("Reading counters");
    let r_counters = records.next().ok_or(ApiError::MissingDictHeaders)??;

    fn read_int(r: &StringRecord, index: usize) -> Result<u32, ApiError> {
        Ok(r.get(index)
            .ok_or(ApiError::MissingDictHeaders)?
            .replace('.', "")
            .parse()?)
    }

    let counters = Counters {
        total: read_int(&r_counters, 0)?,
        sem: read_int(&r_counters, 1)?,
        lat: read_int(&r_counters, 8)?,
        iro: read_int(&r_counters, 9)?,
        por: read_int(&r_counters, 10)?,
        spa: read_int(&r_counters, 11)?,
        cat: read_int(&r_counters, 12)?,
        occ: read_int(&r_counters, 13)?,
        fra: read_int(&r_counters, 14)?,
        srd: read_int(&r_counters, 15)?,
        ita: read_int(&r_counters, 16)?,
        rom: read_int(&r_counters, 17)?,
        eng: read_int(&r_counters, 18)?,
        fol: read_int(&r_counters, 19)?,
        frk: read_int(&r_counters, 20)?,
        sla: read_int(&r_counters, 21)?,
    };
    println!("{:?}", counters);

    let _ = records.next();

    println!("Reading entries");

    for r in records.flatten() {
        if let Ok(r) = r.deserialize::<RawEntry>(None) {
            if let Ok(r) = Entry::try_from(r) {
                entries.insert(r.id, r);
            }
        }
    }

    println!("Entries count {}", entries.len());
    Ok((entries, counters))
}

#[derive(Deserialize, Debug)]
pub struct RawEntry {
    pub id: u32,
    pub sem_id: Option<u32>,
    pub category: Option<String>,
    #[serde(deserialize_with = "ok_or_default")]
    pub topic: Option<Topic>,
    pub sub_topic: Option<String>,
    pub sub_sub_topic: Option<String>,
    pub essential_flag: Option<String>,
    pub basic_flag: Option<String>,
    pub lat: Option<String>,
    pub iro: Option<String>,
    pub por: Option<String>,
    pub spa: Option<String>,
    pub cat: Option<String>,
    pub occ: Option<String>,
    pub fra: Option<String>,
    pub srd: Option<String>,
    pub ita: Option<String>,
    pub rom: Option<String>,
    pub eng: Option<String>,
    pub fol: Option<String>,
    pub frk: Option<String>,
    pub sla: Option<String>,
}

fn ok_or_default<'a, T, D>(deserializer: D) -> Result<T, D::Error>
where
    T: Deserialize<'a> + Default,
    D: Deserializer<'a>,
{
    let v: Value = Deserialize::deserialize(deserializer)?;
    Ok(T::deserialize(v).unwrap_or_default())
}

impl TryFrom<RawEntry> for Entry {
    type Error = ApiError;

    fn try_from(r: RawEntry) -> Result<Self, Self::Error> {
        Ok(Entry {
            id: r.id,
            sem_id: r.sem_id,
            topic: r.topic,
            essential_flag: matches!(r.essential_flag, Some(s) if s == "e"),
            basic_flag: matches!(r.basic_flag, Some(s) if s == "b"),
            lat: r.lat,
            iro: r.iro,
            por: r.por,
            spa: r.spa,
            cat: r.cat,
            occ: r.occ,
            fra: r.fra,
            srd: r.srd,
            ita: r.ita,
            rom: r.rom,
            eng: r.eng,
            fol: r.fol,
            frk: r.frk,
            sla: r.sla,
        })
    }
}

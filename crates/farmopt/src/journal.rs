//! The run's journal: every evaluation ever made for a farm's space (one line each, appended
//! as it finishes), so a search resumes where it stopped and a changed objective re-ranks
//! without evaluating anything again. A line: `version \t key \t level \t note \t metrics`
//! (metrics as JSON). Lines of another evaluator version are skipped.

use crate::{Level, Metrics};
use anyhow::Result;
use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// One evaluation.
#[derive(Clone, Debug)]
pub struct Entry {
    pub level: Level,
    pub metrics: Metrics,
    /// The judge's text lines (a card's facts).
    pub lines: std::collections::BTreeMap<String, String>,
    pub note: String,
}

/// A candidate's evaluations so far, by level.
#[derive(Clone, Debug, Default)]
pub struct Record {
    pub by_level: HashMap<Level, Entry>,
}

impl Record {
    /// The furthest level reached.
    pub fn level(&self) -> Option<Level> {
        self.by_level.keys().copied().max()
    }
}

/// The journal of one space.
pub struct Journal {
    path: PathBuf,
    version: String,
    map: Mutex<HashMap<String, Record>>,
    file: Mutex<std::fs::File>,
}

fn clean(s: &str) -> String {
    s.replace(['\t', '\n', '\r'], " ")
}

impl Journal {
    /// Open (or create) the journal at `path`, reading back every line of `version`.
    pub fn open(path: &Path, version: &str) -> Result<Journal> {
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d)?;
        }
        let mut map: HashMap<String, Record> = HashMap::new();
        if path.exists() {
            for line in std::io::BufReader::new(std::fs::File::open(path)?).lines() {
                let line = line?;
                let f: Vec<&str> = line.splitn(6, '\t').collect();
                let (v, key, level, note, metrics, lines) = match f.as_slice() {
                    [v, key, level, note, metrics] => (*v, *key, *level, *note, *metrics, "{}"),
                    [v, key, level, note, metrics, lines] => (*v, *key, *level, *note, *metrics, *lines),
                    _ => continue,
                };
                if v != version {
                    continue;
                }
                // a forgotten key: its earlier lines no longer count (`forget`)
                if level == "forget" {
                    map.remove(key);
                    continue;
                }
                let level = match level {
                    "invalid" => Level::Invalid,
                    "static" => Level::Static,
                    "quick" => Level::Quick,
                    "full" => Level::Full,
                    _ => continue,
                };
                let Ok(metrics) = serde_json::from_str::<Metrics>(metrics) else { continue };
                let lines = serde_json::from_str(lines).unwrap_or_default();
                map.entry(key.to_string()).or_default().by_level.insert(level, Entry { level, metrics, lines, note: note.to_string() });
            }
        }
        let file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Journal { path: path.to_path_buf(), version: version.to_string(), map: Mutex::new(map), file: Mutex::new(file) })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The record of `key`, if any.
    pub fn get(&self, key: &str) -> Option<Record> {
        self.map.lock().ok()?.get(key).cloned()
    }

    /// The entry of `key` at `level`, if any.
    pub fn at(&self, key: &str, level: Level) -> Option<Entry> {
        self.map.lock().ok()?.get(key).and_then(|r| r.by_level.get(&level).cloned())
    }

    /// Record an evaluation (appended to the file at once).
    pub fn put(&self, key: &str, e: Entry) -> Result<()> {
        let line = format!("{}\t{}\t{}\t{}\t{}\t{}\n", self.version, clean(key), e.level.name(), clean(&e.note), serde_json::to_string(&e.metrics)?, clean(&serde_json::to_string(&e.lines)?));
        if let Ok(mut f) = self.file.lock() {
            f.write_all(line.as_bytes())?;
        }
        if let Ok(mut m) = self.map.lock() {
            m.entry(key.to_string()).or_default().by_level.insert(e.level, e);
        }
        Ok(())
    }

    /// Keys recorded.
    pub fn len(&self) -> usize {
        self.map.lock().map(|m| m.len()).unwrap_or(0)
    }

    /// Forget every key containing `pat` (a part of the evaluator changed for them only): a
    /// `forget` line is appended for each, so they are evaluated again now and on every later
    /// open. Returns how many.
    pub fn forget(&self, pat: &str) -> Result<usize> {
        let keys: Vec<String> = self.map.lock().map(|m| m.keys().filter(|k| k.contains(pat)).cloned().collect()).unwrap_or_default();
        for k in &keys {
            let line = format!("{}\t{}\tforget\t\t{{}}\t{{}}\n", self.version, clean(k));
            if let Ok(mut f) = self.file.lock() {
                f.write_all(line.as_bytes())?;
            }
            if let Ok(mut m) = self.map.lock() {
                m.remove(k);
            }
        }
        Ok(keys.len())
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Value;

    #[test]
    fn a_journal_reads_back_its_own_version_only() {
        let dir = std::env::temp_dir().join(format!("farmopt-journal-{}", std::process::id()));
        let path = dir.join("j.tsv");
        let _ = std::fs::remove_file(&path);
        let mut m = Metrics::new();
        m.insert("rate".into(), Value { v: 1.5, lo: 1.0, hi: 2.0 });
        {
            let j = Journal::open(&path, "v1").unwrap();
            let lines = std::collections::BTreeMap::from([("dims".to_string(), "5 x 6\tx 7".to_string())]);
            j.put("a\tb", Entry { level: Level::Quick, metrics: m.clone(), lines, note: "two\nlines".into() }).unwrap();
            j.put("c", Entry { level: Level::Invalid, metrics: Metrics::new(), lines: Default::default(), note: "refused".into() }).unwrap();
        }
        let j = Journal::open(&path, "v1").unwrap();
        assert_eq!(j.len(), 2);
        let e = j.at("a b", Level::Quick).unwrap();
        assert_eq!(e.metrics["rate"], m["rate"]);
        assert_eq!(e.lines["dims"], "5 x 6\tx 7");
        assert_eq!(e.note, "two lines");
        assert_eq!(j.get("c").unwrap().level(), Some(Level::Invalid));
        assert!(Journal::open(&path, "v2").unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

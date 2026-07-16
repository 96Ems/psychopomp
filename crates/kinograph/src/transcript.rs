use std::{fs, path::Path};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::composition::{Cue, Time, TimeRange};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TranscriptFile {
    word_timings: Vec<WordTiming>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct WordTiming {
    pub word: String,
    pub start: f64,
    pub end: f64,
}

#[derive(Clone, Debug)]
pub struct Transcript {
    words: Vec<WordTiming>,
}

impl Transcript {
    pub fn load(path: &Path) -> Result<Self> {
        let bytes = fs::read(path)
            .with_context(|| format!("read transcript timings from {}", path.display()))?;
        let file: TranscriptFile = serde_json::from_slice(&bytes)
            .with_context(|| format!("parse transcript timings from {}", path.display()))?;
        Self::new(file.word_timings)
    }

    pub fn new(words: Vec<WordTiming>) -> Result<Self> {
        let mut previous_end = 0.0;
        for word in &words {
            if !word.start.is_finite()
                || !word.end.is_finite()
                || word.start < 0.0
                || word.end < word.start
            {
                bail!("word '{}' has an invalid timing range", word.word);
            }
            if word.start < previous_end {
                bail!("word '{}' overlaps the previous word", word.word);
            }
            previous_end = word.end;
        }
        Ok(Self { words })
    }

    pub fn word(&self, word: &str) -> Result<Cue> {
        self.word_occurrence(word, 0)
    }

    pub fn word_occurrence(&self, word: &str, occurrence: usize) -> Result<Cue> {
        let timing = self
            .words
            .iter()
            .filter(|timing| timing.word == word)
            .nth(occurrence)
            .with_context(|| {
                format!("transcript does not contain occurrence {occurrence} of '{word}'")
            })?;
        let id = if occurrence == 0 {
            word.to_owned()
        } else {
            format!("{word}#{occurrence}")
        };
        Ok(Cue::new(
            id,
            TimeRange::new(Time::seconds(timing.start), Time::seconds(timing.end)),
        ))
    }

    pub fn duration(&self) -> Time {
        self.words
            .last()
            .map_or(Time::ZERO, |word| Time::seconds(word.end))
    }
}

#[cfg(test)]
mod tests {
    use super::{Transcript, WordTiming};

    #[test]
    fn word_occurrences_become_exact_cue_ranges() {
        let transcript = Transcript::new(vec![
            WordTiming {
                word: "Effect".to_owned(),
                start: 0.7,
                end: 1.06,
            },
            WordTiming {
                word: "Effect".to_owned(),
                start: 2.0,
                end: 2.4,
            },
        ])
        .unwrap();

        let cue = transcript.word_occurrence("Effect", 1).unwrap();

        assert_eq!(cue.id().as_str(), "Effect#1");
        assert_eq!(cue.start().as_seconds(), 2.0);
        assert_eq!(cue.end().as_seconds(), 2.4);
    }
}

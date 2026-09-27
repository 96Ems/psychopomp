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

    /// The first occurrence of `phrase` whose first word starts at or after
    /// `after_seconds`, as a cue from that word's start to the last word's end.
    /// Matching compares whole words, ignoring case and punctuation, so narration
    /// timing like `"404,"` matches the phrase `"404"`.
    pub fn phrase_after(&self, phrase: &str, after_seconds: f64) -> Result<Cue> {
        let wanted = phrase
            .split_whitespace()
            .map(normalize)
            .filter(|word| !word.is_empty())
            .collect::<Vec<_>>();
        if wanted.is_empty() {
            bail!("transcript phrase must contain a word");
        }
        let spoken = self
            .words
            .iter()
            .map(|timing| normalize(&timing.word))
            .collect::<Vec<_>>();
        let start = (0..spoken.len().saturating_sub(wanted.len() - 1))
            .filter(|&index| self.words[index].start >= after_seconds)
            .find(|&index| spoken[index..index + wanted.len()] == wanted[..])
            .with_context(|| {
                format!("transcript does not contain '{phrase}' after {after_seconds:.2}s")
            })?;
        let end = &self.words[start + wanted.len() - 1];
        Ok(Cue::new(
            wanted.join("-"),
            TimeRange::new(
                Time::seconds(self.words[start].start),
                Time::seconds(end.end),
            ),
        ))
    }

    pub fn phrase(&self, phrase: &str) -> Result<Cue> {
        self.phrase_after(phrase, 0.0)
    }

    pub fn words(&self) -> &[WordTiming] {
        &self.words
    }

    pub fn duration(&self) -> Time {
        self.words
            .last()
            .map_or(Time::ZERO, |word| Time::seconds(word.end))
    }
}

/// Lowercase alphanumerics, with number words as digits: speech recognition
/// writes "fifteen" or "15" depending on the voice, and both must match.
fn normalize(word: &str) -> String {
    let word = word
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    const NUMBERS: [&str; 21] = [
        "zero",
        "one",
        "two",
        "three",
        "four",
        "five",
        "six",
        "seven",
        "eight",
        "nine",
        "ten",
        "eleven",
        "twelve",
        "thirteen",
        "fourteen",
        "fifteen",
        "sixteen",
        "seventeen",
        "eighteen",
        "nineteen",
        "twenty",
    ];
    const TENS: [(&str, &str); 8] = [
        ("thirty", "30"),
        ("forty", "40"),
        ("fifty", "50"),
        ("sixty", "60"),
        ("seventy", "70"),
        ("eighty", "80"),
        ("ninety", "90"),
        ("hundred", "100"),
    ];
    if let Some(value) = NUMBERS.iter().position(|number| *number == word) {
        return value.to_string();
    }
    TENS.iter()
        .find(|(name, _)| *name == word)
        .map_or(word, |(_, digits)| (*digits).to_owned())
}

#[cfg(test)]
mod tests {
    use super::{Transcript, WordTiming};

    fn words(timed: &[(&str, f64)]) -> Transcript {
        Transcript::new(
            timed
                .iter()
                .map(|&(word, start)| WordTiming {
                    word: word.to_owned(),
                    start,
                    end: start + 0.125,
                })
                .collect(),
        )
        .unwrap()
    }

    #[test]
    fn phrases_match_whole_words_without_case_or_punctuation() {
        let transcript = words(&[
            ("The", 0.0),
            ("server", 0.3),
            ("answers", 0.6),
            ("404,", 0.9),
            ("not", 1.2),
            ("found.", 1.5),
            ("Not", 2.0),
            ("found", 2.3),
        ]);
        let cue = transcript.phrase("answers 404").unwrap();
        assert_eq!(cue.id().as_str(), "answers-404");
        assert_eq!(
            (cue.start().as_seconds(), cue.end().as_seconds()),
            (0.6, 1.025)
        );
        assert_eq!(
            transcript.phrase("not found").unwrap().start().as_seconds(),
            1.2
        );
        assert_eq!(
            transcript
                .phrase_after("not found", 1.3)
                .unwrap()
                .start()
                .as_seconds(),
            2.0
        );
        assert!(transcript.phrase("found server").is_err());
        let spoken = words(&[
            ("waits", 0.0),
            ("up", 0.2),
            ("to", 0.4),
            ("15", 0.6),
            ("seconds", 0.8),
        ]);
        assert_eq!(
            spoken
                .phrase("up to fifteen seconds")
                .unwrap()
                .start()
                .as_seconds(),
            0.2
        );
        assert!(transcript.phrase("  ").is_err());
    }

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

//! Word timings from provider character alignment, from Whisper, or estimated
//! from text before any audio exists.
use anyhow::{Context, Result};
use psychopomp::transcript::WordTiming;
use serde::Deserialize;

/// Per-character timing, as ElevenLabs returns it for the original text.
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Alignment {
    pub characters: Vec<String>,
    pub character_start_times_seconds: Vec<f64>,
    pub character_end_times_seconds: Vec<f64>,
}

/// Spoken words from character alignment. Bracketed directions such as
/// `[whispering]` are performance, not words, and tokens without a letter or
/// digit (a lone `...`) are pauses; neither becomes a word. Words keep the
/// script's spelling and punctuation, which phrase lookups ignore.
pub(crate) fn from_alignment(alignment: &Alignment) -> Vec<WordTiming> {
    let mut words = Vec::new();
    let mut current: Option<WordTiming> = None;
    let mut depth = 0usize;
    let timed = alignment
        .characters
        .iter()
        .zip(&alignment.character_start_times_seconds)
        .zip(&alignment.character_end_times_seconds);
    for ((characters, &start), &end) in timed {
        for character in characters.chars() {
            match character {
                '[' => {
                    depth += 1;
                    words.extend(current.take());
                }
                ']' => depth = depth.saturating_sub(1),
                _ if depth > 0 => {}
                c if c.is_whitespace() => words.extend(current.take()),
                c => {
                    let word = current.get_or_insert_with(|| WordTiming {
                        word: String::new(),
                        start,
                        end,
                    });
                    word.word.push(c);
                    word.end = end;
                }
            }
        }
    }
    words.extend(current);
    ordered(
        words
            .into_iter()
            .filter(|word| word.word.chars().any(char::is_alphanumeric))
            .collect(),
    )
}

#[derive(Deserialize)]
struct WhisperFile {
    segments: Vec<WhisperSegment>,
}

#[derive(Deserialize)]
struct WhisperSegment {
    #[serde(default)]
    words: Vec<WordTiming>,
}

/// Whisper's word timestamps, exactly as `scripts/narrate.ts` reads them.
pub(crate) fn from_whisper(json: &[u8]) -> Result<Vec<WordTiming>> {
    let file: WhisperFile = serde_json::from_slice(json).context("parse Whisper JSON")?;
    Ok(ordered(
        file.segments
            .into_iter()
            .flat_map(|segment| segment.words)
            .map(|word| WordTiming {
                word: word.word.trim().to_owned(),
                ..word
            })
            .collect(),
    ))
}

/// In order without overlap (aligners can overlap neighbors by a few
/// milliseconds, and transcripts require order), on whole microseconds.
pub(crate) fn ordered(words: Vec<WordTiming>) -> Vec<WordTiming> {
    let mut previous_end = 0.0f64;
    words
        .into_iter()
        .map(|word| {
            let start = micros(word.start).max(previous_end);
            let end = micros(word.end).max(start);
            previous_end = end;
            WordTiming { start, end, ..word }
        })
        .collect()
}

/// Ends no later than the audio does.
pub(crate) fn clamped(words: Vec<WordTiming>, duration: u64) -> Vec<WordTiming> {
    let seconds = micros(duration as f64 / 1e9);
    words
        .into_iter()
        .map(|word| WordTiming {
            start: word.start.min(seconds),
            end: word.end.min(seconds),
            ..word
        })
        .collect()
}

/// `seconds` on a whole microsecond. Short decimals survive the lock's JSON
/// exactly; arithmetic such as `0.15 + 0.8` would not.
fn micros(seconds: f64) -> f64 {
    (seconds * 1e6).round() / 1e6
}

/// `text` without bracketed directions, as `say` should read it.
pub(crate) fn strip_tags(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '[' {
            out.push(c);
            continue;
        }
        for c in chars.by_ref() {
            if c == ']' {
                break;
            }
        }
        while chars.peek().is_some_and(|c| c.is_whitespace()) {
            chars.next();
        }
    }
    out
}

/// Spoken words and a duration guessed from text: a placeholder clock that
/// lets an offline plan run every phrase lookup before any audio exists.
pub(crate) fn estimate(text: &str) -> (Vec<WordTiming>, u64) {
    const LEAD: f64 = 0.15;
    const PER_WORD: f64 = 0.4;
    let words = ordered(
        strip_tags(text)
            .split_whitespace()
            .filter(|word| word.chars().any(char::is_alphanumeric))
            .enumerate()
            .map(|(index, word)| WordTiming {
                word: word.to_owned(),
                start: LEAD + index as f64 * PER_WORD,
                end: LEAD + index as f64 * PER_WORD + PER_WORD * 0.85,
            })
            .collect(),
    );
    let seconds = LEAD + words.len() as f64 * PER_WORD + 0.3;
    (words, (seconds * 1e9) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An alignment the way ElevenLabs reports it: one entry per character of
    /// the original text, directions included.
    fn align(text: &str) -> Alignment {
        let characters = text.chars().map(String::from).collect::<Vec<_>>();
        let starts = (0..characters.len())
            .map(|i| i as f64 * 0.1)
            .collect::<Vec<_>>();
        let ends = starts.iter().map(|start| start + 0.1).collect();
        Alignment {
            characters,
            character_start_times_seconds: starts,
            character_end_times_seconds: ends,
        }
    }

    fn spoken(words: &[WordTiming]) -> Vec<&str> {
        words.iter().map(|word| word.word.as_str()).collect()
    }

    #[test]
    fn alignment_words_drop_directions_and_pauses() {
        let text =
            "[extremely soft ASMR whisper] Oh... I hear you like... balls. [SHOUTING] BALLS!";
        let words = from_alignment(&align(text));
        assert_eq!(
            spoken(&words),
            ["Oh...", "I", "hear", "you", "like...", "balls.", "BALLS!"]
        );
        let oh = text.find("Oh").unwrap() as f64 * 0.1;
        assert!((words[0].start - oh).abs() < 1e-9);
        assert!(
            (words[0].end - (oh + 0.5)).abs() < 1e-9,
            "through the ellipsis"
        );
        assert!(words.windows(2).all(|pair| pair[0].end <= pair[1].start));
        let transcript = psychopomp::transcript::Transcript::new(words).unwrap();
        assert_eq!(transcript.phrases("balls").unwrap().len(), 2);
    }

    #[test]
    fn a_lone_pause_and_a_tag_between_words_split_them() {
        let words = from_alignment(&align("one ... two[sighs]three"));
        assert_eq!(spoken(&words), ["one", "two", "three"]);
    }

    #[test]
    fn whisper_words_are_trimmed_and_ordered() {
        let json = br#"{"segments":[{"words":[{"word":" Hi,","start":0.0,"end":0.5},{"word":" there","start":0.45,"end":0.9}]},{"words":[]}]}"#;
        let words = from_whisper(json).unwrap();
        assert_eq!(spoken(&words), ["Hi,", "there"]);
        assert_eq!((words[1].start, words[1].end), (0.5, 0.9));
    }

    #[test]
    fn tags_strip_like_narrate_ts() {
        assert_eq!(strip_tags("[soft] Hi. [loud]  THERE[x]!"), "Hi. THERE!");
    }

    #[test]
    fn estimates_time_every_spoken_word() {
        let (words, duration) = estimate("[whisper] Balls... balls! [shout] BALLS");
        assert_eq!(spoken(&words), ["Balls...", "balls!", "BALLS"]);
        assert!(duration as f64 / 1e9 > words[2].end);
    }
}

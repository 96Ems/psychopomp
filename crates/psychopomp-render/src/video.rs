//! The input-video boundary: FFmpeg decodes a video (or an image sequence)
//! once into a seekable raw RGBA file under `target/`, keyed by the source's
//! content and the decode contract, so every actor and plan that shows the
//! same source at the same size shares one decode. Fixed-size frames then
//! give deterministic arbitrary-time sampling without codec bindings.
use std::{
    fs::{self, File},
    hash::{DefaultHasher, Hash, Hasher},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::{self, Command, Stdio},
};

use anyhow::{Context, Result, bail};

/// What FFmpeg decodes: `source` (or, for an image sequence, its printf
/// pattern read at `sequence` frames per second) scaled to `size` at `fps`,
/// through `decoder` when a WebM keeps its alpha where only libvpx reads it,
/// and only the source nanoseconds in `range` when it has one (frame 0 is
/// then the range's start). Output is straight-alpha RGBA; opaque sources
/// decode with alpha 255.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct Decode {
    pub source: PathBuf,
    pub size: [u32; 2],
    pub fps: u32,
    pub sequence: Option<u32>,
    pub decoder: Option<&'static str>,
    pub range: Option<(u64, u64)>,
}

pub struct VideoFrameCache {
    file: File,
    width: u32,
    height: u32,
    fps: u32,
    frame_count: u64,
    current_frame: Option<u64>,
    pixels: Vec<u8>,
}

impl VideoFrameCache {
    /// Open `decode`'s frames from `directory`, decoding them there first
    /// when no earlier run has.
    pub(crate) fn open(decode: &Decode, directory: &Path) -> Result<Self> {
        let [width, height] = decode.size;
        if width == 0 || height == 0 || decode.fps == 0 {
            bail!("video dimensions and frame rate must be non-zero");
        }
        let cache = cache_path(decode, directory)?;
        if cache_needs_refresh(&cache) {
            decode_rgba_cache(decode, &cache)?;
        }
        let file = File::open(&cache)
            .with_context(|| format!("open decoded video cache {}", cache.display()))?;
        let frame_bytes = frame_bytes(width, height)?;
        let cache_bytes = file
            .metadata()
            .with_context(|| format!("read decoded video cache metadata {}", cache.display()))?
            .len();
        if cache_bytes == 0 || cache_bytes % frame_bytes as u64 != 0 {
            bail!(
                "decoded video cache {} has invalid size {cache_bytes} for {width}x{height} RGBA frames",
                cache.display()
            );
        }
        Ok(Self {
            file,
            width,
            height,
            fps: decode.fps,
            frame_count: cache_bytes / frame_bytes as u64,
            current_frame: None,
            pixels: vec![0; frame_bytes],
        })
    }

    pub fn size(&self) -> [u32; 2] {
        [self.width, self.height]
    }

    pub fn fps(&self) -> u32 {
        self.fps
    }

    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    pub fn frame_index_at(&self, seconds: f32) -> u64 {
        frame_index(seconds, self.fps, self.frame_count)
    }

    pub(crate) fn frame_at_index(&mut self, index: u64) -> Result<&[u8]> {
        anyhow::ensure!(
            index < self.frame_count,
            "decoded frame index is out of range"
        );
        if self.current_frame != Some(index) {
            let offset = index
                .checked_mul(self.pixels.len() as u64)
                .context("decoded video frame offset overflow")?;
            // A failed read may overwrite only part of the old frame's pixels.
            self.current_frame = None;
            self.file
                .seek(SeekFrom::Start(offset))
                .context("seek decoded video frame")?;
            self.file
                .read_exact(&mut self.pixels)
                .context("read decoded video frame")?;
            self.current_frame = Some(index);
        }
        Ok(&self.pixels)
    }
}

fn frame_bytes(width: u32, height: u32) -> Result<usize> {
    (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .context("video frame byte count overflow")
}

fn frame_index(seconds: f32, fps: u32, frame_count: u64) -> u64 {
    let requested = (seconds.max(0.0) * fps as f32).floor() as u64;
    requested.min(frame_count.saturating_sub(1))
}

fn cache_needs_refresh(cache: &Path) -> bool {
    fs::metadata(cache).map_or(true, |metadata| metadata.len() == 0)
}

/// An image sequence's frame files, from its first existing number (0 to 4,
/// as FFmpeg looks) while they continue.
pub(crate) fn sequence_frames(pattern: &Path) -> Result<(u32, Vec<PathBuf>)> {
    let text = pattern
        .to_str()
        .context("image sequence pattern must be UTF-8")?;
    let percent = text
        .rfind('%')
        .context("image sequence needs a %d pattern")?;
    let rest = &text[percent + 1..];
    let digits = rest.chars().take_while(char::is_ascii_digit).count();
    if !rest[digits..].starts_with('d') {
        bail!("image sequence needs a %d pattern");
    }
    let width = rest[..digits].parse::<usize>().unwrap_or(0);
    let (prefix, suffix) = (&text[..percent], &rest[digits + 1..]);
    let frame = |n: u32| PathBuf::from(format!("{prefix}{n:0width$}{suffix}"));
    let start = (0..=4)
        .find(|n| frame(*n).is_file())
        .with_context(|| format!("no frames match {}", pattern.display()))?;
    let frames = (start..)
        .map(frame)
        .take_while(|path| path.is_file())
        .collect();
    Ok((start, frames))
}

/// Where `decode`'s frames are cached in `directory`.
pub(crate) fn cache_path(decode: &Decode, directory: &Path) -> Result<PathBuf> {
    Ok(directory.join(format!("{:016x}.rgba", cache_key(decode)?)))
}

/// A decode's identity: the bytes of every file it reads and its contract.
fn cache_key(decode: &Decode) -> Result<u64> {
    let mut hasher = DefaultHasher::new();
    let files = match decode.sequence {
        Some(_) => sequence_frames(&decode.source)?.1,
        None => vec![decode.source.clone()],
    };
    for source in &files {
        hash_file(source, &mut hasher)?;
    }
    decode.size.hash(&mut hasher);
    decode.fps.hash(&mut hasher);
    decode.sequence.hash(&mut hasher);
    decode.decoder.hash(&mut hasher);
    if decode.range.is_some() {
        decode.range.hash(&mut hasher);
    }
    Ok(hasher.finish())
}

fn hash_file(source: &Path, hasher: &mut DefaultHasher) -> Result<()> {
    let mut file = File::open(source)
        .with_context(|| format!("open video source for cache identity {}", source.display()))?;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("hash video source {}", source.display()))?;
        if read == 0 {
            break;
        }
        hasher.write(&buffer[..read]);
    }
    Ok(())
}

fn decode_rgba_cache(decode: &Decode, cache: &Path) -> Result<()> {
    if let Some(parent) = cache.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create video cache directory {}", parent.display()))?;
    }
    let temporary = temporary_cache_path(cache);
    let [width, height] = decode.size;
    let source = &decode.source;
    let mut command = Command::new("ffmpeg");
    // The parent may own a pipelined JSON protocol on stdin, not hotkeys.
    command
        .stdin(Stdio::null())
        .args(["-y", "-loglevel", "error"]);
    if let Some(decoder) = decode.decoder {
        command.args(["-c:v", decoder]);
    }
    if let Some((from, to)) = decode.range {
        let seconds =
            |nanos: u64| format!("{}.{:09}", nanos / 1_000_000_000, nanos % 1_000_000_000);
        command.args([
            "-ss",
            &seconds(from),
            "-t",
            &seconds(to.saturating_sub(from)),
        ]);
    }
    if let Some(rate) = decode.sequence {
        let (start, _) = sequence_frames(source)?;
        command.args([
            "-f",
            "image2",
            "-framerate",
            &rate.to_string(),
            "-start_number",
            &start.to_string(),
        ]);
    }
    let status = command
        .arg("-i")
        .arg(source)
        .args(["-an", "-vf"])
        .arg(format!(
            "fps={},scale={width}:{height}:flags=lanczos",
            decode.fps
        ))
        .args(["-pix_fmt", "rgba", "-f", "rawvideo"])
        .arg(&temporary)
        .status()
        .with_context(|| format!("start FFmpeg video decoder for {}", source.display()))?;
    if !status.success() {
        let _ = fs::remove_file(&temporary);
        bail!(
            "FFmpeg video decoder failed for {} with status {status}",
            source.display()
        );
    }
    if let Err(error) = fs::rename(&temporary, cache) {
        if !cache.exists() {
            let _ = fs::remove_file(&temporary);
            return Err(error).with_context(|| {
                format!(
                    "move decoded video cache from {} to {}",
                    temporary.display(),
                    cache.display()
                )
            });
        }
        let _ = fs::remove_file(&temporary);
    }
    Ok(())
}

fn temporary_cache_path(cache: &Path) -> PathBuf {
    let mut path = cache.as_os_str().to_owned();
    path.push(format!(".{}.tmp", process::id()));
    PathBuf::from(path)
}

#[cfg(test)]
mod tests {
    use std::{fs, io::ErrorKind, path::PathBuf};

    use super::{Decode, cache_key, frame_index, sequence_frames};

    struct TestSource(PathBuf);

    impl TestSource {
        fn new(name: &str, bytes: &[u8]) -> Self {
            let path = std::env::temp_dir().join(format!(
                "psychopomp-video-{name}-{}.bin",
                std::process::id()
            ));
            fs::write(&path, bytes).unwrap();
            Self(path)
        }
    }

    impl Drop for TestSource {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }

    fn decode(source: &TestSource, size: [u32; 2], fps: u32) -> Decode {
        Decode {
            source: source.0.clone(),
            size,
            fps,
            sequence: None,
            decoder: None,
            range: None,
        }
    }

    #[test]
    fn failed_partial_read_does_not_poison_the_previously_cached_frame() {
        let source = TestSource::new(
            "partial-read",
            &[1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2],
        );
        let mut cache = super::VideoFrameCache {
            file: fs::File::open(&source.0).unwrap(),
            width: 2,
            height: 1,
            fps: 1,
            frame_count: 2,
            current_frame: None,
            pixels: vec![0; 8],
        };
        assert_eq!(cache.frame_at_index(0).unwrap(), &[1; 8]);
        fs::OpenOptions::new()
            .write(true)
            .open(&source.0)
            .unwrap()
            .set_len(12)
            .unwrap();
        let error = cache.frame_at_index(1).unwrap_err();
        assert_eq!(
            error.downcast_ref::<std::io::Error>().unwrap().kind(),
            ErrorKind::UnexpectedEof
        );
        assert_eq!(cache.frame_at_index(0).unwrap(), &[1; 8]);
        assert_eq!(cache.frame_at_index(0).unwrap(), &[1; 8]);
    }

    #[test]
    #[ignore = "requires FFmpeg; decodes a temporary recording to check parent stdin ownership"]
    fn decoder_preserves_parent_protocol_input() {
        use std::{io::Read, process::Command};

        const SENTINEL: &str = "{\"id\":\"sentinel\",\"command\":\"shutdown\"}\n";
        if std::env::var_os("PSYCHOPOMP_DECODER_STDIN_CHILD").is_some() {
            let cache = TestSource::new("decoder-output", &[]);
            let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../assets/opencode-hot-reload/fire-the-missiles.mp4");
            let decode = Decode {
                source,
                size: [1200, 720],
                fps: 25,
                sequence: None,
                decoder: None,
                range: None,
            };
            super::decode_rgba_cache(&decode, &cache.0).unwrap();
            let mut remaining = String::new();
            std::io::stdin().read_to_string(&mut remaining).unwrap();
            assert_eq!(remaining, SENTINEL);
            return;
        }
        // Prefilled owned input avoids a writer/decoder startup race. The child
        // must leave its protocol stream untouched while FFmpeg decodes a file.
        let input = TestSource::new("decoder-stdin", SENTINEL.as_bytes());
        let child = Command::new(std::env::current_exe().unwrap())
            .args([
                "video::tests::decoder_preserves_parent_protocol_input",
                "--exact",
                "--ignored",
                "--nocapture",
            ])
            .env("PSYCHOPOMP_DECODER_STDIN_CHILD", "1")
            .stdin(fs::File::open(&input.0).unwrap())
            .output()
            .unwrap();
        assert!(
            child.status.success(),
            "{}",
            String::from_utf8_lossy(&child.stderr)
        );
    }

    #[test]
    fn frame_sampling_is_clamped_and_deterministic() {
        assert_eq!(frame_index(-1.0, 20, 181), 0);
        assert_eq!(frame_index(0.049, 20, 181), 0);
        assert_eq!(frame_index(0.05, 20, 181), 1);
        assert_eq!(frame_index(4.808, 25, 202), 120);
        assert_eq!(frame_index(90.0, 20, 181), 180);
    }

    #[test]
    fn cache_identity_includes_source_and_decode_contract() {
        let mut bytes = vec![7; 64 * 1024 + 17];
        let source = TestSource::new("identity", &bytes);
        let copy = TestSource::new("identity-copy", &bytes);
        let key = |size, fps| cache_key(&decode(&source, size, fps)).unwrap();
        let base = key([1200, 720], 25);
        assert_eq!(base, cache_key(&decode(&copy, [1200, 720], 25)).unwrap());
        assert_ne!(base, key([1920, 720], 25));
        assert_ne!(base, key([1200, 1080], 25));
        assert_ne!(base, key([1200, 720], 30));
        let alpha = Decode {
            decoder: Some("libvpx-vp9"),
            ..decode(&source, [1200, 720], 25)
        };
        assert_ne!(base, cache_key(&alpha).unwrap());
        let trimmed = Decode {
            range: Some((1_000_000_000, 2_000_000_000)),
            ..decode(&source, [1200, 720], 25)
        };
        assert_ne!(base, cache_key(&trimmed).unwrap());
        // A same-length change beyond the first read must change source identity.
        *bytes.last_mut().unwrap() = 8;
        fs::write(&source.0, &bytes).unwrap();
        assert_ne!(base, key([1200, 720], 25));
    }

    #[test]
    fn sequences_start_where_ffmpeg_looks_and_run_while_frames_continue() {
        let directory =
            std::env::temp_dir().join(format!("psychopomp-sequence-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        for n in [1, 2, 3, 5] {
            fs::write(directory.join(format!("frame-{n:03}.png")), [n as u8]).unwrap();
        }
        let (start, frames) = sequence_frames(&directory.join("frame-%03d.png")).unwrap();
        assert_eq!(start, 1);
        assert_eq!(
            frames.len(),
            3,
            "frame 4 is missing, so 5 is not part of it"
        );
        assert!(frames[2].ends_with("frame-003.png"));
        assert!(sequence_frames(&directory.join("other-%d.png")).is_err());
        assert!(sequence_frames(&directory.join("frame.png")).is_err());
        let _ = fs::remove_dir_all(&directory);
    }
}

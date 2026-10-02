use std::{
    fs::{self, File},
    hash::{DefaultHasher, Hash, Hasher},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::{self, Command, Stdio},
};

use anyhow::{Context, Result, bail};

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
    pub fn open(
        source: impl AsRef<Path>,
        cache: impl AsRef<Path>,
        width: u32,
        height: u32,
        fps: u32,
    ) -> Result<Self> {
        if width == 0 || height == 0 || fps == 0 {
            bail!("video dimensions and frame rate must be non-zero");
        }
        let source = source.as_ref();
        let cache_base = cache.as_ref();
        let cache = contracted_cache_path(source, cache_base, width, height, fps)?;
        if cache_needs_refresh(&cache) {
            decode_rgba_cache(source, &cache, width, height, fps)?;
        }
        let file = File::open(&cache)
            .with_context(|| format!("open decoded video cache {}", cache.display()))?;
        prune_cache_variants(cache_base, &cache);
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
            fps,
            frame_count: cache_bytes / frame_bytes as u64,
            current_frame: None,
            pixels: vec![0; frame_bytes],
        })
    }

    pub fn size(&self) -> [u32; 2] {
        [self.width, self.height]
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

fn contracted_cache_path(
    source: &Path,
    cache: &Path,
    width: u32,
    height: u32,
    fps: u32,
) -> Result<PathBuf> {
    let key = cache_key_file(source, width, height, fps)?;
    let name = cache
        .file_name()
        .and_then(|name| name.to_str())
        .context("video cache path must have a UTF-8 file name")?;
    Ok(cache.with_file_name(format!("{name}-{key:016x}.rgba")))
}

fn cache_key_file(source: &Path, width: u32, height: u32, fps: u32) -> Result<u64> {
    let mut file = File::open(source)
        .with_context(|| format!("open video source for cache identity {}", source.display()))?;
    let mut hasher = DefaultHasher::new();
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
    width.hash(&mut hasher);
    height.hash(&mut hasher);
    fps.hash(&mut hasher);
    Ok(hasher.finish())
}

fn prune_cache_variants(base: &Path, keep: &Path) {
    let Some(parent) = base.parent() else {
        return;
    };
    let Some(name) = base.file_name().and_then(|name| name.to_str()) else {
        return;
    };
    let Ok(entries) = fs::read_dir(parent) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let variant = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|candidate| {
                candidate.starts_with(&format!("{name}-")) && candidate.ends_with(".rgba")
            });
        if variant && path != keep {
            let _ = fs::remove_file(path);
        }
    }
}

fn decode_rgba_cache(source: &Path, cache: &Path, width: u32, height: u32, fps: u32) -> Result<()> {
    if let Some(parent) = cache.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create video cache directory {}", parent.display()))?;
    }
    let temporary = temporary_cache_path(cache);
    let status = Command::new("ffmpeg")
        // The parent may own a pipelined JSON protocol on stdin, not hotkeys.
        .stdin(Stdio::null())
        .args(["-y", "-loglevel", "error", "-i"])
        .arg(source)
        .args(["-an", "-vf"])
        .arg(format!("fps={fps},scale={width}:{height}:flags=lanczos"))
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

    use super::{cache_key_file, frame_index};

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
            super::decode_rgba_cache(&source, &cache.0, 1200, 720, 25).unwrap();
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
        let key = |width, height, fps| cache_key_file(&source.0, width, height, fps).unwrap();
        let base = key(1200, 720, 25);
        assert_eq!(base, cache_key_file(&copy.0, 1200, 720, 25).unwrap());
        assert_ne!(base, key(1920, 720, 25));
        assert_ne!(base, key(1200, 1080, 25));
        assert_ne!(base, key(1200, 720, 30));
        // A same-length change beyond the first read must change source identity.
        *bytes.last_mut().unwrap() = 8;
        fs::write(&source.0, &bytes).unwrap();
        assert_ne!(base, key(1200, 720, 25));
    }
}

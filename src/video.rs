use std::{
    fs::{self, File},
    hash::{DefaultHasher, Hash, Hasher},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::{self, Command},
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

    pub fn frame_at(&mut self, seconds: f32) -> Result<&[u8]> {
        let index = frame_index(seconds, self.fps, self.frame_count);
        if self.current_frame != Some(index) {
            let offset = index
                .checked_mul(self.pixels.len() as u64)
                .context("decoded video frame offset overflow")?;
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

#[cfg(test)]
fn cache_key(source: &[u8], width: u32, height: u32, fps: u32) -> u64 {
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    width.hash(&mut hasher);
    height.hash(&mut hasher);
    fps.hash(&mut hasher);
    hasher.finish()
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
    use super::{cache_key, frame_index};

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
        let base = cache_key(b"video", 1200, 720, 25);
        assert_ne!(base, cache_key(b"other video", 1200, 720, 25));
        assert_ne!(base, cache_key(b"video", 1920, 720, 25));
        assert_ne!(base, cache_key(b"video", 1200, 1080, 25));
        assert_ne!(base, cache_key(b"video", 1200, 720, 30));
    }
}

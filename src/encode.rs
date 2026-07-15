use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
};

use anyhow::{Context, Result, bail};

use crate::composition::{AssetKind, MediaPlacement};

#[derive(Clone, Copy)]
pub struct VideoSpec {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

pub struct FfmpegEncoder {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    frame_bytes: usize,
    output: PathBuf,
    temporary_output: PathBuf,
    finished: bool,
}

impl FfmpegEncoder {
    pub fn start(output: &Path, spec: VideoSpec) -> Result<Self> {
        Self::start_with_media(output, spec, &[])
    }

    pub fn start_with_media(
        output: &Path,
        spec: VideoSpec,
        media: &[MediaPlacement],
    ) -> Result<Self> {
        let file_name = output
            .file_name()
            .context("output path must include a file name")?
            .to_string_lossy();
        let temporary_output = output.with_file_name(format!(".{file_name}.kinograph-tmp.mp4"));
        let temporary_output_string = temporary_output.to_string_lossy().into_owned();
        let mut arguments = [
            "-y",
            "-loglevel",
            "error",
            "-f",
            "rawvideo",
            "-pixel_format",
            "rgba",
            "-video_size",
            &format!("{}x{}", spec.width, spec.height),
            "-framerate",
            &spec.fps.to_string(),
            "-i",
            "-",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
        let mut filters = Vec::with_capacity(media.len());
        for (index, placement) in media.iter().enumerate() {
            let asset = placement.clip().asset();
            if asset.kind() != AssetKind::Audio {
                bail!(
                    "FFmpeg media assembly currently supports audio clips, not {:?} asset '{}'",
                    asset.kind(),
                    asset.id().as_str()
                );
            }
            arguments.push("-i".to_owned());
            arguments.push(asset.path().to_string_lossy().into_owned());
            let source = placement.clip().source_range();
            filters.push(format!(
                "[{}:a]atrim=start={:.9}:end={:.9},volume={:.3}dB,asetpts=PTS-STARTPTS+{:.9}/TB[a{index}]",
                index + 1,
                source.start().as_seconds(),
                source.end().as_seconds(),
                placement.clip().audio_gain_db(),
                placement.timeline_range().start().as_seconds(),
            ));
        }
        if media.is_empty() {
            arguments.push("-an".to_owned());
        } else {
            let inputs = (0..media.len())
                .map(|index| format!("[a{index}]"))
                .collect::<String>();
            filters.push(format!(
                "{inputs}amix=inputs={}:duration=longest:normalize=0,alimiter=limit=0.95:level=0:latency=1[aout]",
                media.len()
            ));
            arguments.extend([
                "-filter_complex".to_owned(),
                filters.join(";"),
                "-map".to_owned(),
                "0:v:0".to_owned(),
                "-map".to_owned(),
                "[aout]".to_owned(),
                "-c:a".to_owned(),
                "aac".to_owned(),
                "-b:a".to_owned(),
                "192k".to_owned(),
            ]);
        }
        arguments.extend(
            [
                "-c:v",
                "libx264",
                "-preset",
                "medium",
                "-crf",
                "17",
                "-pix_fmt",
                "yuv420p",
                "-movflags",
                "+faststart",
                &temporary_output_string,
            ]
            .into_iter()
            .map(str::to_owned),
        );
        let mut child = Command::new("ffmpeg")
            .args(&arguments)
            .stdin(Stdio::piped())
            .spawn()
            .context("start FFmpeg")?;
        let stdin = child.stdin.take().context("open FFmpeg stdin")?;

        Ok(Self {
            child: Some(child),
            stdin: Some(stdin),
            frame_bytes: spec.width as usize * spec.height as usize * 4,
            output: output.to_owned(),
            temporary_output,
            finished: false,
        })
    }

    pub fn write_frame(&mut self, rgba: &[u8]) -> Result<()> {
        if rgba.len() != self.frame_bytes {
            bail!(
                "expected {} RGBA frame bytes, received {}",
                self.frame_bytes,
                rgba.len()
            );
        }
        self.stdin
            .as_mut()
            .context("FFmpeg input is already closed")?
            .write_all(rgba)
            .context("stream frame to FFmpeg")
    }

    pub fn finish(mut self) -> Result<()> {
        self.stdin.take();
        let status = self
            .child
            .as_mut()
            .context("FFmpeg process is already closed")?
            .wait()
            .context("wait for FFmpeg")?;
        self.child.take();
        if !status.success() {
            bail!("FFmpeg exited with {status}");
        }
        fs::rename(&self.temporary_output, &self.output).with_context(|| {
            format!(
                "move completed video from {} to {}",
                self.temporary_output.display(),
                self.output.display()
            )
        })?;
        self.finished = true;
        Ok(())
    }
}

impl Drop for FfmpegEncoder {
    fn drop(&mut self) {
        self.stdin.take();
        if let Some(mut child) = self.child.take() {
            if child.try_wait().ok().flatten().is_none() {
                let _ = child.kill();
            }
            let _ = child.wait();
        }
        if !self.finished {
            let _ = fs::remove_file(&self.temporary_output);
        }
    }
}

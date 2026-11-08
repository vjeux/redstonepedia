use anyhow::{Result, bail};
use std::{
    io::Write,
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

/// One encoding thread; the heavy work remains the one server and one genuine client.
pub struct Recording {
    child: Child,
}
impl Recording {
    pub fn start(display: &str, path: &Path) -> Result<Self> {
        let child = Command::new(mctas::video::ffmpeg()?)
            .args([
                "-y",
                "-loglevel",
                "error",
                "-f",
                "x11grab",
                "-draw_mouse",
                "0",
                "-framerate",
                "20",
                "-video_size",
                "960x540",
                "-i",
                display,
                "-vf",
                "scale=out_range=tv,format=yuv420p",
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-crf",
                "25",
                "-threads",
                "1",
                "-color_range",
                "tv",
                "-t",
                "180",
            ])
            .arg(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(std::fs::File::create(path.with_extension("ffmpeg.log"))?)
            .spawn()?;
        Ok(Self { child })
    }
    pub fn finish(mut self) -> Result<()> {
        if let Some(stdin) = self.child.stdin.as_mut() {
            stdin.write_all(b"q")?;
        }
        let status = self.child.wait()?;
        if !status.success() {
            bail!("recording failed {status}");
        }
        Ok(())
    }
}
impl Drop for Recording {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_some() {
            return;
        }
        if let Some(stdin) = self.child.stdin.as_mut() {
            let _ = stdin.write_all(b"q");
        }
        let until = Instant::now() + Duration::from_secs(3);
        while Instant::now() < until {
            if self.child.try_wait().ok().flatten().is_some() {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

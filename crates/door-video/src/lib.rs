//! `mctas door` commands: videos, cards and worlds: video-all, card, gallery, montage, video, tick-video, world.
//! One of the door CLI crates (crates/door-*), flattened into `mctas door` by
//! crates/mctas-cli/src/door.rs: a change here rebuilds only this crate and the binary.
//! Compiled unoptimized (Cargo.toml: argument parsing and dispatch only): put any heavy
//! loop in a piston-door function, which is optimized.

use anyhow::Result;
use clap::Subcommand;
// The command bodies (cmd/*.rs) name piston-door's modules `crate::<module>` through this glob.
use piston_door::*;

mod cmd;

#[derive(Subcommand)]
pub enum DoorCmd {
    /// Publish a video of every design in a folder (`<out>/<stem>.mp4`, uploaded), cut from each
    /// design's one clip (recorded first when missing or stale, several at once), then write the
    /// folder's catalog (README.md, see `door catalog`). A video is cut again only when its clip,
    /// card or label changed (seconds, no Minecraft).
    VideoAll(cmd::video::VideoAllArgs),
    /// Draw the title card (and the corner label, as a transparent PNG) of a design's video,
    /// without recording anything.
    Card(cmd::video::CardArgs),
    /// The best design of each board (size boards, speed boards) as a looping GIF (labeled with
    /// its size, the volume, big and its blocks placed small), cut from the design's one clip (the
    /// clips of `door video-all`, recorded here only when missing), uploaded to the release, plus
    /// the night's timeline chart. No page of its own: `door leaderboard` shows each board
    /// leader's GIF.
    Gallery(cmd::gallery::GalleryArgs),
    /// A board's record progression by blocks placed (solid: archive/records.tsv, plus a newer
    /// record; the history before sizes were volumes) as one video: every design filmed by the
    /// same camera, labeled with its blocks placed and how it got there.
    Montage(cmd::gallery::MontageArgs),
    /// Record the door opening and closing with the real Minecraft client (needs Xvfb) and publish
    /// it: the design's one clip (recorded only when missing or stale) cut into the video.
    Video(cmd::video::VideoArgs),
    /// One open/close cycle tick by tick, still camera, with captions: the lever flips, every
    /// tick after each flip until nothing moves anymore (the state the judge checks), a 3x3 map
    /// of the doorway as the server sees it, and a strip of the whole phase (see tickvideo.rs).
    TickVideo(cmd::video::TickVideoArgs),
    /// One Minecraft world (a save folder for the real game) with every working archived design
    /// in rows by board, smallest first, its size (the volume) above each in red concrete and its
    /// name on a floating label; built by the official server (see world.rs).
    World(cmd::world::WorldArgs),
}

pub fn run(version: &str, cmd: DoorCmd) -> Result<()> {
    match cmd {
        DoorCmd::VideoAll(a) => cmd::video::cmd_video_all(version, a),
        DoorCmd::Card(a) => cmd::video::cmd_card(version, a),
        DoorCmd::Gallery(a) => cmd::gallery::cmd_gallery(version, a),
        DoorCmd::Montage(a) => cmd::gallery::cmd_montage(version, a),
        DoorCmd::Video(a) => cmd::video::cmd_video(version, a),
        DoorCmd::TickVideo(a) => cmd::video::cmd_tick_video(version, a),
        DoorCmd::World(a) => cmd::world::cmd_world(version, a),
    }
}

#[cfg(test)]
mod tests {
    use clap::{CommandFactory, Parser};

    #[derive(Parser)]
    struct Probe {
        #[command(subcommand)]
        cmd: super::DoorCmd,
    }

    /// The video commands' clap definitions hold together (a duplicate flag, a bad default or a
    /// clashing name fails here, not at the first `door video` run; `mctas test`'s unit piece).
    #[test]
    fn the_cli_definition_is_valid() {
        Probe::command().debug_assert();
    }
}

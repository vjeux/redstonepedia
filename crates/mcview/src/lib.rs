//! `mcview`: the contraption viewer's core, shared by the browser app (`crates/mcview-web`,
//! compiled to wasm) and the native tools (`mctas web shot`, `mctas web check`).
//!
//! * [`design`]: the design text format.
//! * [`session`]: a design running on redsim, placed and kicked like its judge.
//! * [`atlas`]: Minecraft-like 16x16 textures drawn procedurally (no game asset is shipped).
//! * [`models`]: block models (cuboids with per-face tiles) for every block redsim knows and the
//!   others the archives use.
//! * [`mesh`]: a scene's triangles (face culling, ambient occlusion, moving blocks, markers).
//! * [`camera`]: orbit camera framed like the real-game pictures.
//! * [`render`]: the wgpu renderer (WebGL2 in the browser, Vulkan headless natively).

pub mod atlas;
pub mod camera;
pub mod circuit;
pub mod design;
pub mod entitymodel;
pub mod gamemodel;
pub mod json;
pub mod labels;
pub mod light;
pub mod mesh;
pub mod mobfarm;
pub mod models;
pub mod occlude;
pub mod remesh;
pub mod particles;
pub mod pick;
pub mod player;
pub mod render;
pub mod replay;
pub mod scene;
pub mod session;
pub mod staged;
pub mod terrain;
pub mod daylight;
#[cfg(not(target_arch = "wasm32"))]
pub mod tools;

pub use design::{Design, Pos};
pub use session::{Mode, Options, RBlock, Session};

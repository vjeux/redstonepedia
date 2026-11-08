use anyhow::{Context, Result, bail};
use mctas::{
    vanilla::{SurvivalProfile, VanillaOptions, VanillaServer},
    video::Display,
};
use serde::Deserialize;
use serde_json::json;
use std::{
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

#[derive(Deserialize)]
struct Scenario {
    name: String,
    setup: Vec<String>,
    steps: Vec<Step>,
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Step {
    Rcon(String),
    Actor(String),
    ActorExpect { command: String, contains: String },
    Graph { mixed: bool, leashed: bool },
    Wait(u64),
    Step(u32),
    Note(String),
    Screenshot(String),
}

struct Actor {
    dir: PathBuf,
    seq: u32,
}
impl Actor {
    fn command(&mut self, text: &str) -> Result<String> {
        self.seq += 1;
        let key = format!("{:06}", self.seq);
        let temp = self.dir.join(format!("cmd-{key}.part"));
        std::fs::write(&temp, text)?;
        std::fs::rename(temp, self.dir.join(format!("cmd-{key}.txt")))?;
        let answer = self.dir.join(format!("done-{key}.txt"));
        let until = Instant::now() + Duration::from_secs(90);
        while Instant::now() < until {
            if let Ok(result) = std::fs::read_to_string(&answer) {
                if !result.starts_with("ok") {
                    bail!("actor `{text}`: {result}");
                }
                return Ok(result.trim().into());
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        bail!("actor timeout `{text}`; see {}", self.dir.display())
    }
}

pub fn run(file: &Path, output: &Path) -> Result<()> {
    let scenario: Scenario = serde_json::from_slice(&std::fs::read(file)?)?;
    if output.exists() {
        bail!("refusing to overwrite evidence {}", output.display());
    }
    std::fs::create_dir_all(output)?;
    let output = output.canonicalize()?;
    std::fs::copy(file, output.join("scenario.json"))?;
    let build_dir = output.join("build");
    std::fs::create_dir_all(&build_dir)?;
    let client_build = mctas::video::build_client("26.3")?;
    let mut loader = crate::build::loader()?;
    loader
        .mods
        .push(crate::build::actor(&client_build, &loader, &build_dir)?);
    let agent = crate::build::observer(&client_build, &loader, &build_dir)?;
    let opts = VanillaOptions {
        keep_dir: true,
        view_distance: 5,
        ..Default::default()
    };
    let profile = SurvivalProfile {
        observer_agent: Some(agent),
        trace_file: output.join("trace.jsonl"),
    };
    let mut server = VanillaServer::start_survival(&opts, &profile)?;
    let props = std::fs::read_to_string(server.dir.join("server.properties"))?;
    if !props.contains("allow-flight=false\n")
        || !props.contains("gamemode=survival\n")
        || server.bot.is_some()
    {
        bail!("survival profile invariant failed");
    }
    std::fs::write(output.join("server.properties"), &props)?;
    let mut events = std::fs::File::create(output.join("interventions.jsonl"))?;
    for command in &scenario.setup {
        let response = server.cmd_ok(command)?;
        event(&mut events, &mut server, "setup", command, &response)?;
    }
    let display = Display::start(960, 540)?;
    let game_dir = output.join("client");
    std::fs::create_dir_all(game_dir.join("mctas-actor"))?;
    let client = mctas::fabric::launch(
        &client_build,
        &loader,
        &display,
        Some(server.game_port()),
        960,
        540,
        &game_dir,
    )?;
    let mut actor = Actor {
        dir: game_dir.join("mctas-actor"),
        seq: 0,
    };
    let answer = actor.command("pos")?;
    if !answer.contains("mode=SURVIVAL") || !answer.contains("creative=false") {
        bail!("not a genuine survival actor: {answer}");
    }
    event(&mut events, &mut server, "actor", "pos", &answer)?;
    std::fs::write(
        output.join("run.json"),
        serde_json::to_string_pretty(&json!({
            "name":scenario.name, "base":"35bcee3c777a01d6102d95ab28ff8c127034a005",
            "actor_source":"4b8f8f96cb111838adbfadcdeed0e9e57e4563a7", "server_dir":server.dir,
            "game_port":server.game_port(), "display":display.name(), "evidence":"command-arranged mechanics fixture, genuine survival client",
        }))?,
    )?;
    let current_head = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()?;
    std::fs::write(
        output.join("provenance.json"),
        serde_json::to_string_pretty(&json!({
            "source_head":String::from_utf8_lossy(&current_head.stdout).trim(),
            "source_working_diff":String::from_utf8_lossy(&std::process::Command::new("git").args(["diff","HEAD","--","crates/boat-lead-vanilla","crates/mctas/src/vanilla/mod.rs","bridge/actor"]).output()?.stdout),
            "server_sha256":mctas::net::sha256_hex(&std::fs::read(mctas::paths::server_jar("26.3"))?),
            "bundle_sha256":mctas::net::sha256_hex(&std::fs::read(mctas::paths::downloads().join("server-bundle-26.3.jar"))?),
            "observer_sha256":mctas::net::sha256_hex(&std::fs::read(&profile.observer_agent.as_ref().unwrap())?),
        }))?,
    )?;
    let recording =
        crate::recording::Recording::start(&display.name(), &output.join("gameplay.mp4"))?;
    let result = scenario.steps.iter().try_for_each(|step| {
        execute(
            step,
            &mut server,
            &mut actor,
            &display,
            &output,
            &mut events,
        )
    });
    recording.finish()?;
    std::fs::copy(&server.log, output.join("server.log"))?;
    std::fs::copy(&client.log, output.join("client.log"))?;
    drop(client);
    drop(display);
    drop(server);
    result?;
    let rows = crate::trace::read(&output.join("trace.jsonl"))?;
    println!(
        "Completed {}: {} phase records, {}",
        scenario.name,
        rows.len(),
        output.display()
    );
    Ok(())
}

fn event(
    file: &mut std::fs::File,
    server: &mut VanillaServer,
    kind: &str,
    command: &str,
    result: &str,
) -> Result<()> {
    writeln!(
        file,
        "{}",
        json!({"wall_unix_ms":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_millis(),"tick":server.game_time()?,"kind":kind,"command":command,"result":result})
    )?;
    file.flush()?;
    println!("{kind}: {command} -> {result}");
    Ok(())
}

fn execute(
    step: &Step,
    server: &mut VanillaServer,
    actor: &mut Actor,
    display: &Display,
    output: &Path,
    log: &mut std::fs::File,
) -> Result<()> {
    let (kind, command, answer) = match step {
        Step::Rcon(command) => ("rcon", command.clone(), server.cmd_ok(command)?),
        Step::Actor(command) => ("actor", command.clone(), actor.command(command)?),
        Step::ActorExpect { command, contains } => {
            let response = actor.command(command)?;
            if !response.contains(contains) {
                bail!("actor `{command}` expected `{contains}`, got `{response}`");
            }
            ("actor_assert", command.clone(), response)
        }
        Step::Graph { mixed, leashed } => (
            "graph_assert",
            format!("mixed={mixed} leashed={leashed}"),
            crate::graph::check(server, *mixed, *leashed)?,
        ),
        Step::Step(n) => {
            server.step(*n)?;
            ("step", n.to_string(), "ok".into())
        }
        Step::Wait(ms) => {
            std::thread::sleep(Duration::from_millis(*ms));
            ("wait_ms", ms.to_string(), "ok".into())
        }
        Step::Note(text) => ("note", text.clone(), "".into()),
        Step::Screenshot(name) => {
            let out = std::process::Command::new(mctas::video::ffmpeg()?)
                .args([
                    "-y",
                    "-loglevel",
                    "error",
                    "-f",
                    "x11grab",
                    "-video_size",
                    "960x540",
                    "-i",
                    &display.name(),
                    "-frames:v",
                    "1",
                ])
                .arg(output.join(name))
                .output()
                .context("screenshot")?;
            if !out.status.success() {
                bail!("screenshot: {}", String::from_utf8_lossy(&out.stderr));
            }
            ("screenshot", name.clone(), "ok".into())
        }
    };
    event(log, server, kind, &command, &answer)
}

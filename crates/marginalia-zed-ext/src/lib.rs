use zed_extension_api::{
    self as zed, Command, ContextServerId, Os, Project, Result, current_platform,
    settings::ContextServerSettings,
};

const SERVER_BINARY: &str = "marginalia-mcp";
const SHELL: &str = "/bin/sh";

struct MarginaliaExtension;

impl zed::Extension for MarginaliaExtension {
    fn new() -> Self {
        Self
    }

    fn context_server_command(
        &mut self,
        id: &ContextServerId,
        project: &Project,
    ) -> Result<Command> {
        let settings = ContextServerSettings::for_project(id.as_ref(), project);
        if let Some(command) = settings.as_ref().ok().and_then(pinned_command) {
            return Ok(command);
        }
        discovered_command(id).map_err(|err| match settings {
            Ok(_) => err,
            Err(settings_err) => format!("{err} Reading its settings also failed: {settings_err}"),
        })
    }
}

/// The binary the user pinned under `context_servers.<id>.command`, if they pinned one.
fn pinned_command(settings: &ContextServerSettings) -> Option<Command> {
    let pinned = settings.command.as_ref()?;
    Some(Command {
        command: pinned.path.clone()?,
        args: pinned.arguments.clone().unwrap_or_default(),
        env: pinned
            .env
            .iter()
            .flatten()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect(),
    })
}

/// Resolves the binary and the shell environment it needs. The wasm guest sees neither
/// the user's `PATH` nor their environment, so both come from a login shell.
fn discovered_command(id: &ContextServerId) -> Result<Command> {
    let (os, _) = current_platform();
    if matches!(os, Os::Windows) {
        return Err(unresolved(
            id,
            format!("cannot look `{SERVER_BINARY}` up automatically on Windows"),
        ));
    }

    let lookup = format!("command -v {SERVER_BINARY}");
    let path = match login_shell_output(&lookup) {
        Ok(path) if !path.is_empty() => path,
        Ok(_) => return Err(unresolved(id, format!("`{lookup}` found nothing"))),
        Err(err) => return Err(unresolved(id, err)),
    };
    let env = login_shell_output("env").map_err(|err| unresolved(id, err))?;

    Ok(Command {
        command: path,
        args: Vec::new(),
        env: parse_env(&env),
    })
}

/// Runs `script` through a login shell and returns its trimmed stdout.
fn login_shell_output(script: &str) -> Result<String> {
    let output = Command::new(SHELL).arg("-lc").arg(script).output()?;
    if output.status != Some(0) {
        return Err(format!(
            "`{SHELL} -lc '{script}'` exited with {:?}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Parses `env` output into pairs. A multi-line value continues onto lines that carry no
/// `=`; those lines are dropped rather than turned into bogus variables.
fn parse_env(output: &str) -> Vec<(String, String)> {
    output
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

fn unresolved(id: &ContextServerId, reason: String) -> String {
    format!(
        "Could not resolve the {SERVER_BINARY} binary: {reason}. \
         Set `context_servers.{}.command.path` in your Zed settings to its absolute path.",
        id.as_ref()
    )
}

zed::register_extension!(MarginaliaExtension);

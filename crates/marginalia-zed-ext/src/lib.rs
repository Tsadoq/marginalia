use zed_extension_api::{
    self as zed, Command, ContextServerId, Os, Project, Result, current_platform,
    settings::ContextServerSettings,
};

const SERVER_BINARY: &str = "marginalia-mcp";
const SHELL: &str = "/bin/sh";

/// What the server is given, in the order the shell prints them. It reads one file under
/// the working directory Zed hands it and spawns nothing, so this is already generous.
const PASSED_ENV_VARS: &[&str] = &["PATH", "HOME"];

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

/// Resolves the binary from a login shell, because the wasm guest sees none of the
/// user's `PATH`.
///
/// Zed puts a context server's environment into the agent's command line, where any
/// local process can read it out of `/proc`, so the shell is asked only for the two
/// variables the server could plausibly want. Capturing the whole environment and
/// filtering afterwards would still have carried every secret in it through the guest
/// and Zed's logs on the way.
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
    let env = login_shell_output(r#"printf '%s\n%s\n' "$PATH" "$HOME""#)
        .map(|probed| named_pairs(PASSED_ENV_VARS, &probed))
        .map_err(|err| unresolved(id, err))?;

    Ok(Command {
        command: path,
        args: Vec::new(),
        env,
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

/// Zips `names` with the lines the shell printed for them, dropping any that came back
/// empty so an unset variable is absent rather than set to nothing.
fn named_pairs(names: &[&str], probed: &str) -> Vec<(String, String)> {
    names
        .iter()
        .zip(probed.lines())
        .filter(|(_, value)| !value.is_empty())
        .map(|(name, value)| ((*name).to_owned(), value.to_owned()))
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

use std::path::{Path, PathBuf};

use contract::{Actor, Context, Result, ScriptError};
use package::{DependencySpec, InstallPackage, PackageBin, PackageJson, VersionRequirement};
use process::{Process, ScriptPackage, script_path};
use progress::{OutputMode, Reporter};
use resolver::short_hash;
use serde_json::json;

use crate::{InstallActor, InstallOptions};

#[derive(Debug)]
pub struct DlxPayload {
    /// The package to install, when it differs from the command (`-p typescript tsc`).
    pub package: Option<String>,
    pub command: String,
    pub args: Vec<String>,
}

/// Runs a package's bin without adding it to the project: the package is installed into a
/// cached directory under the craft home and its bin runs from the current directory.
pub struct DlxActor {
    context: Context,
    payload: DlxPayload,
}

impl Actor<DlxPayload> for DlxActor {
    fn with(context: Context, payload: DlxPayload) -> Self {
        Self { context, payload }
    }

    async fn run(&self) -> Result<()> {
        let literal = self
            .payload
            .package
            .as_deref()
            .unwrap_or(&self.payload.command);
        let package = InstallPackage::from_literal(literal, false);
        let spec = package
            .version
            .clone()
            .unwrap_or_else(|| "latest".to_string());
        let dir = self.prepare_dir(literal, &package.name, &spec).await?;

        let install_context = Context {
            cwd: dir.clone(),
            reporter: Reporter::new(OutputMode::Silent),
            ..self.context.clone()
        };
        InstallActor::with(install_context, InstallOptions::default())
            .run()
            .await?;

        let bin_name = match &self.payload.package {
            Some(_) => self.payload.command.clone(),
            None => default_bin(&dir, &package.name).await?,
        };
        let bin = fsutil::bin_command(&dir.join("node_modules").join(".bin"), &bin_name);
        if !bin.is_file() {
            return Err(ScriptError::BinNotFound { name: bin_name }.into());
        }
        self.run_bin(&bin, &bin_name, &dir).await
    }
}

impl DlxActor {
    /// One directory per requested package and spec, reused across runs. A tag such as
    /// `latest` is resolved again each time, so the newest version runs.
    async fn prepare_dir(&self, literal: &str, name: &str, spec: &str) -> Result<PathBuf> {
        let dir = self
            .context
            .craft_home
            .join("dlx")
            .join(short_hash(literal));
        tokio::fs::create_dir_all(&dir).await?;
        let manifest = json!({
            "name": "craft-dlx",
            "private": true,
            "dependencies": { name: spec },
        });
        let mut content = serde_json::to_string_pretty(&manifest)?;
        content.push('\n');
        fsutil::write_if_changed(&dir.join("package.json"), content.as_bytes()).await?;

        let is_tag = DependencySpec::parse(name, Some(spec)).is_ok_and(|parsed| {
            matches!(
                parsed.registry_target(),
                Some((_, VersionRequirement::Tag(_)))
            )
        });
        if is_tag {
            fsutil::remove_entry(&dir.join("craft.lock")).await?;
        }
        Ok(dir)
    }

    async fn run_bin(&self, bin: &Path, name: &str, dir: &Path) -> Result<()> {
        let manifest_path = dir.join("package.json");
        let package = ScriptPackage {
            name: None,
            version: None,
            manifest_path: &manifest_path,
        };
        let dlx_bins = dir.join("node_modules").join(".bin");
        let inherited = script_path(&self.context.cwd)?;
        let path = std::env::join_paths(
            std::iter::once(dlx_bins).chain(std::env::split_paths(&inherited)),
        )?;

        let code = Process::new(bin)
            .args(&self.payload.args)
            .script_env(&self.context.cwd, &self.context.cwd, &package)?
            .env("PATH", path)
            .run()
            .await?;
        if code != 0 {
            return Err(ScriptError::Failed {
                name: name.to_string(),
                code,
            }
            .into());
        }
        Ok(())
    }
}

/// The bin `craft dlx <pkg>` runs: the package's only bin, or the one named after it.
async fn default_bin(dir: &Path, name: &str) -> Result<String> {
    let manifest_path = dir.join("node_modules").join(name).join("package.json");
    let manifest = PackageJson::from_file(&manifest_path).await?;
    let unscoped = name.rsplit('/').next().unwrap_or(name).to_string();
    let available: Vec<String> = match &manifest.bin {
        Some(PackageBin::String(_)) => vec![unscoped.clone()],
        Some(PackageBin::Map(bins)) => {
            let mut names: Vec<String> = bins.keys().cloned().collect();
            names.sort();
            names
        }
        None => Vec::new(),
    };

    match available.as_slice() {
        [only] => Ok(only.clone()),
        _ if available.contains(&unscoped) => Ok(unscoped),
        _ => Err(ScriptError::NoSuchBin {
            package: name.to_string(),
            name: unscoped,
            available,
        }
        .into()),
    }
}

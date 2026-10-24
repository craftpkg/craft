use contract::{Context, Pipeline, Result};
use package::PackageJson;
use pipeline::{LifecyclePipe, ScriptPolicy, run_project_scripts};
use resolver::ResolutionGraph;

use crate::summary::packages;

/// Dependencies may run install scripts only when package.json lists them in
/// `trustedDependencies`; `--ignore-scripts` turns all scripts off.
pub(crate) fn script_policy(package_json: &PackageJson, ignore_scripts: bool) -> ScriptPolicy {
    ScriptPolicy {
        ignore_scripts,
        trusted: package_json.trusted_dependencies().into_iter().collect(),
    }
}

/// Runs trusted dependencies' install scripts, reports the ones skipped, then (for a full
/// install) the project's own lifecycle scripts.
pub(crate) async fn run_install_scripts(
    context: &Context,
    graph: ResolutionGraph,
    package_json: &PackageJson,
    policy: &ScriptPolicy,
    include_project: bool,
) -> Result<()> {
    let report = LifecyclePipe::new(context, graph, policy.clone())
        .run()
        .await?;
    if !report.skipped.is_empty() {
        context.reporter.warn(format!(
            "skipped install scripts of {} not in trustedDependencies: {}\n  to allow them, add their names to \"trustedDependencies\" in package.json and run `craft install`",
            packages(report.skipped.len()),
            report.skipped.join(", ")
        ));
    }

    let runs_project_scripts = include_project && !policy.ignore_scripts;
    if runs_project_scripts {
        run_project_scripts(context, package_json).await?;
    }
    Ok(())
}

use std::{collections::BTreeMap, path::Path};
use cargo_toml::{
    Badges, Dependency, DependencyDetail, DepsSet, Edition, FeatureSet,
    Manifest, Package, PatchSet, Product, Profiles, Publish, Resolver,
    TargetDepsSet, Workspace,
};
use legion::{Query, systems::CommandBuffer, world::SubWorld};
use crate::{BuildContext, FeatureFlags, codegen::File, lowering::ProcBlock, parse};

const REPO: &'static str = "https://github.com/hotg-ai/rune";
/// The version of core crates that we want to target.
const CORE_VERSION: &'static str = hotg_rune_core::VERSION;
const PROC_BLOCK_VERSION: &'static str = hotg_rune_proc_blocks::VERSION;

/// Generate a `Cargo.toml` file which includes all the relevant dependencies
/// for this crate.
#[legion::system]
pub(crate) fn run(
    world: &SubWorld,
    cmd: &mut CommandBuffer,
    #[resource] ctx: &BuildContext,
    #[resource] features: &FeatureFlags,
    query: &mut Query<&ProcBlock>,
) {
    if CORE_VERSION.contains("-dev") && features.rune_repo_dir.is_none() {
        let msg = "
            It looks like you are using a development version of \"rune\", but
            haven't specified a \"rune_repo_dir\". Internal crates are resolved
            using the \"$CORE_VERSION\" version from crates.io and builtin
            proc-blocks are found using the \"v$CORE_VERSION\" tag from the Rune
            repo, so there is a good chance you'll get compile errors about
            unresolved dependencies. Specify the \"rune_repo_dir\" to resolve
            this.
        ";
        log::warn!(
            "{}",
            msg.replace("\n", " ")
                .replace("$CORE_VERSION", CORE_VERSION)
        );
    }

    let proc_blocks = query.iter(world);
    let mut manifest =
        generate_manifest(proc_blocks, &ctx.name, &ctx.current_directory);

    if let Some(hotg_repo_dir) = features.rune_repo_dir.as_deref() {
        patch_hotg_dependencies(hotg_repo_dir, &mut manifest);
    }

    let manifest = toml::to_string_pretty(&manifest)
        .expect("Serializing to a string should never fail");
    let file = File::new("Cargo.toml", manifest.into_bytes());
    cmd.push((file,));
}

// Generate the `Cargo.toml` manifest.
fn generate_manifest<'rune, I>(
    proc_blocks: I,
    name: &str,
    current_dir: &Path,
) -> Manifest
where
    I: IntoIterator<Item = &'rune ProcBlock> + 'rune,
{
    let product = Product {
        path: Some("lib.rs".to_string()),
        edition: Some(Edition::E2018),
        crate_type: Some(vec!["cdylib".to_string()]),
        ..Default::default()
    };

    Manifest {
        package: Some(package(name)),
        lib: Some(product),
        dependencies: dependencies(proc_blocks, current_dir),
        workspace: Some(Workspace {
            members: vec![String::from(".")],
            default_members: vec![String::from(".")],
            exclude: Vec::new(),
            metadata: None,
        }),
        ..empty_manifest()
    }
}

fn package(name: &str) -> Package {
    Package {
        name: name.into(),
        edition: Edition::E2018,
        version: String::from("0.0.0"),
        publish: Publish::Flag(false),
        resolver: Some(Resolver::V2),
        ..empty_package()
    }
}

fn dependencies<'rune, I>(proc_blocks: I, current_dir: &Path) -> DepsSet
where
    I: IntoIterator<Item = &'rune ProcBlock> + 'rune,
{
    let mut deps = DepsSet::new();

    // We always need the log crate
    let log = Dependency::Detailed(DependencyDetail {
        version: Some(String::from("0.4")),
        features: vec![
            String::from("max_level_debug"),
            String::from("release_max_level_info"),
        ],
        ..empty_dependency_detail()
    });
    deps.insert(String::from("log"), log);

    // we also need lazy_static
    let lazy_static = Dependency::Detailed(DependencyDetail {
        version: Some(String::from("1.0")),
        features: vec![String::from("spin_no_std")],
        ..empty_dependency_detail()
    });
    deps.insert(String::from("lazy_static"), lazy_static);

    // We'll always use the following HOTG dependencies.
    deps.insert(
        "hotg-rune-core".to_string(),
        Dependency::Simple(hotg_rune_core::VERSION.to_string()),
    );
    deps.insert(
        "hotg-rune-proc-blocks".to_string(),
        Dependency::Simple(hotg_rune_proc_blocks::VERSION.to_string()),
    );
    // FIXME: We should probably use the actual version number instead of
    // assuming it'll be in sync with core.
    deps.insert(
        "hotg-runicos-base-wasm".to_string(),
        Dependency::Simple(hotg_rune_core::VERSION.to_string()),
    );

    for proc_block in proc_blocks {
        let dep = proc_block_dependency(&proc_block.path, current_dir);
        let name = proc_block.name();
        deps.insert(name.to_string(), Dependency::Detailed(dep));
    }

    deps
}

fn proc_block_dependency(
    path: &parse::Path,
    current_dir: &Path,
) -> DependencyDetail {
    if is_builtin(path) {
        let tag = format!("v{}", PROC_BLOCK_VERSION);
        return git_tagged_dependency(REPO, &tag);
    } else if path.base.starts_with('.') {
        return local_proc_block(path, current_dir);
    }

    if path.sub_path.is_none() && !path.base.contains('/') {
        if let Some(version) = &path.version {
            // it's from crates.io
            return DependencyDetail {
                version: Some(version.clone()),
                ..empty_dependency_detail()
            };
        }
    }

    // fall back to using git
    let repo = format!("https://github.com/{}.git", path.base);

    DependencyDetail {
        git: Some(repo),
        ..empty_dependency_detail()
    }
}

fn local_proc_block(
    path: &parse::Path,
    current_dir: &Path,
) -> DependencyDetail {
    DependencyDetail {
        path: Some(current_dir.join(&path.base).display().to_string()),
        ..empty_dependency_detail()
    }
}

fn is_builtin(path: &parse::Path) -> bool { path.base == "hotg-ai/rune" }

fn git_tagged_dependency(repo: &str, tag: &str) -> DependencyDetail {
    DependencyDetail {
        git: Some(repo.into()),
        tag: Some(tag.into()),
        ..empty_dependency_detail()
    }
}

fn empty_manifest() -> Manifest {
    Manifest {
        package: None,
        dependencies: DepsSet::default(),
        lib: None,
        workspace: None,
        dev_dependencies: DepsSet::default(),
        build_dependencies: DepsSet::default(),
        target: TargetDepsSet::default(),
        features: FeatureSet::default(),
        patch: PatchSet::default(),
        profile: Profiles::default(),
        badges: Badges::default(),
        bin: Vec::default(),
        bench: Vec::default(),
        test: Vec::default(),
        example: Vec::default(),
    }
}

fn empty_package() -> Package {
    Package {
        name: String::default(),
        edition: Edition::default(),
        version: String::default(),
        build: None,
        workspace: None,
        authors: Default::default(),
        links: None,
        description: None,
        homepage: None,
        documentation: None,
        readme: None,
        keywords: Vec::new(),
        categories: Vec::new(),
        license: None,
        license_file: None,
        repository: None,
        metadata: None,
        default_run: None,
        autobins: false,
        autoexamples: false,
        autotests: false,
        autobenches: false,
        publish: Publish::default(),
        resolver: None,
    }
}

fn empty_dependency_detail() -> DependencyDetail {
    DependencyDetail {
        version: None,
        registry: None,
        registry_index: None,
        path: None,
        git: None,
        branch: None,
        tag: None,
        rev: None,
        features: Vec::new(),
        optional: false,
        default_features: None,
        package: None,
    }
}

fn path_dependency(path: impl AsRef<Path>) -> Dependency {
    Dependency::Detailed(DependencyDetail {
        path: Some(path.as_ref().to_string_lossy().into()),
        ..empty_dependency_detail()
    })
}

fn patch_hotg_dependencies(hotg_repo_dir: &Path, manifest: &mut Manifest) {
    let known_paths = &[
        ("hotg-rune-core", "crates/rune-core"),
        ("hotg-rune-proc-blocks", "proc-blocks/proc-blocks"),
        ("hotg-runicos-base-wasm", "images/runicos-base/wasm"),
    ];
    let mut overrides = BTreeMap::new();

    let proc_blocks_dir = hotg_repo_dir.join("proc-blocks");

    for (name, dep) in &manifest.dependencies {
        let uses_hotg_github =
            dep.git().map(|repo| repo == REPO).unwrap_or(false);

        if !name.starts_with("hotg-") && !uses_hotg_github {
            continue;
        }

        // We're pretty sure this is a hotg crate, now we need to figure out
        // which local crate to redirect to. First we'll check a list of known
        // crates, otherwise we'll assume it is a proc block.

        let path = known_paths
            .iter()
            .find_map(|(n, p)| {
                if name == *n {
                    Some(hotg_repo_dir.join(p))
                } else {
                    None
                }
            })
            .unwrap_or_else(|| proc_blocks_dir.join(name));

        overrides.insert(name.clone(), path_dependency(path));
    }

    overrides.insert(
        "hotg-rune-core".to_string(),
        path_dependency(hotg_repo_dir.join("crates").join("rune-core")),
    );
    overrides.insert(
        "hotg-rune-proc-blocks".to_string(),
        path_dependency(hotg_repo_dir.join("proc-blocks").join("proc-blocks")),
    );

    manifest
        .patch
        .entry("crates-io".to_string())
        .or_default()
        .extend(overrides.clone());
    manifest
        .patch
        .entry("https://github.com/hotg-ai/rune".to_string())
        .or_default()
        .extend(overrides.clone());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_dependencies() {
        let got = dependencies(Vec::new(), Path::new("."));

        assert_eq!(got.len(), 5);
        assert!(got.contains_key("log"));
        assert!(got.contains_key("lazy_static"));
        assert!(got.contains_key("hotg-rune-core"));
        assert!(got.contains_key("hotg-rune-proc-blocks"));
        assert!(got.contains_key("hotg-runicos-base-wasm"));

        assert_eq!(
            got["hotg-rune-core"].clone(),
            Dependency::Simple(CORE_VERSION.to_string())
        );
        assert_eq!(
            got["hotg-rune-proc-blocks"].clone(),
            Dependency::Simple(PROC_BLOCK_VERSION.to_string())
        );
        assert_eq!(
            got["hotg-runicos-base-wasm"].clone(),
            Dependency::Simple(CORE_VERSION.to_string())
        );
    }

    #[test]
    fn builtin_proc_blocks_always_use_nightly_tag() {
        let path = "hotg-ai/rune#proc_blocks/modulo".parse().unwrap();
        let should_be = DependencyDetail {
            git: Some(REPO.to_string()),
            tag: Some(format!("v{}", CORE_VERSION)),
            ..empty_dependency_detail()
        };

        let got = proc_block_dependency(&path, Path::new("."));

        assert_eq!(got, should_be);
    }

    #[test]
    fn external_proc_block() {
        let path = "whatever@1.2".parse().unwrap();
        let should_be = DependencyDetail {
            version: Some("1.2".to_string()),
            ..empty_dependency_detail()
        };

        let got = proc_block_dependency(&path, Path::new("."));

        assert_eq!(got, should_be);
    }

    #[test]
    fn manifest_generates_cdylib() {
        let got = generate_manifest(Vec::new(), "foo", Path::new("."));

        let crate_type = got.lib.unwrap().crate_type.unwrap();
        assert!(crate_type.contains(&String::from("cdylib")));
    }

    #[test]
    fn manifest_is_in_its_own_workspace() {
        let got = generate_manifest(Vec::new(), "foo", Path::new("."));

        assert!(got.workspace.is_some());
    }
}

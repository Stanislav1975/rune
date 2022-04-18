use std::sync::Arc;

use im::Vector;
use legion::{systems::Runnable, IntoQuery, Resources, World};

use crate::{
    codegen::{inputs::CodegenInputs, Codegen},
    compile::{CompilationResult, Compile},
    hooks::{Continuation, Ctx, Hooks},
    inputs::Inputs,
    lowering::{self, Name},
    parse::parse,
    type_check, BuildContext, Diagnostics, FeatureFlags,
};

/// Execute the `rune build` process.
pub fn build(ctx: BuildContext) -> (World, Resources) {
    struct NopHooks;
    impl Hooks for NopHooks {}

    build_with_hooks(ctx, FeatureFlags::production(), &mut NopHooks)
}

/// Execute the `rune build` process, passing in custom [`Hooks`] which will
/// be fired after each phase.
pub fn build_with_hooks(
    ctx: BuildContext,
    features: FeatureFlags,
    hooks: &mut dyn Hooks,
) -> (World, Resources) {
    let mut db = Database::default();
    db.set_build_context(Arc::new(ctx.clone()));
    db.set_feature_flags(features.clone());

    let mut world = World::default();
    let mut res = Resources::default();

    res.insert(ctx);
    res.insert(features);
    res.insert(Diagnostics::default());

    if hooks.before_parse(&mut c(&mut world, &mut res))
        != Continuation::Continue
    {
        return (world, res);
    }

    log::debug!("Beginning the \"parse\" phase");
    match parse(db.build_context().runefile.as_str()) {
        Ok(d) => {
            res.insert(d.clone().to_v1());
        },
        Err(e) => {
            res.get_mut_or_default::<Diagnostics>()
                .push(e.as_codespan_diagnostic());
        },
    }

    if hooks.after_parse(&mut c(&mut world, &mut res)) != Continuation::Continue
    {
        return (world, res);
    }

    log::debug!("Beginning the \"lowering\" phase");
    lowering::phase().run(&mut world, &mut res);

    if hooks.after_lowering(&mut c(&mut world, &mut res))
        != Continuation::Continue
    {
        return (world, res);
    }

    log::debug!("Beginning the \"type_check\" phase");
    type_check::phase().run(&mut world, &mut res);

    if hooks.after_type_checking(&mut c(&mut world, &mut res))
        != Continuation::Continue
    {
        return (world, res);
    }

    log::debug!("Beginning the \"codegen\" phase");

    update_db_before_codegen(&world, &mut db);

    let _files = db.files();

    if hooks.after_codegen(&mut c(&mut world, &mut res))
        != Continuation::Continue
    {
        return (world, res);
    }

    let result = db.build();
    res.insert(CompilationResult(result));

    if hooks.after_compile(&mut c(&mut world, &mut res))
        != Continuation::Continue
    {
        return (world, res);
    }

    (world, res)
}

fn update_db_before_codegen(world: &World, db: &mut Database) {
    let mut pb_names = Vector::new();
    <(
        &Name,
        &crate::lowering::ProcBlock,
        &crate::lowering::Inputs,
        &crate::lowering::Outputs,
    )>::query()
    .for_each(world, |(n, p, i, o)| {
        pb_names.push_back(n.clone());
        db.set_node_inputs(n.clone(), i.clone());
        db.set_node_outputs(n.clone(), o.clone());
        db.set_proc_block_info(n.clone(), p.clone());
    });
    db.set_proc_block_names(pb_names);

    let mut model_names = Vector::new();
    <(
        &Name,
        &crate::lowering::Model,
        &crate::lowering::ModelData,
        &crate::lowering::Inputs,
        &crate::lowering::Outputs,
    )>::query()
    .for_each(world, |(n, m, d, i, o)| {
        model_names.push_back(n.clone());
        db.set_node_inputs(n.clone(), i.clone());
        db.set_node_outputs(n.clone(), o.clone());
        db.set_model_info(n.clone(), m.clone());
        db.set_model_data(n.clone(), d.clone());
    });
    db.set_model_names(model_names);
}

#[derive(Default)]
#[salsa::database(
    crate::codegen::inputs::CodegenInputsGroup,
    crate::codegen::CodegenGroup,
    crate::compile::CompileGroup,
    crate::inputs::InputsGroup
)]
struct Database {
    storage: salsa::Storage<Self>,
}

impl salsa::Database for Database {}
impl crate::inputs::FileSystem for Database {}

/// A group of operations which make up a single "phase" in the build process.
pub struct Phase(legion::systems::Builder);

impl Phase {
    pub(crate) fn new() -> Self { Phase(legion::Schedule::builder()) }

    pub(crate) fn with_setup(
        mut setup: impl FnMut(&mut Resources) + 'static,
    ) -> Self {
        let mut phase = Phase::new();
        phase.0.add_thread_local_fn(move |_, res| setup(res));

        phase
    }

    pub(crate) fn and_then<F, R>(mut self, run_system: F) -> Self
    where
        R: legion::systems::ParallelRunnable + 'static,
        F: FnOnce() -> R,
    {
        self.0
            .add_system(TracingRunnable {
                runnable: run_system(),
                name: std::any::type_name::<F>(),
            })
            .flush();

        self
    }

    /// Execute the phase, updating the [`World`].
    pub fn run(&mut self, world: &mut World, resources: &mut Resources) {
        self.0.build().execute(world, resources);
    }
}

/// A wrapper around some [`Runnable`] which logs whenever it starts.
struct TracingRunnable<R> {
    runnable: R,
    name: &'static str,
}

impl<R: Runnable> Runnable for TracingRunnable<R> {
    fn name(&self) -> Option<&legion::systems::SystemId> {
        self.runnable.name()
    }

    fn reads(
        &self,
    ) -> (
        &[legion::systems::ResourceTypeId],
        &[legion::storage::ComponentTypeId],
    ) {
        self.runnable.reads()
    }

    fn writes(
        &self,
    ) -> (
        &[legion::systems::ResourceTypeId],
        &[legion::storage::ComponentTypeId],
    ) {
        self.runnable.writes()
    }

    fn prepare(&mut self, world: &World) { self.runnable.prepare(world); }

    fn accesses_archetypes(&self) -> &legion::world::ArchetypeAccess {
        self.runnable.accesses_archetypes()
    }

    unsafe fn run_unsafe(
        &mut self,
        world: &World,
        resources: &legion::systems::UnsafeResources,
    ) {
        let pretty_name = self
            .name
            .trim_start_matches(env!("CARGO_CRATE_NAME"))
            .trim_end_matches("_system")
            .trim_end_matches("::run")
            .trim_matches(':');
        log::debug!("Starting the \"{}\" pass", pretty_name);

        self.runnable.run_unsafe(world, resources);
    }

    fn command_buffer_mut(
        &mut self,
        world: legion::world::WorldId,
    ) -> Option<&mut legion::systems::CommandBuffer> {
        self.runnable.command_buffer_mut(world)
    }
}

fn c<'world, 'res>(
    world: &'world mut World,
    res: &'res mut Resources,
) -> Ctx<'world, 'res> {
    Ctx { world, res }
}

#[cfg(test)]
#[cfg(never)]
mod tests {
    use indexmap::IndexMap;

    use super::*;

    #[test]
    fn detect_pipeline_cycle() {
        let src = r#"
image: runicos/base
version: 1

pipeline:
  audio:
    proc-block: "hotg-ai/rune#proc_blocks/fft"
    inputs:
    - model
    outputs:
    - type: i16
      dimensions: [16000]

  fft:
    proc-block: "hotg-ai/rune#proc_blocks/fft"
    inputs:
    - audio
    outputs:
    - type: i8
      dimensions: [1960]

  model:
    model: "./model.tflite"
    inputs:
    - fft
    outputs:
    - type: i8
      dimensions: [6]
            "#;
        let doc = Document::parse(src).unwrap();
        let mut diags = Diagnostics::new();

        let _ = crate::analyse(doc, &mut diags);

        assert!(diags.has_errors());
        let errors: Vec<_> = diags
            .iter_severity(codespan_reporting::diagnostic::Severity::Error)
            .collect();
        assert_eq!(errors.len(), 1);
        let diag = errors[0];
        assert_eq!(diag.message, "Cycle detected when checking \"audio\"");
        assert!(diag.notes[0].contains("model"));
        assert!(diag.notes[1].contains("fft"));
        assert_eq!(
            diag.notes[2],
            "... which receives input from \"audio\", completing the cycle."
        );
    }
}

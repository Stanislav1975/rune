use super::{
    ALLOCATOR,
    alloc::{Region, DebugAllocator, StatsAllocator},
    Logger,
};
use log::LevelFilter;
use alloc::alloc::GlobalAlloc;
use wee_alloc::WeeAlloc;

/// A guard type which should be alive for the duration of the setup process,
/// letting `runic-types` run code at the start and end.
#[derive(Debug)]
pub struct SetupGuard<'a, T: GlobalAlloc> {
    region: Region<'a, T>,
}

impl<'a, T: GlobalAlloc> SetupGuard<'a, T> {
    pub fn new(stats: &'a StatsAllocator<T>) -> Self {
        static LOGGER: Logger = Logger::new();

        log::set_max_level(LevelFilter::Debug);
        log::set_logger(&LOGGER).unwrap();

        SetupGuard {
            region: Region::new(stats),
        }
    }
}

impl Default for SetupGuard<'static, DebugAllocator<WeeAlloc<'static>>> {
    fn default() -> Self { SetupGuard::new(&ALLOCATOR) }
}

impl<'a, T: GlobalAlloc> Drop for SetupGuard<'a, T> {
    fn drop(&mut self) {
        let stats = self.region.change_and_reset();
        log::debug!("Allocations during startup: {:?}", stats);
    }
}

/// A guard type which should be alive for the duration of a single pipeline
/// run, letting `runic-types` run code as necessary.
#[derive(Debug)]
pub struct PipelineGuard<'a, T: GlobalAlloc> {
    region: Region<'a, T>,
}

impl<'a, T: GlobalAlloc> PipelineGuard<'a, T> {
    pub fn new(stats: &'a StatsAllocator<T>) -> Self {
        PipelineGuard {
            region: Region::new(stats),
        }
    }
}

impl Default for PipelineGuard<'static, DebugAllocator<WeeAlloc<'static>>> {
    fn default() -> Self { PipelineGuard::new(&ALLOCATOR) }
}

impl<'a, T: GlobalAlloc> Drop for PipelineGuard<'a, T> {
    fn drop(&mut self) {
        let stats = self.region.change_and_reset();
        log::debug!("Allocations during pipeline run: {:?}", stats);
    }
}

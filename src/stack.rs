//! 固定的编译工作栈。深度上限由解析器负责，这里只保证上限以内的
//! debug 构建也不依赖 Windows 主线程较小的默认栈。
//!
//! Project loaders pass their entire parse loop to [`run`] to share one worker.

const COMPILER_STACK_SIZE: usize = 16 * 1024 * 1024;

pub(crate) fn run<T: Send>(work: impl FnOnce() -> T + Send) -> std::io::Result<T> {
    std::thread::scope(|scope| {
        let worker = std::thread::Builder::new()
            .name("mclang-compiler".into())
            .stack_size(COMPILER_STACK_SIZE)
            .spawn_scoped(scope, work)?;
        match worker.join() {
            Ok(result) => Ok(result),
            Err(panic) => std::panic::resume_unwind(panic),
        }
    })
}

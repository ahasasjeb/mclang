use mclang::{BuildOptions, build_file};
use std::fs::{self, FileTimes, OpenOptions};
use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

#[test]
fn identical_generated_files_are_not_rewritten() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = repo.join("tests/valid/optimizations");
    let output = repo.join("target/incremental-output-test");
    let options = BuildOptions {
        description: "增量写入回归".to_owned(),
        deny_raw: true,
    };
    build_file(&source, &output, &options).unwrap();

    let generated = output.join("data/optimizations/function/nested.mcfunction");
    let expected = fs::read(&generated).unwrap();
    let timestamp = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    OpenOptions::new()
        .write(true)
        .open(&generated)
        .unwrap()
        .set_times(FileTimes::new().set_modified(timestamp))
        .unwrap();
    let modified = fs::metadata(&generated).unwrap().modified().unwrap();

    build_file(&source, &output, &options).unwrap();
    assert_eq!(
        fs::metadata(&generated).unwrap().modified().unwrap(),
        modified,
        "内容没变时应保留文件写入时间"
    );

    fs::write(&generated, b"stale manual edit\n").unwrap();
    build_file(&source, &output, &options).unwrap();
    assert_eq!(fs::read(&generated).unwrap(), expected);
}

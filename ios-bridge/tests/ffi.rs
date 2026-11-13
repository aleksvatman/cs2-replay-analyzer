//! Exercises the C interface on the host (not cross-compiled): the FFI plumbing (path
//! marshaling, progress callback, error strings) is target-independent, so testing it here is
//! equivalent to testing it on-device and much faster to run. The cross-compile itself (the
//! actual iOS-specific risk) is checked separately by building this crate for
//! `aarch64-apple-ios`/`aarch64-apple-ios-sim` (see `build-xcframework.sh`).
//!
//! Like `parser-core`'s own tests, the real-demo cases are skipped without
//! `samples/test_demo.dem` (demos are not committed to git).

use std::ffi::{c_void, CStr, CString};
use std::io::Read;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use flate2::read::GzDecoder;
use ios_bridge::{cs2_last_error, cs2_parse_file, cs2_string_free};

fn sample_demo_path() -> Option<PathBuf> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("samples")
        .join("test_demo.dem");
    path.exists().then_some(path)
}

fn take_last_error() -> Option<String> {
    unsafe {
        let ptr = cs2_last_error();
        if ptr.is_null() {
            return None;
        }
        let message = CStr::from_ptr(ptr).to_string_lossy().into_owned();
        cs2_string_free(ptr);
        Some(message)
    }
}

extern "C" fn count_progress_calls(_stage: u32, _percent: u32, user_data: *mut c_void) {
    let counter = unsafe { &*(user_data as *const AtomicU32) };
    counter.fetch_add(1, Ordering::SeqCst);
}

#[test]
fn parses_the_sample_demo_and_reports_progress() {
    let Some(demo_path) = sample_demo_path() else {
        eprintln!("skipping: samples/test_demo.dem not present");
        return;
    };
    let out_dir = std::env::temp_dir().join(format!("ios-bridge-test-{}", std::process::id()));
    std::fs::create_dir_all(&out_dir).unwrap();
    let out_path = out_dir.join("out.json.gz");

    let demo_c = CString::new(demo_path.to_str().unwrap()).unwrap();
    let out_c = CString::new(out_path.to_str().unwrap()).unwrap();
    let progress_calls = AtomicU32::new(0);

    let status = unsafe {
        cs2_parse_file(
            demo_c.as_ptr(),
            out_c.as_ptr(),
            Some(count_progress_calls),
            &progress_calls as *const AtomicU32 as *mut c_void,
        )
    };

    assert_eq!(status, 0, "cs2_parse_file failed: {:?}", take_last_error());
    assert!(
        progress_calls.load(Ordering::SeqCst) > 0,
        "progress callback was never called"
    );

    let mut gz = GzDecoder::new(std::fs::File::open(&out_path).unwrap());
    let mut json = String::new();
    gz.read_to_string(&mut json).unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(value.get("schemaVersion").is_some());
    assert!(value.get("rounds").is_some());

    std::fs::remove_dir_all(&out_dir).ok();
}

#[test]
fn invalid_path_reports_an_error_without_panicking() {
    let missing = CString::new("/no/such/demo.dem").unwrap();
    let out_path = std::env::temp_dir().join(format!(
        "ios-bridge-test-missing-{}.json.gz",
        std::process::id()
    ));
    let out_c = CString::new(out_path.to_str().unwrap()).unwrap();

    let status =
        unsafe { cs2_parse_file(missing.as_ptr(), out_c.as_ptr(), None, std::ptr::null_mut()) };

    assert_ne!(status, 0);
    let message = take_last_error();
    assert!(message.is_some(), "expected cs2_last_error to be set");
}

#[test]
fn null_path_is_an_invalid_argument_not_a_crash() {
    let out_path = std::env::temp_dir().join(format!(
        "ios-bridge-test-null-{}.json.gz",
        std::process::id()
    ));
    let out_c = CString::new(out_path.to_str().unwrap()).unwrap();

    let status =
        unsafe { cs2_parse_file(std::ptr::null(), out_c.as_ptr(), None, std::ptr::null_mut()) };

    assert_eq!(status, 1, "expected Cs2Status::InvalidArgument (1)");
    assert!(take_last_error().is_some());
}

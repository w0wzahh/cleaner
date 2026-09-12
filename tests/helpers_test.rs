use cleaner::helpers::{is_excluded, is_hidden, matches_glob};
use std::path::PathBuf;

#[test]
fn excluded_is_case_insensitive() {
    let excl = vec!["C:\\Keep".to_string()];
    assert!(is_excluded(&PathBuf::from("c:\\keep\\x.txt"), &excl));
    assert!(is_excluded(&PathBuf::from("C:\\KEEP"), &excl));
    assert!(is_excluded(&PathBuf::from("C:\\Keep\\deep\\y.txt"), &excl));
}

#[test]
fn excluded_respects_boundaries() {
    let excl = vec!["C:\\Keep".to_string()];
    // "KeepOther" is a sibling, not a child — must NOT be excluded.
    assert!(!is_excluded(&PathBuf::from("C:\\KeepOther\\x.txt"), &excl));
    assert!(!is_excluded(&PathBuf::from("C:\\Keep2"), &excl));
    assert!(!is_excluded(&PathBuf::from("C:\\Other\\x.txt"), &excl));
}

#[test]
fn excluded_handles_slashes_and_trailing_sep() {
    let excl = vec!["C:/Keep/".to_string()];
    assert!(is_excluded(&PathBuf::from("C:\\keep\\x.txt"), &excl));
    let excl2 = vec!["D:\\Data\\".to_string()];
    assert!(is_excluded(&PathBuf::from("d:/data/file"), &excl2));
}

#[test]
fn excluded_ignores_empty_entries() {
    let excl = vec![String::new(), "   ".to_string()];
    assert!(!is_excluded(&PathBuf::from("C:\\anything"), &excl));
}

#[test]
fn glob_simple_pattern_matches_filename() {
    let p = PathBuf::from("C:\\deep\\nested\\file.tmp");
    assert!(matches_glob(&p, "*.tmp"));
    assert!(!matches_glob(&p, "*.log"));
}

#[test]
fn glob_is_case_insensitive() {
    let p = PathBuf::from("C:\\x\\FILE.TMP");
    assert!(matches_glob(&p, "*.tmp"));
    assert!(matches_glob(&p, "*.TMP"));
}

#[test]
fn glob_empty_matches_everything() {
    let p = PathBuf::from("C:\\x\\anything.bin");
    assert!(matches_glob(&p, ""));
}

#[test]
fn glob_invalid_pattern_matches_nothing() {
    let p = PathBuf::from("C:\\x\\a.tmp");
    assert!(!matches_glob(&p, "[invalid"));
}

#[test]
fn glob_path_pattern_matches_full_path() {
    let p = PathBuf::from("C:\\data\\logs\\app.log");
    assert!(matches_glob(&p, "*\\logs\\*"));
}

#[test]
fn hidden_file_detected_by_attribute() {
    let dir = std::env::temp_dir().join("cleaner_hidden_test");
    let _ = std::fs::create_dir_all(&dir);
    let f = dir.join("normal.txt");
    std::fs::write(&f, "x").unwrap();
    assert!(!is_hidden(&f));
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("attrib")
            .args(["+H", f.to_str().unwrap()])
            .output();
        assert!(is_hidden(&f));
        let _ = std::process::Command::new("attrib")
            .args(["-H", f.to_str().unwrap()])
            .output();
    }
    let _ = std::fs::remove_dir_all(&dir);
}

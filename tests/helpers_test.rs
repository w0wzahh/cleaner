use cleaner::helpers::{
    human_size, is_excluded, is_excluded_prepped, is_hidden, matches_glob,
    parse_human_size, path_key, prep_excludes,
};
use std::path::{Path, PathBuf};

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
fn path_key_normalizes_spellings() {
    assert_eq!(path_key(Path::new("C:\\Foo\\")), "c:\\foo");
    assert_eq!(path_key(Path::new("c:/Foo")), "c:\\foo");
    assert_eq!(path_key(Path::new("\\\\?\\C:\\Foo")), "c:\\foo");
    // Drive root keeps its trailing separator.
    assert_eq!(path_key(Path::new("C:\\")), "c:\\");
}

#[test]
fn prepped_excludes_match_and_dedupe_empty() {
    let prepped = prep_excludes(&["C:\\Keep".to_string(), "  ".to_string()]);
    assert_eq!(prepped.len(), 1);
    assert!(is_excluded_prepped(Path::new("c:\\keep\\f.tmp"), &prepped));
    assert!(!is_excluded_prepped(Path::new("c:\\keep2\\f.tmp"), &prepped));
}

#[test]
fn human_size_roundtrips_round_values() {
    for n in [0u64, 512, 1024, 5 * 1024 * 1024, 3 * 1024 * 1024 * 1024] {
        let s = human_size(n);
        assert_eq!(parse_human_size(&s), Some(n), "roundtrip failed for {}", s);
    }
    assert_eq!(parse_human_size("not a size"), None);
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

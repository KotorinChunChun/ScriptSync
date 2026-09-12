//! エクスプローラーからの位置引数と開発用 --project の入口。
pub fn startup_path(args: &[String]) -> Option<&str> {
    if let Some(index) = args.iter().position(|arg| arg == "--project") {
        return args.get(index + 1).map(String::as_str);
    }
    args.iter().skip(1).find(|arg| arg.to_ascii_lowercase().ends_with(".scriptsync")).map(String::as_str)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn association_and_explicit_paths() {
        let args=vec!["app.exe".into(),"C:/資料/私の原稿.SCRIPTSYNC".into()];
        assert_eq!(startup_path(&args),Some("C:/資料/私の原稿.SCRIPTSYNC"));
        let args=vec!["app.exe".into(),"--project".into(),"C:/project data".into()];
        assert_eq!(startup_path(&args),Some("C:/project data"));
        assert_eq!(startup_path(&["app.exe".into(),"other.txt".into()]),None);
    }
}

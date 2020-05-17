use std::path::Path;

pub trait AsHumanizedString {
    fn as_humanized_string(&self) -> String;
}

impl<P: AsRef<Path>> AsHumanizedString for P {
    #[cfg(not(target_os = "windows"))]
    fn as_humanized_string(&self) -> String {
        self.as_ref().display().to_string()
    }

    #[cfg(target_os = "windows")]
    fn as_humanized_string(&self) -> String {
        const VERBATIM_PREFIX: &str = r#"\\?\"#;
        let p = self.as_ref().display().to_string();
        if p.starts_with(VERBATIM_PREFIX) {
            p[VERBATIM_PREFIX.len()..].to_string()
        } else {
            p
        }
    }
}

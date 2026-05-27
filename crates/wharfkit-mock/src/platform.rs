use std::sync::Mutex;
use wharfkit_session::{Platform, PlatformName};

pub struct MockPlatform {
    name: PlatformName,
    pub shell_opens: Mutex<Vec<String>>,
}

impl MockPlatform {
    pub fn new(name: PlatformName) -> Self {
        Self {
            name,
            shell_opens: Mutex::new(Vec::new()),
        }
    }
}

impl Platform for MockPlatform {
    fn name(&self) -> PlatformName {
        self.name
    }
    fn shell_open(&self, uri: &str) {
        self.shell_opens.lock().unwrap().push(uri.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_shell_opens() {
        let p = MockPlatform::new(PlatformName::Macos);
        p.shell_open("esr:foo");
        p.shell_open("esr:bar");
        let opens = p.shell_opens.lock().unwrap();
        assert_eq!(opens.len(), 2);
        assert_eq!(opens[0], "esr:foo");
        assert_eq!(opens[1], "esr:bar");
    }
}

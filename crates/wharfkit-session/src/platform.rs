#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformName {
    Macos,
    Windows,
    Linux,
    IOS,
    Android,
    Web,
    Headless,
}

pub trait Platform: Send + Sync {
    fn name(&self) -> PlatformName;
    fn shell_open(&self, uri: &str);

    fn is_known_mobile(&self) -> bool {
        matches!(self.name(), PlatformName::IOS | PlatformName::Android)
    }

    fn is_apple_handheld(&self) -> bool {
        matches!(self.name(), PlatformName::IOS)
    }
}

pub struct HeadlessPlatform;

impl Platform for HeadlessPlatform {
    fn name(&self) -> PlatformName {
        PlatformName::Headless
    }

    fn shell_open(&self, _uri: &str) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headless_defaults() {
        let p = HeadlessPlatform;
        assert_eq!(p.name(), PlatformName::Headless);
        assert!(!p.is_known_mobile());
        assert!(!p.is_apple_handheld());
        p.shell_open("esr:test");
    }

    #[test]
    fn ios_is_mobile_and_apple() {
        struct IosTest;
        impl Platform for IosTest {
            fn name(&self) -> PlatformName {
                PlatformName::IOS
            }
            fn shell_open(&self, _: &str) {}
        }
        let p = IosTest;
        assert!(p.is_known_mobile());
        assert!(p.is_apple_handheld());
    }

    #[test]
    fn android_is_mobile_but_not_apple() {
        struct AndroidTest;
        impl Platform for AndroidTest {
            fn name(&self) -> PlatformName {
                PlatformName::Android
            }
            fn shell_open(&self, _: &str) {}
        }
        let p = AndroidTest;
        assert!(p.is_known_mobile());
        assert!(!p.is_apple_handheld());
    }
}

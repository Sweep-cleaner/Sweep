//! Keşif ve değişiklik arayüzü.
//!
//! Tüm platform mantığı [`Backend`] arkasındadır. Sınıflandırma, filtreleme,
//! defter ve geri alma mantığı bu arayüz üzerinden — gerçek kayıt defterine
//! dokunmadan — test edilebilir.

use super::Item;

/// Ne keşfedilecek?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Discovery {
    /// Yalnız başlangıç girdileri (kayıt defteri, klasörler, betikler).
    Startup,
    /// Yalnız zamanlanmış görevler.
    Tasks,
    /// İkisi birlikte.
    All,
}

/// Bir girdinin ham durumunu okuyup değiştirebilen arka uç.
pub trait Backend {
    /// Girdileri keşfet.
    fn discover(&self, what: Discovery) -> Vec<Item>;

    /// Değişiklik öncesi ham durumu yakala (yedek `payload`ı).
    fn capture(&self, item: &Item) -> Result<serde_json::Value, String>;

    /// Etkin durumu ayarla (geri alınabilir).
    fn set_enabled(&self, item: &Item, enabled: bool) -> Result<(), String>;

    /// Komutu/değeri değiştir.
    fn edit(&self, item: &Item, command: &str) -> Result<(), String>;

    /// Girdiyi kaldır (önce yedek alınır; çağıran garanti eder).
    fn remove(&self, item: &Item) -> Result<(), String>;

    /// Yakalanmış ham durumu geri kur (geri alma).
    fn restore(&self, payload: &serde_json::Value) -> Result<(), String>;

    /// Girdinin güncel kısa durumu: `enabled` | `disabled` | `absent`.
    fn state_of(&self, item: &Item) -> Result<String, String>;
}

/// Platforma uygun gerçek arka uç.
#[cfg(windows)]
pub fn system_backend() -> Box<dyn Backend> {
    Box::new(super::windows::WindowsBackend::new())
}

#[cfg(unix)]
pub fn system_backend() -> Box<dyn Backend> {
    Box::new(super::unix::UnixBackend::new())
}

#[cfg(not(any(unix, windows)))]
pub fn system_backend() -> Box<dyn Backend> {
    Box::new(NullBackend)
}

/// Desteklenmeyen platform: hiçbir şey keşfedilmez.
#[cfg(not(any(unix, windows)))]
pub struct NullBackend;

#[cfg(not(any(unix, windows)))]
impl Backend for NullBackend {
    fn discover(&self, _what: Discovery) -> Vec<Item> {
        Vec::new()
    }
    fn capture(&self, _item: &Item) -> Result<serde_json::Value, String> {
        Err("unsupported platform".into())
    }
    fn set_enabled(&self, _item: &Item, _enabled: bool) -> Result<(), String> {
        Err("unsupported platform".into())
    }
    fn edit(&self, _item: &Item, _command: &str) -> Result<(), String> {
        Err("unsupported platform".into())
    }
    fn remove(&self, _item: &Item) -> Result<(), String> {
        Err("unsupported platform".into())
    }
    fn restore(&self, _payload: &serde_json::Value) -> Result<(), String> {
        Err("unsupported platform".into())
    }
    fn state_of(&self, _item: &Item) -> Result<String, String> {
        Err("unsupported platform".into())
    }
}

// ---------------------------------------------------------------------------
// Testler için sahte arka uç (gerçek sisteme dokunmaz)
// ---------------------------------------------------------------------------

#[cfg(test)]
pub(crate) mod fake {
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    use crate::engine::autostart::{Handle, Kind, Scope, Source, Trigger};

    /// Sahte arka ucun diskte tuttuğu durum (yedek `payload`ı da budur).
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
    pub struct FakeState {
        pub id: String,
        pub kind: String,
        pub name: String,
        pub command: Option<String>,
        pub source: String,
        pub location: String,
        pub scope: String,
        pub enabled: bool,
        pub trigger: Option<String>,
        pub protected: bool,
        pub removable: bool,
        pub editable: bool,
        pub reversible: bool,
    }

    impl FakeState {
        fn to_item(&self) -> Item {
            let kind: Kind = self.kind.parse().unwrap_or(Kind::Startup);
            let scope: Scope = self.scope.parse().unwrap_or(Scope::User);
            let source: Source = self.source.parse().unwrap_or(Source::RegistryRun);
            let trigger: Option<Trigger> = self
                .trigger
                .as_deref()
                .and_then(|t| t.parse::<Trigger>().ok());
            let mut item = Item::new(
                kind,
                self.name.clone(),
                source,
                self.location.clone(),
                scope,
                self.enabled,
                trigger,
                Handle::Fake(self.id.clone()),
            );
            item.id = self.id.clone();
            item.command = self.command.clone();
            item.reclassify();
            item.reversible = self.reversible;
            item.editable = self.editable;
            item.removable = self.removable;
            if self.protected {
                item.removable = false;
                item.notes = Some("protected (test fixture)".to_string());
            }
            item
        }
    }

    /// Bellekte tutan, gerçek sisteme hiç dokunmayan arka uç.
    pub struct FakeBackend {
        state: Mutex<BTreeMap<String, FakeState>>,
    }

    impl Default for FakeBackend {
        fn default() -> Self {
            Self::new()
        }
    }

    impl FakeBackend {
        pub fn new() -> Self {
            Self {
                state: Mutex::new(BTreeMap::new()),
            }
        }

        /// Girdi ekle (id'si zaten atanmış olmalı).
        pub fn add(&self, item: &Item) {
            let state = FakeState {
                id: item.id.clone(),
                kind: item.kind.as_str().to_string(),
                name: item.name.clone(),
                command: item.command.clone(),
                source: item.source.as_str().to_string(),
                location: item.location.clone(),
                scope: item.scope.as_str().to_string(),
                enabled: item.enabled,
                trigger: item.trigger.map(|t| t.as_str().to_string()),
                protected: matches!(item.handle, Handle::Fake(ref s) if s.starts_with("protected:")),
                removable: item.removable,
                editable: item.editable,
                reversible: item.reversible,
            };
            self.state.lock().unwrap().insert(item.id.clone(), state);
        }

        pub fn get(&self, id: &str) -> Option<FakeState> {
            self.state.lock().unwrap().get(id).cloned()
        }

        fn with_state<T>(&self, id: &str, f: impl FnOnce(&mut FakeState) -> T) -> Result<T, String> {
            let mut guard = self.state.lock().unwrap();
            let slot = guard.get_mut(id).ok_or_else(|| format!("absent: {id}"))?;
            Ok(f(slot))
        }
    }

    impl Backend for FakeBackend {
        fn discover(&self, what: Discovery) -> Vec<Item> {
            let guard = self.state.lock().unwrap();
            guard
                .values()
                .filter(|s| match what {
                    Discovery::All => true,
                    Discovery::Startup => s.kind == "startup",
                    Discovery::Tasks => s.kind == "task",
                })
                .map(FakeState::to_item)
                .collect()
        }

        fn capture(&self, item: &Item) -> Result<serde_json::Value, String> {
            let state = self
                .get(&item.id)
                .ok_or_else(|| format!("absent: {}", item.id))?;
            serde_json::to_value(state).map_err(|e| e.to_string())
        }

        fn set_enabled(&self, item: &Item, enabled: bool) -> Result<(), String> {
            self.with_state(&item.id, |s| s.enabled = enabled)
        }

        fn edit(&self, item: &Item, command: &str) -> Result<(), String> {
            self.with_state(&item.id, |s| s.command = Some(command.to_string()))
        }

        fn remove(&self, item: &Item) -> Result<(), String> {
            self.state.lock().unwrap().remove(&item.id);
            Ok(())
        }

        fn restore(&self, payload: &serde_json::Value) -> Result<(), String> {
            let state: FakeState =
                serde_json::from_value(payload.clone()).map_err(|e| e.to_string())?;
            self.state.lock().unwrap().insert(state.id.clone(), state);
            Ok(())
        }

        fn state_of(&self, item: &Item) -> Result<String, String> {
            match self.get(&item.id) {
                Some(s) if s.enabled => Ok("enabled".to_string()),
                Some(_) => Ok("disabled".to_string()),
                None => Ok("absent".to_string()),
            }
        }
    }
}

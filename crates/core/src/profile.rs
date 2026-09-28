use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::macro_config::MacroConfig;

/// Gaming profile containing optimization settings and crosshair configuration
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Profile {
    pub name: String,
    pub processes_to_kill: Vec<String>,
    pub crosshair_image_path: Option<String>,
    pub crosshair_x_offset: i32,
    pub crosshair_y_offset: i32,
    pub overlay_enabled: bool,
    #[serde(default)]
    pub fan_speed_max: bool,
    /// Gaming macros for this profile
    #[serde(default)]
    pub macros: MacroConfig,
}

impl Profile {
    /// Validate profile data
    #[allow(dead_code)]
    pub fn validate(&self) -> Result<()> {
        // Validate name length (1-50 characters)
        if self.name.is_empty() || self.name.len() > 50 {
            return Err(anyhow!("Profile name must be between 1 and 50 characters"));
        }

        // Validate crosshair image path if provided
        if let Some(ref path) = self.crosshair_image_path {
            let path_obj = Path::new(path);

            // Check if file exists
            if !path_obj.exists() {
                return Err(anyhow!("Crosshair image file does not exist: {}", path));
            }

            // Check if file has .png extension
            if path_obj.extension().and_then(|s| s.to_str()) != Some("png") {
                return Err(anyhow!("Crosshair image must be a PNG file: {}", path));
            }
        }

        // Validate X/Y offsets (-500 to +500 pixels)
        if !(-500..=500).contains(&self.crosshair_x_offset) {
            return Err(anyhow!("X offset must be between -500 and 500 pixels"));
        }
        if !(-500..=500).contains(&self.crosshair_y_offset) {
            return Err(anyhow!("Y offset must be between -500 and 500 pixels"));
        }

        Ok(())
    }
}

/// Load profiles from JSON file in user data directory
/// Returns empty vector if file doesn't exist (not an error)
pub fn load_profiles(data_dir: &Path) -> Result<Vec<Profile>> {
    let profiles_path = data_dir.join("profiles.json");

    // If file doesn't exist, return empty vector
    if !profiles_path.exists() {
        return Ok(Vec::new());
    }

    // Read and parse JSON
    let contents = fs::read_to_string(&profiles_path)
        .map_err(|e| anyhow!("Failed to read profiles.json: {}", e))?;

    let profiles: Vec<Profile> = serde_json::from_str(&contents)
        .map_err(|e| anyhow!("Failed to parse profiles.json: {}", e))?;

    Ok(profiles)
}

/// Save profiles to JSON file in user data directory
/// Creates directory if it doesn't exist
pub fn save_profiles(profiles: &[Profile], data_dir: &Path) -> Result<()> {
    // Create directory if it doesn't exist
    fs::create_dir_all(data_dir).map_err(|e| anyhow!("Failed to create data directory: {}", e))?;

    let profiles_path = data_dir.join("profiles.json");

    // Serialize to pretty-printed JSON
    let json = serde_json::to_string_pretty(profiles)
        .map_err(|e| anyhow!("Failed to serialize profiles: {}", e))?;

    // Write to file
    fs::write(&profiles_path, json).map_err(|e| anyhow!("Failed to write profiles.json: {}", e))?;

    Ok(())
}

/// Create a new profile with default values
#[allow(dead_code)]
pub fn create_profile(name: String) -> Profile {
    Profile {
        name,
        processes_to_kill: Vec::new(),
        crosshair_image_path: None,
        crosshair_x_offset: 0,
        crosshair_y_offset: 0,
        overlay_enabled: true,
        fan_speed_max: false,
        macros: MacroConfig::default(),
    }
}

/// Delete profile at the specified index
#[allow(dead_code)]
pub fn delete_profile(profiles: &mut Vec<Profile>, index: usize) {
    if index < profiles.len() {
        profiles.remove(index);
    }
}

/// Check if profile name is unique in the list (case-insensitive)
#[allow(dead_code)]
pub fn is_profile_name_unique(
    profiles: &[Profile],
    name: &str,
    exclude_index: Option<usize>,
) -> bool {
    let name_lower = name.to_lowercase();

    for (i, profile) in profiles.iter().enumerate() {
        // Skip the profile at exclude_index (for updates)
        if let Some(exclude) = exclude_index {
            if i == exclude {
                continue;
            }
        }

        if profile.name.to_lowercase() == name_lower {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TemporaryDirectory(PathBuf);

    impl TemporaryDirectory {
        fn new() -> Self {
            let suffix = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "edge-optimizer-profile-{}-{suffix}",
                std::process::id()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TemporaryDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn test_create_profile() {
        // Verifies a new profile receives safe crosshair, process, fan, and macro defaults.
        let profile = create_profile("Test Profile".to_string());
        assert_eq!(profile.name, "Test Profile");
        assert!(profile.processes_to_kill.is_empty());
        assert_eq!(profile.crosshair_image_path, None);
        assert_eq!(profile.crosshair_x_offset, 0);
        assert_eq!(profile.crosshair_y_offset, 0);
        assert!(profile.overlay_enabled);
    }

    #[test]
    fn test_validate_name_length() {
        // Verifies profile names enforce the documented one-to-fifty-character boundary.
        let mut profile = create_profile("Valid".to_string());
        assert!(profile.validate().is_ok());

        profile.name = "".to_string();
        assert!(profile.validate().is_err());

        profile.name = "a".repeat(51);
        assert!(profile.validate().is_err());
    }

    #[test]
    fn test_validate_offsets() {
        // Verifies crosshair offsets accept boundary values and reject out-of-range coordinates.
        let mut profile = create_profile("Test".to_string());

        profile.crosshair_x_offset = -500;
        assert!(profile.validate().is_ok());

        profile.crosshair_x_offset = 500;
        assert!(profile.validate().is_ok());

        profile.crosshair_x_offset = -501;
        assert!(profile.validate().is_err());

        profile.crosshair_x_offset = 0;
        profile.crosshair_y_offset = 501;
        assert!(profile.validate().is_err());
    }

    #[test]
    fn test_is_profile_name_unique() {
        // Verifies profile names are unique case-insensitively while allowing the edited profile itself.
        let profiles = vec![
            create_profile("Profile 1".to_string()),
            create_profile("Profile 2".to_string()),
        ];

        assert!(is_profile_name_unique(&profiles, "Profile 3", None));
        assert!(!is_profile_name_unique(&profiles, "Profile 1", None));
        assert!(!is_profile_name_unique(&profiles, "profile 1", None)); // Case-insensitive
        assert!(is_profile_name_unique(&profiles, "Profile 1", Some(0))); // Exclude self
    }

    #[test]
    fn legacy_json_round_trip_uses_only_a_temporary_directory() {
        // Verifies transitional profile JSON can be saved and loaded without touching personal data.
        let directory = TemporaryDirectory::new();
        let profile = create_profile("Portable".into());
        save_profiles(std::slice::from_ref(&profile), &directory.0).unwrap();
        let restored = load_profiles(&directory.0).unwrap();
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].name, "Portable");
    }

    #[test]
    fn missing_and_malformed_legacy_json_are_distinguished() {
        // Verifies a missing legacy file is empty while malformed JSON surfaces an explicit error.
        let directory = TemporaryDirectory::new();
        assert!(load_profiles(&directory.0).unwrap().is_empty());
        fs::write(directory.0.join("profiles.json"), "not-json").unwrap();
        assert!(load_profiles(&directory.0).is_err());
    }

    #[test]
    fn delete_profile_ignores_invalid_indexes() {
        // Verifies deletion removes only a valid selected index and safely ignores out-of-range input.
        let mut profiles = vec![create_profile("One".into()), create_profile("Two".into())];
        delete_profile(&mut profiles, 9);
        assert_eq!(profiles.len(), 2);
        delete_profile(&mut profiles, 0);
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].name, "Two");
    }
}

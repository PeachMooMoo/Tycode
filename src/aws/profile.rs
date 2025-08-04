use anyhow::Result;
use std::env;
use std::fs;
use std::path::PathBuf;

/// Read AWS profiles from the config file
pub fn list_aws_profiles() -> Result<Vec<String>> {
    let config_path = get_aws_config_path()?;

    // Read the config file
    let content = fs::read_to_string(config_path)?;

    // Parse the profiles
    let profiles = parse_profiles(&content);

    Ok(profiles)
}

/// Parse AWS profiles from config content
fn parse_profiles(content: &str) -> Vec<String> {
    let mut profiles = Vec::new();

    // Add 'default' profile first, it's always available
    profiles.push("default".to_string());

    // Parse the file for profile definitions like [profile name]
    for line in content.lines() {
        let line = line.trim();

        if line.starts_with("[profile ") && line.ends_with(']') {
            // Extract the profile name between "[profile " and "]"
            let profile_name = line[9..line.len() - 1].trim().to_string();
            profiles.push(profile_name);
        }
    }

    profiles
}

/// Get the path to the AWS config file
fn get_aws_config_path() -> Result<PathBuf> {
    // First check if AWS_CONFIG_FILE environment variable is set
    if let Ok(path) = env::var("AWS_CONFIG_FILE") {
        return Ok(PathBuf::from(path));
    }

    // Otherwise use the default location
    let home_dir = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Home directory not found"))?;

    Ok(home_dir.join(".aws").join("config"))
}

/// Get the current AWS profile
pub fn get_current_profile() -> String {
    // Check AWS_PROFILE environment variable
    env::var("AWS_PROFILE").unwrap_or_else(|_| "default".to_string())
}

/// Set the AWS profile
pub fn set_aws_profile(profile: &str) -> Result<()> {
    // In a real application, this would manage environment variables
    // or persist the selection. For now, we'll just validate.
    let available_profiles = list_aws_profiles()?;

    if !available_profiles.contains(&profile.to_string()) {
        return Err(anyhow::anyhow!("AWS profile '{}' not found", profile));
    }

    // Profile exists
    Ok(())
}

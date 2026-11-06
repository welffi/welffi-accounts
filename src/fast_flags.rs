use crate::data::local_app_data;
use std::fs;
use std::io;
use std::path::Path;

const VALUES: [(&str, &str); 15] = [
    ("DFFlagTextureQualityOverrideEnabled", "true"),
    ("DFIntCSGLevelOfDetailSwitchingDistance", "0"),
    ("DFIntCSGLevelOfDetailSwitchingDistanceL12", "0"),
    ("DFIntCSGLevelOfDetailSwitchingDistanceL23", "0"),
    ("DFIntCSGLevelOfDetailSwitchingDistanceL34", "0"),
    ("DFIntDebugFRMQualityLevelOverride", "1"),
    ("DFIntTaskSchedulerTargetFps", "30"),
    ("DFIntTextureQualityOverride", "0"),
    ("FFlagDebugSkyGray", "true"),
    ("FFlagDisablePostFx", "true"),
    ("FIntDebugForceMSAASamples", "1"),
    ("FIntFRMMaxGrassDistance", "0"),
    ("FIntFRMMinGrassDistance", "0"),
    ("FIntRenderGrassDetailStrands", "0"),
    ("FIntRenderShadowIntensity", "0"),
];

pub fn json() -> String {
    let lines: Vec<String> = VALUES.iter().map(|(key, value)| format!("  \"{key}\": {value}")).collect();
    format!("{{\n{}\n}}", lines.join(",\n"))
}

pub fn apply(on: bool) -> io::Result<()> {
    apply_in(on, &local_app_data().join("Roblox").join("Versions"))
}

pub fn apply_in(on: bool, versions: &Path) -> io::Result<()> {
    let Ok(entries) = fs::read_dir(versions) else {
        return Ok(());
    };
    for version in entries.flatten().map(|entry| entry.path()).filter(|path| path.join("RobloxPlayerBeta.exe").is_file()) {
        apply_to_file(on, &version.join("ClientSettings").join("ClientAppSettings.json"))?;
    }
    Ok(())
}

fn apply_to_file(on: bool, file: &Path) -> io::Result<()> {
    let backup = file.with_extension("json.welffi.bak");
    let ours = json();
    let exists = file.exists();
    let mine = exists && fs::read_to_string(file)? == ours;
    if on {
        if exists && !mine && !backup.exists() {
            fs::copy(file, &backup)?;
        }
        if let Some(folder) = file.parent() {
            fs::create_dir_all(folder)?;
        }
        fs::write(file, ours)
    } else if backup.exists() {
        fs::rename(backup, file)
    } else if mine {
        fs::remove_file(file)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::tests::scratch;

    #[test]
    fn flags_back_up_and_restore_the_old_file() {
        let root = scratch("flags");
        let version = root.join("version-1");
        let settings = version.join("ClientSettings").join("ClientAppSettings.json");
        let other = root.join("not-a-player");
        fs::create_dir_all(settings.parent().unwrap()).unwrap();
        fs::create_dir_all(&other).unwrap();
        fs::write(version.join("RobloxPlayerBeta.exe"), "").unwrap();
        fs::write(&settings, "{\"Old\": 1}").unwrap();

        apply_in(true, &root).unwrap();
        assert_eq!(fs::read_to_string(&settings).unwrap(), json());
        assert!(!other.join("ClientSettings").exists());
        apply_in(true, &root).unwrap();
        apply_in(false, &root).unwrap();
        assert_eq!(fs::read_to_string(&settings).unwrap(), "{\"Old\": 1}");

        fs::remove_file(&settings).unwrap();
        apply_in(true, &root).unwrap();
        apply_in(false, &root).unwrap();
        assert!(!settings.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn json_matches_the_old_format() {
        let text = json();
        assert!(text.starts_with("{\n  \"DFFlagTextureQualityOverrideEnabled\": true,\n"));
        assert!(text.ends_with("  \"FIntRenderShadowIntensity\": 0\n}"));
    }
}

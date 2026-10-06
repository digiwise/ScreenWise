// Screenpipe — AI that knows everything you've seen, said, or heard.
// https://screenpi.pe
// Safe promotion and instructions for the explicit Windows Pi provisioner.

use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

pub const WINDOWS_PROVISION_SCRIPT: &str =
    include_str!("../../../../scripts/windows/Provision-ScreenWisePi.ps1");

/// Place the embedded helper in the user's data directory without replacing
/// existing content. Reparse points are refused throughout the destination
/// path so setup cannot redirect the write through a link or junction.
pub fn materialize_windows_provisioner(data_dir: &Path) -> Result<PathBuf, String> {
    let data_dir = if data_dir.is_absolute() {
        data_dir.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| format!("Could not resolve the ScreenWise data directory: {error}"))?
            .join(data_dir)
    };
    let data_dir = normalize_path(&data_dir)?;
    let script_dir = data_dir.join("setup");
    let script_path = script_dir.join("Provision-ScreenWisePi.ps1");

    reject_reparse_components(&data_dir)?;
    fs::create_dir_all(&data_dir)
        .map_err(|error| format!("Could not prepare the ScreenWise data directory: {error}"))?;
    reject_reparse_components(&script_dir)?;
    match fs::create_dir(&script_dir) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(format!("Could not create the Pi setup directory: {error}")),
    }
    reject_reparse_components(&script_path)?;

    match fs::symlink_metadata(&script_path) {
        Ok(_) => verify_existing_provisioner(&script_path)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&script_path)
            {
                Ok(mut file) => {
                    if let Err(error) = file.write_all(WINDOWS_PROVISION_SCRIPT.as_bytes()) {
                        return Err(format!("Could not write the Pi provisioner: {error}"));
                    }
                    file.sync_all()
                        .map_err(|error| format!("Could not sync the Pi provisioner: {error}"))?;
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    verify_existing_provisioner(&script_path)?;
                }
                Err(error) => {
                    return Err(format!("Could not create the Pi provisioner: {error}"));
                }
            }
        }
        Err(error) => {
            return Err(format!(
                "Could not inspect the Pi provisioner path: {error}"
            ));
        }
    }

    fs::canonicalize(&script_path)
        .map_err(|error| format!("Could not resolve the Pi provisioner path: {error}"))
}

fn verify_existing_provisioner(script_path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(script_path)
        .map_err(|error| format!("Could not inspect the existing Pi provisioner: {error}"))?;
    if !metadata.file_type().is_file() || is_reparse_point(&metadata) {
        return Err("Refusing to use an unexpected Pi provisioner file or link.".into());
    }
    let expected = WINDOWS_PROVISION_SCRIPT.as_bytes();
    if metadata.len() != expected.len() as u64 {
        return Err(format!(
            "Refusing to overwrite conflicting setup content at {}.",
            script_path.display()
        ));
    }
    let file = fs::File::open(script_path)
        .map_err(|error| format!("Could not read the existing Pi provisioner: {error}"))?;
    let mut existing = Vec::with_capacity(expected.len());
    file.take(expected.len() as u64 + 1)
        .read_to_end(&mut existing)
        .map_err(|error| format!("Could not read the existing Pi provisioner: {error}"))?;
    if existing != expected {
        return Err(format!(
            "Refusing to overwrite conflicting setup content at {}.",
            script_path.display()
        ));
    }
    Ok(())
}

pub fn windows_command(script_path: &Path, data_dir: &Path) -> Result<String, String> {
    if !script_path.is_absolute() {
        return Err("The Pi provisioner path must be absolute.".into());
    }
    let data_dir = if data_dir.is_absolute() {
        data_dir.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| format!("Could not resolve the ScreenWise data directory: {error}"))?
            .join(data_dir)
    };
    let data_dir = normalize_path(&data_dir)?;
    let script = powershell_literal(&windows_display_path(script_path));
    let data = powershell_literal(&windows_display_path(&data_dir));
    Ok(format!("& {script} -DataDir {data}"))
}

#[cfg(windows)]
fn windows_display_path(path: &Path) -> String {
    let value = path.to_string_lossy();
    if let Some(unc) = value.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{}", unc)
    } else if let Some(dos) = value.strip_prefix(r"\\?\") {
        if dos.as_bytes().get(1) == Some(&b':') && dos.as_bytes().get(2) == Some(&b'\\') {
            dos.to_string()
        } else {
            value.into_owned()
        }
    } else {
        value.into_owned()
    }
}

#[cfg(not(windows))]
fn windows_display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn powershell_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn normalize_path(path: &Path) -> Result<PathBuf, String> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            component => normalized.push(component.as_os_str()),
        }
    }
    if normalized.is_absolute() {
        Ok(normalized)
    } else {
        Err("The ScreenWise data directory must resolve to an absolute path.".into())
    }
}

fn reject_reparse_components(path: &Path) -> Result<(), String> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if is_reparse_point(&metadata) => {
                return Err(format!(
                    "Refusing to place the Pi provisioner below a symbolic link or reparse point: {}",
                    current.display()
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!("Could not inspect the Pi setup path: {error}"));
            }
        }
    }
    Ok(())
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(test)]
mod tests {
    use super::{materialize_windows_provisioner, windows_command, WINDOWS_PROVISION_SCRIPT};
    use std::path::Path;

    fn test_dir(label: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "screenwise-pi-provisioning-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn materializes_exact_embedded_script_bytes_and_reuses_them() {
        let root = test_dir("embedded");
        let installed = materialize_windows_provisioner(&root).unwrap();
        assert_eq!(
            std::fs::read(&installed).unwrap(),
            WINDOWS_PROVISION_SCRIPT.as_bytes()
        );
        assert_eq!(materialize_windows_provisioner(&root).unwrap(), installed);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn refuses_conflicting_existing_setup_file_without_overwriting_it() {
        let root = test_dir("conflict");
        let setup = root.join("setup");
        std::fs::create_dir(&setup).unwrap();
        let script = setup.join("Provision-ScreenWisePi.ps1");
        std::fs::write(&script, b"user content").unwrap();
        assert!(materialize_windows_provisioner(&root)
            .unwrap_err()
            .contains("Refusing to overwrite"));
        assert_eq!(std::fs::read(script).unwrap(), b"user content");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn refuses_oversized_or_non_file_setup_content() {
        let oversized_root = test_dir("oversized");
        let oversized_setup = oversized_root.join("setup");
        std::fs::create_dir(&oversized_setup).unwrap();
        let oversized_script = oversized_setup.join("Provision-ScreenWisePi.ps1");
        let oversized = vec![b'x'; WINDOWS_PROVISION_SCRIPT.len() + 1024 * 1024];
        std::fs::write(&oversized_script, &oversized).unwrap();
        assert!(materialize_windows_provisioner(&oversized_root)
            .unwrap_err()
            .contains("Refusing to overwrite"));
        assert_eq!(
            std::fs::metadata(&oversized_script).unwrap().len(),
            oversized.len() as u64
        );
        std::fs::remove_dir_all(oversized_root).unwrap();

        let non_file_root = test_dir("non-file");
        let non_file_setup = non_file_root.join("setup");
        std::fs::create_dir(&non_file_setup).unwrap();
        std::fs::create_dir(non_file_setup.join("Provision-ScreenWisePi.ps1")).unwrap();
        assert!(materialize_windows_provisioner(&non_file_root)
            .unwrap_err()
            .contains("unexpected Pi provisioner"));
        std::fs::remove_dir_all(non_file_root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn refuses_a_junction_in_the_destination_path_when_available() {
        let root = test_dir("junction");
        let target = root.join("real");
        let junction = root.join("junction");
        std::fs::create_dir(&target).unwrap();
        let status = std::process::Command::new("cmd.exe")
            .args([
                "/C",
                "mklink",
                "/J",
                junction.to_str().unwrap(),
                target.to_str().unwrap(),
            ])
            .status();
        if status.map(|status| status.success()).unwrap_or(false) {
            assert!(materialize_windows_provisioner(&junction.join("data"))
                .unwrap_err()
                .contains("symbolic link or reparse point"));
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn command_uses_absolute_paths_and_escapes_powershell_literals() {
        let script =
            Path::new("C:\\Users\\O'Neil\\ScreenWise Data\\setup\\Provision-ScreenWisePi.ps1");
        let data = Path::new("C:\\Users\\O'Neil\\$recording` data");
        let command = windows_command(script, data).unwrap();
        assert_eq!(command, "& 'C:\\Users\\O''Neil\\ScreenWise Data\\setup\\Provision-ScreenWisePi.ps1' -DataDir 'C:\\Users\\O''Neil\\$recording` data'");
        assert!(windows_command(Path::new("relative.ps1"), data).is_err());
        let relative_data = windows_command(script, Path::new("relative-data")).unwrap();
        assert!(relative_data.contains(
            &std::env::current_dir()
                .unwrap()
                .join("relative-data")
                .display()
                .to_string()
        ));
    }

    #[cfg(windows)]
    #[test]
    fn command_removes_extended_dos_and_unc_prefixes() {
        let script = Path::new(r"\\?\C:\ScreenWise Data\setup\Provision-ScreenWisePi.ps1");
        let data = Path::new(r"\\?\UNC\server\share\ScreenWise Data");
        let command = windows_command(script, data).unwrap();
        assert!(command.starts_with("& 'C:\\ScreenWise Data\\setup\\"));
        assert!(command.ends_with("-DataDir '\\\\server\\share\\ScreenWise Data'"));
        assert!(!command.contains(r"\\?\"));
    }
}

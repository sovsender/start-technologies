use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const SCSI_TYPE_MMC: &str = "5";

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct OpticalNode {
    pub path: PathBuf,
    pub ty: char,
    pub major: u32,
    pub minor: u32,
}

fn parse_devt(raw: &str) -> Option<(u32, u32)> {
    let (major, minor) = raw.trim().split_once(':')?;
    Some((major.parse().ok()?, minor.parse().ok()?))
}

async fn read_trimmed(path: impl AsRef<Path>) -> Option<String> {
    tokio::fs::read_to_string(path)
        .await
        .ok()
        .map(|s| s.trim().to_owned())
}

/// The block and SCSI-generic nodes of every optical drive under `sys`.
pub async fn scan(sys: &Path) -> BTreeSet<OpticalNode> {
    let mut nodes = BTreeSet::new();
    let Ok(mut blocks) = tokio::fs::read_dir(sys.join("class/block")).await else {
        return nodes;
    };
    while let Ok(Some(entry)) = blocks.next_entry().await {
        let name = entry.file_name();
        if !name.to_string_lossy().starts_with("sr") {
            continue;
        }
        let dir = entry.path();
        if read_trimmed(dir.join("device/type")).await.as_deref() != Some(SCSI_TYPE_MMC) {
            continue;
        }
        let Some((major, minor)) = read_trimmed(dir.join("dev"))
            .await
            .as_deref()
            .and_then(parse_devt)
        else {
            continue;
        };
        nodes.insert(OpticalNode {
            path: Path::new("/dev").join(&name),
            ty: 'b',
            major,
            minor,
        });
        let Ok(mut generic) = tokio::fs::read_dir(dir.join("device/scsi_generic")).await else {
            continue;
        };
        while let Ok(Some(sg)) = generic.next_entry().await {
            let sg_name = sg.file_name();
            let Some((major, minor)) =
                read_trimmed(sys.join("class/scsi_generic").join(&sg_name).join("dev"))
                    .await
                    .as_deref()
                    .and_then(parse_devt)
            else {
                continue;
            };
            nodes.insert(OpticalNode {
                path: Path::new("/dev").join(&sg_name),
                ty: 'c',
                major,
                minor,
            });
        }
    }
    nodes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, rel: &str, contents: &str) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    fn node(path: &str, ty: char, major: u32, minor: u32) -> OpticalNode {
        OpticalNode {
            path: PathBuf::from(path),
            ty,
            major,
            minor,
        }
    }

    #[test]
    fn parses_major_and_minor() {
        assert_eq!(parse_devt("11:0\n"), Some((11, 0)));
        assert_eq!(parse_devt("259:12"), Some((259, 12)));
        assert_eq!(parse_devt("11"), None);
        assert_eq!(parse_devt("a:b"), None);
    }

    #[tokio::test]
    async fn finds_the_drive_and_its_generic_node() {
        let sys = tempfile::tempdir().unwrap();
        let root = sys.path();
        write(root, "class/block/sr0/dev", "11:0\n");
        write(root, "class/block/sr0/device/type", "5\n");
        write(root, "class/block/sr0/device/scsi_generic/sg1/.keep", "");
        write(root, "class/scsi_generic/sg1/dev", "21:1\n");

        assert_eq!(
            scan(root).await,
            BTreeSet::from([node("/dev/sr0", 'b', 11, 0), node("/dev/sg1", 'c', 21, 1)])
        );
    }

    #[tokio::test]
    async fn ignores_disks_and_non_optical_scsi_devices() {
        let sys = tempfile::tempdir().unwrap();
        let root = sys.path();
        write(root, "class/block/sda/dev", "8:0\n");
        write(root, "class/block/sda/device/type", "0\n");
        write(root, "class/block/sda/device/scsi_generic/sg0/.keep", "");
        write(root, "class/scsi_generic/sg0/dev", "21:0\n");
        write(root, "class/block/sr1/dev", "11:1\n");
        write(root, "class/block/sr1/device/type", "0\n");
        write(root, "class/block/sr1/device/scsi_generic/sg2/.keep", "");
        write(root, "class/scsi_generic/sg2/dev", "21:2\n");

        assert!(scan(root).await.is_empty());
    }

    #[tokio::test]
    async fn a_missing_sysfs_yields_no_drives() {
        let sys = tempfile::tempdir().unwrap();
        assert!(scan(&sys.path().join("absent")).await.is_empty());
    }
}

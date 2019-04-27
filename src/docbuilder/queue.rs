//! Updates crates.io index and builds new packages

use super::DocBuilder;
use crate::db::connect_db;
use crate::error::Result;
use crates_index_diff::{ChangeKind, Index};

impl DocBuilder {
    /// Updates crates.io-index repository and adds new crates into build queue.
    /// Returns size of queue
    pub fn get_new_crates(&mut self) -> Result<i64> {
        let conn = connect_db()?;
        let index = Index::from_path_or_cloned(&self.options.crates_io_index_path)?;
        let mut changes = index.fetch_changes()?;

        // I belive this will fix ordering of queue if we get more than one crate from changes
        changes.reverse();

        for krate in changes.iter().filter(|k| k.kind != ChangeKind::Yanked) {
            conn.execute(
                "INSERT INTO queue (name, version) VALUES ($1, $2)",
                &[&krate.name, &krate.version],
            )
            .ok();
            debug!("{}-{} added into build queue", krate.name, krate.version);
        }

        let queue_count = conn
            .query("SELECT COUNT(*) FROM queue WHERE attempt < 5", &[])
            .unwrap()
            .get(0)
            .get(0);

        Ok(queue_count)
    }

    /// Builds packages from queue
    pub fn build_packages_queue(&mut self) -> Result<usize> {
        let conn = connect_db()?;
        let mut build_count = 0;

        for row in &conn.query(
            "SELECT id, name, version
                                     FROM queue
                                     WHERE attempt < 5
                                     ORDER BY id ASC",
            &[],
        )? {
            let id: i32 = row.get(0);
            let name: String = row.get(1);
            let version: String = row.get(2);

            match self.build_package(&name[..], &version[..]) {
                Ok(_) => {
                    build_count += 1;
                    let _ = conn.execute("DELETE FROM queue WHERE id = $1", &[&id]);
                }
                Err(e) => {
                    // Increase attempt count
                    let _ = conn.execute(
                        "UPDATE queue SET attempt = attempt + 1 WHERE id = $1",
                        &[&id],
                    );
                    error!(
                        "Failed to build package {}-{} from queue: {}",
                        name, version, e
                    )
                }
            }
        }

        Ok(build_count)
    }
}

#[cfg(test)]
mod test {
    use crate::{DocBuilder, DocBuilderOptions};
    use std::path::PathBuf;

    #[test]
    #[ignore]
    fn test_get_new_crates() {
        let _ = env_logger::try_init();
        let options = DocBuilderOptions::from_prefix(PathBuf::from("../cratesfyi-prefix"));
        let mut docbuilder = DocBuilder::new(options);
        let res = docbuilder.get_new_crates();
        if res.is_err() {
            error!("{:?}", res);
        }
        assert!(res.is_ok());
    }

    #[test]
    #[ignore]
    fn test_build_packages_queue() {
        let _ = env_logger::try_init();
        let options = DocBuilderOptions::from_prefix(PathBuf::from("../cratesfyi-prefix"));
        let mut docbuilder = DocBuilder::new(options);
        assert!(docbuilder.build_packages_queue().is_ok());
    }
}

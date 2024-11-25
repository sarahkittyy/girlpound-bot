use common::Error;
use poise::serenity_prelude::async_trait;

#[async_trait]
pub trait ServerFiles: Send + Sync {
    /// download the contents of a file on the server.
    async fn fetch_file(&self, path: &str) -> Result<Vec<u8>, Error>;

    /// upload the contents of a file on the server.
    async fn upload_file(&self, path: &str, contents: &[u8]) -> Result<(), Error>;

    /// download the contents of a file and split it into lines, trimming whitespace
    async fn fetch_file_lines(&self, path: &str) -> Result<Vec<String>, Error> {
        self.fetch_file(path).await.map(|bytes| {
            bytes
                .split(|&c| c == b'\n')
                .map(|line| String::from_utf8_lossy(line).trim().to_owned())
                .collect()
        })
    }

    /// edits a configuration line, or adds it if it doesn't exist. returns true if the line was added
    async fn add_or_edit_line(
        &self,
        path: &str,
        starts_with: &str,
        new_value: &str,
    ) -> Result<bool, Error> {
        let mut lines = self.fetch_file_lines(path).await?;
        let mut exists = false;
        for line in &mut lines {
            if line.starts_with(starts_with) {
                *line = new_value.to_owned();
                exists = true;
                break;
            }
        }
        if !exists {
            lines.push(new_value.to_owned());
        }
        self.upload_file(path, lines.join("\n").as_bytes()).await?;
        Ok(!exists)
    }
}

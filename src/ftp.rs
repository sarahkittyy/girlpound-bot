use ftp::FtpStream;
use std::{io::Cursor, net::SocketAddr};

use crate::Error;

/// for reliably accessing server files via ftp
#[derive(Clone)]
pub struct ServerFtp {
    addr: SocketAddr,
    creds: (String, String),
}

impl ServerFtp {
    /// constructor
    pub fn new(addr: SocketAddr, creds: (String, String)) -> Self {
        Self { addr, creds }
    }

    /// run an operation on the ftp stream
    pub fn exec<T>(&self, f: impl FnOnce(&mut FtpStream) -> Result<T, Error>) -> Result<T, Error> {
        let mut ftp = FtpStream::connect(self.addr)?;
        ftp.login(&self.creds.0, &self.creds.1)?;
        let r = f(&mut ftp);
        ftp.quit()?;
        r
    }

    /// download the contents of a file on the server.
    pub async fn fetch_file(&self, path: &str) -> Result<Vec<u8>, Error> {
        self.exec(|ftp| Ok(ftp.simple_retr(path)?.into_inner()))
    }

    /// download the contents of a file and split it into lines, trimming whitespace
    pub async fn fetch_file_lines(&self, path: &str) -> Result<Vec<String>, Error> {
        self.exec(|ftp| Ok(ftp.simple_retr(path)?.into_inner()))
            .map(|bytes| {
                bytes
                    .split(|&c| c == b'\n')
                    .map(|line| String::from_utf8_lossy(line).trim().to_owned())
                    .collect()
            })
    }

    /// edits a configuration line, or adds it if it doesn't exist. returns true if the line was added
    pub async fn add_or_edit_line(
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

    /// upload the contents of a file on the server.
    pub async fn upload_file(&self, path: &str, contents: &[u8]) -> Result<(), Error> {
        self.exec(|ftp| Ok(ftp.put(path, &mut Cursor::new(contents))?))
    }
}

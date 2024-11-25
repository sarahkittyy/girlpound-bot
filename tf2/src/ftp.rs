use crate::files::ServerFiles;
use ftp::FtpStream;
use poise::serenity_prelude::async_trait;
use std::{io::Cursor, net::SocketAddr};

use common::Error;

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
    fn exec<T>(&self, f: impl FnOnce(&mut FtpStream) -> Result<T, Error>) -> Result<T, Error> {
        let mut ftp = FtpStream::connect(self.addr)?;
        ftp.login(&self.creds.0, &self.creds.1)?;
        let r = f(&mut ftp);
        ftp.quit()?;
        r
    }
}

#[async_trait]
impl ServerFiles for ServerFtp {
    /// download the contents of a file on the server.
    async fn fetch_file(&self, path: &str) -> Result<Vec<u8>, Error> {
        self.exec(|ftp| Ok(ftp.simple_retr(path)?.into_inner()))
    }

    /// upload the contents of a file on the server.
    async fn upload_file(&self, path: &str, contents: &[u8]) -> Result<(), Error> {
        self.exec(|ftp| Ok(ftp.put(path, &mut Cursor::new(contents))?))
    }
}

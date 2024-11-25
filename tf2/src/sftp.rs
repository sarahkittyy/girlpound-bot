use std::{
    io::{stdout, Read, Write},
    net::{SocketAddr, TcpStream},
    path::Path,
};

use crate::files::ServerFiles;
use common::Error;
use poise::serenity_prelude::async_trait;
use ssh2::{KeyboardInteractivePrompt, Session, Sftp};

pub struct ServerSftp {
    pub addr: SocketAddr,
    pub username: String,
    pub password: String,
}

struct KeyPrompt {
    password: String,
}

impl KeyboardInteractivePrompt for KeyPrompt {
    fn prompt<'a>(
        &mut self,
        username: &str,
        instructions: &str,
        prompts: &[ssh2::Prompt<'a>],
    ) -> Vec<String> {
        log::info!("{username}");
        log::info!("{instructions}");
        log::info!("{prompts:?}");
        stdout().flush().unwrap();
        vec![self.password.clone()]
    }
}

impl ServerSftp {
    pub fn new(addr: SocketAddr, username: String, password: String) -> Self {
        Self {
            addr,
            username,
            password,
        }
    }

    async fn connect(&self) -> Result<Sftp, Error> {
        let tcp = TcpStream::connect(self.addr)?;
        let u = self.username.clone();
        let p = self.password.clone();
        tokio::task::spawn_blocking(move || -> Result<Sftp, Error> {
            let mut sess = Session::new()?;
            sess.set_tcp_stream(tcp);
            sess.handshake()?;

            sess.userauth_password(&u, &p)?;
            if !sess.authenticated() {
                Err("Failed auth.".into())
            } else {
                Ok(sess.sftp()?)
            }
        })
        .await?
    }
}

#[async_trait]
impl ServerFiles for ServerSftp {
    /// download the contents of a file on the server.
    async fn fetch_file(&self, path: &str) -> Result<Vec<u8>, Error> {
        let sftp = self.connect().await?;
        let path = path.to_owned();
        tokio::task::spawn_blocking(move || -> Result<Vec<u8>, Error> {
            let mut contents = Vec::new();
            sftp.open(Path::new(&path))?.read_to_end(&mut contents)?;
            Ok(contents)
        })
        .await?
    }

    /// upload the contents of a file on the server.
    async fn upload_file(&self, path: &str, contents: &[u8]) -> Result<(), Error> {
        let sftp = self.connect().await?;
        let contents = contents.to_vec();
        let path = path.to_owned();
        tokio::task::spawn_blocking(move || {
            sftp.create(Path::new(&path))?.write_all(&contents)?;
            Ok(())
        })
        .await?
    }
}
